//! Locomotion: each kinematic model stays within its limits, reaches its goals, and
//! carries out brain intent (brake, hover, altitude) in a scene.
use emergent::locomotion::{simulate, type_catalogue, Course, LocomotionTrace};
use emergent::{
    Engine, EngineConfig, EngineContext, Game, LocomotionProfile, Project, SceneDefinition, Vec3,
};
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

fn profile(kind: &str, fields: &str) -> LocomotionProfile {
    LocomotionProfile::from_source(
        "assets/test.locomotion.toml",
        &format!("schema_version=1\nid='t'\nname='T'\nkind='locomotion'\ntype='{kind}'\n{fields}"),
    )
    .unwrap()
}
fn run(kind: &str, fields: &str, course: Course, seconds: f32) -> LocomotionTrace {
    simulate(&profile(kind, fields), course, seconds).unwrap()
}

#[test]
fn profiles_validate_types_fields_and_limits() {
    let bad = |kind: &str, fields: &str| {
        LocomotionProfile::from_source(
            "x.locomotion.toml",
            &format!("schema_version=1\nid='t'\nkind='locomotion'\ntype='{kind}'\n{fields}"),
        )
        .unwrap_err()
    };
    assert!(bad("teleport", "").contains("unknown locomotion type"));
    assert!(bad("biped_ik", "").contains("planned"));
    assert!(bad("wheeled", "speed=3").contains("no field 'speed'"));
    assert!(bad("wheeled", "max_steer=120").contains("must be a number"));
    assert!(bad("fixed_wing", "min_speed=30\ncruise_speed=20").contains("stall speed"));
    let available: Vec<_> = type_catalogue()
        .into_iter()
        .filter(|t| t.status == "available")
        .map(|t| t.id)
        .collect();
    assert_eq!(
        available,
        [
            "slide",
            "wheeled",
            "drone",
            "fixed_wing",
            "fixed_wing_physics",
            "ballistic",
            "rocket"
        ]
    );
    assert!(emergent::locomotion::export_type_catalogue()
        .unwrap()
        .contains("Rotorcraft & VTOL"));
}

#[test]
fn wheeled_never_turns_tighter_than_its_lock_and_brakes_to_a_stop() {
    let car = "wheelbase=2.5\nmax_steer=35\nmax_speed=10\n";
    let trace = run("wheeled", car, Course::Orbit, 30.0);
    let min_radius = 2.5 / 35f32.to_radians().tan();
    for pair in trace.frames.windows(3).skip(5) {
        let (a, b, c) = (pair[0].0, pair[1].0, pair[2].0);
        let (ab, bc) = ((b - a).length(), (c - b).length());
        if ab < 0.05 || bc < 0.05 {
            continue;
        }
        let turn = (b - a).angle_between(c - b);
        let radius = (ab + bc) / 2.0 / turn.max(1e-6);
        assert!(
            radius > min_radius * 0.95,
            "radius {radius} below the {min_radius} lock limit"
        );
    }
    assert!(trace
        .frames
        .iter()
        .all(|(_, a)| a.steer.abs() <= 35f32.to_radians() + 1e-4));
    let stop = run("wheeled", car, Course::Stop, 20.0);
    let (end, activity) = *stop.frames.last().unwrap();
    assert!(
        activity.speed < 0.01 && end.distance(Vec3::new(0., 0., -20.)) < 6.0,
        "stopped near the mark: {end} {}",
        activity.speed
    );
    assert!(
        stop.frames.iter().any(|(_, a)| a.brake > 0.0),
        "the brakes came on"
    );
}

#[test]
fn drone_holds_altitude_hovers_and_never_over_tilts() {
    let trace = run("drone", "max_tilt=25\nmax_speed=8\n", Course::Stop, 20.0);
    let (end, activity) = *trace.frames.last().unwrap();
    assert!(
        (end.y - 6.0).abs() < 0.2,
        "holds the 6 m altitude: {}",
        end.y
    );
    assert!(
        end.distance(Vec3::new(0., 6., -20.)) < 3.5 && activity.speed < 0.2,
        "hovers at the stop point: {end} {}",
        activity.speed
    );
    assert!(trace
        .frames
        .iter()
        .all(|(_, a)| a.pitch.abs() <= 25f32.to_radians() + 1e-3
            && a.bank.abs() <= 25f32.to_radians() + 1e-3));
}

