use std::time::Duration;

use emergent::{EntityId, PhysicsError, Vec3, World, WorldError};

/// Approximate vertical clearance reached after each impact.
pub const DEFAULT_BOUNCE_HEIGHT: f32 = 4.0;
/// Project gravity used by the deterministic ballistic bounce.
pub const DEFAULT_GRAVITY: f32 = 9.81;
/// Time between the falling cube's occasional jumping-bean pops.
pub const DEFAULT_POP_INTERVAL: Duration = Duration::from_secs(4);
/// Upward impulse in newton-seconds applied to the authored ten-kilogram cube.
pub const DEFAULT_POP_IMPULSE: f32 = 40.0;

/// Fixed-step ballistic motion above a flat authored ground entity.
///
/// The runtime entities are canonical `EntityId`s resolved from durable scene
/// instance IDs. The behavior only changes world position; conversion through
/// `local_from_global_position` retains the current local rotation and scale.
#[derive(Clone, Copy, Debug)]
pub struct GroundBounce {
    entity: EntityId,
    ground: EntityId,
    local_half_extent: f32,
    gravity: f32,
    launch_speed: f32,
    vertical_speed: f32,
}

impl GroundBounce {
    pub fn new(entity: EntityId, ground: EntityId, local_half_extent: f32) -> Self {
        Self::with_height(entity, ground, local_half_extent, DEFAULT_BOUNCE_HEIGHT)
    }

    pub fn with_height(
        entity: EntityId,
        ground: EntityId,
        local_half_extent: f32,
        height: f32,
    ) -> Self {
        debug_assert!(local_half_extent.is_finite() && local_half_extent > 0.0);
        debug_assert!(height.is_finite() && height > 0.0);
        Self {
            entity,
            ground,
            local_half_extent,
            gravity: DEFAULT_GRAVITY,
            launch_speed: (2.0 * DEFAULT_GRAVITY * height).sqrt(),
            vertical_speed: 0.0,
        }
    }

    /// Advances falling motion and applies an upward launch at ground contact.
    ///
    /// Returns `true` on a tick that contacts the ground. This project-owned
    /// implementation is used because the public physics contract exposes
    /// force submission but not a contact/velocity query needed to calculate a
    /// repeatable four-metre impulse.
    pub fn update(&mut self, world: &mut World, fixed_delta: Duration) -> Result<bool, WorldError> {
        let seconds = fixed_delta.as_secs_f32();
        let current = world.global_transform(self.entity)?.translation();
        let entity_matrix = world.global_transform(self.entity)?.matrix();
        let ground_y = world.global_transform(self.ground)?.translation().y;

        // Vertical support of a transformed cube. This remains correct if its
        // authored rotation or non-uniform scale changes while bouncing.
        let support = self.local_half_extent
            * (entity_matrix.x_axis.y.abs()
                + entity_matrix.y_axis.y.abs()
                + entity_matrix.z_axis.y.abs());
        let floor_y = ground_y + support;

        self.vertical_speed -= self.gravity * seconds;
        let mut next = current + Vec3::Y * (self.vertical_speed * seconds);
        let contacted = next.y <= floor_y && self.vertical_speed <= 0.0;
        if contacted {
            next.y = floor_y;
            self.vertical_speed = self.launch_speed;
        }

        let local = world.local_from_global_position(self.entity, next)?;
        world.set_local_transform(self.entity, local)?;
        Ok(contacted)
    }
}

/// An occasional upward impulse for a physics-owned dynamic entity.
///
/// The interval is measured exclusively with fixed simulation time. At each
/// interval the desired impulse is converted to a one-tick force, keeping the
/// launch independent of the configured fixed timestep. Physics retains full
/// ownership of translation, rotation, scale, collisions, and settling.
#[derive(Clone, Copy, Debug)]
pub struct PeriodicPop {
    entity: EntityId,
    interval: Duration,
    elapsed: Duration,
    impulse: f32,
}

impl PeriodicPop {
    pub const fn new(entity: EntityId) -> Self {
        Self {
            entity,
            interval: DEFAULT_POP_INTERVAL,
            elapsed: Duration::ZERO,
            impulse: DEFAULT_POP_IMPULSE,
        }
    }

    /// Advances the fixed-step timer and submits one upward impulse when due.
    ///
    /// Returns `true` when a pop was submitted. Resetting the timer after a pop
    /// keeps the work bounded even if a caller supplies an unusually large
    /// fixed delta.
    pub fn update(
        &mut self,
        world: &mut World,
        fixed_delta: Duration,
    ) -> Result<bool, PhysicsError> {
        self.elapsed = self.elapsed.saturating_add(fixed_delta);
        if self.elapsed < self.interval {
            return Ok(false);
        }

        self.elapsed = Duration::ZERO;
        let seconds = fixed_delta.as_secs_f32();
        if seconds <= 0.0 {
            return Ok(false);
        }

        let force = Vec3::Y * (self.impulse / seconds);
        world.add_force(self.entity, force)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use emergent::{Quat, Transform, Vec3, World};

    use super::{GroundBounce, DEFAULT_BOUNCE_HEIGHT};

    #[test]
    fn impact_launches_about_four_metres_and_preserves_rotation_and_scale() {
        let mut world = World::new();
        let ground = world.spawn("Ground");
        let cube = world.spawn("Bouncer");
        let rotation = Quat::from_rotation_z(0.2);
        let scale = Vec3::new(1.25, 1.0, 0.75);
        world
            .set_local_transform(
                ground,
                Transform::from_translation(Vec3::new(0.0, -1.0, 0.0)),
            )
            .unwrap();
        world
            .set_local_transform(
                cube,
                Transform::new(Vec3::new(2.0, 0.0, 3.0), rotation, scale),
            )
            .unwrap();
        world.propagate_transforms();

        let mut bounce = GroundBounce::new(cube, ground, 0.5);
        let step = Duration::from_secs_f64(1.0 / 60.0);
        let mut contacted = false;
        let mut floor = 0.0;
        let mut apex = f32::NEG_INFINITY;
        for _ in 0..240 {
            if bounce.update(&mut world, step).unwrap() {
                contacted = true;
                floor = world.local_transform(cube).unwrap().translation.y;
                apex = floor;
            }
            world.propagate_transforms();
            if contacted {
                apex = apex.max(world.global_transform(cube).unwrap().translation().y);
                if bounce.vertical_speed < 0.0 {
                    break;
                }
            }
        }

        let transform = *world.local_transform(cube).unwrap();
        assert!(contacted);
        assert!(((apex - floor) - DEFAULT_BOUNCE_HEIGHT).abs() < 0.15);
        assert_eq!(transform.translation.x, 2.0);
        assert_eq!(transform.translation.z, 3.0);
        assert_eq!(transform.rotation, rotation);
        assert_eq!(transform.scale, scale);
    }
}
