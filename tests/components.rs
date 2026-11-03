//! Authored motion components: loading rules and behaviour under scripted input.
use emergent::{
    Engine, EngineConfig, EngineContext, Game, Key, KeySet, Project, SceneDefinition, Vec3, World,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(assets: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "emergent-components-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::create_dir_all(root.join("src/generated")).unwrap();
        fs::write(root.join("emergent.toml"), "schema_version = 1\nid = 'fixture'\nname = 'Fixture'\nentry_scene = 'assets/main.scene.toml'\ngenerated_code = 'src/generated'\nasset_roots = ['assets']\n[[outputs]]\nid = 'host'\nos = 'macos'\narchitecture = 'aarch64'\nrust_target = 'aarch64-apple-darwin'\nprofile = 'development'\n").unwrap();
        for (name, source) in assets {
            fs::write(root.join("assets").join(name), source).unwrap();
        }
        Self(root)
    }
    fn scene(&self, source: &str) -> Result<SceneDefinition, emergent::SceneError> {
        fs::write(
            self.0.join("assets/main.scene.toml"),
            format!("schema_version = 1\nid = 'main'\n{source}"),
        )
        .unwrap();
        SceneDefinition::load(&Project::load(&self.0).unwrap(), "assets/main.scene.toml")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const CUBE: &str = "schema_version=1\nid='cube'\nkind='primitive'\n[geometry]\nprimitive='cube'\nhalf_extent=0.5\ncolor=[1,1,1,1]\n";
struct Idle;
impl Game for Idle {
    fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
}
fn engine() -> Engine {
    Engine::new(EngineConfig {
        fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
        ..EngineConfig::default()
    })
    .unwrap()
}
fn position(world: &World, id: emergent::EntityId) -> Vec3 {
    world.global_transform(id).unwrap().translation()
}

#[test]
fn loader_attaches_motion_components_and_rejects_conflicts() {
    let f = Fixture::new(&[("cube.entity.toml", CUBE)]);
    let ok = f.scene("[[entities]]\nid='leader'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.character_controller'\n[[entities.components]]\ntype='emergent.spin'\n[[entities]]\nid='tail'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.follow'\ntarget='leader'\n[[entities]]\nid='off'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.spin'\nenabled=false\n").unwrap();
    let mut world = World::new();
    let loaded = ok.instantiate(&mut world).unwrap();
    let leader = loaded.entities["leader"];
    let controller = world
        .component::<emergent::CharacterController>(leader)
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            controller.speed,
            controller.jump_speed,
            controller.camera_relative
        ),
        (4.0, 5.0, true)
    );
    assert!(world.component::<emergent::Spin>(leader).unwrap().is_some());
    let follow = world
        .component::<emergent::Follow>(loaded.entities["tail"])
        .unwrap()
        .unwrap();
    assert_eq!(follow.target, leader);
    assert!(
        world
            .component::<emergent::Spin>(loaded.entities["off"])
            .unwrap()
            .is_none(),
        "disabled components are not attached"
    );
    for (scene, message) in [
        ("[[entities.components]]\ntype='emergent.follow'\n", "requires field 'target'"),
        ("[[entities.components]]\ntype='emergent.follow'\ntarget='ghost'\n", "not in this scene"),
        ("[[entities.components]]\ntype='emergent.follow'\ntarget='a'\n", "its own object"),
        ("[[entities.components]]\ntype='emergent.spin'\naxis=[0,0,0]\n", "must not be zero"),
        ("[[entities.components]]\ntype='emergent.character_controller'\n[[entities.components]]\ntype='emergent.follow'\ntarget='b'\n", "only one of"),
        ("[[entities.components]]\ntype='project.jumping_bean'\n", "unknown component type"),
    ]
    .into_iter()
    .chain(cfg!(feature = "physics").then_some((
        "[physics]\nbody='dynamic'\n[[entities.components]]\ntype='emergent.spin'\n",
        "dynamic physics body",
    ))) {
        let source = format!("[[entities]]\nid='a'\nasset='assets/cube.entity.toml'\n{}[[entities]]\nid='b'\nasset='assets/cube.entity.toml'\n", scene.replace("[physics]", "[entities.physics]"));
        let error = f.scene(&source).err().unwrap().to_string();
        assert!(error.contains(message), "{message}: {error}");
    }
}

