//! Scene map cost at the instance limit: load and build a 4,096-instance map scene, then
//! time the per-tick refresh, point and segment queries and waypoint routes. Run --release.
use emergent::{Engine, EngineConfig, EngineContext, Game, Project, SceneDefinition, Vec3};
use std::time::{Duration, Instant};

struct Idle;
impl Game for Idle {
    fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("emergent-map-bench-{}", std::process::id()));
    std::fs::create_dir_all(root.join("assets"))?;
    std::fs::create_dir_all(root.join("src/generated"))?;
    std::fs::write(root.join("emergent.toml"), "schema_version = 1\nid = 'bench'\nname = 'Bench'\nentry_scene = 'assets/main.scene.toml'\ngenerated_code = 'src/generated'\nasset_roots = ['assets']\n[[outputs]]\nid = 'host'\nos = 'macos'\narchitecture = 'aarch64'\nrust_target = 'aarch64-apple-darwin'\nprofile = 'development'\n")?;
    std::fs::write(root.join("assets/point.entity.toml"), "schema_version = 1\nid = 'point'\nkind = 'entity'\n[transform]\ntranslation = [0.0, 0.0, 0.0]\n")?;
    // A 50 × 40 waypoint grid (2,000) 10 m apart, linked right and down, plus 2,096 zones.
    let (columns, rows, spacing) = (50usize, 40usize, 10.0f32);
    let mut scene = String::from("schema_version = 1\nid = 'main'\n");
    for row in 0..rows {
        for column in 0..columns {
            let mut links = Vec::new();
            if column + 1 < columns {
                links.push(format!("'w{}-{}'", row, column + 1));
            }
            if row + 1 < rows {
                links.push(format!("'w{}-{}'", row + 1, column));
            }
            let links = if links.is_empty() {
                String::new()
            } else {
                format!("links = [{}]\n", links.join(", "))
            };
            scene += &format!("[[entities]]\nid = 'w{row}-{column}'\nasset = 'assets/point.entity.toml'\n[entities.transform]\ntranslation = [{}, 0.0, {}]\n[[entities.components]]\ntype = 'emergent.waypoint'\n{links}", column as f32 * spacing, row as f32 * spacing);
        }
    }
    let zones = 4096 - columns * rows;
    for i in 0..zones {
        let (x, z) = ((i * 37 % 500) as f32, (i * 53 % 400) as f32);
        let shape = if i % 2 == 0 {
            "radius = 6\n".to_string()
        } else {
            "shape = 'rect'\nwidth = 12\ndepth = 4\n".to_string()
        };
        scene += &format!("[[entities]]\nid = 'z{i}'\nasset = 'assets/point.entity.toml'\n[entities.transform]\ntranslation = [{x}, 0.0, {z}]\nrotation_degrees = [0.0, {}, 0.0]\n[[entities.components]]\ntype = 'emergent.zone'\n{shape}cost = {}\n[[entities.components]]\ntype = 'emergent.metadata'\nvalues = {{ type = 'area', index = {i} }}\n", i % 90, 1 + i % 5);
    }
    // --plain: the same instances without map components, to separate the map's share of a tick.
    if std::env::args().any(|arg| arg == "--plain") {
        scene = scene
            .split("[[entities]]")
            .map(|block| {
                block
                    .split("[[entities.components]]")
                    .next()
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join("[[entities]]");
    }
    std::fs::write(root.join("assets/main.scene.toml"), &scene)?;
    let project = Project::load(&root)?;
    let start = Instant::now();
    let definition = SceneDefinition::load(&project, "assets/main.scene.toml")?;
    let validated = start.elapsed();
    let mut engine = Engine::new(EngineConfig::default())?;
    let start = Instant::now();
    let loaded = definition.instantiate(engine.world_mut())?;
    let built = start.elapsed();
    println!("scene: 4096 instances ({} waypoints, {zones} zones), {} KiB; validate {:.1} ms, instantiate + map build {:.1} ms", columns * rows, scene.len() / 1024, validated.as_secs_f64() * 1e3, built.as_secs_f64() * 1e3);

    let report = |name: &str, mut times: Vec<Duration>| {
        times.sort();
        let at = |q: f64| times[((times.len() - 1) as f64 * q) as usize].as_secs_f64() * 1e6;
        println!(
            "{name}: p50={:.2}us p99={:.2}us max={:.2}us",
            at(0.5),
            at(0.99),
            at(1.0)
        );
    };
    // Engine ticks include the map refresh; with nothing else in the world, that is the cost.
    let mut ticks = Vec::new();
    for _ in 0..500 {
        let start = Instant::now();
        engine.run_ticks(&mut Idle, 1)?;
        ticks.push(start.elapsed());
    }
    report("tick incl. map refresh (nothing moved)", ticks);
    let world = engine.world();
    let points: Vec<Vec3> = (0..1000)
        .map(|i| Vec3::new((i * 7 % 500) as f32, 0.0, (i * 11 % 400) as f32))
        .collect();
    report(
        "cost_at",
        points
            .iter()
            .map(|p| {
                let s = Instant::now();
                std::hint::black_box(world.cost_at(*p, None));
                s.elapsed()
            })
            .collect(),
    );
    report(
        "zones_at (collect count)",
        points
            .iter()
            .map(|p| {
                let s = Instant::now();
                std::hint::black_box(world.zones_at(*p).count());
                s.elapsed()
            })
            .collect(),
    );
    report(
        "segment_cost 50 m",
        points
            .iter()
            .map(|p| {
                let s = Instant::now();
                std::hint::black_box(world.segment_cost(*p, *p + Vec3::new(30.0, 0.0, 40.0), None));
                s.elapsed()
            })
            .collect(),
    );
    if std::env::args().any(|arg| arg == "--plain") {
        return Ok(std::fs::remove_dir_all(&root)?);
    }
    let (from, to) = (
        loaded.entities["w0-0"],
        loaded.entities[&format!("w{}-{}", rows - 1, columns - 1)],
    );
    let mut path = Vec::new();
    let mut routes = Vec::new();
    for _ in 0..20 {
        let start = Instant::now();
        engine
            .world_mut()
            .waypoint_path(from, to, None, &mut path)?;
        routes.push(start.elapsed());
    }
    println!("route corner to corner: {} waypoints", path.len());
    report("waypoint_path across 2,000 waypoints", routes);
    std::fs::remove_dir_all(&root)?;
    Ok(())
}
