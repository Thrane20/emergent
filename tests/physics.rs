#![cfg(feature = "physics")]
use emergent::{
    BodyKind, BoxCollider, Engine, EngineConfig, EngineContext, EngineError, Game, PhysicsBody,
    PhysicsError, Quat, Transform, Vec3,
};
struct Idle;
impl Game for Idle {
    fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
}
fn body(kind: BodyKind, half: [f32; 3]) -> PhysicsBody {
    PhysicsBody::new(
        kind,
        BoxCollider {
            center: [0.0; 3],
            half_extents: half,
        },
    )
    .unwrap()
}
struct ForceGame {
    id: emergent::EntityId,
    force: Vec3,
}
impl Game for ForceGame {
    fn fixed_update(&mut self, context: &mut EngineContext<'_>) {
        context.world.add_force(self.id, self.force).unwrap();
    }
}
fn spawn(
    engine: &mut Engine,
    kind: BodyKind,
    position: Vec3,
    half: [f32; 3],
) -> emergent::EntityId {
    let id = engine.world_mut().spawn("body");
    engine
        .world_mut()
        .set_local_transform(id, Transform::from_translation(position))
        .unwrap();
    engine
        .world_mut()
        .insert_component(id, body(kind, half))
        .unwrap();
    id
}
#[test]
fn gravity_contact_pause_reset_and_removed_ground() {
    let mut engine = Engine::new(EngineConfig::default()).unwrap();
    let ground = spawn(
        &mut engine,
        BodyKind::Static,
        Vec3::new(0.0, -0.5, 0.0),
        [10.0, 0.5, 10.0],
    );
    let cube = spawn(
        &mut engine,
        BodyKind::Dynamic,
        Vec3::new(0.0, 5.0, 0.0),
        [0.5; 3],
    );
    engine.start(&mut Idle).unwrap();
    for _ in 0..10 {
        engine.world_mut().propagate_transforms();
    }
    assert_eq!(engine.tick(), 0);
    assert_eq!(
        engine.world().local_transform(cube).unwrap().translation.y,
        5.0
    );
    engine.stop(&mut Idle);
    engine.run_ticks(&mut Idle, 30).unwrap();
    let falling = engine
        .world()
        .global_transform(cube)
        .unwrap()
        .translation()
        .y;
    assert!(falling < 4.0 && falling > 3.0, "{falling}");
    engine.run_ticks(&mut Idle, 330).unwrap();
    let resting = engine
        .world()
        .global_transform(cube)
        .unwrap()
        .translation()
        .y;
    assert!((resting - 0.5).abs() < 0.03, "{resting}");
    assert_eq!(
        engine
            .world()
            .local_transform(ground)
            .unwrap()
            .translation
            .y,
        -0.5
    );
    engine.world_mut().remove_subtree(ground).unwrap();
    // Force a replacement slot; the old collider must not survive deletion.
    let replacement = engine.world_mut().spawn("replacement");
    assert_ne!(replacement, ground);
    engine.run_ticks(&mut Idle, 120).unwrap();
    assert!(
        engine
            .world()
            .global_transform(cube)
            .unwrap()
            .translation()
            .y
            < -5.0
    );
    let mut reset = Engine::new(EngineConfig::default()).unwrap();
    let fresh = spawn(
        &mut reset,
        BodyKind::Dynamic,
        Vec3::new(0.0, 5.0, 0.0),
        [0.5; 3],
    );
    assert_eq!(
        reset.world().local_transform(fresh).unwrap().translation.y,
        5.0
    );
}
#[test]
fn zero_gravity_and_scaled_offset_box() {
    let mut e = Engine::new(EngineConfig::default()).unwrap();
    let id = spawn(&mut e, BodyKind::Dynamic, Vec3::Y * 4.0, [0.5; 3]);
    e.world_mut()
        .component_mut::<PhysicsBody>(id)
        .unwrap()
        .unwrap()
        .gravity_scale = 0.0;
    e.run_ticks(&mut Idle, 120).unwrap();
    assert_eq!(e.world().local_transform(id).unwrap().translation.y, 4.0);
    e.world_mut().remove_component::<PhysicsBody>(id).unwrap();
    e.run_ticks(&mut Idle, 1).unwrap();
    let ground = spawn(&mut e, BodyKind::Static, Vec3::ZERO, [5.0, 0.5, 5.0]);
    e.world_mut()
        .set_local_transform(ground, Transform::from_scale(Vec3::new(2.0, 2.0, 2.0)))
        .unwrap();
    let mut settings = body(BodyKind::Dynamic, [0.5; 3]);
    let mut collider = settings.collider.as_box().unwrap();
    collider.center = [0., 0.5, 0.];
    settings.collider = collider.into();
    e.world_mut().insert_component(id, settings).unwrap();
    e.run_ticks(&mut Idle, 360).unwrap();
    assert!((e.world().local_transform(id).unwrap().translation.y - 1.0).abs() < 0.04);
}

