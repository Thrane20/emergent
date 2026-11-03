#![cfg(feature = "physics")]
use emergent::locomotion::{simulate, type_catalogue, Course, Model};
use emergent::{
    Engine, EngineConfig, EngineContext, EngineError, EntityId, Game, LocomotionAgent,
    LocomotionProfile, PhysicsBody, PhysicsError, Project, SceneDefinition, Spin, Vec3,
    PHYSICS_GRAVITY,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "emergent-flight-boundary-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for dir in [
            "assets/locomotion",
            "assets/brains",
            "assets/scenes",
            "src/generated",
        ] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        fs::write(
            root.join("emergent.toml"),
            include_str!("../templates/standard/emergent.toml"),
        )
        .unwrap();
        fs::write(root.join("assets/cube.entity.toml"), "schema_version=1\nid='cube'\nkind='primitive'\n[geometry]\nprimitive='cube'\nhalf_extent=0.5\ncolor=[1,1,1,1]\n").unwrap();
        fs::write(
            root.join("assets/locomotion/physical.locomotion.toml"),
            "schema_version=1\nid='physical'\nkind='locomotion'\ntype='drone_physics'\n",
        )
        .unwrap();
        fs::write(root.join("assets/brains/hold.brain.toml"), "schema_version=1\nid='hold'\nkind='brain'\n[[nodes]]\nid='alt'\ntype='altitude'\nheight=5\n[[nodes]]\nid='hover'\nparent='alt'\ntype='hover'\n").unwrap();
        fs::write(
            root.join("assets/brains/look.brain.toml"),
            "schema_version=1\nid='look'\nkind='brain'\n[[nodes]]\nid='look'\ntype='look_along'\n",
        )
        .unwrap();
        Self(root)
    }
    fn scene(
        &self,
        physics: &str,
        extra: &str,
        scale: &str,
    ) -> Result<SceneDefinition, emergent::SceneError> {
        fs::write(self.0.join("assets/scenes/main.scene.toml"), format!("schema_version=1\nid='main'\n[[entities]]\nid='drone'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[0,5,0]\nscale={scale}\n{physics}\n[[entities.components]]\ntype='emergent.locomotion'\nprofile='assets/locomotion/physical.locomotion.toml'\n{extra}\n")).unwrap();
        SceneDefinition::load(
            &Project::load(&self.0).unwrap(),
            "assets/scenes/main.scene.toml",
        )
    }
    fn engine(&self, extra: &str) -> (Engine, EntityId) {
        let scene = self
            .scene("[entities.physics]\nbody='dynamic'", extra, "[1,1,1]")
            .unwrap();
        let mut engine = Engine::new(EngineConfig::default()).unwrap();
        let loaded = scene.instantiate(engine.world_mut()).unwrap();
        (engine, loaded.entities["drone"])
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct Idle;
impl Game for Idle {
    fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
}

#[test]
fn physical_profile_is_loadable_boundary_not_available_flight_or_preview() {
    let spec = type_catalogue()
        .into_iter()
        .find(|t| t.id == "drone_physics")
        .unwrap();
    assert_eq!(spec.status, "boundary");
    assert!(spec.manoeuvres.is_empty() && spec.fields.is_empty());
    let p = LocomotionProfile::from_source(
        "x.locomotion.toml",
        "schema_version=1\nid='x'\ntype='drone_physics'",
    )
    .unwrap();
    assert_eq!(p.model, Model::DronePhysics);
    assert!(simulate(&p, Course::Stop, 2.0)
        .unwrap_err()
        .to_string()
        .contains("unpowered boundary"));
    assert!(LocomotionProfile::from_source(
        "x.locomotion.toml",
        "schema_version=1\nid='x'\ntype='drone_physics'\nmass=2"
    )
    .unwrap_err()
    .contains("no field 'mass'"));
}

#[test]
fn scene_validates_body_feature_scale_and_pose_owners_by_mode() {
    let f = Fixture::new();
    for physics in [
        "",
        "[entities.physics]\nbody='static'",
        "[entities.physics]\nenabled=false",
    ] {
        let error = f.scene(physics, "", "[1,1,1]").err().unwrap().to_string();
        assert!(error.contains("requires an enabled dynamic"), "{error}");
    }
    for scale in ["[1,2,1]", "[-1,-1,-1]"] {
        let error = f
            .scene("[entities.physics]\nbody='dynamic'", "", scale)
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("uniform local scale"), "{error}");
    }
    let extra = "[[entities.components]]\ntype='emergent.spin'";
    assert!(f
        .scene("[entities.physics]\nbody='dynamic'", extra, "[1,1,1]")
        .err()
        .unwrap()
        .to_string()
        .contains("not with a character controller, follow or spin"));
    let look =
        "[[entities.components]]\ntype='emergent.brain'\nbrain='assets/brains/look.brain.toml'";
    assert!(f
        .scene("[entities.physics]\nbody='dynamic'", look, "[1,1,1]")
        .err()
        .unwrap()
        .to_string()
        .contains("cannot execute brain look goals"));
    fs::write(
        f.0.join("assets/locomotion/physical.locomotion.toml"),
        "schema_version=1\nid='legacy'\ntype='drone'",
    )
    .unwrap();
    assert!(f
        .scene("[entities.physics]\nbody='dynamic'", "", "[1,1,1]")
        .err()
        .unwrap()
        .to_string()
        .contains("another pose or velocity writer"));
    assert!(
        f.scene("", "", "[1,1,1]").is_ok(),
        "legacy drone still loads without physics"
    );
}

