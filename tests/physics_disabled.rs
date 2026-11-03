#![cfg(not(feature = "physics"))]
use emergent::{PhysicsError, Vec3, World};

#[test]
fn force_and_torque_commands_report_disabled_physics() {
    let mut world = World::new();
    let id = world.spawn("unpowered");
    assert_eq!(
        world.add_force(id, Vec3::Y),
        Err(PhysicsError::FeatureDisabled)
    );
    assert_eq!(
        world.add_torque(id, Vec3::Y),
        Err(PhysicsError::FeatureDisabled)
    );
}

#[test]
fn physical_profiles_and_programmatic_boundary_cannot_fall_back_without_physics() {
    use emergent::locomotion::Model;
    use emergent::{
        Engine, EngineConfig, EngineContext, EngineError, Game, LocomotionAgent, LocomotionProfile,
    };
    assert!(LocomotionProfile::from_source(
        "physical.locomotion.toml",
        "schema_version=1\nid='p'\ntype='drone_physics'"
    )
    .unwrap_err()
    .contains("optional physics feature"));
    let mut engine = Engine::new(EngineConfig::default()).unwrap();
    let id = engine.world_mut().spawn("physical");
    engine
        .world_mut()
        .insert_component(id, LocomotionAgent::new(Model::DronePhysics))
        .unwrap();
    struct Idle;
    impl Game for Idle {
        fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
    }
    assert_eq!(
        engine.run_ticks(&mut Idle, 1),
        Err(EngineError::Physics(PhysicsError::FeatureDisabled))
    );
}