#[test]
fn forces_are_applied_for_each_fixed_tick_and_counter_gravity() {
    let mut engine = Engine::new(EngineConfig::default()).unwrap();
    let id = spawn(&mut engine, BodyKind::Dynamic, Vec3::Y * 5.0, [0.5; 3]);
    let mut game = ForceGame {
        id,
        force: Vec3::new(0.0, 9.81, 0.0),
    };
    engine.run_ticks(&mut game, 120).unwrap();
    let hover_height = engine.world().local_transform(id).unwrap().translation.y;
    assert!((hover_height - 5.0).abs() < 0.05, "{hover_height}");

    game.force = Vec3::new(0.0, 20.0, 0.0);
    engine.run_ticks(&mut game, 60).unwrap();
    let rising_height = engine.world().local_transform(id).unwrap().translation.y;
    assert!(rising_height > hover_height + 4.0, "{rising_height}");
}

#[test]
fn force_commands_require_finite_force_on_dynamic_bodies() {
    let mut engine = Engine::new(EngineConfig::default()).unwrap();
    let dynamic = spawn(&mut engine, BodyKind::Dynamic, Vec3::ZERO, [0.5; 3]);
    let fixed = spawn(&mut engine, BodyKind::Static, Vec3::ZERO, [0.5; 3]);
    assert_eq!(
        engine.world_mut().add_force(dynamic, Vec3::splat(f32::NAN)),
        Err(PhysicsError::InvalidForce)
    );
    assert_eq!(
        engine.world_mut().add_force(fixed, Vec3::Y),
        Err(PhysicsError::NotDynamicBody)
    );
}

#[test]
fn rejects_conflicting_pose_writes_invalid_settings_and_parenting() {
    let mut e = Engine::new(EngineConfig::default()).unwrap();
    let id = spawn(&mut e, BodyKind::Dynamic, Vec3::Y * 4.0, [0.5; 3]);
    e.run_ticks(&mut Idle, 1).unwrap();
    e.world_mut()
        .set_local_transform(id, Transform::from_translation(Vec3::Y * 3.0))
        .unwrap();
    assert_eq!(
        e.run_ticks(&mut Idle, 1),
        Err(EngineError::Physics(PhysicsError::TransformConflict))
    );
    e.world_mut()
        .component_mut::<PhysicsBody>(id)
        .unwrap()
        .unwrap()
        .mass = f32::NAN;
    assert_eq!(
        e.run_ticks(&mut Idle, 1),
        Err(EngineError::Physics(PhysicsError::InvalidSettings))
    );
    e.world_mut()
        .insert_component(id, body(BodyKind::Dynamic, [0.5; 3]))
        .unwrap();
    let parent = e.world_mut().spawn("parent");
    e.world_mut()
        .set_local_transform(parent, Transform::from_translation(Vec3::X))
        .unwrap();
    e.world_mut().reparent(id, Some(parent)).unwrap();
    assert_eq!(
        e.run_ticks(&mut Idle, 1),
        Err(EngineError::Physics(PhysicsError::UnsupportedParent))
    );
}
#[test]
fn reflected_and_sheared_transforms_are_rejected() {
    let mut e = Engine::new(EngineConfig::default()).unwrap();
    let id = spawn(&mut e, BodyKind::Static, Vec3::ZERO, [0.5; 3]);
    e.world_mut()
        .set_local_transform(id, Transform::from_scale(Vec3::new(-1.0, 1.0, 1.0)))
        .unwrap();
    assert_eq!(
        e.run_ticks(&mut Idle, 1),
        Err(EngineError::Physics(PhysicsError::UnsupportedTransform))
    );
    let parent = e.world_mut().spawn("parent");
    e.world_mut()
        .set_local_transform(parent, Transform::from_scale(Vec3::new(2.0, 1.0, 1.0)))
        .unwrap();
    e.world_mut()
        .set_local_transform(id, Transform::from_rotation(Quat::from_rotation_z(0.7)))
        .unwrap();
    e.world_mut().reparent(id, Some(parent)).unwrap();
    assert_eq!(
        e.run_ticks(&mut Idle, 1),
        Err(EngineError::Physics(PhysicsError::UnsupportedTransform))
    );
}

