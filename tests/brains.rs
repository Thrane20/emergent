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
    }
}
fn last(trace: &emergent::brain::PreviewTrace) -> &[Vec3] {
    trace.frames.last().unwrap()
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
                "group" => "'target'",
                "members" => "'agents'",
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