#[test]
fn scripted_keys_slide_a_controller_and_follow_and_spin_run_each_tick() {
    let f = Fixture::new(&[("cube.entity.toml", CUBE)]);
    let scene = f.scene("[[entities]]\nid='player'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.character_controller'\nspeed=3\ncamera_relative=false\n[[entities]]\nid='dog'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[-10,0,0]\n[[entities.components]]\ntype='emergent.follow'\ntarget='player'\nspeed=20\nstop_distance=2\n[[entities.components]]\ntype='emergent.spin'\naxis=[0,1,0]\ndegrees_per_second=90\n").unwrap();
    let mut engine = engine();
    let loaded = scene.instantiate(engine.world_mut()).unwrap();
    let (player, dog) = (loaded.entities["player"], loaded.entities["dog"]);
    engine.set_keys(KeySet::EMPTY.with(Key::W).with(Key::D));
    engine.run_ticks(&mut Idle, 60).unwrap();
    let moved = position(engine.world(), player);
    // Diagonal input is normalised: 3 m along (+X, -Z) / sqrt(2).
    assert!(
        moved.abs_diff_eq(Vec3::new(3.0, 0.0, -3.0) / 2f32.sqrt(), 1e-3),
        "{moved}"
    );
    let gap = position(engine.world(), dog).distance(moved);
    // Follow reads the target's previous committed position: one tick of player travel behind.
    assert!(
        (gap - 2.0).abs() <= 3.0 / 60.0 + 1e-4 && gap > 2.0,
        "follower stops at its distance: {gap}"
    );
    let (axis, angle) = engine
        .world()
        .local_transform(dog)
        .unwrap()
        .rotation
        .to_axis_angle();
    assert!(
        axis.abs_diff_eq(Vec3::Y, 1e-4) && (angle.to_degrees() - 90.0).abs() < 0.01,
        "{axis} {angle}"
    );
    engine.set_keys(KeySet::EMPTY);
    engine.run_ticks(&mut Idle, 30).unwrap();
    assert!(
        position(engine.world(), player).abs_diff_eq(moved, 1e-6),
        "no keys, no movement"
    );
}

#[cfg(feature = "physics")]
#[test]
fn a_falling_dynamic_cube_is_steered_by_wasd_and_jumps_once_per_press() {
    let ground = "schema_version=1\nid='ground'\nkind='primitive'\n[geometry]\nprimitive='cube'\nhalf_extent=0.5\ncolor=[1,1,1,1]\n[physics]\nbody='static'\n";
    let f = Fixture::new(&[("cube.entity.toml", CUBE), ("ground.entity.toml", ground)]);
    let scene = f.scene("[[entities]]\nid='ground'\nasset='assets/ground.entity.toml'\n[entities.transform]\nscale=[40,1,40]\ntranslation=[0,-0.5,0]\n[[entities]]\nid='cube'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[0,3,0]\n[entities.physics]\nbody='dynamic'\n[[entities.components]]\ntype='emergent.character_controller'\nspeed=4\njump_speed=5\ncamera_relative=false\n").unwrap();
    let mut engine = engine();
    let loaded = scene.instantiate(engine.world_mut()).unwrap();
    let cube = loaded.entities["cube"];
    // Fall and settle with no keys, then hold W for one second.
    engine.run_ticks(&mut Idle, 90).unwrap();
    let rest = position(engine.world(), cube);
    assert!((rest.y - 0.5).abs() < 0.05, "landed: {rest}");
    engine.set_keys(KeySet::EMPTY.with(Key::W));
    engine.run_ticks(&mut Idle, 60).unwrap();
    let after = position(engine.world(), cube);
    assert!(
        (rest.z - after.z - 4.0).abs() < 0.3,
        "about 4 m forward: {rest} -> {after}"
    );
    assert!(
        (after.y - 0.5).abs() < 0.05 && after.x.abs() < 0.05,
        "still upright on the ground: {after}"
    );
    let rotation = engine.world().local_transform(cube).unwrap().rotation;
    assert!(
        rotation.angle_between(emergent::Quat::IDENTITY) < 0.01,
        "did not tumble: {rotation}"
    );
    // Space held across ticks jumps once.
    engine.set_keys(KeySet::EMPTY.with(Key::Space));
    engine.run_ticks(&mut Idle, 10).unwrap();
    let velocity = engine.world().linear_velocity(cube).unwrap().unwrap();
    let height = position(engine.world(), cube).y;
    assert!(
        height > 1.0 && velocity.y < 5.0,
        "jumped: y={height} v={velocity}"
    );
    engine.run_ticks(&mut Idle, 120).unwrap();
    assert!(
        (position(engine.world(), cube).y - 0.5).abs() < 0.05,
        "landed again and did not re-jump"
    );
}

