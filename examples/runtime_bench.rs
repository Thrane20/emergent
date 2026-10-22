use std::hint::black_box;
use std::time::Instant;

use emergent::bevy_ecs::prelude::Query;
use emergent::{Component, Engine, EngineConfig, EngineContext, EntityId, Game, SchedulePhase};

const ENTITY_COUNT: usize = 100_000;
const TICKS: u64 = 200;

#[derive(Component)]
struct Position(f32);

#[derive(Component)]
struct Velocity(f32);

#[derive(Default)]
struct BenchmarkGame;

impl Game for BenchmarkGame {
    fn fixed_update(&mut self, _context: &mut EngineContext<'_>) {}
}

fn update_positions(mut bodies: Query<(&mut Position, &Velocity)>) {
    for (mut position, velocity) in &mut bodies {
        position.0 += velocity.0;
    }
}

fn build_engine(with_components: bool) -> (Engine, Vec<EntityId>) {
    let mut engine = Engine::new(EngineConfig::default()).expect("valid benchmark config");
    let mut entities = Vec::with_capacity(ENTITY_COUNT);
    for index in 0..ENTITY_COUNT {
        let entity = engine.world_mut().spawn("benchmark entity");
        entities.push(entity);
        if with_components {
            engine
                .world_mut()
                .insert_component(entity, Position(index as f32))
                .expect("new entity has component storage");
            engine
                .world_mut()
                .insert_component(entity, Velocity(0.25))
                .expect("new entity has component storage");
        }
    }
    if with_components {
        engine
            .schedule_mut(SchedulePhase::Fixed)
            .add_systems(update_positions);
    }
    (engine, entities)
}

fn run_case(label: &str, with_components: bool) {
    let (mut engine, entities) = build_engine(with_components);
    let mut game = BenchmarkGame;
    let started = Instant::now();
    engine
        .run_ticks(&mut game, TICKS)
        .expect("benchmark world must remain coherent");
    let elapsed = started.elapsed();
    black_box(engine.world());

    println!("case: {label}");
    println!("entities: {ENTITY_COUNT}, ticks: {TICKS}");
    println!(
        "fixed tick: {:.2} us/iteration",
        elapsed.as_secs_f64() * 1_000_000.0 / TICKS as f64
    );
    if with_components {
        let first = engine
            .world()
            .component::<Position>(entities[0])
            .expect("mapped first entity")
            .expect("first position")
            .0;
        let last = engine
            .world()
            .component::<Position>(entities[ENTITY_COUNT - 1])
            .expect("mapped last entity")
            .expect("last position")
            .0;
        println!("checksum: {:.3}", first + last);
    }
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("empty") => run_case("empty schedules", false),
        Some("update") => run_case("scheduled dense update", true),
        None | Some("all") => {
            run_case("empty schedules", false);
            run_case("scheduled dense update", true);
        }
        Some(_) => {
            eprintln!("usage: runtime_bench [empty|update|all]");
            std::process::exit(2);
        }
    }
}