#[test]
fn fixed_wing_never_stalls_never_over_banks_and_holds_altitude() {
    let trace = run(
        "fixed_wing",
        "min_speed=18\ncruise_speed=28\nmax_bank=40\n",
        Course::Waypoints,
        60.0,
    );
    assert!(trace
        .frames
        .iter()
        .all(|(_, a)| a.speed >= 18.0 - 1e-3 && a.bank.abs() <= 40f32.to_radians() + 1e-4));
    assert!(
        trace.frames[300..]
            .iter()
            .all(|(p, _)| (p.y - 25.0).abs() < 1.0),
        "holds 25 m"
    );
    let reach = trace
        .frames
        .iter()
        .map(|(p, _)| p.x.abs().max(p.z.abs()))
        .fold(0.0, f32::max);
    assert!(reach < 260.0, "stays around a ±120 m course: {reach}");
    // Hover: a plane cannot stop, so it circles where it is.
    let loiter = run("fixed_wing", "", Course::Stop, 60.0);
    let late: Vec<_> = loiter.frames[900..].iter().map(|(p, _)| *p).collect();
    let centre = late.iter().copied().sum::<Vec3>() / late.len() as f32;
    assert!(
        late.iter().all(|p| p.distance(centre) < 120.0)
            && loiter.frames.iter().all(|(_, a)| a.speed >= 18.0 - 1e-3),
        "loiters above the stop point"
    );
}

