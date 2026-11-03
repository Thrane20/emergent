//! Plugin registration rules and a plugin component running in a loaded scene.
use emergent::plugin::registry;
use emergent::{
    ComponentField, ComponentType, Engine, EngineConfig, EngineContext, EntityId, FieldKind, Game,
    LoadedScene, Plugin, PluginRuntime, PluginRuntimes, Project, SceneDefinition, World,
};
use std::{fs, path::PathBuf, time::Duration};

const LIFT: &str = "fixture.lift.rise";

/// Fixture plugin: `fixture.lift.rise` moves its object up at `speed` m/s.
struct Lift;
struct Rising(Vec<(EntityId, f32)>);

impl Plugin for Lift {
    fn id(&self) -> &'static str {
        "fixture.lift"
    }
    fn components(&self) -> Vec<ComponentType> {
        vec![ComponentType {
            id: LIFT,
            label: "Rise",
            description: "Moves the object up at a constant speed.",
            applies_to: "any",
            fields: vec![ComponentField::new("speed", "Speed", FieldKind::Number)
                .range(0.0, 100.0)
                .default(1.0)],
        }]
    }
    fn attach(
        &self,
        loaded: &LoadedScene,
        _world: &mut World,
    ) -> Result<Option<Box<dyn PluginRuntime>>, String> {
        let schema = registry(&[&Lift])?;
        let schema = schema.get(LIFT).ok_or("rise is registered")?;
        let rising: Vec<_> = loaded
            .components
            .iter()
            .filter(|c| c.type_id == LIFT)
            .map(|c| {
                (
                    c.entity,
                    schema.values(&c.fields).number("speed").unwrap_or(1.0),
                )
            })
            .collect();
        Ok((!rising.is_empty()).then(|| Box::new(Rising(rising)) as Box<dyn PluginRuntime>))
    }
}

