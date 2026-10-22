use std::hint::black_box;
use std::time::{Duration, Instant};

use bevy_ecs::prelude::{Component, Query, Schedule};
use glam::Vec3;
use slotmap::{new_key_type, SecondaryMap, SlotMap};

const ENTITY_COUNT: usize = 100_000;
const NATIVE_ITERATIONS: u32 = 200;
const LOOKUP_ITERATIONS: u32 = 50;
const CHURN_OPERATIONS: u32 = 50_000;

new_key_type! {
    struct CoreEntity;
}

#[derive(Clone, Copy, Component)]
struct Body {
    position: Vec3,
    velocity: Vec3,
}

impl Body {
    fn seeded(index: usize) -> Self {
        let value = index as f32;
        Self {
            position: Vec3::new(value * 0.001, 0.0, 0.0),
            velocity: Vec3::new(0.25, 0.5, -0.125),
        }
    }

    fn update(&mut self) {
        self.position += self.velocity;
    }
}

#[derive(Clone, Copy)]
struct Measurements {
    native: Duration,
    lookup: Duration,
    churn: Duration,
    checksum: f64,
}

impl Measurements {
    fn print(self, name: &str) {
        println!("implementation: {name}");
        println!("entities: {ENTITY_COUNT}");
        println!(
            "native dense update: {:.2} us/iteration ({NATIVE_ITERATIONS} iterations)",
            micros_per(self.native, NATIVE_ITERATIONS)
        );
        println!(
            "canonical-handle lookup: {:.2} us/iteration ({LOOKUP_ITERATIONS} iterations)",
            micros_per(self.lookup, LOOKUP_ITERATIONS)
        );
        println!(
            "spawn/despawn churn: {:.2} ns/operation ({CHURN_OPERATIONS} operations)",
            nanos_per(self.churn, CHURN_OPERATIONS)
        );
        println!("checksum: {:.3}", self.checksum);
    }
}

fn micros_per(duration: Duration, count: u32) -> f64 {
    duration.as_secs_f64() * 1_000_000.0 / f64::from(count)
}

fn nanos_per(duration: Duration, count: u32) -> f64 {
    duration.as_secs_f64() * 1_000_000_000.0 / f64::from(count)
}

fn slotmap_measurements() -> Measurements {
    let mut entities = SlotMap::<CoreEntity, ()>::with_capacity_and_key(ENTITY_COUNT);
    let mut bodies = SecondaryMap::<CoreEntity, Body>::new();
    let mut handles = Vec::with_capacity(ENTITY_COUNT);
    for index in 0..ENTITY_COUNT {
        let entity = entities.insert(());
        bodies.insert(entity, Body::seeded(index));
        handles.push(entity);
    }

    let start = Instant::now();
    for _ in 0..NATIVE_ITERATIONS {
        for body in bodies.values_mut() {
            body.update();
        }
    }
    let native = start.elapsed();

    let start = Instant::now();
    for _ in 0..LOOKUP_ITERATIONS {
        for &entity in &handles {
            bodies
                .get_mut(entity)
                .expect("canonical body mapping must remain valid")
                .update();
        }
    }
    let lookup = start.elapsed();

    let start = Instant::now();
    for operation in 0..CHURN_OPERATIONS {
        let entity = entities.insert(());
        bodies.insert(entity, Body::seeded(operation as usize));
        black_box(bodies.remove(entity));
        black_box(entities.remove(entity));
    }
    let churn = start.elapsed();

    let checksum = bodies
        .values()
        .map(|body| f64::from(body.position.element_sum()))
        .sum();
    black_box(&bodies);

    Measurements {
        native,
        lookup,
        churn,
        checksum,
    }
}