#[test]
fn ballistic_and_rocket_land_on_target_and_replay_identically() {
    let shell = run("ballistic", "launch_speed=25\n", Course::Launch, 10.0);
    let (end, activity) = *shell.frames.last().unwrap();
    assert!(
        activity.landed && end.distance(Vec3::new(40., 0., 0.)) < 0.6,
        "low arc lands on target: {end}"
    );
    let high = run(
        "ballistic",
        "launch_speed=25\narc='high'\n",
        Course::Launch,
        10.0,
    );
    let peak = |t: &LocomotionTrace| t.frames.iter().map(|(p, _)| p.y).fold(0.0, f32::max);
    assert!(
        peak(&high) > peak(&shell) * 2.0
            && high
                .frames
                .last()
                .unwrap()
                .0
                .distance(Vec3::new(40., 0., 0.))
                < 0.8,
        "high arc lands there too"
    );
    let rocket = run("rocket", "thrust=25\nburn_time=3\n", Course::Launch, 20.0);
    let (end, activity) = *rocket.frames.last().unwrap();
    assert!(
        activity.landed && end.distance(Vec3::new(40., 0., 0.)) < 3.0,
        "guided onto the target: {end}"
    );
    let burning =
        rocket.frames.iter().filter(|(_, a)| a.thrust > 0.0).count() as f32 * rocket.frame_seconds;
    assert!(
        burning > 0.5 && burning <= 3.0 + rocket.frame_seconds,
        "burns no longer than its burn time (it may hit first): {burning}"
    );
    let short = run("rocket", "thrust=25\nburn_time=1\n", Course::Launch, 20.0);
    let last = short.frames.last().unwrap().1;
    assert!(
        last.fuel == 0.0 && last.landed,
        "a short burn runs dry, then coasts down"
    );
    assert_eq!(
        run("rocket", "thrust=25\n", Course::Launch, 8.0).frames,
        run("rocket", "thrust=25\n", Course::Launch, 8.0).frames
    );
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Idle;
impl Game for Idle {
    fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
}

#[test]
fn a_brain_drives_a_car_and_hold_brakes_it() {
    let root = std::env::temp_dir().join(format!(
        "emergent-locomotion-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    for dir in ["assets/brains", "assets/locomotion", "src/generated"] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    fs::write(root.join("emergent.toml"), "schema_version = 1\nid = 'fixture'\nname = 'Fixture'\nentry_scene = 'assets/main.scene.toml'\ngenerated_code = 'src/generated'\nasset_roots = ['assets']\n[[outputs]]\nid = 'host'\nos = 'macos'\narchitecture = 'aarch64'\nrust_target = 'aarch64-apple-darwin'\nprofile = 'development'\n").unwrap();
    fs::write(root.join("assets/cube.entity.toml"), "schema_version=1\nid='cube'\nkind='primitive'\n[geometry]\nprimitive='cube'\nhalf_extent=0.5\ncolor=[1,1,1,1]\n").unwrap();
    fs::write(root.join("assets/locomotion/car.locomotion.toml"), "schema_version=1\nid='car'\nkind='locomotion'\ntype='wheeled'\nmax_speed=8\nacceleration=4\nbraking=6\n").unwrap();
    // Drive at the flag until within 6 m, then hold (brake).
    fs::write(root.join("assets/brains/drive.brain.toml"), "schema_version=1\nid='drive'\nkind='brain'\n[[nodes]]\nid='p'\ntype='priority'\n[[nodes]]\nid='near'\nparent='p'\ntype='within_distance'\ngroup='flags'\ndistance=6\n[[nodes]]\nid='stop'\nparent='near'\ntype='hold'\n[[nodes]]\nid='go'\nparent='p'\ntype='seek'\ngroup='flags'\n").unwrap();
    fs::write(root.join("assets/main.scene.toml"), "schema_version=1\nid='main'\n[[entities]]\nid='car'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.brain'\nbrain='assets/brains/drive.brain.toml'\nmax_speed=8\n[[entities.components]]\ntype='emergent.locomotion'\nprofile='assets/locomotion/car.locomotion.toml'\n[[entities]]\nid='flag'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[30,0,0]\n[[entities.components]]\ntype='emergent.group'\nname='flags'\n").unwrap();
    let project = Project::load(&root).unwrap();
    let scene = SceneDefinition::load(&project, "assets/main.scene.toml").unwrap();
    let mut engine = Engine::new(EngineConfig {
        fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
        ..EngineConfig::default()
    })
    .unwrap();
    let loaded = scene.instantiate(engine.world_mut()).unwrap();
    let car = loaded.entities["car"];
    engine.run_ticks(&mut Idle, 60 * 15).unwrap();
    let world = engine.world();
    let position = world.global_transform(car).unwrap().translation();
    let activity = world
        .component::<emergent::LocomotionAgent>(car)
        .unwrap()
        .unwrap()
        .activity();
    assert!(activity.speed < 0.01, "stopped: {}", activity.speed);
    // Starting facing -Z with the flag on +X, the car swings round within its lock and brakes once within 6 m.
    assert!(
        position.distance(Vec3::new(30., 0., 0.)) < 9.0,
        "{position}"
    );
    // Locomotion and a second mover are rejected.
    let scene_text = fs::read_to_string(root.join("assets/main.scene.toml")).unwrap();
    fs::write(
        root.join("assets/main.scene.toml"),
        scene_text.replacen(
            "[[entities]]\nid='flag'",
            "[[entities.components]]\ntype='emergent.spin'\n[[entities]]\nid='flag'",
            1,
        ),
    )
    .unwrap();
    let error = SceneDefinition::load(&project, "assets/main.scene.toml")
        .err()
        .unwrap()
        .to_string();
    assert!(
        error.contains("not with a character controller, follow or spin"),
        "{error}"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn wheel_rig_steers_front_wheels_rolls_all_and_validates_wheels() {
    let root = std::env::temp_dir().join(format!(
        "emergent-wheels-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    for dir in ["assets/brains", "assets/locomotion", "src/generated"] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    fs::write(root.join("emergent.toml"), "schema_version = 1\nid = 'fixture'\nname = 'Fixture'\nentry_scene = 'assets/main.scene.toml'\ngenerated_code = 'src/generated'\nasset_roots = ['assets']\n[[outputs]]\nid = 'host'\nos = 'macos'\narchitecture = 'aarch64'\nrust_target = 'aarch64-apple-darwin'\nprofile = 'development'\n").unwrap();
    fs::write(root.join("assets/cube.entity.toml"), "schema_version=1\nid='cube'\nkind='primitive'\n[geometry]\nprimitive='cube'\nhalf_extent=0.5\ncolor=[1,1,1,1]\n").unwrap();
    fs::write(root.join("assets/locomotion/car.locomotion.toml"), "schema_version=1\nid='car'\nkind='locomotion'\ntype='wheeled'\nmax_speed=8\nacceleration=4\nbraking=6\n").unwrap();
    fs::write(root.join("assets/brains/drive.brain.toml"), "schema_version=1\nid='drive'\nkind='brain'\n[[nodes]]\nid='go'\ntype='seek'\ngroup='flags'\n").unwrap();
    // Wheels are modelled as cylinders lying on their side (rolled 90° about Z), to show the rest pose is kept.
    let wheel = |id: &str, x: f32, z: f32| {
        format!("[[entities]]\nid='{id}'\nparent='car'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[{x},-0.3,{z}]\nrotation_degrees=[0,0,90]\n")
    };
    let scene = |components: &str, extra: &str| {
        format!(
            "schema_version=1\nid='main'\n[[entities]]\nid='car'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.brain'\nbrain='assets/brains/drive.brain.toml'\nmax_speed=8\n[[entities.components]]\ntype='emergent.locomotion'\nprofile='assets/locomotion/car.locomotion.toml'\n{components}\n{}{}{}{}[[entities]]\nid='flag'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[30,0,-10]\n[[entities.components]]\ntype='emergent.group'\nname='flags'\n{extra}",
            wheel("fl", -0.8, -1.2), wheel("fr", 0.8, -1.2), wheel("rl", -0.8, 1.2), wheel("rr", 0.8, 1.2)
        )
    };
    let rig = "steering_wheels=['fl','fr']\nrolling_wheels=['rl','rr']\nwheel_radius=0.4";
    fs::write(
        root.join("assets/main.scene.toml"),
        "schema_version=1\nid='main'\n",
    )
    .unwrap();
    let project = Project::load(&root).unwrap();
    let write = |source: String| fs::write(root.join("assets/main.scene.toml"), source).unwrap();
    let load_error = |source: String| {
        write(source);
        SceneDefinition::load(&project, "assets/main.scene.toml")
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default()
    };
    // Loader rules.
    for (components, extra, expected) in [
        ("steering_wheels=['fl','fl']", "", "listed more than once"),
        ("rolling_wheels=['ghost']", "", "is not in this scene"),
        ("rolling_wheels=['flag']", "", "must be a child"),
        ("rolling_wheels=['car']", "", "must be a child"),
        ("rolling_wheels=['rl']", "", ""),
    ] {
        let error = load_error(scene(components, extra));
        assert!(
            if expected.is_empty() {
                error.is_empty()
            } else {
                error.contains(expected)
            },
            "{components}: {error}"
        );
    }
    let spinning = scene(rig, "").replacen("id='rr'\nparent='car'\nasset='assets/cube.entity.toml'\n", "id='rr'\nparent='car'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.spin'\n", 1);
    assert!(load_error(spinning).contains("has its own mover"));

    write(scene(rig, ""));
    let definition = SceneDefinition::load(&project, "assets/main.scene.toml").unwrap();
    let mut engine = Engine::new(EngineConfig {
        fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
        ..EngineConfig::default()
    })
    .unwrap();
    let loaded = definition.instantiate(engine.world_mut()).unwrap();
    let [car, fl, rl, rr] = ["car", "fl", "rl", "rr"].map(|id| loaded.entities[id]);
    let rest = engine.world().local_transform(fl).unwrap().rotation;
    // The flag is ahead and to the right, so the car steers right while it rolls.
    // Sum the path tick by tick: the car drives an arc, so the straight-line distance is shorter.
    let mut driven = 0.0;
    for _ in 0..60 {
        let before = engine.world().global_transform(car).unwrap().translation();
        engine.run_ticks(&mut Idle, 1).unwrap();
        driven += engine
            .world()
            .global_transform(car)
            .unwrap()
            .translation()
            .distance(before);
    }
    let world = engine.world();
    let activity = world
        .component::<emergent::LocomotionAgent>(car)
        .unwrap()
        .unwrap()
        .activity();
    let roll = world
        .component::<emergent::WheelRig>(car)
        .unwrap()
        .unwrap()
        .roll();
    assert!(
        activity.steer < -0.05 && activity.speed > 0.5,
        "{activity:?}"
    );
    let expected = |steer: f32| {
        emergent::Quat::from_rotation_y(steer) * emergent::Quat::from_rotation_x(roll) * rest
    };
    let front = world.local_transform(fl).unwrap().rotation;
    let rear = world.local_transform(rl).unwrap().rotation;
    // angle_between is acos of an f32 dot product, so ~1e-3 rad is rounding.
    assert!(
        front.angle_between(expected(activity.steer)) < 5e-3,
        "front wheel steers and rolls"
    );
    assert!(
        rear.angle_between(expected(0.0)) < 5e-3,
        "rear wheel only rolls"
    );
    assert!(
        front.angle_between(expected(0.0)) > 0.1,
        "front wheel visibly steered"
    );
    // Translation and scale stay authored.
    assert_eq!(
        world.local_transform(fl).unwrap().translation,
        Vec3::new(-0.8, -0.3, -1.2)
    );
    // Roll matches the distance driven: speed integrates to distance, distance ÷ radius to angle.
    let wrapped = |a: f32| {
        (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
    };
    assert!(
        (wrapped(-driven / 0.4) - roll).abs() < 0.05,
        "roll {roll} for {driven} m"
    );
    // A deleted wheel is skipped; the others keep turning.
    engine.world_mut().remove_subtree(rr).unwrap();
    engine.run_ticks(&mut Idle, 10).unwrap();
    assert!(
        engine
            .world()
            .local_transform(rl)
            .unwrap()
            .rotation
            .angle_between(rear)
            > 1e-3
    );
    let _ = fs::remove_dir_all(&root);
}
