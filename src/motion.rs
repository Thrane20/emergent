//! Built-in motion components authored on scene objects (`emergent.character_controller`,
//! `emergent.follow`, `emergent.spin`).
//! Each runs once per fixed tick, before physics, over a dense scratch list that is
//! reused between ticks. Runtime identity stays the owning entity's `EntityId`.
use std::time::Duration;

use bevy_ecs::component::Component;

use crate::{EngineError, EntityId, Input, Key, Vec3, World};

/// Moves its object from WASD/arrow keys. Every enabled controller in the world responds.
#[derive(Clone, Copy, Component, Debug, PartialEq)]
pub struct CharacterController {
    /// Horizontal speed in metres per second.
    pub speed: f32,
    /// Speed while Shift is held; 0 leaves Shift unused.
    pub run_speed: f32,
    /// Upward speed applied by Space when the body has supporting ground contact; 0 disables jumping.
    pub jump_speed: f32,
    /// Move relative to where the active camera looks (third person), or along world axes.
    pub camera_relative: bool,
    /// Newtons pushing a dynamic body toward `speed`; releasing keys leaves it to friction.
    /// 0 sets the velocity directly (instant start and stop). Bodies without physics ignore it.
    pub force: f32,
}

/// Relative mouse look on the active camera. Angles are radians; sensitivity is degrees/pixel.
#[derive(Clone, Copy, Component, Debug, PartialEq)]
pub struct MouseLook {
    pub sensitivity: f32,
    pub pitch_limit: f32,
    pub yaw: f32,
    pub pitch: f32,
}
impl MouseLook {
    pub fn new(
        sensitivity: f32,
        pitch_limit_degrees: f32,
        rotation: crate::Quat,
    ) -> Result<Self, &'static str> {
        if !sensitivity.is_finite()
            || !(0.001..=5.0).contains(&sensitivity)
            || !pitch_limit_degrees.is_finite()
            || !(1.0..=89.0).contains(&pitch_limit_degrees)
            || !rotation.is_finite()
        {
            return Err("Invalid mouse-look settings");
        }
        let (yaw, pitch, _) = rotation.to_euler(crate::EulerRot::YXZ);
        let pitch_limit = pitch_limit_degrees.to_radians();
        Ok(Self {
            sensitivity,
            pitch_limit,
            yaw,
            pitch: pitch.clamp(-pitch_limit, pitch_limit),
        })
    }
}

/// Rotates its object around a local axis.
#[derive(Clone, Copy, Component, Debug, PartialEq)]
pub struct Spin {
    /// Unit axis in the object's local space.
    pub axis: Vec3,
    pub degrees_per_second: f32,
}

/// Moves its object towards another object's position as committed on the previous tick
/// (transforms propagate after the motion step), so a moving target is followed one tick behind.
#[derive(Clone, Copy, Component, Debug, PartialEq)]
pub struct Follow {
    pub target: EntityId,
    /// Metres per second.
    pub speed: f32,
    /// Stops this far from the target, in metres.
    pub stop_distance: f32,
}

/// Turns its object so its forward (-Z) faces a target.
#[derive(Clone, Copy, Component, Debug, PartialEq)]
pub struct LookAt {
    pub target: EntityId,
    /// Added to the target's position (aim above its feet, for example).
    pub offset: Vec3,
    /// Radians per second; 0 turns instantly.
    pub turn_rate: f32,
}

/// Keeps its object at an offset from a target, smoothly, optionally looking at it.
#[derive(Clone, Copy, Component, Debug, PartialEq)]
pub struct FollowCamera {
    pub target: EntityId,
    /// Right, up and back from the target. With `behind`, measured in the target's heading;
    /// otherwise in world axes (x, y, z).
    pub offset: Vec3,
    pub behind: bool,
    /// Seconds to close most of the gap; 0 snaps.
    pub smoothing: f32,
    pub look: bool,
}

