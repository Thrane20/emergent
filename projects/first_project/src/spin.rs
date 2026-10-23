use std::time::Duration;

use emergent::{EntityId, Quat, Transform, World, WorldError};

/// The project's default rate for deliberately slow authored rotation.
pub const SLOW_SPIN_DEGREES_PER_SECOND: f32 = 15.0;

/// Rotates an entity around its local Z axis using fixed-step simulation time.
///
/// Translation and scale are copied from the current local transform so this
/// behavior owns only rotation. `entity` remains the canonical runtime
/// `EntityId` resolved when the generated scene is constructed.
pub fn rotate_z(
    world: &mut World,
    entity: EntityId,
    degrees_per_second: f32,
    fixed_delta: Duration,
) -> Result<(), WorldError> {
    let current = *world.local_transform(entity)?;
    let radians = degrees_per_second.to_radians() * fixed_delta.as_secs_f32();
    let rotation = (current.rotation * Quat::from_rotation_z(radians)).normalize();

    world.set_local_transform(
        entity,
        Transform::new(current.translation, rotation, current.scale),
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use emergent::{Quat, Transform, Vec3, World};

    use super::{rotate_z, SLOW_SPIN_DEGREES_PER_SECOND};

    #[test]
    fn z_rotation_uses_fixed_delta_and_preserves_translation_and_scale() {
        let mut world = World::new();
        let cube = world.spawn("Cube");
        let translation = Vec3::new(2.0, -3.0, 4.0);
        let scale = Vec3::new(1.5, 2.0, 0.5);
        let initial_rotation = Quat::from_rotation_x(0.25);
        world
            .set_local_transform(cube, Transform::new(translation, initial_rotation, scale))
            .unwrap();

        rotate_z(
            &mut world,
            cube,
            SLOW_SPIN_DEGREES_PER_SECOND,
            Duration::from_secs(2),
        )
        .unwrap();

        let transform = *world.local_transform(cube).unwrap();
        let expected =
            (initial_rotation * Quat::from_rotation_z(30.0_f32.to_radians())).normalize();
        assert_eq!(transform.translation, translation);
        assert_eq!(transform.scale, scale);
        assert!((transform.rotation.dot(expected).abs() - 1.0).abs() < 1.0e-5);
    }
}