impl PluginRuntime for Rising {
    fn fixed_update(&mut self, context: &mut EngineContext<'_>) -> Result<(), String> {
        let dt = context.fixed_delta.as_secs_f32();
        for &(entity, speed) in &self.0 {
            let mut transform = *context
                .world
                .local_transform(entity)
                .map_err(|e| e.to_string())?;
            transform.translation.y += speed * dt;
            context
                .world
                .set_local_transform(entity, transform)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

struct Named(&'static str, Vec<ComponentType>);
impl Plugin for Named {
    fn id(&self) -> &'static str {
        self.0
    }
    fn components(&self) -> Vec<ComponentType> {
        self.1.clone()
    }
}

fn component(id: &'static str) -> ComponentType {
    ComponentType {
        id,
        label: "",
        description: "",
        applies_to: "any",
        fields: Vec::new(),
    }
}

#[test]
fn plugins_register_only_inside_their_namespace() {
    let ok = registry(&[&Lift]).unwrap();
    assert!(ok.get(LIFT).is_some() && ok.get("emergent.spin").is_some());
    for id in [
        "lift",
        "emergent.lift",
        "project.lift",
        "Acme.lift",
        "acme..lift",
    ] {
        assert!(registry(&[&Named(id, Vec::new())]).is_err(), "{id}");
    }
    let outside = Named("acme.beans", vec![component("project.bean")]);
    assert!(registry(&[&outside])
        .unwrap_err()
        .contains("acme.beans.my_type"));
    let nested = Named("acme.beans", vec![component("acme.beans.a.b")]);
    assert!(registry(&[&nested]).is_err());
    assert!(registry(&[&Lift, &Lift]).unwrap_err().contains("twice"));
}

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn plugin_component_loads_and_runs_each_tick() {
    let root = std::env::temp_dir().join(format!("emergent-plugins-{}", std::process::id()));
    let _cleanup = Fixture(root.clone());
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::create_dir_all(root.join("src/generated")).unwrap();
    fs::write(root.join("emergent.toml"), "schema_version = 1\nid = 'fixture'\nname = 'Fixture'\nentry_scene = 'assets/main.scene.toml'\ngenerated_code = 'src/generated'\nasset_roots = ['assets']\n[[outputs]]\nid = 'host'\nos = 'macos'\narchitecture = 'aarch64'\nrust_target = 'aarch64-apple-darwin'\nprofile = 'development'\n").unwrap();
    fs::write(root.join("assets/cube.entity.toml"), "schema_version=1\nid='cube'\nkind='primitive'\n[geometry]\nprimitive='cube'\nhalf_extent=0.5\ncolor=[1,1,1,1]\n").unwrap();
    fs::write(root.join("assets/main.scene.toml"), "schema_version = 1\nid = 'main'\n[[entities]]\nid='cube'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='fixture.lift.rise'\nspeed=2.0\n").unwrap();
    let project = Project::load(&root).unwrap();

    // Without the plugin the scene is invalid rather than silently skipping the component.
    assert!(SceneDefinition::load_with(
        &project,
        "assets/main.scene.toml",
        &registry(&[]).unwrap()
    )
    .is_err());

    let plugins: &[&dyn Plugin] = &[&Lift];
    let mut engine = Engine::new(EngineConfig {
        fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
        ..EngineConfig::default()
    })
    .unwrap();
    let loaded = SceneDefinition::load_with(
        &project,
        "assets/main.scene.toml",
        &registry(plugins).unwrap(),
    )
    .unwrap()
    .instantiate(engine.world_mut())
    .unwrap();
    let cube = loaded.entities["cube"];
    let start = engine.world().local_transform(cube).unwrap().translation.y;

    struct Host(PluginRuntimes);
    impl Game for Host {
        fn fixed_update(&mut self, context: &mut EngineContext<'_>) {
            self.0.fixed_update(context).unwrap();
        }
    }
    let mut host = Host(PluginRuntimes::attach(plugins, &loaded, engine.world_mut()).unwrap());
    assert!(!host.0.is_empty());
    engine.run_ticks(&mut host, 60).unwrap();
    let risen = engine.world().local_transform(cube).unwrap().translation.y - start;
    assert!(
        (risen - 2.0).abs() < 1e-3,
        "rose {risen} m in one second at 2 m/s"
    );
}

// --- PLUG-2: brain nodes and locomotion types -------------------------------------

use emergent::brain::{Intent, NodeRole, NodeSpec, PreviewConfig};
use emergent::locomotion::{Activity, LocomotionType};
use emergent::plugin::{BrainInput, BrainNode, LocomotionModel, Params};
use emergent::{BrainDefinition, BrainSettings, LocomotionProfile, Vec3};

/// Steers straight at its target at `gain` × the brain's max speed.
struct Chase;
impl BrainNode for Chase {
    fn spec(&self) -> NodeSpec {
        NodeSpec {
            id: "fixture.move.chase",
            label: "Chase",
            category: "Seek & flee",
            role: NodeRole::Steering,
            status: "available",
            description: "Heads straight for the target.",
            fields: vec![
                ComponentField::new("target", "Target", FieldKind::Entity),
                ComponentField::new("group", "Group", FieldKind::Text),
                ComponentField::new("gain", "Gain", FieldKind::Number)
                    .range(0.0, 1.0)
                    .default(1.0),
            ],
        }
    }
    fn steer(&self, params: &Params, input: &BrainInput) -> Option<Vec3> {
        let (target, _) = input.target?;
        Some((target - input.position).try_normalize()? * input.max_speed * params[0])
    }
}

/// Moves at the intended velocity, capped at `max_speed`.
struct Glide;
impl LocomotionModel for Glide {
    fn spec(&self) -> LocomotionType {
        LocomotionType {
            id: "fixture.move.glide",
            label: "Glide",
            category: "Ground",
            status: "available",
            description: "Instant velocity, capped.",
            manoeuvres: &[],
            rig: "body",
            fields: vec![
                ComponentField::new("max_speed", "Max speed", FieldKind::Number)
                    .range(0.0, 100.0)
                    .default(1.0),
            ],
        }
    }
    fn advance(
        &self,
        params: &Params,
        intent: &Intent,
        velocity: &mut Vec3,
        activity: &mut Activity,
        position: Vec3,
        dt: f32,
    ) -> Vec3 {
        *velocity = intent.velocity.clamp_length_max(params[0]);
        activity.speed = velocity.length();
        position + *velocity * dt
    }
}

struct Movers;
impl Plugin for Movers {
    fn id(&self) -> &'static str {
        "fixture.move"
    }
    fn brain_nodes(&self) -> Vec<&'static dyn BrainNode> {
        vec![&Chase]
    }
    fn locomotion_types(&self) -> Vec<&'static dyn LocomotionModel> {
        vec![&Glide]
    }
}

