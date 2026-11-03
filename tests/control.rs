use emergent::control::{ControlError, Pid, PidConfig, PidInput};
use std::alloc::{GlobalAlloc, Layout, System};
use std::{cell::Cell, time::Duration};

thread_local! {
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}
struct CountAlloc;
fn count_allocation() {
    let _ = ALLOCATIONS.try_with(|counter| {
        if let Some(count) = counter.get() {
            counter.set(Some(count + 1));
        }
    });
}
// SAFETY: pointers/layouts are forwarded unchanged to System; Cell bookkeeping does not allocate.
unsafe impl GlobalAlloc for CountAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count_allocation();
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountAlloc = CountAlloc;

fn config() -> PidConfig {
    PidConfig {
        output_min: -100.0,
        output_max: 100.0,
        integral_limit: 20.0,
        ..PidConfig::default()
    }
}
fn input(setpoint: f32, measurement: f32) -> PidInput {
    PidInput {
        setpoint,
        measurement,
        ..PidInput::default()
    }
}
fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 1e-4, "{actual} != {expected}");
}

#[test]
fn proportional_feedforward_and_asymmetric_limits() {
    let mut pid = Pid::new(PidConfig {
        kp: 2.0,
        output_min: 0.0,
        output_max: 25.0,
        ..config()
    })
    .unwrap();
    let out = pid
        .step(
            PidInput {
                feedforward: 9.81,
                ..input(5.0, 3.0)
            },
            Duration::from_millis(10),
        )
        .unwrap();
    near(out.proportional, 4.0);
    near(out.output, 13.81);
    assert!(!out.saturated);
    let out = pid
        .step(input(100.0, 0.0), Duration::from_millis(10))
        .unwrap();
    assert!(out.saturated);
    near(out.requested, 200.0);
    near(out.output, 25.0);
    let out = pid
        .step(input(0.0, 100.0), Duration::from_millis(10))
        .unwrap();
    near(out.output, 0.0);
    near(out.integral, 0.0);
}

#[test]
fn integral_uses_elapsed_seconds_and_reset_clears_all_history() {
    let c = PidConfig {
        kp: 0.0,
        ki: 2.0,
        kd: 1.0,
        ..config()
    };
    let mut ends = Vec::new();
    for hz in [60, 120] {
        let mut pid = Pid::new(c).unwrap();
        let dt = Duration::from_secs_f64(1.0 / hz as f64);
        let mut out = None;
        for _ in 0..hz {
            out = Some(pid.step(input(2.0, 1.0), dt).unwrap());
        }
        let out = out.unwrap();
        near(out.integral, 2.0);
        ends.push(out.output);
        pid.reset();
        assert_eq!(pid, Pid::new(c).unwrap());
        let out = pid.step(input(100.0, 100.0), dt).unwrap();
        near(out.output, 0.0);
    }
    near(ends[0], ends[1]);
}

#[test]
fn derivative_is_on_measurement_with_no_setpoint_kick_and_optional_rate() {
    let mut pid = Pid::new(PidConfig {
        kp: 0.0,
        kd: 2.0,
        ..config()
    })
    .unwrap();
    let dt = Duration::from_millis(100);
    near(pid.step(input(0.0, 10.0), dt).unwrap().derivative, 0.0);
    near(pid.step(input(100.0, 10.0), dt).unwrap().derivative, 0.0);
    near(pid.step(input(100.0, 11.0), dt).unwrap().derivative, -20.0);
    near(
        pid.step(
            PidInput {
                measurement_rate: Some(3.0),
                ..input(100.0, 11.0)
            },
            dt,
        )
        .unwrap()
        .derivative,
        -6.0,
    );
    let mut filtered = Pid::new(PidConfig {
        kp: 0.0,
        kd: 2.0,
        derivative_filter_seconds: 0.1,
        ..config()
    })
    .unwrap();
    filtered.step(input(0.0, 0.0), dt).unwrap();
    near(
        filtered.step(input(0.0, 1.0), dt).unwrap().derivative,
        -10.0,
    );
    near(filtered.step(input(0.0, 1.0), dt).unwrap().derivative, -5.0);
}

#[test]
fn conditional_integration_prevents_windup_at_both_output_limits() {
    for sign in [-1.0, 1.0] {
        let mut pid = Pid::new(PidConfig {
            kp: 2.0,
            ki: 1.0,
            tracking_gain: 0.0,
            output_min: -1.0,
            output_max: 1.0,
            ..config()
        })
        .unwrap();
        for _ in 0..1000 {
            let out = pid
                .step(input(sign * 10.0, 0.0), Duration::from_millis(10))
                .unwrap();
            near(out.integral, 0.0);
            near(out.output, sign);
        }
        near(
            pid.step(input(0.0, 0.0), Duration::from_millis(10))
                .unwrap()
                .output,
            0.0,
        );
    }
}