#[cfg(feature = "physics")]
#[test]
fn a_drive_force_accelerates_by_mass_and_coasts_to_rest_under_friction() {
    let ground = "schema_version=1\nid='ground'\nkind='primitive'\n[geometry]\nprimitive='cube'\nhalf_extent=0.5\ncolor=[1,1,1,1]\n[physics]\nbody='static'\n";
    let f = Fixture::new(&[("cube.entity.toml", CUBE), ("ground.entity.toml", ground)]);
    // 1 kg cube, friction 0.5 on both colliders: coasting decelerates at μ·g ≈ 4.9 m/s².
    let scene = |force: f32| {
        f.scene(&format!("[[entities]]\nid='ground'\nasset='assets/ground.entity.toml'\n[entities.transform]\nscale=[80,1,80]\ntranslation=[0,-0.5,0]\n[[entities]]\nid='cube'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[0,1,0]\n[entities.physics]\nbody='dynamic'\n[[entities.components]]\ntype='emergent.character_controller'\nspeed=4\njump_speed=0\ncamera_relative=false\nforce={force}\n")).unwrap()
    };
    let mut engine = engine();
    let cube = scene(20.0)
        .instantiate(engine.world_mut())
        .unwrap()
        .entities["cube"];
    engine.run_ticks(&mut Idle, 60).unwrap();
    engine.set_keys(KeySet::EMPTY.with(Key::W));
    engine.run_ticks(&mut Idle, 3).unwrap();
    let early = engine
        .world()
        .linear_velocity(cube)
        .unwrap()
        .unwrap()
        .z
        .abs();
    assert!(
        early > 0.1 && early < 2.0,
        "accelerates rather than jumping to speed: {early}"
    );
    engine.run_ticks(&mut Idle, 60).unwrap();
    let cruising = engine
        .world()
        .linear_velocity(cube)
        .unwrap()
        .unwrap()
        .z
        .abs();
    assert!(
        (cruising - 4.0).abs() < 0.2,
        "reaches top speed: {cruising}"
    );
    let released = position(engine.world(), cube);
    engine.set_keys(KeySet::EMPTY);
    engine.run_ticks(&mut Idle, 120).unwrap();
    let slide = released.z - position(engine.world(), cube).z;
    // v² / (2·μ·g) = 16 / 9.81 ≈ 1.63 m.
    assert!((slide - 1.63).abs() < 0.3, "coasts under friction: {slide}");
    let rest = engine.world().linear_velocity(cube).unwrap().unwrap();
    assert!(
        rest.x.abs() < 0.01 && rest.z.abs() < 0.01,
        "friction brings it to rest: {rest}"
    );

    // 3 N cannot overcome μ·m·g ≈ 4.9 N of friction.
    let mut engine = self::engine();
    let weak = scene(3.0).instantiate(engine.world_mut()).unwrap().entities["cube"];
    engine.run_ticks(&mut Idle, 60).unwrap();
    let start = position(engine.world(), weak);
    engine.set_keys(KeySet::EMPTY.with(Key::W));
    engine.run_ticks(&mut Idle, 60).unwrap();
    assert!(
        position(engine.world(), weak).distance(start) < 0.05,
        "too weak to move"
    );
}