/// A plugin offering one brain node built by `make`, for registration rule checks.
struct BadNode(fn() -> NodeSpec);
impl BrainNode for BadNode {
    fn spec(&self) -> NodeSpec {
        (self.0)()
    }
    fn steer(&self, _: &Params, _: &BrainInput) -> Option<Vec3> {
        None
    }
}
struct OneNode(&'static BadNode);
impl Plugin for OneNode {
    fn id(&self) -> &'static str {
        "fixture.move"
    }
    fn brain_nodes(&self) -> Vec<&'static dyn BrainNode> {
        vec![self.0]
    }
}

#[test]
fn plugin_brain_nodes_and_locomotion_types_follow_registration_rules() {
    let ok = registry(&[&Movers]).unwrap();
    let nodes = emergent::brain::export_node_catalogue_with(&ok).unwrap();
    let types = emergent::locomotion::export_type_catalogue_with(&ok).unwrap();
    assert!(nodes.contains("fixture.move.chase") && nodes.contains("\"seek\""));
    assert!(types.contains("fixture.move.glide") && types.contains("\"drone\""));
    // Built-in exports are unchanged by plugins.
    assert!(!emergent::brain::export_node_catalogue()
        .unwrap()
        .contains("fixture."));

    static OUTSIDE: BadNode = BadNode(|| NodeSpec {
        id: "chase",
        ..Chase.spec()
    });
    static DECORATOR: BadNode = BadNode(|| NodeSpec {
        role: NodeRole::Decorator,
        ..Chase.spec()
    });
    static CATEGORY: BadNode = BadNode(|| NodeSpec {
        category: "Plugins",
        ..Chase.spec()
    });
    static VECTOR: BadNode = BadNode(|| NodeSpec {
        fields: vec![ComponentField::new("offset", "Offset", FieldKind::Vector3)],
        ..Chase.spec()
    });
    static WEIGHT: BadNode = BadNode(|| NodeSpec {
        fields: vec![ComponentField::new("weight", "Weight", FieldKind::Number)],
        ..Chase.spec()
    });
    static MANY: BadNode = BadNode(|| NodeSpec {
        fields: (0..17)
            .map(|_| ComponentField::new("n", "N", FieldKind::Number))
            .collect(),
        ..Chase.spec()
    });
    for (bad, reason) in [
        (&OUTSIDE, "fixture.move.my_node"),
        (&DECORATOR, "Steering"),
        (&CATEGORY, "category"),
        (&VECTOR, "unsupported kind"),
        (&WEIGHT, "added by the engine"),
        (&MANY, "more than 16"),
    ] {
        let error = registry(&[&OneNode(bad)]).unwrap_err();
        assert!(error.contains(reason), "{error}");
    }
}