#[test]
fn rotated_dynamic_body_preserves_scale_and_freefall_velocity() {
    let mut e = Engine::new(EngineConfig::default()).unwrap();
    let id = spawn(&mut e, BodyKind::Dynamic, Vec3::Y * 20.0, [0.5; 3]);
    let scale = Vec3::new(1.1, 0.7, 2.3);
    e.world_mut()
        .set_local_transform(
            id,
            Transform::new(
                Vec3::Y * 20.0,
                Quat::from_euler(emergent::EulerRot::XYZ, 0.3, 0.6, 0.7),
                scale,
            ),
        )
        .unwrap();
    e.run_ticks(&mut Idle, 60).unwrap();
    let t = e.world().local_transform(id).unwrap();
    assert!((t.translation.y - 15.0).abs() < 0.2, "{}", t.translation.y);
    assert_eq!(t.scale, scale);
}

#[test]
fn dynamic_double_negative_scale_is_rejected_before_backend_shape_creation() {
    let mut e = Engine::new(EngineConfig::default()).unwrap();
    let id = spawn(&mut e, BodyKind::Dynamic, Vec3::ZERO, [0.5; 3]);
    e.world_mut()
        .set_local_transform(id, Transform::from_scale(Vec3::new(-1.0, -1.0, 1.0)))
        .unwrap();
    assert_eq!(
        e.run_ticks(&mut Idle, 1),
        Err(EngineError::Physics(PhysicsError::UnsupportedTransform))
    );
}

#[test]
fn terrain_heightfield_contacts_follow_scaled_translated_ground_and_remove_cleanly() {
    use emergent::{TerrainCollider, TerrainSettings};
    let recipe = TerrainSettings {
        resolution: 32,
        size: 40.,
        height: 0.3,
        ..Default::default()
    };
    let field = recipe.generate().unwrap();
    let mut engine = Engine::new(EngineConfig::default()).unwrap();
    let ground = engine.world_mut().spawn("terrain");
    let transform = Transform::new(
        Vec3::new(2., 1., -3.),
        Quat::IDENTITY,
        Vec3::new(1.5, 2., 0.8),
    );
    engine
        .world_mut()
        .set_local_transform(ground, transform)
        .unwrap();
    engine
        .world_mut()
        .insert_component(
            ground,
            PhysicsBody::new(BodyKind::Static, TerrainCollider { settings: recipe }).unwrap(),
        )
        .unwrap();
    assert!(PhysicsBody::new(BodyKind::Dynamic, TerrainCollider { settings: recipe }).is_err());
    let cube = spawn(
        &mut engine,
        BodyKind::Dynamic,
        Vec3::new(3., 5., -1.),
        [0.5; 3],
    );
    engine.run_ticks(&mut Idle, 300).unwrap();
    let position = engine.world().local_transform(cube).unwrap().translation;
    let x = ((position.x - transform.translation.x) / transform.scale.x / 40. + 0.5) * 31.;
    let z = ((position.z - transform.translation.z) / transform.scale.z / 40. + 0.5) * 31.;
    assert!(x > 1. && x < 30. && z > 1. && z < 30.);
    let ix = x.floor() as usize;
    let iz = z.floor() as usize;
    let fx = x - ix as f32;
    let fz = z - iz as f32;
    let h = field.values();
    let i = iz * 32 + ix;
    let height = if fx + fz <= 1. {
        h[i] * (1. - fx - fz) + h[i + 1] * fx + h[i + 32] * fz
    } else {
        h[i + 33] * (fx + fz - 1.) + h[i + 32] * (1. - fx) + h[i + 1] * (1. - fz)
    };
    let expected = 1. + height * 2. + 0.5;
    assert!(
        (position.y - expected).abs() < 0.08,
        "resting {} expected {}",
        position.y,
        expected
    );
    engine.world_mut().remove_subtree(ground).unwrap();
    engine.run_ticks(&mut Idle, 120).unwrap();
    assert!(engine.world().local_transform(cube).unwrap().translation.y < 0.);
}
