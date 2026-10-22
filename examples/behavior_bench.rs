use std::hint::black_box;
use std::time::{Duration, Instant};

use emergent::{behavior_ports, BehaviorSpec, BehaviorValue, Vec3, World};

const BEHAVIOR_COUNT: usize = 20_000;
const TICKS: u64 = 200;
const FIXED_STEP: Duration = Duration::from_nanos(16_666_667);

fn linked_motion() {
    let mut world = World::new();
    let leader_entity = world.spawn("leader");
    let leader = world
        .add_behavior(
            leader_entity,
            BehaviorSpec::linear_motion(Vec3::ZERO, Vec3::new(1.0, 0.5, -0.25)).unwrap(),
        )
        .expect("leader behaviour");
    let mut last = leader;

    for index in 1..BEHAVIOR_COUNT {
        let entity = world.spawn("follower");
        let follower = world
            .add_behavior(
                entity,
                BehaviorSpec::follow_position(Vec3::splat(-(index as f32)), 10.0).unwrap(),
            )
            .expect("follower behaviour");
        world
            .connect_behaviors(
                leader,
                behavior_ports::POSITION,
                follower,
                behavior_ports::TARGET,
            )
            .expect("compatible position link");
        last = follower;
    }

    let started = Instant::now();
    for _ in 0..TICKS {
        world
            .evaluate_behaviors(FIXED_STEP)
            .expect("coherent behaviour graph");
        world.propagate_transforms();
    }
    let elapsed = started.elapsed();
    let BehaviorValue::Vector(checksum) = world.behavior_output(last).expect("last output") else {
        unreachable!("follow behaviour always publishes a vector")
    };
    black_box(&world);

    println!("case: linked motion fan-out");
    println!("behaviours: {BEHAVIOR_COUNT}, ticks: {TICKS}");
    println!(
        "evaluate and propagate: {:.2} us/tick",
        elapsed.as_secs_f64() * 1_000_000.0 / TICKS as f64
    );
    println!("checksum: {:.6}", checksum.x + checksum.y + checksum.z);
}

fn scalar_feedback() {
    let mut world = World::new();
    let mut last = None;
    for _ in 0..BEHAVIOR_COUNT {
        let entity = world.spawn("feedback");
        let behavior = world
            .add_behavior(entity, BehaviorSpec::add_scalar(0.0, 1.0, 0.0).unwrap())
            .expect("feedback behaviour");
        world
            .connect_behaviors(
                behavior,
                behavior_ports::VALUE,
                behavior,
                behavior_ports::VALUE,
            )
            .expect("compatible feedback link");
        last = Some(behavior);
    }

    let started = Instant::now();
    for _ in 0..TICKS {
        world
            .evaluate_behaviors(FIXED_STEP)
            .expect("coherent behaviour graph");
    }
    let elapsed = started.elapsed();
    let BehaviorValue::Scalar(checksum) = world
        .behavior_output(last.expect("at least one behaviour"))
        .expect("last output")
    else {
        unreachable!("add-scalar behaviour always publishes a scalar")
    };
    black_box(&world);

    println!("case: independent scalar feedback");
    println!("behaviours: {BEHAVIOR_COUNT}, ticks: {TICKS}");
    println!(
        "evaluate: {:.2} us/tick",
        elapsed.as_secs_f64() * 1_000_000.0 / TICKS as f64
    );
    println!("checksum: {checksum:.3}");
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("motion") => linked_motion(),
        Some("feedback") => scalar_feedback(),
        None | Some("all") => {
            linked_motion();
            scalar_feedback();
        }
        Some(_) => {
            eprintln!("usage: behavior_bench [motion|feedback|all]");
            std::process::exit(2);
        }
    }
}