/// Rotation whose forward (-Z) points along `direction`, keeping +Y up where possible.
pub(crate) fn look_rotation(direction: Vec3) -> Option<crate::Quat> {
    let forward = direction.try_normalize()?;
    let right = forward.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
    let up = right.cross(forward);
    Some(crate::Quat::from_mat3(&glam::Mat3::from_cols(right, up, -forward)).normalize())
}

fn face(
    world: &mut World,
    id: EntityId,
    point: Vec3,
    turn_rate: f32,
    dt: f32,
) -> Result<(), EngineError> {
    let here = world.global_transform(id)?.translation();
    let Some(goal) = look_rotation(point - here) else {
        return Ok(());
    };
    let mut local = *world.local_transform(id)?;
    // Written in the parent's space; objects nested under rotated parents aim approximately.
    local.rotation = if turn_rate > 0.0 {
        let angle = local.rotation.angle_between(goal);
        if angle > 1e-5 {
            local
                .rotation
                .slerp(goal, (turn_rate * dt / angle).min(1.0))
        } else {
            goal
        }
    } else {
        goal
    };
    world.set_local_transform(id, local)?;
    Ok(())
}

/// The camera that defines "forward" for camera-relative controls. Set by scene loading.
#[derive(Clone, Copy, Debug, bevy_ecs::resource::Resource)]
pub struct ActiveCamera(pub EntityId);

#[derive(Default)]
pub(crate) struct MotionScratch {
    pub(crate) mouse_looks: Vec<(EntityId, MouseLook)>,
    pub(crate) controllers: Vec<(EntityId, CharacterController)>,
    pub(crate) follows: Vec<(EntityId, Follow)>,
    pub(crate) spins: Vec<(EntityId, Spin)>,
    pub(crate) cameras: Vec<(EntityId, FollowCamera)>,
    pub(crate) looks: Vec<(EntityId, LookAt)>,
}

/// World-space horizontal basis: forward and right on the ground plane.
fn basis(world: &World, camera_relative: bool) -> (Vec3, Vec3) {
    let forward = camera_relative
        .then(|| world.resource::<ActiveCamera>().copied())
        .flatten()
        .and_then(|camera| world.global_transform(camera.0).ok())
        .and_then(|camera| camera.look_direction())
        .map(|look| Vec3::new(look.x, 0.0, look.z))
        .and_then(|flat| flat.try_normalize())
        .unwrap_or(Vec3::NEG_Z);
    (forward, forward.cross(Vec3::Y))
}

