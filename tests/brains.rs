//! Brains: catalogue coverage, validation, steering semantics, scenes and determinism.
use emergent::brain::{node_catalogue, simulate, NodeRole, PreviewConfig};
use emergent::{
    BrainDefinition, BrainSettings, Engine, EngineConfig, EngineContext, Game, Project,
    SceneDefinition, Vec3,
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
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "emergent-brains-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("assets/brains")).unwrap();
        fs::create_dir_all(root.join("src/generated")).unwrap();
        fs::write(root.join("emergent.toml"), "schema_version = 1\nid = 'fixture'\nname = 'Fixture'\nentry_scene = 'assets/main.scene.toml'\ngenerated_code = 'src/generated'\nasset_roots = ['assets']\n[[outputs]]\nid = 'host'\nos = 'macos'\narchitecture = 'aarch64'\nrust_target = 'aarch64-apple-darwin'\nprofile = 'development'\n").unwrap();
        fs::write(
            root.join("assets/main.scene.toml"),
            "schema_version=1\nid='main'\nentities=[]\n",
        )
        .unwrap();
        fs::write(root.join("assets/cube.entity.toml"), "schema_version=1\nid='cube'\nkind='primitive'\n[geometry]\nprimitive='cube'\nhalf_extent=0.5\ncolor=[1,1,1,1]\n").unwrap();
        Self(root)
    }
    fn project(&self) -> Project {
        Project::load(&self.0).unwrap()
    }
    fn brain(&self, name: &str, nodes: &str) -> String {
        let path = format!("assets/brains/{name}.brain.toml");
        fs::write(
            self.0.join(&path),
            format!("schema_version=1\nid='{name}'\nname='{name}'\nkind='brain'\n{nodes}"),
        )
        .unwrap();
        path
    }
    fn load(&self, name: &str, nodes: &str) -> Result<BrainDefinition, String> {
        let path = self.brain(name, nodes);
        BrainDefinition::load(&self.project(), &path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const SETTINGS: BrainSettings = BrainSettings {
    max_speed: 3.0,
    acceleration: 30.0,
    planar: true,
    face_movement: false,
    turn_rate: 0.0,
};
fn preview(agents: usize, targets: usize, target_speed: f32, ticks: u64) -> PreviewConfig {
    PreviewConfig {
        agents,
        agent_group: "agents".into(),
        targets,
        target_group: "target".into(),
        target_speed,
        ticks,
        every: 10,
        seed: 7,
        settings: SETTINGS,
        obstacles: 0,
    }
}
fn last(trace: &emergent::brain::PreviewTrace) -> &[Vec3] {
    trace.frames.last().unwrap()
}

#[test]
fn catalogue_node_ids_are_unique() {
    let mut ids = std::collections::HashSet::new();
    for spec in node_catalogue() {
        assert!(ids.insert(spec.id), "duplicate brain node id: {}", spec.id);
    }
}

#[test]
fn every_available_node_loads_compiles_and_runs() {
    let f = Fixture::new();
    let inner = f.brain("inner", "[[nodes]]\nid='root'\ntype='hold'\n");
    for spec in node_catalogue()
        .into_iter()
        .filter(|s| s.status == "available")
    {
        let mut params = String::new();
        for field in &spec.fields {
            let value = match field.name {
                "target" | "label" | "weight" => continue,
                "group" | "threats" | "enemies" => "'target'",
                "members" | "allies" => "'agents'",
                "name" => "'alarm'",
                "waypoints" => continue,
                "brain" => &format!("'{inner}'"),
                _ => continue,
            };
            params.push_str(&format!("{}={value}\n", field.name));
        }
        let child = match spec.role {
            NodeRole::Composite | NodeRole::Decorator => {
                "[[nodes]]\nid='child'\nparent='root'\ntype='seek'\ngroup='target'\n"
            }
            _ => "",
        };
        let source = format!("[[nodes]]\nid='root'\ntype='{}'\n{params}{child}", spec.id);
        let source = if spec.id == "patrol" {
            // Waypoints are scene objects; the preview leaves unknown ones inactive.
            source + "waypoints=['a','b']\n"
        } else {
            source
        };
        let definition = f
            .load(spec.id, &source)
            .unwrap_or_else(|e| panic!("{}: {e}", spec.id));
        simulate(&definition, &preview(4, 1, 6.0, 30))
            .unwrap_or_else(|e| panic!("{}: {e}", spec.id));
    }
    let export = emergent::brain::export_node_catalogue().unwrap();
    assert!(export.contains("Flocking & formation") && export.contains("id = \"flock\""));
}

#[test]
fn invalid_trees_and_references_are_rejected_with_reasons() {
    let f = Fixture::new();
    for (nodes, message) in [
        ("", "exactly one root"),
        ("[[nodes]]\nid='a'\ntype='hold'\n[[nodes]]\nid='b'\ntype='hold'\n", "exactly one root"),
        ("[[nodes]]\nid='a'\ntype='teleport'\n", "unknown type"),
        ("[[nodes]]\nid='a'\ntype='pathfind'\n", "planned"),
        ("[[nodes]]\nid='a'\ntype='blend'\n", "at least one child"),
        ("[[nodes]]\nid='a'\ntype='within_distance'\ngroup='x'\n", "exactly one child"),
        ("[[nodes]]\nid='a'\ntype='seek'\n", "needs a target"),
        ("[[nodes]]\nid='a'\ntype='flock'\n", "needs a group"),
        ("[[nodes]]\nid='a'\ntype='seek'\ngroup='x'\nspeed=3\n", "no field 'speed'"),
        ("[[nodes]]\nid='a'\ntype='flee'\ngroup='x'\npanic_distance=-1\n", "must be a number"),
        ("[[nodes]]\nid='a'\ntype='hold'\n[[nodes]]\nid='b'\ntype='hold'\nparent='ghost'\n", "unknown parent"),
        ("[[nodes]]\nid='a'\ntype='blend'\n[[nodes]]\nid='b'\ntype='blend'\nparent='c'\n[[nodes]]\nid='c'\ntype='blend'\nparent='b'\n", "descend from the root"),
        ("[[nodes]]\nid='a'\ntype='hold'\nparent='a'\n", "exactly one root"),
    ] {
        let error = f.load("bad", nodes).unwrap_err();
        assert!(error.contains(message), "{message}: {error}");
    }
    let deep = (0..40)
        .map(|i| {
            format!(
                "[[nodes]]\nid='n{i}'\ntype='speed_limit'\n{}",
                if i == 0 {
                    String::new()
                } else {
                    format!("parent='n{}'\n", i - 1)
                }
            )
        })
        .collect::<String>()
        + "[[nodes]]\nid='leaf'\ntype='hold'\nparent='n39'\n";
    assert!(f.load("deep", &deep).unwrap_err().contains("deeper than"));
    f.brain(
        "loop_a",
        "[[nodes]]\nid='r'\ntype='brain'\nbrain='assets/brains/loop_b.brain.toml'\n",
    );
    f.brain(
        "loop_b",
        "[[nodes]]\nid='r'\ntype='brain'\nbrain='assets/brains/loop_a.brain.toml'\n",
    );
    let error = BrainDefinition::load(&f.project(), "assets/brains/loop_a.brain.toml").unwrap_err();
    assert!(error.contains("reference cycle"), "{error}");
}

#[test]
fn seek_flee_arrive_and_priority_move_as_documented() {
    let f = Fixture::new();
    let distance = |trace: &emergent::brain::PreviewTrace| {
        let frame = last(trace);
        frame[0].distance(frame[1])
    };
    let seek = f
        .load("seek", "[[nodes]]\nid='a'\ntype='seek'\ngroup='target'\n")
        .unwrap();
    let arrive = f
        .load(
            "arrive",
            "[[nodes]]\nid='a'\ntype='arrive'\ngroup='target'\nslow_radius=3\nstop_radius=0.5\n",
        )
        .unwrap();
    let flee = f
        .load(
            "flee",
            "[[nodes]]\nid='a'\ntype='flee'\ngroup='target'\npanic_distance=0\n",
        )
        .unwrap();
    let still = |brain| simulate(brain, &preview(1, 1, 0.0, 600)).unwrap();
    let start = still(&seek).frames[0][0].distance(still(&seek).frames[0][1]);
    assert!(distance(&still(&seek)) < 0.2, "seek reaches a still target");
    let arrived = distance(&still(&arrive));
    assert!(
        (0.3..=0.55).contains(&arrived),
        "arrive stops at its radius: {arrived}"
    );
    assert!(distance(&still(&flee)) > start + 20.0, "flee runs away");
    // Priority: flee while the target is within 4 m, otherwise wander back towards it.
    let priority = f.load("priority", "[[nodes]]\nid='p'\ntype='priority'\n[[nodes]]\nid='f'\nparent='p'\ntype='flee'\ngroup='target'\npanic_distance=4\n[[nodes]]\nid='s'\nparent='p'\ntype='seek'\ngroup='target'\n").unwrap();
    let trace = still(&priority);
    let settled = trace.frames[30..].iter().map(|f| f[0].distance(f[1]));
    assert!(
        settled.clone().all(|d| (3.0..5.0).contains(&d)),
        "hovers at the panic edge: {:?}",
        settled.collect::<Vec<_>>()
    );
}

#[test]
fn altitude_can_finish_in_a_sequence_without_changing_continuous_hold() {
    let f = Fixture::new();
    fs::create_dir_all(f.0.join("assets/locomotion")).unwrap();
    fs::write(f.0.join("assets/locomotion/drone.locomotion.toml"), "schema_version=1\nid='drone'\nkind='locomotion'\ntype='drone'\nmax_speed=2\nmax_climb=2\nground_height=0.5\n").unwrap();
    for finish in [false, true] {
        let completion = if finish {
            "finish_on_arrival=true\narrival_radius=0.1\n"
        } else {
            ""
        };
        let brain = f.brain("flight", &format!("[[nodes]]\nid='flight'\ntype='sequence'\nloop=false\n[[nodes]]\nid='takeoff'\nparent='flight'\ntype='altitude'\nheight=5\n{completion}[[nodes]]\nid='climb_hover'\nparent='takeoff'\ntype='hover'\n[[nodes]]\nid='land'\nparent='flight'\ntype='altitude'\nheight=0.5\n{completion}[[nodes]]\nid='landing_hover'\nparent='land'\ntype='hover'\n"));
        fs::write(f.0.join("assets/main.scene.toml"), format!("schema_version=1\nid='main'\n[[entities]]\nid='drone'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[0,1.1236403,0]\n[[entities.components]]\ntype='emergent.brain'\nbrain='{brain}'\nmax_speed=12\n[[entities.components]]\ntype='emergent.locomotion'\nprofile='assets/locomotion/drone.locomotion.toml'\n")).unwrap();
        let scene = SceneDefinition::load(&f.project(), "assets/main.scene.toml").unwrap();
        let mut engine = Engine::new(EngineConfig::default()).unwrap();
        let loaded = scene.instantiate(engine.world_mut()).unwrap();
        let drone = loaded.entities["drone"];
        struct Idle;
        impl Game for Idle {
            fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
        }
        let mut peak = 0.0f32;
        let mut landing_started = false;
        for _ in 0..60 * 20 {
            engine.run_ticks(&mut Idle, 1).unwrap();
            let position = engine
                .world()
                .global_transform(drone)
                .unwrap()
                .translation();
            peak = peak.max(position.y);
            let intent = engine
                .world()
                .component::<emergent::LocomotionAgent>(drone)
                .unwrap()
                .unwrap()
                .intent;
            if intent.altitude == Some(0.5) && !landing_started {
                assert!(
                    peak >= 4.9,
                    "landing started before reaching takeoff height: {peak}"
                );
                landing_started = true;
            }
        }
        let end = engine
            .world()
            .global_transform(drone)
            .unwrap()
            .translation();
        let agent = engine
            .world()
            .component::<emergent::LocomotionAgent>(drone)
            .unwrap()
            .unwrap();
        if finish {
            assert!(
                landing_started && peak < 5.3,
                "takeoff then land: peak={peak}, end={end}"
            );
            assert!(
                (end.y - 0.5).abs() < 0.01 && agent.activity().speed < 0.01,
                "landed and settled: {end}"
            );
            assert_eq!(
                agent.intent.altitude, None,
                "the one-shot sequence completed"
            );
        } else {
            assert!(
                !landing_started && (end.y - 5.0).abs() < 0.01,
                "continuous altitude hold is preserved: {end}"
            );
        }
    }
    assert!(f.load("bad_radius", "[[nodes]]\nid='a'\ntype='altitude'\narrival_radius=0\n[[nodes]]\nid='h'\nparent='a'\ntype='hover'\n").unwrap_err().contains("must be a number"));
}

#[test]
fn sequence_advances_when_children_finish_and_patrol_keeps_order_in_scenes() {
    let f = Fixture::new();
    let brain = f.brain("patrol", "[[nodes]]\nid='p'\ntype='patrol'\nwaypoints=['w1','w2','w3']\nmode='once'\narrive_radius=0.2\n");
    fs::write(f.0.join("assets/main.scene.toml"), format!("schema_version=1\nid='main'\n[[entities]]\nid='guard'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.brain'\nbrain='{brain}'\nmax_speed=4\nacceleration=100\n[[entities]]\nid='w1'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[4,0,0]\n[[entities]]\nid='w2'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[4,0,4]\n[[entities]]\nid='w3'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[0,0,4]\n")).unwrap();
    let scene = SceneDefinition::load(&f.project(), "assets/main.scene.toml").unwrap();
    let mut engine = Engine::new(EngineConfig {
        fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
        ..EngineConfig::default()
    })
    .unwrap();
    let loaded = scene.instantiate(engine.world_mut()).unwrap();
    struct Watch(emergent::EntityId, Vec<Vec3>);
    impl Game for Watch {
        fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
        fn after_fixed_update(&mut self, c: &mut EngineContext<'_>) {
            self.1
                .push(c.world.global_transform(self.0).unwrap().translation());
        }
    }
    let mut watch = Watch(loaded.entities["guard"], Vec::new());
    engine.run_ticks(&mut watch, 400).unwrap();
    let reached = |p: Vec3| {
        watch
            .1
            .iter()
            .position(|q| q.distance(p) < 0.25)
            .unwrap_or(usize::MAX)
    };
    let (a, b, c) = (
        reached(Vec3::new(4., 0., 0.)),
        reached(Vec3::new(4., 0., 4.)),
        reached(Vec3::new(0., 0., 4.)),
    );
    assert!(a < b && b < c && c < 400, "visits in order: {a} {b} {c}");
    let end = *watch.1.last().unwrap();
    assert!(
        end.distance(Vec3::new(0., 0., 4.)) < 0.25,
        "once: stops at the last waypoint: {end}"
    );

    // Scenes reject object names a shared brain cannot find, and a second mover.
    let scene_with = |extra: &str| {
        fs::write(f.0.join("assets/main.scene.toml"), format!("schema_version=1\nid='main'\n[[entities]]\nid='guard'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.brain'\nbrain='{brain}'\n{extra}")).unwrap();
        SceneDefinition::load(&f.project(), "assets/main.scene.toml")
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default()
    };
    assert!(scene_with("").contains("names 'w1'"));
    assert!(
        scene_with("[[entities.components]]\ntype='emergent.character_controller'\n")
            .contains("only one of")
    );
}

#[test]
fn flocking_keeps_spacing_and_replays_identically() {
    let f = Fixture::new();
    let flock = f
        .load("flock", "[[nodes]]\nid='b'\ntype='blend'\n[[nodes]]\nid='f'\nparent='b'\ntype='flock'\ngroup='agents'\nradius=4\n[[nodes]]\nid='w'\nparent='b'\ntype='wander'\nweight=0.5\n")
        .unwrap();
    let config = preview(40, 0, 0.0, 600);
    let first = simulate(&flock, &config).unwrap();
    let second = simulate(&flock, &config).unwrap();
    assert_eq!(first.frames, second.frames, "same seed, same trajectories");
    let frame = last(&first);
    let closest = (0..frame.len())
        .flat_map(|i| (i + 1..frame.len()).map(move |j| (i, j)))
        .map(|(i, j)| frame[i].distance(frame[j]))
        .fold(f32::INFINITY, f32::min);
    assert!(closest > 0.3, "separation keeps agents apart: {closest}");
    let other = simulate(&flock, &PreviewConfig { seed: 8, ..config }).unwrap();
    assert_ne!(first.frames, other.frames, "the seed matters");
}

/// `cargo test --release --test brains -- --ignored --nocapture`
#[test]
#[ignore = "timing; run in release"]
fn measure_flock_tick_cost() {
    let f = Fixture::new();
    let flock = f
        .load(
            "flock",
            "[[nodes]]\nid='f'\ntype='flock'\ngroup='agents'\nradius=4\n",
        )
        .unwrap();
    for agents in [64, 256] {
        let started = std::time::Instant::now();
        simulate(
            &flock,
            &PreviewConfig {
                every: 1000,
                ..preview(agents, 0, 0.0, 600)
            },
        )
        .unwrap();
        println!(
            "{agents} agents: {:.3} ms/tick",
            started.elapsed().as_secs_f64() * 1000.0 / 600.0
        );
    }
}

#[test]
fn battle_nodes_engage_retreat_kite_and_encircle() {
    let f = Fixture::new();
    let still = |name: &str, nodes: &str, agents: usize, targets: usize| {
        let brain = f.load(name, nodes).unwrap();
        simulate(&brain, &preview(agents, targets, 0.0, 900)).unwrap()
    };
    let gap = |frame: &[Vec3], agents: usize, i: usize| {
        frame[agents..]
            .iter()
            .map(|t| t.distance(frame[i]))
            .fold(f32::INFINITY, f32::min)
    };
    // Attack closes in and holds at striking range.
    let trace = still(
        "attack",
        "[[nodes]]\nid='a'\ntype='attack'\ngroup='target'\nrange=1.5\n",
        1,
        1,
    );
    let engaged = gap(last(&trace), 1, 0);
    assert!(
        (1.2..=1.6).contains(&engaged),
        "attack holds at range: {engaged}"
    );
    // Retreat falls back to the panic edge and no further while the threat stands still.
    let trace = still("retreat", "[[nodes]]\nid='r'\ntype='priority'\n[[nodes]]\nid='back'\nparent='r'\ntype='retreat'\nthreats='target'\npanic_distance=8\n[[nodes]]\nid='go'\nparent='r'\ntype='seek'\ngroup='target'\n", 1, 1);
    let held = gap(last(&trace), 1, 0);
    assert!(
        (7.0..=8.6).contains(&held),
        "retreat holds at its panic distance: {held}"
    );
    // Outnumbered decides between retreat and attack.
    let odds = "[[nodes]]\nid='p'\ntype='priority'\n[[nodes]]\nid='odds'\nparent='p'\ntype='outnumbered'\nallies='agents'\nenemies='target'\nradius=40\nratio=1\n[[nodes]]\nid='run'\nparent='odds'\ntype='retreat'\nthreats='target'\npanic_distance=30\n[[nodes]]\nid='fight'\nparent='p'\ntype='attack'\ngroup='target'\n";
    let alone = still("alone", odds, 1, 3);
    assert!(
        gap(last(&alone), 1, 0) > gap(&alone.frames[0], 1, 0),
        "one against three retreats"
    );
    let army = still("army", odds, 8, 1);
    assert!(
        (0..8).all(|i| gap(last(&army), 8, i) < 3.0),
        "eight against one attack"
    );
    // Kite settles between its distances; encircle spreads a ring around the enemy.
    let trace = still(
        "kite",
        "[[nodes]]\nid='k'\ntype='kite'\ngroup='target'\nmin_distance=4\nmax_distance=8\n",
        3,
        1,
    );
    assert!(
        (0..3).all(|i| (3.5..=8.5).contains(&gap(last(&trace), 3, i))),
        "kite keeps between 4 and 8 m"
    );
    let trace = still(
        "ring",
        "[[nodes]]\nid='e'\ntype='encircle'\ngroup='target'\nmembers='agents'\nradius=4\n",
        6,
        1,
    );
    let frame = last(&trace);
    assert!(
        (0..6).all(|i| (gap(frame, 6, i) - 4.0).abs() < 0.4),
        "on the ring"
    );
    let closest = (0..6)
        .flat_map(|i| (i + 1..6).map(move |j| (i, j)))
        .map(|(i, j)| frame[i].distance(frame[j]))
        .fold(f32::INFINITY, f32::min);
    assert!(closest > 3.0, "spread around it: {closest}");
}

/// A scene with a scout, an enemy and optional walls (static box colliders) between them.
#[cfg(feature = "physics")]
fn sight_scene(f: &Fixture, brain: &str, walls: &[[f32; 3]]) -> (Engine, emergent::LoadedScene) {
    fs::write(f.0.join("assets/wall.entity.toml"), "schema_version=1\nid='wall'\nkind='primitive'\n[geometry]\nprimitive='cube'\nhalf_extent=0.5\ncolor=[1,1,1,1]\n[physics]\nbody='static'\n").unwrap();
    let walls = walls
        .iter()
        .enumerate()
        .map(|(i, p)| format!("[[entities]]\nid='wall{i}'\nasset='assets/wall.entity.toml'\n[entities.transform]\ntranslation={p:?}\nscale=[1,2,1]\n"))
        .collect::<String>();
    fs::write(f.0.join("assets/main.scene.toml"), format!("schema_version=1\nid='main'\n[[entities]]\nid='scout'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.brain'\nbrain='{brain}'\nmax_speed=3\nacceleration=50\nplanar=true\n[[entities]]\nid='enemy'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[10,0,0]\n[[entities.components]]\ntype='emergent.group'\nname='enemies'\n{walls}")).unwrap();
    let scene = SceneDefinition::load(&f.project(), "assets/main.scene.toml").unwrap();
    let mut engine = Engine::new(EngineConfig {
        fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
        ..EngineConfig::default()
    })
    .unwrap();
    let loaded = scene.instantiate(engine.world_mut()).unwrap();
    (engine, loaded)
}

#[cfg(feature = "physics")]
struct Idle;
#[cfg(feature = "physics")]
impl Game for Idle {
    fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
}

#[cfg(feature = "physics")]
#[test]
fn line_of_sight_is_blocked_by_box_colliders_and_hide_uses_cover() {
    let f = Fixture::new();
    // Chase only while the enemy is visible.
    let chase = f.brain("chase", "[[nodes]]\nid='s'\ntype='line_of_sight'\ngroup='enemies'\n[[nodes]]\nid='go'\nparent='s'\ntype='seek'\ngroup='enemies'\n");
    let position = |engine: &Engine, loaded: &emergent::LoadedScene, id: &str| {
        engine
            .world()
            .global_transform(loaded.entities[id])
            .unwrap()
            .translation()
    };
    let (mut engine, loaded) = sight_scene(&f, &chase, &[]);
    engine.run_ticks(&mut Idle, 60).unwrap();
    assert!(
        position(&engine, &loaded, "scout").x > 2.0,
        "open ground: the scout sees and chases"
    );
    let (mut engine, loaded) = sight_scene(&f, &chase, &[[5.0, 0.0, 0.0]]);
    engine.run_ticks(&mut Idle, 60).unwrap();
    assert!(
        position(&engine, &loaded, "scout").length() < 1e-3,
        "a wall between them blocks sight"
    );
    // The inverse: move only while hidden.
    let sneak = f.brain("sneak", "[[nodes]]\nid='s'\ntype='line_of_sight'\ngroup='enemies'\ninvert=true\n[[nodes]]\nid='go'\nparent='s'\ntype='seek'\ngroup='enemies'\n");
    let (mut engine, loaded) = sight_scene(&f, &sneak, &[[5.0, 0.0, 0.0]]);
    engine.run_ticks(&mut Idle, 30).unwrap();
    assert!(
        position(&engine, &loaded, "scout").x > 0.5,
        "hidden: the sneak advances"
    );
    // Hide: go to the far side of the wall from the enemy.
    let hide = f.brain(
        "hide",
        "[[nodes]]\nid='h'\ntype='hide'\ngroup='enemies'\nmargin=0.8\nmax_cover=3\n",
    );
    let (mut engine, loaded) = sight_scene(&f, &hide, &[[3.0, 0.0, 3.0]]);
    engine.run_ticks(&mut Idle, 300).unwrap();
    let scout = position(&engine, &loaded, "scout");
    let wall = Vec3::new(3.0, 0.0, 3.0);
    let enemy = Vec3::new(10.0, 0.0, 0.0);
    let expected = wall + (wall - enemy).normalize() * (std::f32::consts::FRAC_1_SQRT_2 + 0.8);
    assert!(
        scout.distance(Vec3::new(expected.x, scout.y, expected.z)) < 0.3,
        "behind the cover: {scout} vs {expected}"
    );
    // From there the straight line to the enemy runs through the cover.
    let blocked = (0..32).any(|i| {
        let t = i as f32 / 31.0;
        let p = enemy.lerp(scout, t);
        (p.x - wall.x).abs() <= 0.5 && (p.z - wall.z).abs() <= 0.5
    });
    assert!(
        blocked,
        "the line from the enemy to the hidden scout crosses the cover"
    );
    // Obstacles larger than max_cover (this wall's footprint radius is ~0.71 m) are not cover.
    let picky = f.brain(
        "picky",
        "[[nodes]]\nid='h'\ntype='hide'\ngroup='enemies'\nmax_cover=0.5\n",
    );
    let (mut engine, loaded) = sight_scene(&f, &picky, &[[3.0, 0.0, 3.0]]);
    engine.run_ticks(&mut Idle, 60).unwrap();
    assert!(
        position(&engine, &loaded, "scout").length() < 1e-3,
        "no usable cover: hide is inactive"
    );
}

/// A scene of named cubes; `rest` adds components to the first one.
fn stack_scene(
    f: &Fixture,
    actor: &str,
    others: &[(&str, [f32; 3], &str)],
) -> Result<(Engine, emergent::LoadedScene), emergent::SceneError> {
    let mut source = format!("schema_version=1\nid='main'\n[[entities]]\nid='actor'\nasset='assets/cube.entity.toml'\n{actor}");
    for (id, at, extra) in others {
        source.push_str(&format!("[[entities]]\nid='{id}'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation={at:?}\n{extra}"));
    }
    fs::write(f.0.join("assets/main.scene.toml"), source).unwrap();
    let scene = SceneDefinition::load(&f.project(), "assets/main.scene.toml")?;
    let mut engine = Engine::new(EngineConfig {
        fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
        ..EngineConfig::default()
    })
    .unwrap();
    let loaded = scene.instantiate(engine.world_mut()).unwrap();
    Ok((engine, loaded))
}

struct Quiet;
impl Game for Quiet {
    fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
}

#[test]
fn stacked_brains_move_and_look_independently_and_signal_each_other() {
    let f = Fixture::new();
    let group =
        |name: &str| format!("[[entities.components]]\ntype='emergent.group'\nname='{name}'\n");
    let facing = |engine: &Engine, id| {
        engine
            .world()
            .global_transform(id)
            .unwrap()
            .matrix()
            .transform_vector3(Vec3::NEG_Z)
            .normalize()
    };
    let at = |engine: &Engine, id| engine.world().global_transform(id).unwrap().translation();
    // Two brains in a stack: one walks to the flag, the other watches the tower.
    let walk = f.brain(
        "walk",
        "[[nodes]]\nid='go'\ntype='arrive'\ngroup='flags'\nstop_radius=0.2\n",
    );
    let watch = f.brain(
        "watch",
        "[[nodes]]\nid='l'\ntype='look_at'\ngroup='towers'\n",
    );
    let (mut engine, loaded) = stack_scene(&f, &format!("[[entities.components]]\ntype='emergent.brain'\nbrain='{walk}'\nstack=['{watch}']\nmax_speed=3\nacceleration=50\nplanar=true\nturn_rate=0\n"), &[("flag", [0., 0., -10.], &group("flags")), ("tower", [10., 0., -5.], &group("towers"))]).unwrap();
    engine.run_ticks(&mut Quiet, 120).unwrap();
    let actor = loaded.entities["actor"];
    let here = at(&engine, actor);
    assert!(
        here.z < -5.0 && here.x.abs() < 0.1,
        "walked towards the flag: {here}"
    );
    let to_tower = (Vec3::new(10., 0., -5.) - here).normalize();
    assert!(
        facing(&engine, actor).angle_between(to_tower) < 0.05,
        "while facing the tower, not the way it walks"
    );

    // One brain, per-channel Priority: the look branch does not block movement.
    let both = f.brain("both", "[[nodes]]\nid='p'\ntype='priority'\n[[nodes]]\nid='l'\nparent='p'\ntype='look_at'\ngroup='towers'\n[[nodes]]\nid='go'\nparent='p'\ntype='seek'\ngroup='flags'\n");
    let (mut engine, loaded) = stack_scene(&f, &format!("[[entities.components]]\ntype='emergent.brain'\nbrain='{both}'\nplanar=true\nturn_rate=0\n"), &[("flag", [0., 0., -10.], &group("flags")), ("tower", [10., 0., -5.], &group("towers"))]).unwrap();
    engine.run_ticks(&mut Quiet, 60).unwrap();
    let actor = loaded.entities["actor"];
    assert!(
        at(&engine, actor).z < -2.0,
        "moves under Priority while a look branch comes first"
    );
    assert!(
        facing(&engine, actor)
            .angle_between((Vec3::new(10., 0., -5.) - at(&engine, actor)).normalize())
            < 0.05
    );

    // Signals: the guard shouts once the intruder is within 6 m; the villager hears it next tick and flees.
    let guard = f.brain("guard", "[[nodes]]\nid='near'\ntype='within_distance'\ngroup='intruders'\ndistance=6\n[[nodes]]\nid='shout'\nparent='near'\ntype='signal'\nname='alarm'\ncooldown=5\n");
    let villager = f.brain("villager", "[[nodes]]\nid='p'\ntype='priority'\n[[nodes]]\nid='heard'\nparent='p'\ntype='on_signal'\nname='alarm'\nhold=10\n[[nodes]]\nid='run'\nparent='heard'\ntype='flee'\ngroup='intruders'\npanic_distance=0\n[[nodes]]\nid='stay'\nparent='p'\ntype='hold'\n");
    let intruder = f.brain(
        "intruder",
        "[[nodes]]\nid='go'\ntype='seek'\ngroup='guards'\n",
    );
    let (mut engine, loaded) = stack_scene(
        &f,
        &format!("[[entities.components]]\ntype='emergent.brain'\nbrain='{guard}'\n{}", group("guards")),
        &[
            ("villager", [-5., 0., 0.], &format!("[[entities.components]]\ntype='emergent.brain'\nbrain='{villager}'\nmax_speed=2\n")),
            ("thief", [20., 0., 0.], &format!("[[entities.components]]\ntype='emergent.brain'\nbrain='{intruder}'\nmax_speed=4\n{}", group("intruders"))),
        ],
    )
    .unwrap();
    let villager = loaded.entities["villager"];
    let mut first_alarm = None;
    let mut moved_before_alarm = false;
    for tick in 0..300 {
        engine.run_ticks(&mut Quiet, 1).unwrap();
        let alarms: Vec<_> = engine
            .world()
            .signals()
            .filter(|(_, name)| *name == "alarm")
            .map(|(s, _)| s.from)
            .collect();
        if !alarms.is_empty() && first_alarm.is_none() {
            assert_eq!(alarms, [loaded.entities["actor"]], "the guard sent it");
            first_alarm = Some(tick);
        }
        if first_alarm.is_none() && at(&engine, villager).distance(Vec3::new(-5., 0., 0.)) > 1e-3 {
            moved_before_alarm = true;
        }
    }
    assert!(
        first_alarm.is_some() && !moved_before_alarm,
        "the villager waits for the alarm"
    );
    assert!(
        at(&engine, villager).x < -6.0,
        "then flees: {}",
        at(&engine, villager)
    );

    // Scan sweeps the gaze; a looking brain cannot share an object with spin.
    let scan = f.brain(
        "scan",
        "[[nodes]]\nid='s'\ntype='scan'\nangle=45\nperiod=2\n",
    );
    let (mut engine, loaded) = stack_scene(
        &f,
        &format!("[[entities.components]]\ntype='emergent.brain'\nbrain='{scan}'\nturn_rate=0\n"),
        &[],
    )
    .unwrap();
    let mut yaws = Vec::new();
    for _ in 0..12 {
        engine.run_ticks(&mut Quiet, 10).unwrap();
        let ahead = facing(&engine, loaded.entities["actor"]);
        yaws.push((-ahead.x).atan2(-ahead.z).to_degrees());
    }
    let (low, high) = yaws
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), y| (l.min(*y), h.max(*y)));
    assert!(
        low < -40.0 && high > 40.0 && high < 46.0,
        "sweeps ±45°: {yaws:?}"
    );
    let error = stack_scene(&f, &format!("[[entities.components]]\ntype='emergent.brain'\nbrain='{scan}'\n[[entities.components]]\ntype='emergent.spin'\n"), &[]).err().unwrap().to_string();
    assert!(error.contains("this brain looks"), "{error}");
    let error = stack_scene(&f, &format!("[[entities.components]]\ntype='emergent.brain'\nbrain='{walk}'\nstack=['assets/brains/missing.brain.toml']\n"), &[]).err().unwrap().to_string();
    assert!(error.contains("missing.brain.toml"), "{error}");
}