#[test]
fn unpowered_boundary_falls_and_publishes_measured_motion_despite_brain_intent() {
    let f = Fixture::new();
    let (mut engine, id) = f.engine(
        "[[entities.components]]\ntype='emergent.brain'\nbrain='assets/brains/hold.brain.toml'",
    );
    assert_eq!(engine.world().physics_body_state(id).unwrap(), None);
    engine.run_ticks(&mut Idle, 60).unwrap();
    let measured = engine.world().physics_body_state(id).unwrap().unwrap();
    let agent = engine
        .world()
        .component::<LocomotionAgent>(id)
        .unwrap()
        .unwrap();
    assert!(measured.transform.translation.y < 0.15, "{measured:?}");
    assert!((measured.linear_velocity.y + 9.81).abs() < 0.01);
    assert_eq!(agent.intent.altitude, Some(5.0));
    assert_eq!(agent.physics_state(), Some(measured));
    assert!((agent.activity().speed - 9.81).abs() < 0.01);
    assert_eq!(agent.activity().rotor, 0.0);
    assert_eq!(measured.mass, 1.0);
    assert_eq!(measured.gravity, PHYSICS_GRAVITY);
}

#[test]
fn brain_conditions_use_actual_self_velocity_instead_of_commanded_velocity() {
    let f = Fixture::new();
    fs::write(f.0.join("assets/brains/hold.brain.toml"), "schema_version=1\nid='view'\n[[nodes]]\nid='view'\ntype='in_view'\ntarget='below'\nangle=10\ndistance=1000\n[[nodes]]\nid='signal'\nparent='view'\ntype='signal'\nname='falling'\ncooldown=0\n").unwrap();
    let scene = f.scene("[entities.physics]\nbody='dynamic'", "[[entities.components]]\ntype='emergent.brain'\nbrain='assets/brains/hold.brain.toml'\n[[entities]]\nid='below'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[0,-100,0]", "[1,1,1]").unwrap();
    let mut engine = Engine::new(EngineConfig::default()).unwrap();
    scene.instantiate(engine.world_mut()).unwrap();
    engine.run_ticks(&mut Idle, 4).unwrap();
    assert!(
        engine.world().signals().any(|(_, name)| name == "falling"),
        "actual downward velocity should point the view condition at the target below"
    );
}

struct Lift {
    id: EntityId,
}
impl Game for Lift {
    fn fixed_update(&mut self, c: &mut EngineContext<'_>) {
        let body = *c.world.component::<PhysicsBody>(self.id).unwrap().unwrap();
        c.world
            .add_force(self.id, -PHYSICS_GRAVITY * body.mass * body.gravity_scale)
            .unwrap();
        if c.tick == 0 {
            c.world.add_torque(self.id, Vec3::Y).unwrap();
        }
    }
}

#[test]
fn first_step_lift_and_torque_work_through_the_authored_physical_boundary() {
    let f = Fixture::new();
    let (mut engine, id) = f.engine("");
    engine.run_ticks(&mut Lift { id }, 120).unwrap();
    let state = engine.world().physics_body_state(id).unwrap().unwrap();
    assert!((state.transform.translation.y - 5.0).abs() < 1e-5);
    assert!((state.angular_velocity.y - 0.1).abs() < 1e-5);
    let agent = engine
        .world()
        .component::<LocomotionAgent>(id)
        .unwrap()
        .unwrap();
    assert_eq!(agent.physics_state(), Some(state));
    assert!(agent.activity().yaw.abs() > 0.1);
}

