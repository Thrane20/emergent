//! FLIGHT-0: repeatable headless reference, before adding the physical drone pilot.
//! Run with `cargo run --release --features physics --example flight_baseline`.
use emergent::locomotion::{simulate, Course, Model};
use emergent::{
    BodyKind, BoxCollider, Engine, EngineConfig, EngineContext, EntityId, Game, LocomotionProfile,
    PhysicsBody, Transform, Vec3,
};
use std::error::Error;
use std::time::{Duration, Instant};

struct Lift {
    bodies: Vec<EntityId>,
    force: f32,
}
impl Game for Lift {
    fn fixed_update(&mut self, context: &mut EngineContext<'_>) {
        for &id in &self.bodies {
            context.world.add_force(id, Vec3::Y * self.force).unwrap();
        }
    }
}

fn bodies(count: usize, force: f32, physical: bool) -> Result<(Engine, Lift), Box<dyn Error>> {
    let mut engine = Engine::new(EngineConfig::default())?;
    let mut bodies = Vec::with_capacity(count);
    for i in 0..count {
        let id = engine.world_mut().spawn("reference body");
        engine.world_mut().set_local_transform(
            id,
            Transform::from_translation(Vec3::new(i as f32 * 3.0, 5.0, 0.0)),
        )?;
        engine.world_mut().insert_component(
            id,
            PhysicsBody::new(
                BodyKind::Dynamic,
                BoxCollider {
                    center: [0.0; 3],
                    half_extents: [0.5; 3],
                },
            )?,
        )?;
        if physical {
            engine
                .world_mut()
                .insert_component(id, emergent::LocomotionAgent::new(Model::DronePhysics))?;
        }
        bodies.push(id);
    }
    Ok((engine, Lift { bodies, force }))
}

fn main() -> Result<(), Box<dyn Error>> {
    let profile = LocomotionProfile::from_source(
        "assets/reference.locomotion.toml",
        "schema_version=1\nid='reference'\nkind='locomotion'\ntype='drone'\nmax_speed=2\nmax_climb=2\n",
    )?;
    let trace = simulate(&profile, Course::Stop, 20.0)?;
    let (position, activity) = trace.frames.last().ok_or("missing kinematic frames")?;
    let peak = trace.frames.iter().map(|(p, _)| p.y).fold(0.0, f32::max);
    assert!((position.y - 6.0).abs() < 0.2 && activity.speed < 0.2);
    println!(
        "kinematic course=stop seconds=20 end={position} peak_y={peak:.6} speed={:.6}",
        activity.speed
    );

    let (mut engine, mut game) = bodies(1, 0.0, false)?;
    engine.run_ticks(&mut game, 60)?;
    let y = engine
        .world()
        .global_transform(game.bodies[0])?
        .translation()
        .y;
    let vy = engine
        .world()
        .linear_velocity(game.bodies[0])?
        .ok_or("body not initialized")?
        .y;
    assert!((y - (5.0 - 9.81 / 2.0)).abs() < 0.15 && (vy + 9.81).abs() < 0.01);
    println!("freefall seconds=1 mass_kg=1 end_y={y:.6} velocity_y={vy:.6}");

    let dt = Duration::from_secs_f64(1.0 / 60.0);
    println!("hover baseline: no controller, no wind, no contacts; mass_kg=1 lift_n=9.81 warmup=120 measured=1200 fixed_hz=60");
    for (mode, physical) in [("raw", false), ("boundary", true)] {
        for count in [1, 16, 64, 128, 256] {
            let (mut engine, mut game) = bodies(count, 9.81, physical)?;
            engine.start(&mut game)?;
            for _ in 0..120 {
                assert_eq!(engine.advance_frame(&mut game, dt)?.fixed_steps, 1);
            }
            let mut times = Vec::with_capacity(1200);
            for _ in 0..1200 {
                let start = Instant::now();
                let frame = engine.advance_frame(&mut game, dt)?;
                times.push(start.elapsed().as_secs_f64() * 1e6);
                assert_eq!(frame.fixed_steps, 1);
            }
            times.sort_by(f64::total_cmp);
            let mut max_error = 0.0f32;
            for &id in &game.bodies {
                max_error = max_error
                    .max((engine.world().global_transform(id)?.translation().y - 5.0).abs());
            }
            assert!(max_error < 0.001, "constant lift drifted: {max_error}");
            engine.stop(&mut game);
            println!("hover mode={mode} bodies={count} tick_us median={:.3} p95={:.3} p99={:.3} max={:.3} max_height_error={max_error:.6}", times[600], times[1139], times[1187], times[1199]);
        }
    }
    Ok(())
}