#[test]
fn plugin_node_steers_and_plugin_locomotion_moves_in_a_scene() {
    let root = std::env::temp_dir().join(format!("emergent-plugins-move-{}", std::process::id()));
    let _cleanup = Fixture(root.clone());
    for dir in ["assets/brains", "assets/locomotion", "src/generated"] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    fs::write(root.join("emergent.toml"), "schema_version = 1\nid = 'fixture'\nname = 'Fixture'\nentry_scene = 'assets/main.scene.toml'\ngenerated_code = 'src/generated'\nasset_roots = ['assets']\n[[outputs]]\nid = 'host'\nos = 'macos'\narchitecture = 'aarch64'\nrust_target = 'aarch64-apple-darwin'\nprofile = 'development'\n").unwrap();
    fs::write(root.join("assets/cube.entity.toml"), "schema_version=1\nid='cube'\nkind='primitive'\n[geometry]\nprimitive='cube'\nhalf_extent=0.5\ncolor=[1,1,1,1]\n").unwrap();
    fs::write(root.join("assets/brains/chase.brain.toml"), "schema_version=1\nid='chase'\nkind='brain'\n[[nodes]]\nid='c'\ntype='fixture.move.chase'\ntarget='goal'\ngain=1\n").unwrap();
    fs::write(
        root.join("assets/locomotion/glide.locomotion.toml"),
        "schema_version=1\nid='glide'\nkind='locomotion'\ntype='fixture.move.glide'\nmax_speed=2\n",
    )
    .unwrap();
    fs::write(root.join("assets/main.scene.toml"), "schema_version=1\nid='main'\n[[entities]]\nid='chaser'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.brain'\nbrain='assets/brains/chase.brain.toml'\nmax_speed=5\nacceleration=1000\nplanar=true\n[[entities.components]]\ntype='emergent.locomotion'\nprofile='assets/locomotion/glide.locomotion.toml'\n[[entities]]\nid='goal'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[10,0,0]\n").unwrap();
    let project = Project::load(&root).unwrap();
    let scene = "assets/main.scene.toml";

    // Without the plugin, its node and type are unknown and the scene is refused.
    let error = SceneDefinition::load_with(&project, scene, &registry(&[]).unwrap())
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("fixture.move"), "{error}");

    let plugins = registry(&[&Movers]).unwrap();
    let profile = LocomotionProfile::load_with(
        &project,
        "assets/locomotion/glide.locomotion.toml",
        &plugins,
    )
    .unwrap();
    assert_eq!(profile.type_id, "fixture.move.glide");

    let mut engine = Engine::new(EngineConfig {
        fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
        ..EngineConfig::default()
    })
    .unwrap();
    let loaded = SceneDefinition::load_with(&project, scene, &plugins)
        .unwrap()
        .instantiate(engine.world_mut())
        .unwrap();
    struct Idle;
    impl Game for Idle {
        fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
    }
    engine.run_ticks(&mut Idle, 60).unwrap();
    let at = engine
        .world()
        .global_transform(loaded.entities["chaser"])
        .unwrap()
        .translation();
    // The brain asks for 5 m/s towards +X; the glide caps it at 2 m/s for one second.
    assert!(at.x > 1.8 && at.x <= 2.0 + 1e-3, "{at:?}");
    assert!(at.z.abs() < 1e-3, "{at:?}");
}

/// Per-tick cost of a plugin steering node vs the built-in Seek it mirrors.
/// `cargo test --release --test plugins -- --ignored --nocapture`
#[test]
#[ignore]
fn measure_plugin_node_tick_cost() {
    let root = std::env::temp_dir().join(format!("emergent-plugins-bench-{}", std::process::id()));
    let _cleanup = Fixture(root.clone());
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::create_dir_all(root.join("src/generated")).unwrap();
    fs::write(root.join("emergent.toml"), "schema_version = 1\nid = 'fixture'\nname = 'Fixture'\nentry_scene = 'assets/main.scene.toml'\ngenerated_code = 'src/generated'\nasset_roots = ['assets']\n[[outputs]]\nid = 'host'\nos = 'macos'\narchitecture = 'aarch64'\nrust_target = 'aarch64-apple-darwin'\nprofile = 'development'\n").unwrap();
    fs::write(
        root.join("assets/main.scene.toml"),
        "schema_version=1\nid='main'\n",
    )
    .unwrap();
    let project = Project::load(&root).unwrap();
    let plugins = registry(&[&Movers]).unwrap();
    let brain = |kind: &str| {
        BrainDefinition::from_source_with(
            &project,
            "assets/b.brain.toml",
            &format!(
                "schema_version=1\nid='b'\n[[nodes]]\nid='n'\ntype='{kind}'\ngroup='target'\n"
            ),
            &plugins,
        )
        .unwrap()
    };
    for agents in [64, 256] {
        for kind in ["seek", "fixture.move.chase"] {
            let config = PreviewConfig {
                agents,
                agent_group: "agents".into(),
                targets: 1,
                target_group: "target".into(),
                target_speed: 0.0,
                ticks: 7200,
                every: 1000,
                seed: 7,
                settings: BrainSettings {
                    max_speed: 3.0,
                    acceleration: 20.0,
                    planar: true,
                    face_movement: false,
                    turn_rate: 0.0,
                },
                obstacles: 0,
            };
            let started = std::time::Instant::now();
            emergent::brain::simulate(&brain(kind), &config).unwrap();
            println!(
                "{agents} agents, {kind}: {:.3} ms/tick",
                started.elapsed().as_secs_f64() * 1000.0 / 7200.0
            );
        }
    }
}

// --- PLUG-3: plugins inside a project ------------------------------------------------

