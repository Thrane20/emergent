//! Humanoid retargeting (SKEL-3): bakes a clip from one skeleton onto another through
//! their humanoid maps. Offline (scene load), never per tick.
//!
//! Each mapped bone copies its source bone's world-space change from rest, `D = Q·R⁻¹`,
//! so the two skeletons may use different joint axes and extra unmapped joints. Rest
//! poses may differ (T-pose vs A-pose): a per-bone arc first turns the target's rest
//! bone direction onto the source's. Facing (+Z vs −Z) is aligned from the hips'
//! left→right axis, and the hips' model-space position is scaled by leg length, which
//! also absorbs units (both models are assumed to stand on y = 0 at their origin).
use glam::{Mat4, Quat, Vec3, Vec4};

use crate::{
    AnimationChannel, AnimationClip, ChannelProperty, HumanoidBone, HumanoidMap, Interpolation,
    Skeleton, Transform,
};

/// Baked samples per second.
pub const RETARGET_FPS: f32 = 30.0;
const MAX_SAMPLES: usize = 100_000;

fn matrix(t: &Transform) -> Mat4 {
    Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.translation)
}

/// Model-space matrices of a local pose (parents precede children).
fn globals(skeleton: &Skeleton, pose: &[Transform], out: &mut Vec<Mat4>) {
    out.clear();
    for (i, t) in pose.iter().enumerate() {
        let m = matrix(t);
        out.push(match skeleton.parents()[i] {
            Some(p) => out[p as usize] * m,
            None => m,
        });
    }
}

fn rotation(m: &Mat4) -> Quat {
    m.to_scale_rotation_translation().1.normalize()
}

fn position(m: &Mat4) -> Vec3 {
    m.w_axis.truncate()
}

fn leg_length(map: &HumanoidMap, rest: &[Mat4]) -> f32 {
    use HumanoidBone::*;
    [
        (LeftUpperLeg, LeftLowerLeg),
        (LeftLowerLeg, LeftFoot),
        (RightUpperLeg, RightLowerLeg),
        (RightLowerLeg, RightFoot),
    ]
    .iter()
    .filter_map(|&(a, b)| {
        Some(position(&rest[map.joint(b)?]).distance(position(&rest[map.joint(a)?])))
    })
    .sum::<f32>()
}

/// Rotation about +Y taking the source's left→right hip axis onto the target's.
fn facing(source: (&HumanoidMap, &[Mat4]), target: (&HumanoidMap, &[Mat4])) -> Quat {
    let lateral = |(map, rest): (&HumanoidMap, &[Mat4])| {
        let l = position(&rest[map.joint(HumanoidBone::LeftUpperLeg)?]);
        let r = position(&rest[map.joint(HumanoidBone::RightUpperLeg)?]);
        let v = r - l;
        Vec3::new(v.x, 0.0, v.z).try_normalize()
    };
    match (lateral(source), lateral(target)) {
        (Some(s), Some(t)) if s.dot(t) < -0.9999 => Quat::from_rotation_y(std::f32::consts::PI),
        (Some(s), Some(t)) => Quat::from_rotation_arc(s, t),
        _ => Quat::IDENTITY,
    }
}

