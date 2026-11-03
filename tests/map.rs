//! Scene map: waypoint, zone and metadata components; loader rules and runtime queries.
use emergent::{
    Engine, EngineConfig, EngineContext, EntityId, Game, MapError, MetaValue, Project,
    SceneDefinition, Transform, Vec3, World,
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
            "emergent-map-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::create_dir_all(root.join("src/generated")).unwrap();
        fs::write(root.join("emergent.toml"), "schema_version = 1\nid = 'fixture'\nname = 'Fixture'\nentry_scene = 'assets/main.scene.toml'\ngenerated_code = 'src/generated'\nasset_roots = ['assets']\n[[outputs]]\nid = 'host'\nos = 'macos'\narchitecture = 'aarch64'\nrust_target = 'aarch64-apple-darwin'\nprofile = 'development'\n").unwrap();
        fs::write(
            root.join("assets/main.scene.toml"),
            "schema_version = 1\nid = 'main'\n",
        )
        .unwrap();
        fs::write(root.join("assets/point.entity.toml"), "schema_version = 1\nid = 'point'\nkind = 'entity'\n[transform]\ntranslation = [0.0, 0.0, 0.0]\n").unwrap();
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

/// One `[[entities]]` block: a point at `at` with extra component tables.
fn item(id: &str, at: [f32; 3], components: &str) -> String {
    format!("[[entities]]\nid = '{id}'\nasset = 'assets/point.entity.toml'\n[entities.transform]\ntranslation = [{}, {}, {}]\n{components}", at[0], at[1], at[2])
}
fn waypoint(links: &[&str]) -> String {
    let links: Vec<String> = links.iter().map(|l| format!("'{l}'")).collect();
    let links = if links.is_empty() {
        String::new()
    } else {
        format!("links = [{}]\n", links.join(", "))
    };
    format!("[[entities.components]]\ntype = 'emergent.waypoint'\n{links}")
}

struct Idle;
impl Game for Idle {
    fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
}

/// A costly circle sits on the straight line from west to east; north is a free detour.
fn routes() -> String {
    [
        item("west", [-20.0, 0.0, 0.0], &waypoint(&["east", "north"])),
        item("north", [0.0, 0.0, 15.0], &waypoint(&["east"])),
        item("east", [20.0, 0.0, 0.0], &waypoint(&[])),
        item("swamp", [0.0, 0.0, 0.0], "[[entities.components]]\ntype = 'emergent.zone'\nradius = 6\ncost = 10\n[[entities.components]]\ntype = 'emergent.metadata'\nvalues = { type = 'swamp', depth = 2, wet = true }\n"),
    ]
    .concat()
}

fn load(f: &Fixture, source: &str) -> (World, std::collections::HashMap<String, EntityId>) {
    let mut world = World::new();
    let loaded = f.scene(source).unwrap().instantiate(&mut world).unwrap();
    (world, loaded.entities)
}

#[test]
fn map_components_are_validated() {
    let f = Fixture::new();
    f.scene(&routes()).unwrap();
    for (source, message) in [
        (
            item("a", [0.0; 3], &waypoint(&["ghost"])),
            "not a waypoint in this scene",
        ),
        (item("a", [0.0; 3], &waypoint(&["a"])), "itself"),
        (
            item("a", [0.0; 3], &waypoint(&["b"])) + &item("b", [1.0, 0.0, 0.0], ""),
            "not a waypoint",
        ),
        (
            item(
                "a",
                [0.0; 3],
                "[[entities.components]]\ntype = 'emergent.zone'\ncost = 0\n",
            ),
            "cost",
        ),
        (
            item(
                "a",
                [0.0; 3],
                "[[entities.components]]\ntype = 'emergent.zone'\nshape = 'hexagon'\n",
            ),
            "shape",
        ),
        (
            item(
                "a",
                [0.0; 3],
                "[[entities.components]]\ntype = 'emergent.metadata'\nvalues = { list = [1, 2] }\n",
            ),
            "values",
        ),
        (
            item(
                "a",
                [0.0; 3],
                &format!(
                    "[[entities.components]]\ntype = 'emergent.metadata'\nvalues = {{ {} }}\n",
                    (0..33)
                        .map(|i| format!("k{i} = {i}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ),
            "values",
        ),
    ] {
        let error = f.scene(&source).err().unwrap().to_string();
        assert!(error.contains(message), "{message}: {error}");
    }
}

#[test]
fn zones_metadata_and_costs_are_queryable() {
    let f = Fixture::new();
    // A blocked air-only rectangle, 10 m wide and 2 m deep, turned 90° about Y.
    let source = routes()
        + "[[entities]]\nid = 'depot'\nasset = 'assets/point.entity.toml'\n[entities.transform]\ntranslation = [40.0, 0.0, 0.0]\nrotation_degrees = [0.0, 90.0, 0.0]\n[[entities.components]]\ntype = 'emergent.zone'\nshape = 'rect'\nwidth = 10\ndepth = 2\nheight = 5\nblocked = true\nlayers = 'air'\n";
    let (world, ids) = load(&f, &source);
    let (swamp, depot) = (ids["swamp"], ids["depot"]);

    assert_eq!(
        world.zones_at(Vec3::new(3.0, 0.0, 3.0)).collect::<Vec<_>>(),
        vec![swamp]
    );
    assert_eq!(
        world.zones_at(Vec3::new(5.0, 0.0, 5.0)).count(),
        0,
        "outside the 6 m circle"
    );
    assert_eq!(world.cost_at(Vec3::new(1.0, 0.0, 0.0), None), 10.0);
    // The rectangle is turned 90°: 10 m along world Z, 2 m along world X.
    assert_eq!(
        world
            .zones_at(Vec3::new(40.0, 1.0, 4.5))
            .collect::<Vec<_>>(),
        vec![depot]
    );
    assert_eq!(world.zones_at(Vec3::new(44.5, 1.0, 0.0)).count(), 0);
    assert_eq!(
        world.zones_at(Vec3::new(40.0, 6.0, 0.0)).count(),
        0,
        "above its 5 m height"
    );
    let inside = Vec3::new(40.0, 1.0, 0.0);
    assert!(
        world.cost_at(inside, Some("air")).is_infinite(),
        "blocked for air"
    );
    assert_eq!(
        world.cost_at(inside, Some("ground")),
        1.0,
        "ground ignores an air-only zone"
    );
    assert!(world.cost_at(inside, None).is_infinite());

    let straight = world.segment_cost(Vec3::new(-20.0, 0.0, 0.0), Vec3::new(20.0, 0.0, 0.0), None);
    assert!(
        (straight - (28.0 + 12.0 * 10.0)).abs() < 2.0,
        "40 m with 12 m at ten times: {straight}"
    );
    assert!(world
        .segment_cost(
            Vec3::new(30.0, 1.0, 0.0),
            Vec3::new(50.0, 1.0, 0.0),
            Some("air")
        )
        .is_infinite());

    let swampy: Vec<_> = world
        .items_where("type", &MetaValue::from("swamp"))
        .collect();
    assert_eq!(swampy, vec![swamp]);
    assert_eq!(
        world.items_where("depth", &MetaValue::Number(2.0)).count(),
        1
    );
    assert_eq!(world.items_where("wet", &MetaValue::Bool(true)).count(), 1);
    assert_eq!(
        world.items_where("type", &MetaValue::from("lake")).count(),
        0
    );
    let metadata = world
        .component::<emergent::Metadata>(swamp)
        .unwrap()
        .unwrap();
    assert_eq!(metadata.get("depth"), Some(&MetaValue::Number(2.0)));
    assert_eq!(
        world.nearest_waypoint(Vec3::new(18.0, 0.0, 3.0)),
        Some(ids["east"])
    );
}

#[test]
fn routes_detour_around_costly_zones_and_never_cross_blocked_ones() {
    let f = Fixture::new();
    let (mut world, ids) = load(&f, &routes());
    let (west, north, east) = (ids["west"], ids["north"], ids["east"]);
    let mut path = Vec::new();
    let cost = world.waypoint_path(west, east, None, &mut path).unwrap();
    assert_eq!(
        path,
        vec![west, north, east],
        "the 50 m detour beats 148 cost units through the swamp"
    );
    assert!(
        (cost - 2.0 * (20f32.powi(2) + 15f32.powi(2)).sqrt()).abs() < 0.5,
        "{cost}"
    );

    // A cheap swamp makes the straight line best.
    let cheap = routes().replace("cost = 10", "cost = 0.5");
    let (mut world2, ids2) = load(&f, &cheap);
    world2
        .waypoint_path(ids2["west"], ids2["east"], None, &mut path)
        .unwrap();
    assert_eq!(path, vec![ids2["west"], ids2["east"]]);

    // Blocked swamp and a blocked northern pass: no route.
    let walled = routes().replace("cost = 10", "blocked = true")
        + &item(
            "wall",
            [0.0, 0.0, 15.0],
            "[[entities.components]]\ntype = 'emergent.zone'\nradius = 3\nblocked = true\n",
        );
    let (mut world3, ids3) = load(&f, &walled);
    assert_eq!(
        world3.waypoint_path(ids3["west"], ids3["east"], None, &mut path),
        Err(MapError::NoRoute)
    );
    assert!(path.is_empty());
    assert_eq!(
        world.waypoint_path(ids["swamp"], east, None, &mut path),
        Err(MapError::NotAWaypoint(ids["swamp"]))
    );

    // One-way links: east doesn't lead back to west.
    let one_way = routes().replace("[[entities.components]]\ntype = 'emergent.waypoint'\nlinks = ['east', 'north']\n", "[[entities.components]]\ntype = 'emergent.waypoint'\nlinks = ['east', 'north']\ntwo_way = false\n");
    let (mut world4, ids4) = load(&f, &one_way);
    assert!(world4
        .waypoint_path(ids4["west"], ids4["east"], None, &mut path)
        .is_ok());
    assert_eq!(
        world4.waypoint_path(ids4["east"], ids4["west"], None, &mut path),
        Err(MapError::NoRoute)
    );
}

#[test]
fn zones_follow_their_objects_and_vanish_when_deleted() {
    let f = Fixture::new();
    let mut engine = Engine::new(EngineConfig::default()).unwrap();
    let loaded = f
        .scene(&routes())
        .unwrap()
        .instantiate(engine.world_mut())
        .unwrap();
    let swamp = loaded.entities["swamp"];
    let world = engine.world_mut();
    world
        .set_local_transform(
            swamp,
            Transform::from_translation(Vec3::new(100.0, 0.0, 0.0)),
        )
        .unwrap();
    engine.run_ticks(&mut Idle, 1).unwrap();
    assert_eq!(
        engine
            .world()
            .zones_at(Vec3::new(101.0, 0.0, 0.0))
            .collect::<Vec<_>>(),
        vec![swamp]
    );
    assert_eq!(
        engine.world().zones_at(Vec3::ZERO).count(),
        0,
        "left its old place"
    );
    engine.world_mut().remove_subtree(swamp).unwrap();
    assert_eq!(
        engine.world().zones_at(Vec3::new(101.0, 0.0, 0.0)).count(),
        0,
        "a deleted zone is gone at once"
    );
    engine.run_ticks(&mut Idle, 1).unwrap();
    assert_eq!(
        engine
            .world()
            .items_where("type", &MetaValue::from("swamp"))
            .count(),
        0
    );
}