#[test]
fn achieved_actuator_output_prevents_windup_and_recovers_when_released() {
    let mut pid = Pid::new(PidConfig {
        kp: 0.0,
        ki: 1.0,
        tracking_gain: 10.0,
        ..config()
    })
    .unwrap();
    let dt = Duration::from_millis(10);
    let mut achieved = None;
    let mut last = 0.0;
    for _ in 0..1000 {
        let out = pid
            .step(
                PidInput {
                    achieved_output: achieved,
                    ..input(1.0, 0.0)
                },
                dt,
            )
            .unwrap();
        achieved = Some(out.output.min(0.1));
        last = out.integral;
    }
    assert!(
        (0.1..0.25).contains(&last),
        "actuator tracking integral={last}"
    );
    let out = pid.step(input(1.0, 0.0), dt).unwrap();
    assert!(out.output > last && out.output < 0.3);
    // A tracking rate shorter than a step still produces bounded, non-oscillatory correction.
    let mut fast = Pid::new(PidConfig {
        kp: 0.0,
        ki: 1.0,
        tracking_gain: 1e6,
        ..config()
    })
    .unwrap();
    for _ in 0..100 {
        let out = fast
            .step(
                PidInput {
                    achieved_output: Some(0.0),
                    ..input(1.0, 0.0)
                },
                dt,
            )
            .unwrap();
        assert!((0.0..0.02).contains(&out.output));
    }
}

#[test]
fn integral_contribution_is_bounded_independently_of_output() {
    let mut pid = Pid::new(PidConfig {
        kp: 0.0,
        ki: 10.0,
        integral_limit: 0.5,
        ..config()
    })
    .unwrap();
    for _ in 0..1000 {
        near(
            pid.step(input(1.0, 0.0), Duration::from_millis(100))
                .unwrap()
                .integral,
            0.5,
        );
    }
}

#[test]
fn invalid_configuration_samples_and_overflow_do_not_poison_history() {
    for bad in [
        PidConfig {
            kp: f32::NAN,
            ..config()
        },
        PidConfig {
            ki: -1.0,
            ..config()
        },
        PidConfig {
            kd: f32::INFINITY,
            ..config()
        },
        PidConfig {
            integral_limit: -1.0,
            ..config()
        },
        PidConfig {
            output_min: 1.0,
            output_max: -1.0,
            ..config()
        },
        PidConfig {
            output_max: f32::INFINITY,
            ..config()
        },
        PidConfig {
            derivative_filter_seconds: -1.0,
            ..config()
        },
        PidConfig {
            tracking_gain: f32::NAN,
            ..config()
        },
    ] {
        assert_eq!(Pid::new(bad), Err(ControlError::InvalidConfig));
    }
    let dt = Duration::from_millis(10);
    let mut pid = Pid::new(config()).unwrap();
    pid.step(input(1.0, 0.0), dt).unwrap();
    let previous = pid.clone();
    for bad in [
        input(f32::NAN, 0.0),
        input(0.0, f32::INFINITY),
        PidInput {
            feedforward: f32::NAN,
            ..input(0.0, 0.0)
        },
        PidInput {
            measurement_rate: Some(f32::NAN),
            ..input(0.0, 0.0)
        },
        PidInput {
            achieved_output: Some(f32::INFINITY),
            ..input(0.0, 0.0)
        },
    ] {
        assert_eq!(pid.step(bad, dt), Err(ControlError::InvalidInput));
        assert_eq!(pid, previous);
    }
    assert_eq!(
        pid.step(input(1.0, 0.0), Duration::ZERO),
        Err(ControlError::InvalidInput)
    );
    assert_eq!(
        pid.step(input(f32::MAX, -f32::MAX), dt),
        Err(ControlError::NonFiniteOutput)
    );
    assert_eq!(pid, previous);
}

#[test]
fn pid_rejects_constant_load_on_a_force_integrated_plant() {
    for hz in [60, 120] {
        let dt = Duration::from_secs_f64(1.0 / hz as f64);
        let mut pid = Pid::new(PidConfig {
            kp: 3.0,
            ki: 2.0,
            kd: 0.1,
            derivative_filter_seconds: 0.05,
            output_min: -10.0,
            output_max: 10.0,
            ..config()
        })
        .unwrap();
        let mut velocity = 0.0;
        let mut achieved = None;
        for _ in 0..hz * 20 {
            let out = pid
                .step(
                    PidInput {
                        achieved_output: achieved,
                        ..input(0.0, velocity)
                    },
                    dt,
                )
                .unwrap();
            velocity += (out.output + 1.5) * dt.as_secs_f32();
            achieved = Some(out.output);
        }
        assert!(velocity.abs() < 0.001, "{hz}Hz velocity={velocity}");
    }
}