fn plugin_project(
    tag: &str,
    plugins: &str,
    plugin_toml: &str,
) -> (Fixture, Result<Project, String>) {
    let root = std::env::temp_dir().join(format!("emergent-plugins-{tag}-{}", std::process::id()));
    let fixture = Fixture(root.clone());
    for dir in [
        "assets",
        "src/generated",
        "plugins/acme.beans/assets/brains",
    ] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    fs::write(root.join("emergent.toml"), format!("schema_version = 1\nid = 'fixture'\nname = 'Fixture'\nentry_scene = 'assets/main.scene.toml'\ngenerated_code = 'src/generated'\nasset_roots = ['assets']\n[[outputs]]\nid = 'host'\nos = 'macos'\narchitecture = 'aarch64'\nrust_target = 'aarch64-apple-darwin'\nprofile = 'development'\n{plugins}")).unwrap();
    fs::write(
        root.join("assets/main.scene.toml"),
        "schema_version=1\nid='main'\n",
    )
    .unwrap();
    fs::write(root.join("plugins/acme.beans/plugin.toml"), plugin_toml).unwrap();
    fs::write(
        root.join("plugins/acme.beans/assets/brains/hold.brain.toml"),
        "schema_version=1\nid='hold'\n[[nodes]]\nid='h'\ntype='hold'\n",
    )
    .unwrap();
    let project = Project::load(&root).map_err(|e| e.to_string());
    (fixture, project)
}

const BEANS: &str = "[[plugins]]\nid = 'acme.beans'\npath = 'plugins/acme.beans'\n";
const BEANS_TOML: &str = "schema_version = 1\nid = 'acme.beans'\nname = 'Beans'\nversion = '0.1.0'\ncrate = 'acme-beans'\nasset_roots = ['assets']\n";

#[test]
fn project_plugins_load_manifests_and_add_asset_roots() {
    let (_f, project) = plugin_project("ok", BEANS, BEANS_TOML);
    let project = project.unwrap();
    let [plugin] = project.plugins() else {
        panic!("one plugin")
    };
    assert_eq!(
        (plugin.id.as_str(), plugin.version.as_str()),
        ("acme.beans", "0.1.0")
    );
    assert_eq!(plugin.crate_name.as_deref(), Some("acme-beans"));
    let roots: Vec<_> = project
        .asset_roots()
        .iter()
        .map(|r| r.to_string_lossy().into_owned())
        .collect();
    assert_eq!(roots, ["assets", "plugins/acme.beans/assets"]);
    // Plugin assets load like project assets.
    BrainDefinition::load(&project, "plugins/acme.beans/assets/brains/hold.brain.toml").unwrap();
    // The plugin manifest itself is not an asset.
    assert!(project
        .read_text_asset("plugins/acme.beans/plugin.toml")
        .is_err());

    // No plugins is the default.
    let (_g, plain) = plugin_project("none", "", BEANS_TOML);
    assert!(plain.unwrap().plugins().is_empty());
}

#[test]
fn project_plugins_reject_bad_entries_with_reasons() {
    let other = BEANS_TOML.replace("'acme.beans'", "'acme.other'");
    let version = BEANS_TOML.replace("'0.1.0'", "'1.0'");
    let schema = BEANS_TOML.replace("schema_version = 1", "schema_version = 2");
    let crate_name = BEANS_TOML.replace("'acme-beans'", "'Acme Beans'");
    let missing_root = BEANS_TOML.replace("['assets']", "['textures']");
    let escaping_root = BEANS_TOML.replace("['assets']", "['../../assets']");
    let cases: [(&str, String, &str, &str); 8] = [
        ("id", BEANS.into(), &other, "declares id 'acme.other'"),
        ("version", BEANS.into(), &version, "major.minor.patch"),
        ("schema", BEANS.into(), &schema, "schema version 2"),
        ("crate", BEANS.into(), &crate_name, "Cargo package name"),
        ("root", BEANS.into(), &missing_root, "does not exist"),
        ("escape", BEANS.into(), &escaping_root, "portable"),
        (
            "twice",
            format!("{BEANS}{BEANS}"),
            BEANS_TOML,
            "listed twice",
        ),
        (
            "folder",
            BEANS.replace("path = 'plugins/acme.beans'", "path = '../elsewhere'"),
            BEANS_TOML,
            "portable",
        ),
    ];
    for (tag, entries, plugin_toml, reason) in cases {
        let (_f, project) = plugin_project(tag, &entries, plugin_toml);
        let error = project.err().unwrap_or_else(|| panic!("{tag} should fail"));
        assert!(error.contains(reason), "{tag}: {error}");
    }
}
