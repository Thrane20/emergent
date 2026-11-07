//! Physical fixed-wing cost: the pilot step alone, and whole engine ticks with N airborne
//! aircraft (Rapier included). `cargo run --release --features physics --example fixed_wing_bench`
use emergent::fixed_wing::{BodyInput, FixedWingParams, FlightGoal, Pilot};
use emergent::{
    BodyKind, BoxCollider, Engine, EngineConfig, EngineContext, Game, LocomotionAgent,
    LocomotionProfile, PhysicsBody, Transform, Vec3,
};
use std::time::{Duration, Instant};

struct Idle;
impl Game for Idle {
    fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
}

fn main() {
    let profile = LocomotionProfile::from_source(
        "bench.locomotion.toml",
        "schema_version=1\nid='b'\nkind='locomotion'\ntype='fixed_wing_physics'\n",
    )
    .unwrap();
    let emergent::locomotion::Model::FixedWingPhysics(params) = profile.model else {
        unreachable!()
    };
    pilot_only(&params);
    for count in [1, 32, 128, 256] {
        ticks(&profile, count);
    }
}

fn pilot_only(params: &FixedWingParams) {
    let mut pilot = Pilot::default();
    let goal = FlightGoal::Hold {
        heading: 90.0,
        altitude: 150.0,
        speed: 40.0,
    };
    let mut body = BodyInput {
        transform: Transform::from_translation(Vec3::new(0.0, 150.0, 0.0)),
        velocity: Vec3::new(0.0, 0.0, -40.0),
        angular_velocity: Vec3::ZERO,
        mass: 1000.0,
        half_extents: Vec3::new(5.0, 0.6, 4.0),
        grounded: false,
        revision: 1,
        wind: Vec3::ZERO,
    };
    let dt = Duration::from_secs_f64(1.0 / 60.0);
    let steps = 1_000_000;
    let start = Instant::now();
    let mut sum = Vec3::ZERO;
    for i in 0..steps {
        body.velocity.x = (i % 7) as f32 * 0.1;
        sum += pilot
            .step(params, goal, None, None, Default::default(), &body, dt)
            .force;
    }
    let each = start.elapsed().as_nanos() as f64 / steps as f64;
    println!("pilot step: {each:.0} ns ({:.1})", sum.length() * 0.0);
}

fn ticks(profile: &LocomotionProfile, count: usize) {
    let mut engine = Engine::new(EngineConfig::default()).unwrap();
    let world = engine.world_mut();
    for i in 0..count {
        let id = world.spawn("plane");
        // Airborne, spread out so they never touch, already at cruise.
        let x = (i % 16) as f32 * 60.0;
        let z = (i / 16) as f32 * 60.0;
        world
            .set_local_transform(id, Transform::from_translation(Vec3::new(x, 300.0, z)))
            .unwrap();
        let mut body = PhysicsBody::new(
            BodyKind::Dynamic,
            BoxCollider {
                center: [0.0; 3],
                half_extents: [5.0, 0.6, 4.0],
            },
        )
        .unwrap();
        body.friction = 0.0;
        body.mass = 1000.0;
        world.insert_component(id, body).unwrap();
        let mut agent = LocomotionAgent::new(profile.model);
        agent.flight = Some(FlightGoal::Hold {
            heading: 0.0,
            altitude: 300.0,
            speed: 40.0,
        });
        world.insert_component(id, agent).unwrap();
    }
    engine.run_ticks(&mut Idle, 120).unwrap();
    let ticks = 600;
    let start = Instant::now();
    engine.run_ticks(&mut Idle, ticks).unwrap();
    let per_tick = start.elapsed().as_secs_f64() * 1e6 / ticks as f64;
    println!(
        "{count:>3} aircraft: {per_tick:.0} µs per 60 Hz tick ({:.1} µs each, physics included)",
        per_tick / count as f64
    );
}
