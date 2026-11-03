//! Physical fixed wing (FW-1): take-off roll, rotation, power-limited climb, altitude and compass
//! heading hold, bank limits, aerobatic manual flight, stalls and loader rules. Flown by forces only.
#![cfg(feature = "physics")]
use emergent::fixed_wing::{FlightGoal, FlightPhase, FlightTelemetry, Sticks};
use emergent::{
    Engine, EngineConfig, EngineContext, EntityId, Game, LocomotionAgent, Project, SceneDefinition,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(profile: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "emergent-fixed-wing-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for dir in ["assets/locomotion", "assets/scenes", "src/generated"] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        fs::write(
            root.join("emergent.toml"),
            include_str!("../templates/standard/emergent.toml"),
        )
        .unwrap();
        fs::write(root.join("assets/cube.entity.toml"), "schema_version=1\nid='cube'\nkind='primitive'\n[geometry]\nprimitive='cube'\nhalf_extent=0.5\ncolor=[1,1,1,1]\n").unwrap();
        fs::write(
            root.join("assets/locomotion/plane.locomotion.toml"),
            format!("schema_version=1\nid='plane'\nkind='locomotion'\ntype='fixed_wing_physics'\n{profile}\n"),
        )
        .unwrap();
        Self(root)
    }
    fn scene(&self, friction: f32) -> Result<SceneDefinition, emergent::SceneError> {
        self.scene_with(friction, "")
    }
    fn scene_with(
        &self,
        friction: f32,
        extra: &str,
    ) -> Result<SceneDefinition, emergent::SceneError> {
        // A 1 t light aircraft (10 m span, 8 m long) on a 6 km field.
        fs::write(self.0.join("assets/scenes/main.scene.toml"), format!("schema_version=1\nid='main'\n[[entities]]\nid='ground'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[0,-1,0]\n[entities.physics]\nbody='static'\nfriction=0.8\ncollider={{shape='box',half_extents=[3000,1,3000]}}\n[[entities]]\nid='plane'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[0,0.62,0]\n[entities.physics]\nbody='dynamic'\nmass=1000\nfriction={friction}\ncollider={{shape='box',half_extents=[5,0.6,4]}}\n[[entities.components]]\ntype='emergent.locomotion'\nprofile='assets/locomotion/plane.locomotion.toml'\n{extra}")).unwrap();
        SceneDefinition::load(
            &Project::load(&self.0).unwrap(),
            "assets/scenes/main.scene.toml",
        )
    }
    fn engine(&self) -> (Engine, EntityId) {
        let scene = self.scene(0.0).unwrap();
        let mut engine = Engine::new(EngineConfig::default()).unwrap();
        let loaded = scene.instantiate(engine.world_mut()).unwrap();
        (engine, loaded.entities["plane"])
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct Idle;
impl Game for Idle {
    fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
}

fn set(engine: &mut Engine, plane: EntityId, goal: FlightGoal) {
    engine
        .world_mut()
        .component_mut::<LocomotionAgent>(plane)
        .unwrap()
        .unwrap()
        .flight = Some(goal);
}
/// Runs `seconds` of fixed ticks, handing each tick's telemetry to `each`.
fn fly(
    engine: &mut Engine,
    plane: EntityId,
    seconds: f32,
    mut each: impl FnMut(&FlightTelemetry),
) -> FlightTelemetry {
    let ticks = (seconds * 60.0) as usize;
    let mut last = FlightTelemetry::default();
    for _ in 0..ticks {
        engine.run_ticks(&mut Idle, 1).unwrap();
        last = engine
            .world()
            .component::<LocomotionAgent>(plane)
            .unwrap()
            .unwrap()
            .flight_telemetry()
            .unwrap_or_default(); // none until the body's first physics step
        each(&last);
    }
    last
}
fn angle_error(a: f32, b: f32) -> f32 {
    ((a - b + 540.0).rem_euclid(360.0) - 180.0).abs()
}
/// Takes off and settles at 150 m on heading 0; returns the take-off speed.
fn airborne(engine: &mut Engine, plane: EntityId) -> f32 {
    set(engine, plane, FlightGoal::TakeOff { altitude: 150.0 });
    let mut liftoff = None;
    let mut tick = 0;
    let t = fly(engine, plane, 100.0, |t| {
        if liftoff.is_none() && t.airborne && t.altitude > 1.5 {
            liftoff = Some(t.airspeed);
        }
        if std::env::var_os("FLIGHT_TRACE").is_some() && tick % 60 == 0 {
            println!("{:>3}s {:?} alt {:.1} as {:.1} hdg {:.1} pitch {:.1} bank {:.1} aoa {:.1} thr {:.2} climb {:.1}", tick / 60, t.phase, t.altitude, t.airspeed, t.heading, t.pitch, t.bank, t.alpha, t.throttle, t.climb_rate);
        }
        tick += 1;
    });
    assert_eq!(t.phase, FlightPhase::Cruise, "{t:?}");
    liftoff.expect("left the ground")
}

#[test]
fn takes_off_at_liftoff_speed_climbs_and_holds_altitude_and_heading() {
    let f = Fixture::new("");
    let (mut engine, plane) = f.engine();
    // Parked: stays put with brakes on.
    set(&mut engine, plane, FlightGoal::Park);
    let parked = fly(&mut engine, plane, 2.0, |_| {});
    assert!(!parked.airborne && parked.ground_speed < 0.05, "{parked:?}");
    let liftoff = airborne(&mut engine, plane);
    assert!(
        (20.0..=28.0).contains(&liftoff),
        "lifts off near the 22 m/s lift-off speed: {liftoff}"
    );
    let mut worst = (0.0f32, 0.0f32);
    let t = fly(&mut engine, plane, 30.0, |t| {
        worst.0 = worst.0.max((t.altitude - 150.0).abs());
        worst.1 = worst.1.max(angle_error(t.heading, 0.0));
    });
    assert!(
        worst.0 < 3.0 && worst.1 < 3.0,
        "holds 150 m and heading 0: {worst:?} {t:?}"
    );
    assert!((t.airspeed - 40.0).abs() < 3.0, "cruise speed: {t:?}");
}

#[test]
fn turns_to_a_compass_heading_within_max_bank_and_holds_altitude() {
    let f = Fixture::new("max_bank=30");
    let (mut engine, plane) = f.engine();
    airborne(&mut engine, plane);
    set(
        &mut engine,
        plane,
        FlightGoal::Hold {
            heading: 270.0,
            altitude: 150.0,
            speed: 40.0,
        },
    );
    let (mut bank, mut altitude) = (0.0f32, 0.0f32);
    fly(&mut engine, plane, 40.0, |t| {
        bank = bank.max(t.bank.abs());
        altitude = altitude.max((t.altitude - 150.0).abs());
    });
    let t = fly(&mut engine, plane, 10.0, |_| {});
    // West is 270°: from north the shorter turn is to the left.
    assert!(angle_error(t.heading, 270.0) < 3.0, "{t:?}");
    assert!(
        bank <= 33.0 && bank > 15.0,
        "banks to turn but not past 30°: {bank}"
    );
    assert!(altitude < 10.0, "turning keeps its height: {altitude}");
}

#[test]
fn an_underpowered_aircraft_cannot_take_off() {
    let f = Fixture::new("thrust_to_weight=0.03");
    let (mut engine, plane) = f.engine();
    set(&mut engine, plane, FlightGoal::TakeOff { altitude: 150.0 });
    let t = fly(&mut engine, plane, 60.0, |t| {
        assert!(t.altitude < 2.0, "{t:?}")
    });
    assert!(
        t.airspeed < 22.0 && t.phase == FlightPhase::TakeoffRoll,
        "{t:?}"
    );
}

#[test]
fn manual_rolls_past_vertical_only_when_max_bank_is_zero_and_slow_flight_stalls() {
    for (max_bank, aerobatic) in [(0.0, true), (45.0, false)] {
        let f = Fixture::new(&format!("max_bank={max_bank}"));
        let (mut engine, plane) = f.engine();
        airborne(&mut engine, plane);
        set(
            &mut engine,
            plane,
            FlightGoal::Manual(Sticks {
                roll: 1.0,
                throttle: 0.6,
                ..Sticks::default()
            }),
        );
        let mut bank = 0.0f32;
        fly(&mut engine, plane, 3.0, |t| bank = bank.max(t.bank.abs()));
        if aerobatic {
            assert!(bank > 120.0, "rolls inverted: {bank}");
        } else {
            assert!(bank < 50.0, "stops at max bank: {bank}");
            // Power off, stick back: the wing stalls.
            set(
                &mut engine,
                plane,
                FlightGoal::Manual(Sticks {
                    pitch: 1.0,
                    ..Sticks::default()
                }),
            );
            let mut stalled = false;
            fly(&mut engine, plane, 20.0, |t| stalled |= t.stalled);
            assert!(stalled, "stalls when slow");
        }
    }
}

#[test]
fn aircraft_need_a_frictionless_box_body() {
    let f = Fixture::new("");
    let error = f
        .scene(0.5)
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default();
    assert!(error.contains("friction 0"), "{error}");
    let bad = Fixture::new("liftoff_speed=50\ncruise_speed=40");
    assert!(bad
        .scene(0.0)
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default()
        .contains("lift-off speed < cruise speed"));
}

#[test]
fn flies_to_a_point_and_circles_it() {
    let f = Fixture::new("");
    let (mut engine, plane) = f.engine();
    airborne(&mut engine, plane);
    let point = emergent::Vec3::new(1500.0, 0.0, -2500.0);
    set(
        &mut engine,
        plane,
        FlightGoal::FlyTo {
            point,
            altitude: 200.0,
            speed: 40.0,
        },
    );
    fly(&mut engine, plane, 120.0, |_| {});
    let mut distances = Vec::new();
    let mut t = FlightTelemetry::default();
    for _ in 0..60 {
        t = fly(&mut engine, plane, 1.0, |_| {});
        let p = engine_position(&engine, plane);
        distances.push(emergent::Vec3::new(p.x - point.x, 0.0, p.z - point.z).length());
    }
    let (near, far) = distances
        .iter()
        .fold((f32::MAX, 0.0f32), |(a, b), &d| (a.min(d), b.max(d)));
    assert!(
        far < 400.0 && near > 50.0,
        "circles the point: {near}..{far}"
    );
    assert!((t.altitude - 200.0).abs() < 10.0, "{t:?}");
}
fn engine_position(engine: &Engine, plane: EntityId) -> emergent::Vec3 {
    engine
        .world()
        .global_transform(plane)
        .unwrap()
        .translation()
}

/// A 900 m × 30 m runway zone centred ahead of the aircraft, landed northbound (heading 0).
const RUNWAY: &str = "[[entities]]\nid='runway'\nasset='assets/point.entity.toml'\n[entities.transform]\ntranslation=[0,0,-300]\n[[entities.components]]\ntype='emergent.zone'\nshape='rect'\nwidth=30\ndepth=900\n";

fn landing_fixture() -> (Fixture, Engine, EntityId, EntityId) {
    let f = Fixture::new("");
    fs::write(
        f.0.join("assets/point.entity.toml"),
        "schema_version = 1\nid = 'point'\nkind = 'entity'\n",
    )
    .unwrap();
    let scene = f.scene_with(0.0, RUNWAY).unwrap();
    let mut engine = Engine::new(EngineConfig::default()).unwrap();
    let loaded = scene.instantiate(engine.world_mut()).unwrap();
    let (plane, zone) = (loaded.entities["plane"], loaded.entities["runway"]);
    (f, engine, plane, zone)
}

#[test]
fn flies_a_circuit_and_lands_inside_the_zone() {
    let (_f, mut engine, plane, zone) = landing_fixture();
    set(
        &mut engine,
        plane,
        FlightGoal::Land {
            zone,
            pattern_height: 300.0,
        },
    );
    let (mut phases, mut final_error, mut touchdown) = (Vec::new(), 0.0f32, None);
    let mut last = FlightTelemetry::default();
    let trace = std::env::var_os("FLIGHT_TRACE").is_some();
    for second in 0..900 {
        let t = fly(&mut engine, plane, 1.0, |t| {
            if last.airborne
                && !t.airborne
                && t.phase == FlightPhase::Rollout
                && touchdown.is_none()
            {
                touchdown = Some((t.climb_rate, t.pitch));
            }
            if t.phase == FlightPhase::Final && t.altitude < 100.0 {
                final_error = final_error.max(t.glide_error.abs());
            }
            last = *t;
        });
        if trace {
            let p = engine_position(&engine, plane);
            println!("{second:>3}s {:?} pos ({:.0},{:.0},{:.0}) as {:.1} hdg {:.0} pitch {:.1} bank {:.1} thr {:.2} glide {:.1} vs {:.1}", t.phase, p.x, p.y, p.z, t.airspeed, t.heading, t.pitch, t.bank, t.throttle, t.glide_error, t.climb_rate);
        }
        if phases.last() != Some(&t.phase) {
            phases.push(t.phase);
        }
        if t.landed {
            break;
        }
    }
    use FlightPhase::*;
    assert!(last.landed, "landed: {phases:?} {last:?}");
    for phase in [
        TakeoffRoll,
        Rotate,
        Climb,
        PatternEntry,
        Downwind,
        Base,
        Final,
        Flare,
        Rollout,
        Parked,
    ] {
        assert!(phases.contains(&phase), "{phase:?} in {phases:?}");
    }
    assert_eq!(last.go_arounds, 0, "{phases:?}");
    let (sink, pitch) = touchdown.expect("touched down");
    assert!(
        sink > -2.5 && pitch > -0.5,
        "gentle, nose-up touchdown: {sink} m/s at {pitch}°"
    );
    assert!(
        final_error < 25.0,
        "stays near the glide path below 100 m: {final_error}"
    );
    // Stopped on the runway: inside the 900 × 30 m zone around (0, -300).
    let p = engine_position(&engine, plane);
    assert!(
        p.x.abs() < 15.0 && (p.z + 300.0).abs() < 450.0,
        "stopped inside the zone at {p}"
    );
}

#[test]
fn a_deleted_landing_zone_means_hold_not_land() {
    let (_f, mut engine, plane, zone) = landing_fixture();
    airborne(&mut engine, plane);
    set(
        &mut engine,
        plane,
        FlightGoal::Land {
            zone,
            pattern_height: 300.0,
        },
    );
    fly(&mut engine, plane, 20.0, |_| {});
    engine.world_mut().remove_subtree(zone).unwrap();
    let t = fly(&mut engine, plane, 30.0, |_| {});
    assert_eq!(t.phase, FlightPhase::Cruise, "{t:?}");
    assert!(t.airborne && !t.landed);
}

#[test]
fn taxis_to_a_point_and_stops() {
    let f = Fixture::new("");
    let (mut engine, plane) = f.engine();
    let point = emergent::Vec3::new(60.0, 0.0, -80.0);
    set(&mut engine, plane, FlightGoal::Taxi { point, stop: true });
    let mut fastest = 0.0f32;
    let t = fly(&mut engine, plane, 60.0, |t| {
        fastest = fastest.max(t.ground_speed)
    });
    let p = engine_position(&engine, plane);
    assert!(!t.airborne && t.phase == FlightPhase::Parked, "{t:?}");
    assert!(
        emergent::Vec3::new(p.x - point.x, 0.0, p.z - point.z).length() < 6.0,
        "{p}"
    );
    assert!(fastest <= 9.0, "taxi speed: {fastest}");
}

#[test]
fn editor_preview_flies_the_circuit_and_reports_phases() {
    use emergent::locomotion::{simulate, Course};
    let profile = emergent::LocomotionProfile::from_source(
        "assets/plane.locomotion.toml",
        "schema_version=1\nid='plane'\nkind='locomotion'\ntype='fixed_wing_physics'\n",
    )
    .unwrap();
    let trace = simulate(&profile, Course::Circuit, 240.0).unwrap();
    assert_eq!(trace.frames.len(), trace.phases.len());
    assert!((trace.frame_seconds - 0.1).abs() < 1e-6);
    for phase in [
        FlightPhase::Climb,
        FlightPhase::Downwind,
        FlightPhase::Final,
        FlightPhase::Rollout,
    ] {
        assert!(trace.phases.contains(&phase), "{phase:?}");
    }
    assert_eq!(trace.phases.last(), Some(&FlightPhase::Parked));
    let (end, _) = trace.frames.last().unwrap();
    assert!(
        end.x.abs() < 15.0 && end.z < 150.0 && end.z > -750.0,
        "stopped on the preview runway: {end}"
    );
    let taxi = simulate(&profile, Course::Stop, 60.0).unwrap();
    assert!(
        taxi.frames.iter().all(|(p, _)| p.y < 2.0),
        "taxiing stays on the ground"
    );
}

#[test]
fn brain_take_off_and_land_nodes_fly_the_aircraft() {
    let f = Fixture::new("");
    fs::write(
        f.0.join("assets/point.entity.toml"),
        "schema_version = 1\nid = 'point'\nkind = 'entity'\n",
    )
    .unwrap();
    fs::create_dir_all(f.0.join("assets/brains")).unwrap();
    // Take off to 200 m, then land on the nearest runway.
    fs::write(f.0.join("assets/brains/sortie.brain.toml"), "schema_version=1\nid='sortie'\nkind='brain'\n[[nodes]]\nid='seq'\ntype='sequence'\nloop=false\n[[nodes]]\nid='up'\nparent='seq'\ntype='take_off'\nheight=200\n[[nodes]]\nid='down'\nparent='seq'\ntype='land'\ngroup='runways'\npattern_height=250\n").unwrap();
    let brain = "[[entities.components]]\ntype='emergent.brain'\nbrain='assets/brains/sortie.brain.toml'\nmax_speed=60\n";
    let runway =
        format!("{RUNWAY}[[entities.components]]\ntype='emergent.group'\nname='runways'\n");
    let scene = f.scene_with(0.0, &format!("{brain}{runway}")).unwrap();
    let mut engine = Engine::new(EngineConfig::default()).unwrap();
    let plane = scene.instantiate(engine.world_mut()).unwrap().entities["plane"];
    let (mut highest, mut last) = (0.0f32, FlightTelemetry::default());
    for _ in 0..400 {
        last = fly(&mut engine, plane, 1.0, |t| {
            highest = highest.max(t.altitude)
        });
        if last.landed {
            break;
        }
    }
    assert!(
        highest > 185.0,
        "climbed to the take-off height first: {highest}"
    );
    assert!(last.landed, "then landed: {last:?}");
}

#[test]
fn crosswind_landings_crab_onto_the_centreline_and_headwinds_shorten_the_roll() {
    // 6 m/s from the west across a northbound runway (blowing towards +X).
    let f = Fixture::new("");
    fs::write(
        f.0.join("assets/point.entity.toml"),
        "schema_version = 1\nid = 'point'\nkind = 'entity'\n",
    )
    .unwrap();
    let scene = f
        .scene_with(0.0, &format!("{RUNWAY}[wind]\nspeed=6\nfrom=270\n"))
        .unwrap();
    let mut engine = Engine::new(EngineConfig::default()).unwrap();
    let loaded = scene.instantiate(engine.world_mut()).unwrap();
    let (plane, zone) = (loaded.entities["plane"], loaded.entities["runway"]);
    assert_eq!(
        engine
            .world()
            .resource::<emergent::Wind>()
            .map(|w| w.compass())
            .map(|(s, f)| (s.round(), f.round())),
        Some((6.0, 270.0))
    );
    set(
        &mut engine,
        plane,
        FlightGoal::Land {
            zone,
            pattern_height: 300.0,
        },
    );
    let (mut crab, mut touchdown, mut last) = (0.0f32, None, FlightTelemetry::default());
    for _ in 0..600 {
        let t = fly(&mut engine, plane, 1.0, |t| {
            if t.phase == FlightPhase::Final && t.altitude < 60.0 {
                crab = crab.max(angle_error(t.heading, 0.0));
            }
            if std::env::var_os("FLIGHT_TRACE").is_some()
                && matches!(
                    t.phase,
                    FlightPhase::Final | FlightPhase::GoAround | FlightPhase::Base
                )
                && (t.altitude * 10.0).round() as i32 % 50 == 0
            {
                println!(
                    "{:?} alt {:.0} hdg {:.1} bank {:.1} glide {:.1} as {:.1} gs {:.1}",
                    t.phase,
                    t.altitude,
                    t.heading,
                    t.bank,
                    t.glide_error,
                    t.airspeed,
                    t.ground_speed
                );
            }
            if last.airborne
                && !t.airborne
                && t.phase == FlightPhase::Rollout
                && touchdown.is_none()
            {
                touchdown = Some(t.climb_rate);
            }
            last = *t;
        });
        if touchdown.is_some() && t.landed {
            break;
        }
    }
    let p = engine_position(&engine, plane);
    assert!(last.landed && last.go_arounds == 0, "{last:?}");
    assert!(
        crab > 6.0 && crab < 25.0,
        "crabs into the wind on final: {crab}°"
    );
    assert!(p.x.abs() < 10.0, "stopped near the centreline: {p}");
    assert!(touchdown.unwrap() > -3.0);
}

#[test]
fn a_headwind_shortens_the_take_off_roll() {
    // Measure where it leaves the ground: ground distance from the start.
    let liftoff_distance = |wind: &str| {
        let f = Fixture::new("");
        let scene = f.scene_with(0.0, wind).unwrap();
        let mut engine = Engine::new(EngineConfig::default()).unwrap();
        let plane = scene.instantiate(engine.world_mut()).unwrap().entities["plane"];
        set(&mut engine, plane, FlightGoal::TakeOff { altitude: 150.0 });
        for _ in 0..(60 * 60) {
            engine.run_ticks(&mut Idle, 1).unwrap();
            if engine_position(&engine, plane).y > 1.5 {
                return -engine_position(&engine, plane).z;
            }
        }
        f32::MAX
    };
    let calm = liftoff_distance("");
    // 8 m/s from the north: straight down the runway into the aircraft's face.
    let headwind = liftoff_distance("[wind]\nspeed=8\nfrom=0\n");
    assert!(
        headwind < calm * 0.75,
        "headwind roll {headwind} m vs calm {calm} m"
    );
}

#[test]
fn a_slow_flyer_holds_its_wings_level_and_its_speed_in_the_circuit() {
    use emergent::locomotion::{simulate, Course};
    // Lift-off at 5 m/s: a fixed heading→bank gain made this rock its wings (bank ±30° every few
    // seconds), and a 2 m/s climb beyond its spare thrust bled it to the stall.
    let profile = emergent::LocomotionProfile::from_source(
        "assets/slow.locomotion.toml",
        "schema_version=1\nid='slow'\nkind='locomotion'\ntype='fixed_wing_physics'\nmax_speed=30\nmax_climb=2\nliftoff_speed=5\ncruise_speed=20\n",
    )
    .unwrap();
    let trace = simulate(&profile, Course::Circuit, 240.0).unwrap();
    let (mut reversals, mut last) = (0, 0.0_f32);
    for (index, (_, activity)) in trace.frames.iter().enumerate() {
        if !matches!(
            trace.phases[index],
            FlightPhase::PatternEntry | FlightPhase::Downwind
        ) {
            continue;
        }
        let bank = activity.bank.to_degrees();
        if bank.abs() > 3.0 {
            reversals += usize::from(last * bank < 0.0);
            last = bank;
        }
    }
    assert!(reversals <= 2, "bank reversals in the circuit: {reversals}");
    let downwind: Vec<f32> = (0..trace.frames.len())
        .filter(|&i| trace.phases[i] == FlightPhase::Downwind)
        .map(|i| trace.frames[i].1.speed)
        .collect();
    assert!(!downwind.is_empty(), "reaches downwind");
    assert!(
        downwind.iter().all(|&v| v > 6.0),
        "keeps circuit speed (≈7.5 m/s), slowest {:?}",
        downwind.iter().cloned().fold(f32::MAX, f32::min)
    );
}