#[test]
fn stepping_and_resetting_allocate_nothing_including_error_paths() {
    let mut pid = Pid::new(PidConfig {
        ki: 0.5,
        kd: 0.1,
        ..config()
    })
    .unwrap();
    ALLOCATIONS.with(|counter| counter.set(Some(0)));
    for _ in 0..10_000 {
        std::hint::black_box(pid.step(
            std::hint::black_box(input(1.0, 0.5)),
            Duration::from_millis(10),
        ))
        .unwrap();
        std::hint::black_box(pid.step(input(f32::NAN, 0.0), Duration::ZERO)).unwrap_err();
        pid.reset();
    }
    let allocations = ALLOCATIONS.with(|counter| counter.replace(None).unwrap());
    assert_eq!(allocations, 0);
}

#[cfg(feature = "physics")]
#[test]
fn pid_can_lift_and_hold_a_real_body_without_writing_its_pose() {
    use emergent::{
        BodyKind, BoxCollider, Engine, EngineConfig, EngineContext, EntityId, Game, PhysicsBody,
        Transform, Vec3,
    };
    struct VerticalPilot {
        body: EntityId,
        pid: Pid,
        actual_thrust: f32,
        peak: f32,
        settled_ticks: u32,
    }
    impl Game for VerticalPilot {
        fn fixed_update(&mut self, c: &mut EngineContext<'_>) {
            let y = c.world.global_transform(self.body).unwrap().translation().y;
            let vy = c
                .world
                .linear_velocity(self.body)
                .unwrap()
                .unwrap_or(Vec3::ZERO)
                .y;
            let out = self
                .pid
                .step(
                    PidInput {
                        setpoint: ((5.0 - y) * 1.5).clamp(-2.0, 2.0),
                        measurement: vy,
                        feedforward: 9.81,
                        achieved_output: Some(self.actual_thrust),
                        ..PidInput::default()
                    },
                    c.fixed_delta,
                )
                .unwrap();
            let dt = c.fixed_delta.as_secs_f32();
            self.actual_thrust += dt / (0.08 + dt) * (out.output - self.actual_thrust);
            c.world
                .add_force(self.body, Vec3::Y * self.actual_thrust)
                .unwrap();
        }
        fn after_fixed_update(&mut self, c: &mut EngineContext<'_>) {
            let y = c.world.global_transform(self.body).unwrap().translation().y;
            let vy = c.world.linear_velocity(self.body).unwrap().unwrap().y;
            self.peak = self.peak.max(y);
            self.settled_ticks = if (y - 5.0).abs() <= 0.1 && vy.abs() <= 0.1 {
                self.settled_ticks + 1
            } else {
                0
            };
        }
    }
    let mut engine = Engine::new(EngineConfig::default()).unwrap();
    let body = engine.world_mut().spawn("vertical PID fixture");
    engine
        .world_mut()
        .set_local_transform(body, Transform::from_translation(Vec3::Y * 0.5))
        .unwrap();
    engine
        .world_mut()
        .insert_component(
            body,
            PhysicsBody::new(
                BodyKind::Dynamic,
                BoxCollider {
                    center: [0.0; 3],
                    half_extents: [0.5; 3],
                },
            )
            .unwrap(),
        )
        .unwrap();
    let mut game = VerticalPilot {
        body,
        pid: Pid::new(PidConfig {
            kp: 4.0,
            ki: 2.0,
            kd: 0.3,
            derivative_filter_seconds: 0.05,
            output_min: 0.0,
            output_max: 25.0,
            integral_limit: 5.0,
            tracking_gain: 2.0,
        })
        .unwrap(),
        actual_thrust: 0.0,
        peak: 0.5,
        settled_ticks: 0,
    };
    engine.run_ticks(&mut game, 60 * 20).unwrap();
    assert!(
        game.settled_ticks > 60 * 10,
        "settled ticks={} peak={}",
        game.settled_ticks,
        game.peak
    );
    assert!(game.peak < 5.5, "peak={}", game.peak);
    assert!(
        (game.actual_thrust - 9.81).abs() < 0.05,
        "thrust={}",
        game.actual_thrust
    );
}