#[test]
fn look_at_and_follow_camera_track_a_moving_target() {
    let f = Fixture::new(&[("cube.entity.toml", CUBE)]);
    let forward = |world: &World, id: emergent::EntityId| {
        world
            .global_transform(id)
            .unwrap()
            .matrix()
            .transform_vector3(Vec3::NEG_Z)
            .normalize()
    };
    // A player driven by W along -Z; a spinning beacon to follow from behind.
    let scene = f.scene("[[entities]]\nid='player'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.character_controller'\nspeed=4\ncamera_relative=false\n[[entities]]\nid='watcher'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[6,3,0]\n[[entities.components]]\ntype='emergent.look_at'\ntarget='player'\n[[entities]]\nid='slow'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[-6,3,0]\n[[entities.components]]\ntype='emergent.look_at'\ntarget='player'\nturn_rate=10\n[[entities]]\nid='beacon'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[20,0,0]\n[[entities.components]]\ntype='emergent.spin'\naxis=[0,1,0]\ndegrees_per_second=90\n[[entities]]\nid='chase'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.follow_camera'\ntarget='beacon'\noffset=[0,3,8]\nsmoothing=0\n[[entities]]\nid='overhead'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.follow_camera'\ntarget='player'\nmode='fixed'\noffset=[0,10,0]\nsmoothing=0.5\nlook_at=false\n").unwrap();
    let mut engine = engine();
    let loaded = scene.instantiate(engine.world_mut()).unwrap();
    let id = |name: &str| loaded.entities[name];
    engine.set_keys(KeySet::EMPTY.with(Key::W));
    engine.run_ticks(&mut Idle, 60).unwrap();
    let world = engine.world();
    let player = position(world, id("player"));
    let aim = (player - position(world, id("watcher"))).normalize();
    // The watcher aimed at the player's position from the previous tick: within a degree or two.
    assert!(
        forward(world, id("watcher")).angle_between(aim) < 0.05,
        "watcher tracks the player"
    );
    let lag = forward(world, id("slow"))
        .angle_between((player - position(world, id("slow"))).normalize());
    assert!(lag > 0.2, "a 10°/s turn rate cannot keep up: {lag}");
    // Chase camera: 8 m behind and 3 m above the beacon, measured in the beacon's turning heading.
    // Cameras read the previous tick's committed pose: one tick of a 90°/s spin is 1.5°, ~0.21 m at 8 m.
    let beacon = *world.global_transform(id("beacon")).unwrap();
    let ahead = beacon.matrix().transform_vector3(Vec3::NEG_Z);
    let ahead = Vec3::new(ahead.x, 0.0, ahead.z).normalize();
    let expected = beacon.translation() + Vec3::Y * 3.0 - ahead * 8.0;
    assert!(
        position(world, id("chase")).distance(expected) < 0.3,
        "{} vs {expected}",
        position(world, id("chase"))
    );
    assert!(
        forward(world, id("chase")).angle_between((beacon.translation() - expected).normalize())
            < 0.05,
        "and looks at it"
    );
    // Overhead camera: a world offset reached with 0.5 s smoothing. It started at the origin, so
    // after 1 s it has closed 1 - e^-2 (86%) of the climb and trails the moving player a little.
    let overhead = position(world, id("overhead"));
    let goal = player + Vec3::Y * 10.0;
    assert!(
        (8.0..10.0).contains(&overhead.y) && overhead.distance(goal) < 2.5,
        "{overhead} vs {goal}"
    );
    assert_eq!(
        engine
            .world()
            .local_transform(id("overhead"))
            .unwrap()
            .rotation,
        emergent::Quat::IDENTITY,
        "look_at=false leaves rotation alone"
    );

    for (extra, message) in [
        ("[[entities.components]]\ntype='emergent.look_at'\ntarget='a'\n", "its own object"),
        ("[[entities.components]]\ntype='emergent.look_at'\ntarget='ghost'\n", "not in this scene"),
        ("[[entities.components]]\ntype='emergent.look_at'\ntarget='b'\n[[entities.components]]\ntype='emergent.spin'\n", "look at turns the object"),
        ("[[entities.components]]\ntype='emergent.follow_camera'\ntarget='b'\n[[entities.components]]\ntype='emergent.character_controller'\n", "only one of"),
    ] {
        let error = f
            .scene(&format!("[[entities]]\nid='a'\nasset='assets/cube.entity.toml'\n{extra}[[entities]]\nid='b'\nasset='assets/cube.entity.toml'\n"))
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains(message), "{message}: {error}");
    }
}