#[test]
fn shared_brain_bindings_target_each_prefab_instance_in_the_same_scene() {
    let f = Fixture::new();
    let brain = f.brain(
        "seek",
        "[[nodes]]\nid='seek'\ntype='seek'\ntarget='rotor'\n",
    );
    let mut source = "schema_version=1\nid='main'\n".to_string();
    for (name, sign) in [("left", -1), ("right", 1)] {
        source.push_str(&format!("[[entities]]\nid='{name}'\nasset='assets/cube.entity.toml'\n[entities.brain_bindings]\nrotor='{name}-rotor'\n[[entities.components]]\ntype='emergent.brain'\nbrain='{brain}'\nmax_speed=3\nacceleration=50\n[[entities]]\nid='{name}-rotor'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[{},0,0]\n", sign * 10));
    }
    fs::write(f.0.join("assets/main.scene.toml"), &source).unwrap();
    let mut engine = Engine::new(EngineConfig::default()).unwrap();
    let loaded = SceneDefinition::load(&f.project(), "assets/main.scene.toml")
        .unwrap()
        .instantiate(engine.world_mut())
        .unwrap();
    struct Idle;
    impl Game for Idle {
        fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
    }
    engine.run_ticks(&mut Idle, 60).unwrap();
    let position = |id: &str| {
        engine
            .world()
            .global_transform(loaded.entities[id])
            .unwrap()
            .translation()
            .x
    };
    assert!(
        position("left") < -1.,
        "left brain must seek its own member"
    );
    assert!(
        position("right") > 1.,
        "right brain must seek its own member"
    );
    fs::write(
        f.0.join("assets/main.scene.toml"),
        source.replace("rotor='left-rotor'", "rotor='absent'"),
    )
    .unwrap();
    assert!(SceneDefinition::load(&f.project(), "assets/main.scene.toml").is_err());
}