#[test]
fn reconstruction_invalidates_old_observations_and_changes_revision() {
    let f = Fixture::new();
    let (mut engine, id) = f.engine("");
    engine.run_ticks(&mut Lift { id }, 1).unwrap();
    let old = engine.world().physics_body_state(id).unwrap().unwrap();
    engine
        .world_mut()
        .component_mut::<PhysicsBody>(id)
        .unwrap()
        .unwrap()
        .mass = 2.0;
    assert_eq!(engine.world().physics_body_state(id).unwrap(), None);
    engine.run_ticks(&mut Lift { id }, 1).unwrap();
    let fresh = engine.world().physics_body_state(id).unwrap().unwrap();
    assert_ne!(old.revision, fresh.revision);
    assert_eq!(fresh.angular_velocity, Vec3::ZERO);
    assert_eq!(fresh.mass, 2.0);
    let other = Engine::new(EngineConfig::default()).unwrap();
    assert!(other.world().physics_body_state(id).is_err());
    engine.run_ticks(&mut Lift { id }, 1).unwrap();
    assert_eq!(
        engine
            .world()
            .physics_body_state(id)
            .unwrap()
            .unwrap()
            .revision,
        fresh.revision
    );
    let mut transform = *engine.world().local_transform(id).unwrap();
    transform.scale = Vec3::splat(2.0);
    engine
        .world_mut()
        .set_local_transform(id, transform)
        .unwrap();
    assert_eq!(engine.world().physics_body_state(id).unwrap(), None);
    engine.run_ticks(&mut Lift { id }, 1).unwrap();
    assert_ne!(
        engine
            .world()
            .physics_body_state(id)
            .unwrap()
            .unwrap()
            .revision,
        fresh.revision
    );
    engine
        .world_mut()
        .component_mut::<PhysicsBody>(id)
        .unwrap()
        .unwrap()
        .gravity_scale = 0.5;
    assert_eq!(engine.world().physics_body_state(id).unwrap(), None);
    engine.run_ticks(&mut Lift { id }, 1).unwrap();
    assert_eq!(
        engine
            .world()
            .physics_body_state(id)
            .unwrap()
            .unwrap()
            .gravity,
        PHYSICS_GRAVITY * 0.5
    );
    engine
        .world_mut()
        .remove_component::<LocomotionAgent>(id)
        .unwrap();
    let body = *engine
        .world()
        .component::<PhysicsBody>(id)
        .unwrap()
        .unwrap();
    engine
        .world_mut()
        .remove_component::<PhysicsBody>(id)
        .unwrap();
    assert_eq!(engine.world().physics_body_state(id).unwrap(), None);
    engine.run_ticks(&mut Idle, 1).unwrap();
    engine.world_mut().insert_component(id, body).unwrap();
    engine
        .world_mut()
        .insert_component(id, LocomotionAgent::new(Model::DronePhysics))
        .unwrap();
    engine.run_ticks(&mut Lift { id }, 1).unwrap();
    assert!(
        engine
            .world()
            .physics_body_state(id)
            .unwrap()
            .unwrap()
            .revision
            > fresh.revision
    );
    engine.world_mut().remove_subtree(id).unwrap();
    assert!(engine.world().physics_body_state(id).is_err());
}

#[test]
fn runtime_pose_writers_and_character_drive_cannot_bypass_the_boundary() {
    let f = Fixture::new();
    let (mut engine, id) = f.engine("");
    assert_eq!(
        engine.world_mut().drive_body(id, Vec3::X, None),
        Err(PhysicsError::LocomotionConflict)
    );
    engine
        .world_mut()
        .insert_component(
            id,
            Spin {
                axis: Vec3::Y,
                degrees_per_second: 10.0,
            },
        )
        .unwrap();
    let before = *engine.world().local_transform(id).unwrap();
    assert_eq!(
        engine.run_ticks(&mut Idle, 1),
        Err(EngineError::Physics(PhysicsError::LocomotionConflict))
    );
    assert_eq!(*engine.world().local_transform(id).unwrap(), before);
    engine.world_mut().remove_component::<Spin>(id).unwrap();
    engine
        .world_mut()
        .remove_component::<PhysicsBody>(id)
        .unwrap();
    assert_eq!(
        engine.run_ticks(&mut Idle, 1),
        Err(EngineError::Physics(
            PhysicsError::PhysicalLocomotionRequiresDynamicBody
        ))
    );
}