#[test]
fn mouse_look_consumes_motion_once_clamps_pitch_and_clears_input() {
    let f = Fixture::new(&[("camera.entity.toml", "schema_version=1\nid='camera'\nkind='camera'\n[camera]\nprojection='perspective'\nfield_of_view_degrees=75\nnear=0.05\nfar=1000\n")]);
    let scene = f.scene("active_camera='camera'\n[[entities]]\nid='camera'\nasset='assets/camera.entity.toml'\n[[entities.components]]\ntype='emergent.mouse_look'\nsensitivity=0.12\npitch_limit=85\n").unwrap();
    let mut e = engine();
    let loaded = scene.instantiate(e.world_mut()).unwrap();
    let id = loaded.entities["camera"];
    assert!(!e.add_mouse_motion(emergent::Vec2::new(f32::NAN, 0.0)));
    e.add_mouse_motion(emergent::Vec2::new(100.0, -10000.0));
    e.start(&mut Idle).unwrap();
    e.advance_frame(&mut Idle, Duration::from_secs_f64(3.0 / 60.0))
        .unwrap();
    let look = e
        .world()
        .component::<emergent::MouseLook>(id)
        .unwrap()
        .unwrap();
    assert!((look.yaw - (-12f32).to_radians().rem_euclid(std::f32::consts::TAU)).abs() < 0.0001);
    assert!((look.pitch - 85f32.to_radians()).abs() < 0.0001);
    let before = *look;
    e.add_mouse_motion(emergent::Vec2::splat(100.0));
    e.clear_input();
    e.advance_frame(&mut Idle, Duration::from_secs_f64(1.0 / 60.0))
        .unwrap();
    assert_eq!(
        *e.world()
            .component::<emergent::MouseLook>(id)
            .unwrap()
            .unwrap(),
        before
    );
    e.world_mut().remove_subtree(id).unwrap();
    e.advance_frame(&mut Idle, Duration::from_secs_f64(1.0 / 60.0))
        .unwrap();
    e.stop(&mut Idle);
}

#[cfg(feature = "physics")]
#[test]
fn first_person_fixture_walks_blocks_jumps_and_prefab_loads() {
    {
        let root = "templates/standard";
        let project =
            Project::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(root)).unwrap();
        emergent::PrefabDefinition::load(&project, "assets/prefabs/first-person.prefab.toml")
            .unwrap();
        let scene =
            SceneDefinition::load(&project, "assets/scenes/first-person.scene.toml").unwrap();
        let mut e = engine();
        let loaded = scene.instantiate(e.world_mut()).unwrap();
        let player = loaded.entities["player"];
        e.run_ticks(&mut Idle, 120).unwrap();
        assert!(e.world().is_grounded(player).unwrap());
        e.set_keys(KeySet::EMPTY.with(Key::W));
        e.run_ticks(&mut Idle, 240).unwrap();
        let p = position(e.world(), player);
        assert!(p.z < -7.0 && p.z > -7.6, "blocked at {p:?}");
        e.set_keys(KeySet::EMPTY.with(Key::Space));
        e.run_ticks(&mut Idle, 1).unwrap();
        assert!(e.world().linear_velocity(player).unwrap().unwrap().y > 4.0);
        e.clear_input();
        e.run_ticks(&mut Idle, 120).unwrap();
        assert!(e.world().is_grounded(player).unwrap());
    }
}

#[test]
fn mouse_look_rejects_invalid_settings_and_non_camera_owners() {
    assert!(emergent::MouseLook::new(0., 85., emergent::Quat::IDENTITY).is_err());
    assert!(emergent::MouseLook::new(0.12, 90., emergent::Quat::IDENTITY).is_err());
    let f = Fixture::new(&[("cube.entity.toml", CUBE)]);
    assert!(f.scene("[[entities]]\nid='cube'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.mouse_look'\n").is_err());
}