pub(crate) fn step(world: &mut World, input: Input, dt: Duration) -> Result<(), EngineError> {
    crate::brain::step(world, dt)?;
    crate::locomotion::step(world, dt)?;
    let scratch = world.take_motion_scratch();
    let result = (|| {
        let active = world.resource::<ActiveCamera>().map(|a| a.0);
        let delta = world
            .resource::<crate::MouseMotion>()
            .map_or(crate::Vec2::ZERO, |m| m.delta);
        if delta != crate::Vec2::ZERO {
            for &(id, mut look) in &scratch.mouse_looks {
                if Some(id) != active {
                    continue;
                }
                let speed = look.sensitivity.to_radians();
                look.yaw = (look.yaw - delta.x * speed).rem_euclid(std::f32::consts::TAU);
                look.pitch =
                    (look.pitch - delta.y * speed).clamp(-look.pitch_limit, look.pitch_limit);
                let mut transform = *world.local_transform(id)?;
                transform.rotation =
                    crate::Quat::from_euler(crate::EulerRot::YXZ, look.yaw, look.pitch, 0.0);
                world.set_local_transform(id, transform)?;
                if let Some(mut stored) = world.component_mut::<MouseLook>(id)? {
                    *stored = look;
                }
            }
            world.propagate_transforms();
        }
        for &(id, controller) in &scratch.controllers {
            let (forward, right) = basis(world, controller.camera_relative);
            let wish = forward * (input.axis(Key::S, Key::W) + input.axis(Key::Down, Key::Up))
                + right * (input.axis(Key::A, Key::D) + input.axis(Key::Left, Key::Right));
            let speed = if controller.run_speed > 0.0 && input.held(Key::Shift) {
                controller.run_speed
            } else {
                controller.speed
            };
            let velocity = wish.normalize_or_zero() * speed;
            let jump = (controller.jump_speed > 0.0 && input.pressed(Key::Space))
                .then_some(controller.jump_speed);
            let dynamic = matches!(world.component::<crate::PhysicsBody>(id)?,
                Some(body) if body.body == crate::BodyKind::Dynamic);
            #[cfg(feature = "physics")]
            if dynamic {
                world
                    .drive_body_with_force(id, velocity, jump, controller.force)
                    .map_err(EngineError::Physics)?;
                continue;
            }
            // Without the physics feature, bodies do not simulate, so the transform moves directly.
            #[cfg(not(feature = "physics"))]
            let _ = (dynamic, jump);
            if velocity != Vec3::ZERO {
                let position =
                    world.global_transform(id)?.translation() + velocity * dt.as_secs_f32();
                let local = world.local_from_global_position(id, position)?;
                world.set_local_transform(id, local)?;
            }
        }
        // Follows read targets as moved so far this tick, in component storage order (stable for a given load).
        for &(id, follow) in &scratch.follows {
            // A deleted target leaves the follower where it is.
            let Ok(target) = world
                .global_transform(follow.target)
                .map(|t| t.translation())
            else {
                continue;
            };
            let here = world.global_transform(id)?.translation();
            let gap = target - here;
            let travel = (gap.length() - follow.stop_distance).min(follow.speed * dt.as_secs_f32());
            if travel > 0.0 {
                let local =
                    world.local_from_global_position(id, here + gap.normalize() * travel)?;
                world.set_local_transform(id, local)?;
            }
        }
        for &(id, spin) in &scratch.spins {
            let mut local = *world.local_transform(id)?;
            let angle = spin.degrees_per_second.to_radians() * dt.as_secs_f32();
            local.rotation =
                (local.rotation * crate::Quat::from_axis_angle(spin.axis, angle)).normalize();
            world.set_local_transform(id, local)?;
        }
        crate::brain::apply_looks(world, dt)?;
        // Cameras last, so they track where everything else has just moved.
        let seconds = dt.as_secs_f32();
        for &(id, camera) in &scratch.cameras {
            let Ok(target) = world.global_transform(camera.target).copied() else {
                continue;
            };
            let centre = target.translation();
            let goal = if camera.behind {
                let ahead = target.matrix().transform_vector3(Vec3::NEG_Z);
                let forward = Vec3::new(ahead.x, 0.0, ahead.z)
                    .try_normalize()
                    .unwrap_or(Vec3::NEG_Z);
                centre + forward.cross(Vec3::Y) * camera.offset.x + Vec3::Y * camera.offset.y
                    - forward * camera.offset.z
            } else {
                centre + camera.offset
            };
            let here = world.global_transform(id)?.translation();
            let blend = if camera.smoothing > 0.0 {
                1.0 - (-seconds / camera.smoothing).exp()
            } else {
                1.0
            };
            let next = here.lerp(goal, blend);
            let mut local = world.local_from_global_position(id, next)?;
            if camera.look {
                if let Some(rotation) = look_rotation(centre - next) {
                    // Written in the parent's space; cameras under rotated parents aim approximately.
                    local.rotation = rotation;
                }
            }
            world.set_local_transform(id, local)?;
        }
        if !scratch.looks.is_empty() {
            world.propagate_transforms();
        }
        for &(id, look) in &scratch.looks {
            let Ok(target) = world.global_transform(look.target).map(|t| t.translation()) else {
                continue;
            };
            face(world, id, target + look.offset, look.turn_rate, seconds)?;
        }
        Ok(())
    })();
    world.restore_motion_scratch(scratch);
    result
}
