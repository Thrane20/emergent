use std::hint::black_box;
use std::time::{Duration, Instant};

use emergent::{EntityId, Transform, Vec3, World};

const ENTITY_COUNT: usize = 20_000;
const ITERATIONS: u32 = 100;

fn deep_world() -> (World, EntityId) {
    let mut world = World::with_capacity(ENTITY_COUNT);
    let root = world.spawn("root");
    let mut parent = root;
    for index in 1..ENTITY_COUNT {
        parent = world
            .spawn_child(parent, index.to_string())
            .expect("deep hierarchy construction must succeed");
    }
    world.propagate_transforms();
    (world, root)
}

fn wide_world() -> (World, EntityId) {
    let mut world = World::with_capacity(ENTITY_COUNT);
    let root = world.spawn("root");
    for index in 1..ENTITY_COUNT {
        world
            .spawn_child(root, index.to_string())
            .expect("wide hierarchy construction must succeed");
    }
    world.propagate_transforms();
    (world, root)
}

fn measure_dirty(world: &mut World, root: EntityId) -> Duration {
    let start = Instant::now();
    for iteration in 0..ITERATIONS {
        world
            .set_local_transform(
                root,
                Transform::from_translation(Vec3::new(iteration as f32 + 1.0, 0.0, 0.0)),
            )
            .expect("benchmark transform must be valid");
        black_box(world.propagate_transforms());
    }
    start.elapsed()
}

fn measure_clean(world: &mut World) -> Duration {
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        black_box(world.propagate_transforms());
    }
    start.elapsed()
}

fn average_micros(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000_000.0 / f64::from(ITERATIONS)
}

fn main() {
    let (mut deep, deep_root) = deep_world();
    let (mut wide, wide_root) = wide_world();

    let deep_dirty = measure_dirty(&mut deep, deep_root);
    let deep_clean = measure_clean(&mut deep);
    let wide_dirty = measure_dirty(&mut wide, wide_root);
    let wide_clean = measure_clean(&mut wide);

    println!("Transform propagation baseline");
    println!("entities: {ENTITY_COUNT}, iterations: {ITERATIONS}");
    println!(
        "deep dirty:  {:.2} us/iteration",
        average_micros(deep_dirty)
    );
    println!(
        "deep clean:  {:.2} us/iteration",
        average_micros(deep_clean)
    );
    println!(
        "wide dirty:  {:.2} us/iteration",
        average_micros(wide_dirty)
    );
    println!(
        "wide clean:  {:.2} us/iteration",
        average_micros(wide_clean)
    );
}
