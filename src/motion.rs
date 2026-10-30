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
    /// Upward speed applied by Space when the body is not moving vertically; 0 disables jumping.
    pub jump_speed: f32,
    /// Move relative to where the active camera looks (third person), or along world axes.
    pub camera_relative: bool,
    /// Newtons pushing a dynamic body toward `speed`; releasing keys leaves it to friction.
    /// 0 sets the velocity directly (instant start and stop). Bodies without physics ignore it.
    pub force: f32,
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

/// The camera that defines "forward" for camera-relative controls. Set by scene loading.
#[derive(Clone, Copy, Debug, bevy_ecs::resource::Resource)]
pub struct ActiveCamera(pub EntityId);

#[derive(Default)]
pub(crate) struct MotionScratch {
    pub(crate) controllers: Vec<(EntityId, CharacterController)>,
    pub(crate) follows: Vec<(EntityId, Follow)>,
    pub(crate) spins: Vec<(EntityId, Spin)>,
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
    let scratch = world.take_motion_scratch();
    let result = (|| {
        for &(id, controller) in &scratch.controllers {
            let (forward, right) = basis(world, controller.camera_relative);
            let wish = forward * (input.axis(Key::S, Key::W) + input.axis(Key::Down, Key::Up))
                + right * (input.axis(Key::A, Key::D) + input.axis(Key::Left, Key::Right));
            let velocity = wish.normalize_or_zero() * controller.speed;
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
        Ok(())
    })();
    world.restore_motion_scratch(scratch);
    result
}