fn hecs_measurements() -> Measurements {
    let mut core = SlotMap::<CoreEntity, ()>::with_capacity_and_key(ENTITY_COUNT);
    let mut mapping = SecondaryMap::<CoreEntity, hecs::Entity>::new();
    let mut handles = Vec::with_capacity(ENTITY_COUNT);
    let mut world = hecs::World::new();
    for index in 0..ENTITY_COUNT {
        let core_entity = core.insert(());
        let ecs_entity = world.spawn((Body::seeded(index),));
        mapping.insert(core_entity, ecs_entity);
        handles.push(core_entity);
    }

    let start = Instant::now();
    for _ in 0..NATIVE_ITERATIONS {
        for body in world.query_mut::<&mut Body>() {
            body.update();
        }
    }
    let native = start.elapsed();

    let start = Instant::now();
    for _ in 0..LOOKUP_ITERATIONS {
        for &core_entity in &handles {
            let ecs_entity = mapping[core_entity];
            world
                .get::<&mut Body>(ecs_entity)
                .expect("ECS mapping must remain valid")
                .update();
        }
    }
    let lookup = start.elapsed();

    let start = Instant::now();
    for operation in 0..CHURN_OPERATIONS {
        let core_entity = core.insert(());
        let ecs_entity = world.spawn((Body::seeded(operation as usize),));
        mapping.insert(core_entity, ecs_entity);
        world
            .despawn(mapping.remove(core_entity).expect("mapping must exist"))
            .expect("ECS entity must exist");
        black_box(core.remove(core_entity));
    }
    let churn = start.elapsed();

    let checksum = world
        .query_mut::<&Body>()
        .into_iter()
        .map(|body| f64::from(body.position.element_sum()))
        .sum();
    black_box(&world);

    Measurements {
        native,
        lookup,
        churn,
        checksum,
    }
}

fn bevy_measurements() -> Measurements {
    let mut core = SlotMap::<CoreEntity, ()>::with_capacity_and_key(ENTITY_COUNT);
    let mut mapping = SecondaryMap::<CoreEntity, bevy_ecs::entity::Entity>::new();
    let mut handles = Vec::with_capacity(ENTITY_COUNT);
    let mut world = bevy_ecs::world::World::new();
    for index in 0..ENTITY_COUNT {
        let core_entity = core.insert(());
        let ecs_entity = world.spawn(Body::seeded(index)).id();
        mapping.insert(core_entity, ecs_entity);
        handles.push(core_entity);
    }

    let mut query = world.query::<&mut Body>();
    let start = Instant::now();
    for _ in 0..NATIVE_ITERATIONS {
        for mut body in query.iter_mut(&mut world) {
            body.update();
        }
    }
    let native = start.elapsed();

    let start = Instant::now();
    for _ in 0..LOOKUP_ITERATIONS {
        for &core_entity in &handles {
            let ecs_entity = mapping[core_entity];
            world
                .get_mut::<Body>(ecs_entity)
                .expect("ECS mapping must remain valid")
                .update();
        }
    }
    let lookup = start.elapsed();

    let start = Instant::now();
    for operation in 0..CHURN_OPERATIONS {
        let core_entity = core.insert(());
        let ecs_entity = world.spawn(Body::seeded(operation as usize)).id();
        mapping.insert(core_entity, ecs_entity);
        assert!(world.despawn(mapping.remove(core_entity).expect("mapping must exist")));
        black_box(core.remove(core_entity));
    }
    let churn = start.elapsed();

    let mut checksum_query = world.query::<&Body>();
    let checksum = checksum_query
        .iter(&world)
        .map(|body| f64::from(body.position.element_sum()))
        .sum();
    black_box(&world);

    Measurements {
        native,
        lookup,
        churn,
        checksum,
    }
}

fn scheduled_movement(mut query: Query<&mut Body>) {
    for mut body in &mut query {
        body.update();
    }
}

fn bevy_schedule_measurement() {
    let mut world = bevy_ecs::world::World::new();
    world.spawn_batch((0..ENTITY_COUNT).map(Body::seeded));
    let mut schedule = Schedule::default();
    schedule.add_systems(scheduled_movement);

    schedule.run(&mut world);
    let start = Instant::now();
    for _ in 0..NATIVE_ITERATIONS {
        schedule.run(&mut world);
    }
    let elapsed = start.elapsed();
    println!("implementation: bevy_ecs schedule");
    println!(
        "scheduled dense update: {:.2} us/iteration ({NATIVE_ITERATIONS} iterations)",
        micros_per(elapsed, NATIVE_ITERATIONS)
    );
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "all".to_owned());

    match mode.as_str() {
        "slotmap" => slotmap_measurements().print("slotmap"),
        "hecs" => hecs_measurements().print("hecs"),
        "bevy" => bevy_measurements().print("bevy_ecs"),
        "bevy-schedule" => bevy_schedule_measurement(),
        "all" => {
            slotmap_measurements().print("slotmap");
            hecs_measurements().print("hecs");
            bevy_measurements().print("bevy_ecs");
            bevy_schedule_measurement();
        }
        _ => {
            eprintln!("usage: ecs_compare [slotmap|hecs|bevy|bevy-schedule|all]");
            std::process::exit(2);
        }
    }
}
