//! Skeletal animation clips and the `emergent.animator` component (SKEL-2; crossfade,
//! speed blend and in-place clips SKEL-4; foot placement, look-at and turning SKEL-5).
//! Clips are imported with their skeleton; channels target joints by runtime index,
//! resolved by name at import. Sampling writes into reusable pose and palette buffers,
//! so a playing animator does not allocate per tick.
use std::sync::Arc;
use std::time::Duration;

use bevy_ecs::component::Component;
use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

use crate::ik::{position, rotation};
use crate::{EntityId, HumanoidBone, HumanoidMap, Skeleton, Transform};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelProperty {
    Translation,
    Rotation,
    Scale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interpolation {
    Linear,
    Step,
}

/// Keyframes for one property of one joint. Rotations are unit quaternions (xyzw).
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationChannel {
    pub joint: u16,
    pub property: ChannelProperty,
    pub interpolation: Interpolation,
    /// Strictly increasing seconds, at least one.
    pub times: Vec<f32>,
    pub values: Vec<Vec4>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimationClip {
    name: String,
    duration: f32,
    channels: Vec<AnimationChannel>,
}

impl AnimationClip {
    pub fn new(
        name: String,
        channels: Vec<AnimationChannel>,
        joints: usize,
    ) -> Result<Self, String> {
        let mut duration = 0.0f32;
        for channel in &channels {
            if channel.joint as usize >= joints {
                return Err(format!(
                    "Clip {name:?} targets a joint outside its skeleton."
                ));
            }
            if channel.times.is_empty()
                || channel.times.len() != channel.values.len()
                || channel.times.iter().any(|t| !t.is_finite() || *t < 0.0)
                || channel.times.windows(2).any(|w| w[1] <= w[0])
                || channel.values.iter().any(|v| !v.is_finite())
            {
                return Err(format!(
                    "Clip {name:?} needs finite, increasing key times with one value each."
                ));
            }
            duration = duration.max(*channel.times.last().expect("non-empty"));
        }
        Ok(Self {
            name,
            duration,
            channels,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Seconds; the last key time of any channel.
    pub fn duration(&self) -> f32 {
        self.duration
    }

    pub fn channels(&self) -> &[AnimationChannel] {
        &self.channels
    }

    /// Removes the horizontal travel of the root-most translated joint (model-space x/z
    /// drift over the clip, and its start offset from rest) so locomotion can move the
    /// object instead. Returns the removed travel speed in model units per second.
    pub fn make_in_place(&mut self, skeleton: &Skeleton) -> f32 {
        let Some(channel) = self
            .channels
            .iter_mut()
            .filter(|c| c.property == ChannelProperty::Translation)
            .min_by_key(|c| c.joint)
        else {
            return 0.0;
        };
        let joint = channel.joint as usize;
        let Some(rest) = skeleton.rest().get(joint) else {
            return 0.0;
        };
        // Translation keys are in the parent's frame; travel is measured in model space.
        let mut parent = Mat4::IDENTITY;
        let mut at = skeleton.parents()[joint];
        while let Some(p) = at {
            let t = &skeleton.rest()[p as usize];
            parent =
                Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.translation) * parent;
            at = skeleton.parents()[p as usize];
        }
        let to_local = parent.inverse();
        let horizontal = |v: Vec3| {
            let m = parent.transform_vector3(v);
            Vec3::new(m.x, 0.0, m.z)
        };
        let first = channel.values[0].truncate();
        let last = channel.values[channel.values.len() - 1].truncate();
        let end = channel.times[channel.times.len() - 1];
        let (start, drift) = (
            horizontal(first - rest.translation),
            horizontal(last - first),
        );
        for (t, v) in channel.times.iter().zip(&mut channel.values) {
            let along = if end > 0.0 { t / end } else { 0.0 };
            *v -= to_local
                .transform_vector3(start + drift * along)
                .extend(0.0);
        }
        if end > 0.0 {
            drift.length() / end
        } else {
            0.0
        }
    }

    /// Overwrites the animated properties of `pose` at `time` seconds (clamped to the keys).
    /// Joints and properties the clip does not animate are left as they are.
    pub fn sample(&self, time: f32, pose: &mut [Transform]) {
        for channel in &self.channels {
            let Some(transform) = pose.get_mut(channel.joint as usize) else {
                continue;
            };
            let value = channel.sample(time);
            match channel.property {
                ChannelProperty::Translation => transform.translation = value.truncate(),
                ChannelProperty::Scale => transform.scale = value.truncate(),
                ChannelProperty::Rotation => {
                    transform.rotation = Quat::from_vec4(value).normalize();
                }
            }
        }
    }
}

impl AnimationChannel {
    fn sample(&self, time: f32) -> Vec4 {
        let times = &self.times;
        // First key strictly after `time`; keys before 0 or past the end clamp.
        let next = times.partition_point(|&t| t <= time);
        if next == 0 {
            return self.values[0];
        }
        if next == times.len() || self.interpolation == Interpolation::Step {
            return self.values[next - 1];
        }
        let (a, b) = (self.values[next - 1], self.values[next]);
        let t = (time - times[next - 1]) / (times[next] - times[next - 1]);
        match self.property {
            ChannelProperty::Rotation => {
                let (qa, qb) = (Quat::from_vec4(a), Quat::from_vec4(b));
                Vec4::from(qa.slerp(qb, t))
            }
            _ => a.lerp(b, t),
        }
    }
}

/// Seconds over which the measured ground speed settles (exponential smoothing).
const SPEED_SMOOTHING: f32 = 0.15;
/// Seconds over which foot, hips and look corrections settle.
const IK_SMOOTHING: f32 = 0.08;
/// Ground steps the feet follow, as a fraction of leg length, up or down.
const MAX_STEP: f32 = 0.45;
/// Steepest ground (radians) a foot tilts to match.
const MAX_FOOT_TILT: f32 = 0.6;
/// Head turn limit (radians); targets further round fade the look out.
const LOOK_LIMIT: f32 = 1.2;
const LOOK_FADE: f32 = 2.0;
/// Slower than this (m/s) the object keeps its heading.
const TURN_MIN_SPEED: f32 = 0.3;

/// Casts straight down from a world point up to a distance; hit point and normal.
pub type GroundRay<'a> = &'a dyn Fn(Vec3, f32) -> Option<(Vec3, Vec3)>;

/// What the engine tells an animator each tick (see [`Animator::advance_with`]).
pub struct AnimatorInputs<'a> {
    /// The object's world matrix as last propagated.
    pub world: Mat4,
    pub ground: Option<GroundRay<'a>>,
    /// World position the head looks towards.
    pub look_target: Option<Vec3>,
}

/// Humanoid joints IK uses, found once per animator.
#[derive(Clone, Debug)]
struct Rig {
    hips: usize,
    /// Upper leg, lower leg, foot; `None` unless each is the direct child of the last.
    legs: Option<[[usize; 3]; 2]>,
    leg_length: f32,
    /// Neck (if directly above the head) and head.
    look: Vec<usize>,
    /// Model forward in the head's own frame.
    head_forward: Vec3,
}

/// Model-space forward: up × (right hip − left hip); glTF's +Z without hips.
fn model_forward(map: &HumanoidMap, rest: &[Mat4]) -> Vec3 {
    map.joint(HumanoidBone::LeftUpperLeg)
        .zip(map.joint(HumanoidBone::RightUpperLeg))
        .and_then(|(l, r)| {
            let right = position(&rest[r]) - position(&rest[l]);
            Vec3::Y
                .cross(Vec3::new(right.x, 0.0, right.z))
                .try_normalize()
        })
        .unwrap_or(Vec3::Z)
}

impl Rig {
    fn new(skeleton: &Skeleton, map: &HumanoidMap, rest: &[Mat4], forward: Vec3) -> Option<Self> {
        use HumanoidBone::*;
        let hips = map.joint(Hips)?;
        let head = map.joint(Head)?;
        let linked = |chain: [usize; 3]| {
            (skeleton.parents()[chain[1]] == Some(chain[0] as u16)
                && skeleton.parents()[chain[2]] == Some(chain[1] as u16))
            .then_some(chain)
        };
        let leg = |u, l, f| linked([map.joint(u)?, map.joint(l)?, map.joint(f)?]);
        let legs = leg(LeftUpperLeg, LeftLowerLeg, LeftFoot)
            .zip(leg(RightUpperLeg, RightLowerLeg, RightFoot))
            .map(|(l, r)| [l, r]);
        let leg_length = legs.map_or(0.0, |[l, _]| {
            position(&rest[l[0]]).distance(position(&rest[l[1]]))
                + position(&rest[l[1]]).distance(position(&rest[l[2]]))
        });
        let mut look = vec![head];
        if let Some(neck) = map.joint(Neck) {
            if skeleton.parents()[head] == Some(neck as u16) {
                look.insert(0, neck);
            }
        }
        Some(Self {
            hips,
            legs,
            leg_length,
            look,
            head_forward: rotation(&rest[head]).inverse() * forward,
        })
    }
}

/// `pose = lerp(pose, other, w)` per joint.
fn mix(pose: &mut [Transform], other: &[Transform], w: f32) {
    for (a, b) in pose.iter_mut().zip(other) {
        a.translation = a.translation.lerp(b.translation, w);
        a.scale = a.scale.lerp(b.scale, w);
        a.rotation = a.rotation.slerp(b.rotation, w);
    }
}

fn wrap(time: f32, duration: f32, looping: bool) -> f32 {
    if duration <= 0.0 {
        0.0
    } else if looping {
        time.rem_euclid(duration)
    } else {
        time.clamp(0.0, duration)
    }
}

/// The clip being faded out after a [`Animator::crossfade`].
#[derive(Clone, Debug)]
struct Fade {
    clip: Option<usize>,
    time: f32,
    elapsed: f32,
    duration: f32,
}

/// Plays one clip of a skinned model, or blends clips by the object's ground speed.
/// The renderer reads `palette` each frame.
#[derive(Clone, Component, Debug)]
pub struct Animator {
    skeleton: Arc<Skeleton>,
    clips: Arc<[AnimationClip]>,
    clip: Option<usize>,
    /// Playback position in seconds.
    pub time: f32,
    /// Playback rate; negative plays backwards.
    pub speed: f32,
    pub looping: bool,
    pub playing: bool,
    /// Places the feet on the ground under them (needs physics ground) and lowers the hips
    /// to reach.
    pub foot_ik: bool,
    /// Object the head turns towards.
    pub look_target: Option<EntityId>,
    /// Radians per second the object turns to face where it moves; 0 keeps its heading.
    pub turn_rate: f32,
    fade: Option<Fade>,
    /// Speed blend: (clip, ground speed it plays at), speeds strictly increasing.
    blend: Vec<(usize, f32)>,
    /// Shared normalised phase (0..1) of the blended clips.
    phase: f32,
    ground_speed: f32,
    /// Smoothed world-space velocity.
    velocity: Vec3,
    last_position: Option<Vec3>,
    rig: Option<Rig>,
    forward: Vec3,
    /// Smoothed ground height under each foot and hips drop, model units.
    foot_offsets: [f32; 2],
    hips_offset: f32,
    /// Smoothed look weight and the point it last looked at (model space).
    look_weight: f32,
    look_point: Vec3,
    globals: Vec<Mat4>,
    pose: Vec<Transform>,
    scratch: Vec<Transform>,
    palette: Vec<Mat4>,
}

impl Animator {
    /// `clip` names one of `clips`; `None` shows the rest pose.
    pub fn new(
        skeleton: Arc<Skeleton>,
        clips: Arc<[AnimationClip]>,
        clip: Option<&str>,
    ) -> Result<Self, String> {
        let clip = clip
            .map(|name| {
                clips
                    .iter()
                    .position(|c| c.name == name)
                    .ok_or_else(|| format!("The model has no animation clip named {name:?}."))
            })
            .transpose()?;
        let map = HumanoidMap::detect(&skeleton);
        let mut rest = Vec::new();
        skeleton
            .globals(skeleton.rest(), &mut rest)
            .expect("rest pose matches its skeleton");
        let forward = model_forward(&map, &rest);
        let rig = Rig::new(&skeleton, &map, &rest, forward);
        let mut animator = Self {
            foot_ik: true,
            look_target: None,
            turn_rate: 0.0,
            velocity: Vec3::ZERO,
            rig,
            forward,
            foot_offsets: [0.0; 2],
            hips_offset: 0.0,
            look_weight: 0.0,
            look_point: Vec3::ZERO,
            globals: Vec::with_capacity(skeleton.len()),
            fade: None,
            blend: Vec::new(),
            phase: 0.0,
            ground_speed: 0.0,
            last_position: None,
            pose: skeleton.rest().to_vec(),
            scratch: skeleton.rest().to_vec(),
            palette: Vec::with_capacity(skeleton.len()),
            skeleton,
            clips,
            clip,
            time: 0.0,
            speed: 1.0,
            looping: true,
            playing: true,
        };
        animator.evaluate();
        Ok(animator)
    }

    pub fn skeleton(&self) -> &Skeleton {
        &self.skeleton
    }

    pub fn clip(&self) -> Option<&AnimationClip> {
        self.clip.map(|i| &self.clips[i])
    }

    /// Switches clip by name (`None`: rest pose) and restarts it.
    pub fn play(&mut self, clip: Option<&str>) -> Result<(), String> {
        self.clip = match clip {
            None => None,
            Some(name) => Some(
                self.clips
                    .iter()
                    .position(|c| c.name == name)
                    .ok_or_else(|| format!("The model has no animation clip named {name:?}."))?,
            ),
        };
        self.time = 0.0;
        self.fade = None;
        self.evaluate();
        Ok(())
    }

    /// Like [`Animator::play`], fading from the current pose over `seconds`.
    pub fn crossfade(&mut self, clip: Option<&str>, seconds: f32) -> Result<(), String> {
        let (from, time) = (self.clip, self.time);
        self.play(clip)?;
        if seconds > 0.0 {
            self.fade = Some(Fade {
                clip: from,
                time,
                elapsed: 0.0,
                duration: seconds,
            });
            self.evaluate();
        }
        Ok(())
    }

    /// Blends clips by the object's horizontal ground speed, in its own units per second:
    /// each entry is a clip and the speed it plays at, strictly increasing. Above the top
    /// speed the top clip plays faster. Empty turns blending off.
    pub fn set_blend(&mut self, entries: &[(&str, f32)]) -> Result<(), String> {
        let mut blend: Vec<(usize, f32)> = Vec::with_capacity(entries.len());
        for &(name, speed) in entries {
            let clip = self
                .clips
                .iter()
                .position(|c| c.name == name)
                .ok_or_else(|| format!("The model has no animation clip named {name:?}."))?;
            if !speed.is_finite() || speed < 0.0 || blend.last().is_some_and(|l| speed <= l.1) {
                return Err(format!(
                    "Blend speeds must be finite and increase; {name:?} has {speed}."
                ));
            }
            blend.push((clip, speed));
        }
        self.blend = blend;
        self.phase = 0.0;
        self.evaluate();
        Ok(())
    }

    /// Smoothed horizontal speed the speed blend last measured.
    pub fn ground_speed(&self) -> f32 {
        self.ground_speed
    }

    /// Measures ground speed from the object's world matrix, then advances (no IK).
    pub fn advance_at(&mut self, dt: f32, world: Mat4) {
        self.advance_with(
            dt,
            &AnimatorInputs {
                world,
                ground: None,
                look_target: None,
            },
        );
    }

    /// One engine tick: measures movement, advances playback, samples, then applies foot
    /// placement and look-at. Returns the yaw (radians about +Y) the object should turn
    /// by this tick to face its movement, when `turn_rate` is set.
    pub fn advance_with(&mut self, dt: f32, inputs: &AnimatorInputs<'_>) -> Option<f32> {
        let world = inputs.world;
        if dt > 0.0 && (!self.blend.is_empty() || self.turn_rate > 0.0) {
            let position = position(&world);
            let raw = self
                .last_position
                .map_or(Vec3::ZERO, |last| (position - last) / dt);
            let raw = if raw.is_finite() {
                Vec3::new(raw.x, 0.0, raw.z)
            } else {
                Vec3::ZERO
            };
            self.last_position = Some(position);
            self.velocity += (raw - self.velocity) * (1.0 - (-dt / SPEED_SMOOTHING).exp());
            let local = world.inverse().transform_vector3(self.velocity);
            let speed = Vec2::new(local.x, local.z).length();
            self.ground_speed = if speed.is_finite() { speed } else { 0.0 };
        }
        self.step_time(dt);
        self.sample();
        self.solve_ik(dt, inputs);
        self.skeleton
            .palette(&self.pose, &mut self.palette)
            .expect("pose is sized from its skeleton");
        let flat = Vec3::new(self.velocity.x, 0.0, self.velocity.z);
        if self.turn_rate <= 0.0 || flat.length() < TURN_MIN_SPEED {
            return None;
        }
        let facing = world.transform_vector3(self.forward);
        let facing = Vec3::new(facing.x, 0.0, facing.z).try_normalize()?;
        let wanted = flat.normalize();
        let angle = facing.cross(wanted).y.atan2(facing.dot(wanted));
        let limit = self.turn_rate * dt;
        Some(angle.clamp(-limit, limit))
    }

    fn solve_ik(&mut self, dt: f32, inputs: &AnimatorInputs<'_>) {
        let Some(rig) = &self.rig else { return };
        let ground = inputs.ground.filter(|_| self.foot_ik);
        let feet = ground.zip(rig.legs);
        if feet.is_none() && inputs.look_target.is_none() && self.look_weight <= 1e-3 {
            return;
        }
        let blend = 1.0 - (-dt.max(0.0) / IK_SMOOTHING).exp();
        let world = inputs.world;
        let inverse = world.inverse();
        let skeleton = &*self.skeleton;
        skeleton
            .globals(&self.pose, &mut self.globals)
            .expect("pose is sized from its skeleton");
        if let Some((ground, legs)) = feet {
            // shortcut: assumes an upright object (model +Y is world up); tilted objects
            // place feet approximately.
            let scale = world.transform_vector3(Vec3::Y).length().max(1e-6);
            let reach = rig.leg_length;
            let mut normals = [Vec3::Y; 2];
            for (i, leg) in legs.iter().enumerate() {
                let foot = position(&self.globals[leg[2]]);
                let floor = world.transform_point3(Vec3::new(foot.x, 0.0, foot.z));
                let hit = ground(floor + Vec3::Y * reach * scale, 2.0 * reach * scale);
                let goal = hit.map_or(0.0, |(point, normal)| {
                    normals[i] = inverse.transform_vector3(normal).normalize_or(Vec3::Y);
                    ((point.y - floor.y) / scale).clamp(-MAX_STEP * reach, MAX_STEP * reach)
                });
                self.foot_offsets[i] += (goal - self.foot_offsets[i]) * blend;
            }
            let drop = self.foot_offsets[0].min(self.foot_offsets[1]).min(0.0);
            self.hips_offset += (drop - self.hips_offset) * blend;
            if self.foot_offsets.iter().any(|o| o.abs() > 1e-4) || self.hips_offset.abs() > 1e-4 {
                let parent = skeleton.parents()[rig.hips]
                    .map_or(Mat4::IDENTITY, |p| self.globals[p as usize]);
                self.pose[rig.hips].translation += parent
                    .inverse()
                    .transform_vector3(Vec3::Y * self.hips_offset);
                skeleton
                    .globals(&self.pose, &mut self.globals)
                    .expect("pose is sized from its skeleton");
                for (i, leg) in legs.iter().enumerate() {
                    let foot = self.globals[leg[2]];
                    let target =
                        position(&foot) + Vec3::Y * (self.foot_offsets[i] - self.hips_offset);
                    let pole = position(&self.globals[leg[1]]) + self.forward * reach;
                    let tilt = Quat::from_rotation_arc(Vec3::Y, normals[i]);
                    let tilt = Quat::IDENTITY.slerp(
                        tilt,
                        (MAX_FOOT_TILT / tilt.angle_between(Quat::IDENTITY).max(1e-6)).min(1.0),
                    );
                    if crate::ik::two_bone(
                        skeleton,
                        &mut self.pose,
                        &self.globals,
                        *leg,
                        target,
                        pole,
                    ) {
                        skeleton
                            .globals(&self.pose, &mut self.globals)
                            .expect("pose is sized from its skeleton");
                        let lower = rotation(&self.globals[leg[1]]);
                        self.pose[leg[2]].rotation =
                            (lower.inverse() * tilt * rotation(&foot)).normalize();
                    }
                }
                skeleton
                    .globals(&self.pose, &mut self.globals)
                    .expect("pose is sized from its skeleton");
            }
        }
        let head = *rig.look.last().expect("head");
        let mut goal = 0.0;
        if let Some(target) = inputs.look_target {
            let point = inverse.transform_point3(target);
            let facing = rotation(&self.globals[head]) * rig.head_forward;
            let wanted = point - position(&self.globals[head]);
            if facing.angle_between(wanted) < LOOK_FADE {
                self.look_point = point;
                goal = 1.0;
            }
        }
        self.look_weight += (goal - self.look_weight) * blend;
        if self.look_weight > 1e-3 {
            crate::ik::look_at(
                skeleton,
                &mut self.pose,
                &self.globals,
                &rig.look,
                rig.head_forward,
                self.look_point,
                LOOK_LIMIT,
                self.look_weight,
            );
        }
    }

    /// Blend entries around the ground speed and the weight of the second.
    fn segment(&self) -> (usize, usize, f32) {
        let s = self.ground_speed;
        let last = self.blend.len() - 1;
        match self.blend.partition_point(|e| e.1 <= s) {
            0 => (0, 0, 0.0),
            i if i > last => (last, last, 0.0),
            i => {
                let (lo, hi) = (self.blend[i - 1].1, self.blend[i].1);
                (i - 1, i, (s - lo) / (hi - lo))
            }
        }
    }

    pub fn pose(&self) -> &[Transform] {
        &self.pose
    }

    /// Model-space joint × inverse bind, one per joint.
    pub fn palette(&self) -> &[Mat4] {
        &self.palette
    }

    /// Advances playback by `dt` and re-evaluates the pose.
    pub fn advance(&mut self, dt: f32) {
        self.step_time(dt);
        self.evaluate();
    }

    fn step_time(&mut self, dt: f32) {
        if self.playing {
            if !self.blend.is_empty() {
                let (a, b, w) = self.segment();
                let duration = self.clips[self.blend[a].0].duration * (1.0 - w)
                    + self.clips[self.blend[b].0].duration * w;
                let top = self.blend[self.blend.len() - 1].1;
                let rate = if self.ground_speed > top && top > 0.0 {
                    self.ground_speed / top
                } else {
                    1.0
                };
                if duration > 0.0 {
                    self.phase = (self.phase + dt * self.speed * rate / duration).rem_euclid(1.0);
                }
            } else if let Some(clip) = self.clip() {
                self.time = wrap(self.time + dt * self.speed, clip.duration, self.looping);
            }
            if let Some(fade) = &mut self.fade {
                fade.elapsed += dt;
                if let Some(i) = fade.clip {
                    let duration = self.clips[i].duration;
                    fade.time = wrap(fade.time + dt * self.speed, duration, self.looping);
                }
                if fade.elapsed >= fade.duration {
                    self.fade = None;
                }
            }
        }
    }

    fn evaluate(&mut self) {
        self.sample();
        self.skeleton
            .palette(&self.pose, &mut self.palette)
            .expect("pose is sized from its skeleton");
    }

    fn sample(&mut self) {
        let rest = self.skeleton.rest();
        self.pose.copy_from_slice(rest);
        if !self.blend.is_empty() {
            let (a, b, w) = self.segment();
            let (a, b) = (&self.clips[self.blend[a].0], &self.clips[self.blend[b].0]);
            a.sample(self.phase * a.duration, &mut self.pose);
            if w > 0.0 {
                self.scratch.copy_from_slice(rest);
                b.sample(self.phase * b.duration, &mut self.scratch);
                mix(&mut self.pose, &self.scratch, w);
            }
        } else if let Some(i) = self.clip {
            self.clips[i].sample(self.time, &mut self.pose);
        }
        if let Some(fade) = &self.fade {
            self.scratch.copy_from_slice(rest);
            if let Some(i) = fade.clip {
                self.clips[i].sample(fade.time, &mut self.scratch);
            }
            mix(
                &mut self.pose,
                &self.scratch,
                1.0 - fade.elapsed / fade.duration,
            );
        }
    }
}

/// Advances every animator by one fixed tick.
pub(crate) fn step(world: &mut crate::World, dt: Duration) {
    world.step_animators(dt.as_secs_f32());
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn skeleton() -> Arc<Skeleton> {
        Arc::new(
            Skeleton::new(
                vec!["root".into()],
                vec![None],
                vec![Transform::IDENTITY],
                vec![Mat4::IDENTITY],
            )
            .unwrap(),
        )
    }

    fn clip(interpolation: Interpolation) -> AnimationClip {
        AnimationClip::new(
            "walk".into(),
            vec![
                AnimationChannel {
                    joint: 0,
                    property: ChannelProperty::Translation,
                    interpolation,
                    times: vec![0.0, 1.0, 2.0],
                    values: vec![Vec4::ZERO, Vec4::X * 2.0, Vec4::ZERO],
                },
                AnimationChannel {
                    joint: 0,
                    property: ChannelProperty::Rotation,
                    interpolation,
                    times: vec![0.0, 2.0],
                    values: vec![
                        Vec4::from(Quat::IDENTITY),
                        // 120°: a 180° key would make the shortest-path direction ambiguous.
                        Vec4::from(Quat::from_rotation_y(std::f32::consts::TAU / 3.0)),
                    ],
                },
            ],
            1,
        )
        .unwrap()
    }

    #[test]
    fn linear_and_slerp_sampling_interpolates_and_clamps() {
        let clip = clip(Interpolation::Linear);
        assert_eq!(clip.duration(), 2.0);
        let mut pose = [Transform::IDENTITY];
        clip.sample(0.5, &mut pose);
        assert!(pose[0].translation.abs_diff_eq(Vec3::X, 1e-6));
        clip.sample(1.0, &mut pose);
        assert!(pose[0].translation.abs_diff_eq(Vec3::X * 2.0, 1e-6));
        assert!(pose[0]
            .rotation
            .abs_diff_eq(Quat::from_rotation_y(std::f32::consts::FRAC_PI_3), 1e-5));
        clip.sample(-1.0, &mut pose);
        assert_eq!(pose[0].translation, Vec3::ZERO);
        clip.sample(9.0, &mut pose);
        assert_eq!(pose[0].translation, Vec3::ZERO);
        assert_eq!(pose[0].scale, Vec3::ONE, "unanimated properties untouched");
    }

    #[test]
    fn step_sampling_holds_the_previous_key() {
        let mut pose = [Transform::IDENTITY];
        clip(Interpolation::Step).sample(1.9, &mut pose);
        assert!(pose[0].translation.abs_diff_eq(Vec3::X * 2.0, 1e-6));
    }

    #[test]
    fn animators_loop_clamp_and_reuse_their_buffers() {
        let clips: Arc<[AnimationClip]> = vec![clip(Interpolation::Linear)].into();
        let mut animator = Animator::new(skeleton(), clips.clone(), Some("walk")).unwrap();
        let capacity = (animator.pose.capacity(), animator.palette.capacity());
        animator.advance(2.5);
        assert!((animator.time - 0.5).abs() < 1e-6, "loops");
        assert!(animator.palette()[0]
            .w_axis
            .truncate()
            .abs_diff_eq(Vec3::X, 1e-5));
        animator.looping = false;
        animator.advance(10.0);
        assert_eq!(animator.time, 2.0, "clamps at the end");
        animator.speed = -1.0;
        animator.advance(5.0);
        assert_eq!(animator.time, 0.0, "clamps at the start backwards");
        assert_eq!(
            (animator.pose.capacity(), animator.palette.capacity()),
            capacity
        );
        assert!(Animator::new(skeleton(), clips.clone(), Some("run")).is_err());
        animator.play(None).unwrap();
        assert!(
            animator.palette()[0].abs_diff_eq(Mat4::IDENTITY, 1e-6),
            "rest"
        );
    }

    fn rise(name: &str, end: f32, height: f32) -> AnimationClip {
        AnimationClip::new(
            name.into(),
            vec![AnimationChannel {
                joint: 0,
                property: ChannelProperty::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, end],
                values: vec![Vec4::ZERO, Vec4::Y * height],
            }],
            1,
        )
        .unwrap()
    }

    #[test]
    fn speed_blend_weights_clips_by_ground_speed_with_a_shared_phase() {
        let clips: Arc<[AnimationClip]> =
            vec![rise("slow", 1.0, 1.0), rise("fast", 0.5, 3.0)].into();
        let mut animator = Animator::new(skeleton(), clips, None).unwrap();
        assert!(animator.set_blend(&[("fast", 3.0), ("slow", 1.0)]).is_err());
        assert!(animator.set_blend(&[("walk", 1.0)]).is_err());
        animator.set_blend(&[("slow", 1.0), ("fast", 3.0)]).unwrap();
        // Scaled ×2 and moving 4 m/s in world: 2 units/s in the object's own space.
        let dt = 1.0 / 60.0;
        for tick in 0..240 {
            let at = Vec3::X * 4.0 * dt * tick as f32;
            animator.advance_at(
                dt,
                Mat4::from_scale_rotation_translation(Vec3::splat(2.0), Quat::IDENTITY, at),
            );
        }
        assert!(
            (animator.ground_speed() - 2.0).abs() < 1e-3,
            "{}",
            animator.ground_speed()
        );
        let (a, b, w) = animator.segment();
        assert!((a, b) == (0, 1) && (w - 0.5).abs() < 1e-4, "{w}");
        // slow samples y = phase, fast y = 3·phase: half and half is 2·phase.
        let y = animator.pose()[0].translation.y;
        assert!(
            (y - 2.0 * animator.phase).abs() < 1e-3,
            "{y} {}",
            animator.phase
        );
        let before = animator.phase;
        animator.advance_at(
            dt,
            Mat4::from_scale_rotation_translation(
                Vec3::splat(2.0),
                Quat::IDENTITY,
                Vec3::X * 4.0 * dt * 240.0,
            ),
        );
        let step = (animator.phase - before).rem_euclid(1.0);
        assert!(
            (step - dt / 0.75).abs() < 1e-4,
            "blended duration 0.75 s: {step}"
        );
        // Standing still settles on the slowest clip.
        for _ in 0..240 {
            animator.advance_at(dt, Mat4::IDENTITY);
        }
        assert_eq!(animator.segment(), (0, 0, 0.0));
    }

    #[test]
    fn turning_faces_the_movement_within_the_turn_rate() {
        let clips: Arc<[AnimationClip]> = vec![rise("up", 1.0, 1.0)].into();
        let mut animator = Animator::new(skeleton(), clips, None).unwrap();
        animator.turn_rate = 1.0;
        let dt = 0.1;
        let moving = |i: i32| Mat4::from_translation(Vec3::X * 2.0 * dt * i as f32);
        let none = AnimatorInputs {
            world: moving(0),
            ground: None,
            look_target: None,
        };
        assert_eq!(animator.advance_with(dt, &none), None, "standing still");
        let mut yaw = None;
        for i in 1..20 {
            let inputs = AnimatorInputs {
                world: moving(i),
                ground: None,
                look_target: None,
            };
            yaw = animator.advance_with(dt, &inputs);
        }
        // Forward is +Z without a humanoid rig; moving +X is a +90° turn about +Y.
        assert!(
            (yaw.unwrap() - 0.1).abs() < 1e-5,
            "clamped to rate × dt: {yaw:?}"
        );
    }

    #[test]
    fn crossfade_mixes_the_outgoing_pose_out_over_its_duration() {
        let clips: Arc<[AnimationClip]> = vec![rise("up", 1.0, 2.0)].into();
        let mut animator = Animator::new(skeleton(), clips, Some("up")).unwrap();
        animator.playing = false;
        animator.time = 1.0;
        animator.advance(0.0);
        animator.playing = true;
        animator.looping = false;
        animator.crossfade(None, 1.0).unwrap();
        assert!(
            (animator.pose()[0].translation.y - 2.0).abs() < 1e-5,
            "starts at the old pose"
        );
        animator.advance(0.5);
        assert!((animator.pose()[0].translation.y - 1.0).abs() < 1e-5);
        animator.advance(0.6);
        assert_eq!(animator.pose()[0].translation, Vec3::ZERO, "fade finished");
    }

    #[test]
    fn in_place_removes_horizontal_travel_and_reports_its_speed() {
        let mut clip = AnimationClip::new(
            "walk".into(),
            vec![AnimationChannel {
                joint: 0,
                property: ChannelProperty::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0, 2.0],
                values: vec![
                    Vec4::new(1.0, 0.0, 0.0, 0.0),
                    Vec4::new(2.5, 0.5, 2.0, 0.0),
                    Vec4::new(3.0, 1.0, 4.0, 0.0),
                ],
            }],
            1,
        )
        .unwrap();
        let speed = clip.make_in_place(&skeleton());
        assert!((speed - 20f32.sqrt() / 2.0).abs() < 1e-5);
        let values = &clip.channels()[0].values;
        assert!(values[0].abs_diff_eq(Vec4::ZERO, 1e-6), "recentred on rest");
        assert!(
            values[1].abs_diff_eq(Vec4::new(0.5, 0.5, 0.0, 0.0), 1e-6),
            "sway kept"
        );
        assert!(
            values[2].abs_diff_eq(Vec4::new(0.0, 1.0, 0.0, 0.0), 1e-6),
            "height kept"
        );
    }

    #[test]
    fn invalid_clips_are_rejected() {
        let channel = |times: Vec<f32>, joint| AnimationChannel {
            joint,
            property: ChannelProperty::Scale,
            interpolation: Interpolation::Linear,
            values: vec![Vec4::ONE; times.len()],
            times,
        };
        assert!(AnimationClip::new("a".into(), vec![channel(vec![1.0, 0.5], 0)], 1).is_err());
        assert!(AnimationClip::new("a".into(), vec![channel(vec![], 0)], 1).is_err());
        assert!(AnimationClip::new("a".into(), vec![channel(vec![0.0], 3)], 1).is_err());
        assert!(AnimationClip::new("a".into(), vec![channel(vec![f32::NAN], 0)], 1).is_err());
    }
}
