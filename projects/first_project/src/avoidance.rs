use emergent::{EntityId, Vec3, World, WorldError};

/// Required world-space clearance between the Defender and its threat.
pub const MINIMUM_ESCAPE_DISTANCE: f32 = 2.0;

/// Canonical runtime identities for a project-owned minimum-distance rule.
///
/// The identities are resolved from durable authored scene IDs during scene
/// construction. They are never serialized or used as replacement identity.
#[derive(Clone, Copy, Debug)]
pub struct DistanceAvoidance {
    defender: EntityId,
    threat: EntityId,
}

impl DistanceAvoidance {
    pub const fn new(defender: EntityId, threat: EntityId) -> Self {
        Self { defender, threat }
    }

    /// Restores the minimum world-space clearance if the threat comes nearer.
    ///
    /// This is a positional constraint rather than a rate, so it does not
    /// introduce frame-dependent movement. `local_from_global_position`
    /// preserves the Defender's current local rotation and scale.
    /// Dynamic rigid bodies retain physics ownership of their pose; this
    /// positional rule resumes when dynamic physics is removed.
    pub fn update(self, world: &mut World) -> Result<bool, WorldError> {
        if world
            .component::<emergent::PhysicsBody>(self.defender)?
            .is_some_and(|body| body.body == emergent::BodyKind::Dynamic)
        {
            return Ok(false);
        }
        let defender_position = world.global_transform(self.defender)?.translation();
        let threat_position = world.global_transform(self.threat)?.translation();
        let offset = defender_position - threat_position;
        let distance_squared = offset.length_squared();
        let minimum_squared = MINIMUM_ESCAPE_DISTANCE * MINIMUM_ESCAPE_DISTANCE;

        if distance_squared >= minimum_squared {
            return Ok(false);
        }

        // Exact overlap has no geometric away direction. Choose a stable axis
        // so the correction remains deterministic in headless and native play.
        let away = if distance_squared > f32::EPSILON {
            offset / distance_squared.sqrt()
        } else {
            Vec3::NEG_X
        };
        let desired_position = threat_position + away * MINIMUM_ESCAPE_DISTANCE;
        let local = world.local_from_global_position(self.defender, desired_position)?;
        world.set_local_transform(self.defender, local)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use emergent::{Quat, Transform, Vec3, World};

    use super::{DistanceAvoidance, MINIMUM_ESCAPE_DISTANCE};

    #[test]
    fn dynamic_defender_runs_under_gravity_without_a_competing_pose_write() {
        use emergent::{
            BodyKind, BoxCollider, Engine, EngineConfig, EngineContext, Game, PhysicsBody,
        };
        struct AvoidanceGame(DistanceAvoidance);
        impl Game for AvoidanceGame {
            fn fixed_update(&mut self, context: &mut EngineContext<'_>) {
                self.0.update(context.world).unwrap();
            }
        }
        let mut engine = Engine::new(EngineConfig::default()).unwrap();
        let defender = engine.world_mut().spawn("Defender");
        let threat = engine.world_mut().spawn("OffenderBox");
        engine
            .world_mut()
            .set_local_transform(threat, Transform::from_translation(Vec3::new(1., -3., 0.)))
            .unwrap();
        engine
            .world_mut()
            .insert_component(
                defender,
                PhysicsBody::new(
                    BodyKind::Dynamic,
                    BoxCollider {
                        center: [0.; 3],
                        half_extents: [0.5; 3],
                    },
                )
                .unwrap(),
            )
            .unwrap();
        let avoidance = DistanceAvoidance::new(defender, threat);
        engine
            .run_ticks(&mut AvoidanceGame(avoidance), 120)
            .unwrap();
        let position = engine
            .world()
            .global_transform(defender)
            .unwrap()
            .translation();
        assert_eq!(position.x, 0.);
        assert!(
            position.y < -10.,
            "gravity should own movement: {position:?}"
        );
        engine
            .world_mut()
            .remove_component::<PhysicsBody>(defender)
            .unwrap();
        engine
            .world_mut()
            .set_local_transform(threat, Transform::from_translation(position + Vec3::X))
            .unwrap();
        engine.world_mut().propagate_transforms();
        assert!(
            avoidance.update(engine.world_mut()).unwrap(),
            "positional avoidance resumes after physics is disabled"
        );
    }

    #[test]
    fn restores_clearance_and_preserves_local_rotation_and_scale() {
        let mut world = World::new();
        let defender = world.spawn("Defender");
        let threat = world.spawn("OffenderBox");
        let rotation = Quat::from_rotation_y(0.4);
        let scale = Vec3::new(1.5, 0.75, 2.0);
        world
            .set_local_transform(defender, Transform::new(Vec3::ZERO, rotation, scale))
            .unwrap();
        world
            .set_local_transform(
                threat,
                Transform::from_translation(Vec3::new(1.0, 0.0, 0.0)),
            )
            .unwrap();
        world.propagate_transforms();

        assert!(DistanceAvoidance::new(defender, threat)
            .update(&mut world)
            .unwrap());
        world.propagate_transforms();

        let transform = *world.local_transform(defender).unwrap();
        let distance = world
            .global_transform(defender)
            .unwrap()
            .translation()
            .distance(world.global_transform(threat).unwrap().translation());
        assert!((distance - MINIMUM_ESCAPE_DISTANCE).abs() < 1.0e-5);
        assert_eq!(transform.rotation, rotation);
        assert_eq!(transform.scale, scale);
    }

    #[test]
    fn leaves_a_defender_outside_the_clearance_unchanged() {
        let mut world = World::new();
        let defender = world.spawn("Defender");
        let threat = world.spawn("OffenderBox");
        world
            .set_local_transform(
                defender,
                Transform::from_translation(Vec3::new(-3.0, 0.0, 0.0)),
            )
            .unwrap();
        world.propagate_transforms();
        let before = *world.local_transform(defender).unwrap();

        assert!(!DistanceAvoidance::new(defender, threat)
            .update(&mut world)
            .unwrap());
        assert_eq!(*world.local_transform(defender).unwrap(), before);
    }
}