/// Bakes `clip` (over `source`) onto `target` at [`RETARGET_FPS`]. The result animates
/// every humanoid bone mapped on both sides, plus hips translation.
pub fn retarget(
    source: &Skeleton,
    source_map: &HumanoidMap,
    clip: &AnimationClip,
    target: &Skeleton,
    target_map: &HumanoidMap,
) -> Result<AnimationClip, String> {
    for (map, side) in [(source_map, "source"), (target_map, "target")] {
        let missing = map.missing();
        if !missing.is_empty() {
            return Err(format!(
                "The {side} skeleton is not a recognised humanoid; missing {}.",
                missing.join(", ")
            ));
        }
    }
    let (mut source_rest, mut target_rest) = (Vec::new(), Vec::new());
    globals(source, source.rest(), &mut source_rest);
    globals(target, target.rest(), &mut target_rest);
    let face = facing((source_map, &source_rest), (target_map, &target_rest));
    let scale = {
        let (s, t) = (
            leg_length(source_map, &source_rest),
            leg_length(target_map, &target_rest),
        );
        if s > 1e-6 && t > 1e-6 {
            t / s
        } else {
            1.0
        }
    };
    // Per target joint: (source joint, rest arc, is hips).
    let mut binding: Vec<Option<(usize, Quat, bool)>> = vec![None; target.len()];
    let mut arcs = [None; HumanoidBone::ALL.len()];
    let direction = |map: &HumanoidMap, rest: &[Mat4], bone: HumanoidBone| {
        let from = position(&rest[map.joint(bone)?]);
        bone.children()
            .iter()
            .filter_map(|c| map.joint(*c))
            .map(|j| position(&rest[j]) - from)
            .find_map(Vec3::try_normalize)
    };
    for bone in HumanoidBone::ALL {
        let (Some(s), Some(t)) = (source_map.joint(bone), target_map.joint(bone)) else {
            continue;
        };
        let arc = match (
            direction(source_map, &source_rest, bone),
            direction(target_map, &target_rest, bone),
        ) {
            (Some(ds), Some(dt)) => Some(Quat::from_rotation_arc(dt, face * ds)),
            _ => None,
        };
        // End bones (hands, head, toes) follow the arc of the bone they hang from.
        let arc = arc
            .or_else(|| {
                let mut at = bone.parent();
                while let Some(p) = at {
                    if let Some(a) = arcs[p as usize] {
                        return Some(a);
                    }
                    at = p.parent();
                }
                None
            })
            .unwrap_or(Quat::IDENTITY);
        arcs[bone as usize] = Some(arc);
        binding[t] = Some((s, arc, bone == HumanoidBone::Hips));
    }
    let hips_source = source_map.joint(HumanoidBone::Hips).expect("required");

    let samples = ((clip.duration() * RETARGET_FPS).ceil() as usize + 1).max(1);
    if samples > MAX_SAMPLES {
        return Err("Clip is too long to retarget.".into());
    }
    let times: Vec<f32> = (0..samples)
        .map(|i| (i as f32 / RETARGET_FPS).min(clip.duration()))
        .collect();
    let mut rotations: Vec<Vec<Vec4>> = vec![Vec::with_capacity(samples); target.len()];
    let mut hips_track = Vec::with_capacity(samples);
    let (mut source_pose, mut source_global, mut target_global) =
        (Vec::new(), Vec::new(), Vec::with_capacity(target.len()));
    for &time in &times {
        source_pose.clear();
        source_pose.extend_from_slice(source.rest());
        clip.sample(time, &mut source_pose);
        globals(source, &source_pose, &mut source_global);
        target_global.clear();
        for (j, rest) in target.rest().iter().enumerate() {
            let parent = target.parents()[j].map(|p| target_global[p as usize]);
            let mut local = *rest;
            if let Some((s, arc, hips)) = binding[j] {
                let change = face
                    * rotation(&source_global[s])
                    * rotation(&source_rest[s]).inverse()
                    * face.inverse();
                let world = change * arc * rotation(&target_rest[j]);
                let parent_rotation = parent.as_ref().map_or(Quat::IDENTITY, rotation);
                local.rotation = (parent_rotation.inverse() * world).normalize();
                rotations[j].push(Vec4::from(local.rotation));
                if hips {
                    // Absolute: both models stand on y = 0 at their origin, and BVH rest
                    // poses carry no standing height to measure travel from.
                    let world = face * position(&source_global[hips_source]) * scale;
                    local.translation =
                        parent.map_or(world, |p| p.inverse().transform_point3(world));
                    hips_track.push(local.translation.extend(0.0));
                }
            }
            let m = matrix(&local);
            target_global.push(parent.map_or(m, |p| p * m));
        }
    }
    // Duplicate final times (when duration is not a whole number of samples) are dropped.
    let mut keep = times.len();
    while keep > 1 && times[keep - 1] <= times[keep - 2] {
        keep -= 1;
    }
    let mut channels = Vec::new();
    for (j, values) in rotations.into_iter().enumerate() {
        if values.is_empty() {
            continue;
        }
        channels.push(AnimationChannel {
            joint: j as u16,
            property: ChannelProperty::Rotation,
            interpolation: Interpolation::Linear,
            times: times[..keep].to_vec(),
            values: values[..keep].to_vec(),
        });
    }
    let hips = target_map.joint(HumanoidBone::Hips).expect("required");
    channels.push(AnimationChannel {
        joint: hips as u16,
        property: ChannelProperty::Translation,
        interpolation: Interpolation::Linear,
        times: times[..keep].to_vec(),
        values: hips_track[..keep].to_vec(),
    });
    AnimationClip::new(clip.name().to_string(), channels, target.len())
}
