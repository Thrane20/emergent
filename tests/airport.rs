//! Airports from scene-map tags and the control tower: departures taxi out, hold short and take off
//! along the centreline; arrivals are sequenced through the circuit, land in turn, vacate and park.
#![cfg(feature = "physics")]
use emergent::fixed_wing::{FlightGoal, FlightPhase, FlightTelemetry};
use emergent::tower::Stage;
use emergent::{
    Engine, EngineConfig, EngineContext, EntityId, Game, LocomotionAgent, Project, SceneDefinition,
    Vec3,
};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct Idle;
impl Game for Idle {
    fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
}

fn item(id: &str, at: [f32; 3], components: &str) -> String {
    format!("[[entities]]\nid='{id}'\nasset='assets/point.entity.toml'\n[entities.transform]\ntranslation=[{},{},{}]\n{components}", at[0], at[1], at[2])
}
fn tag(values: &str) -> String {
    format!("[[entities.components]]\ntype='emergent.metadata'\nvalues={{{values}}}\n")
}
fn waypoint(links: &[&str]) -> String {
    let links: Vec<String> = links.iter().map(|l| format!("'{l}'")).collect();
    if links.is_empty() {
        return "[[entities.components]]\ntype='emergent.waypoint'\n".into();
    }
    format!(
        "[[entities.components]]\ntype='emergent.waypoint'\nlinks=[{}]\n",
        links.join(",")
    )
}
fn plane(id: &str, at: [f32; 3], yaw: f32) -> String {
    format!("[[entities]]\nid='{id}'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[{},{},{}]\nrotation_degrees=[0,{yaw},0]\n[entities.physics]\nbody='dynamic'\nmass=1000\nfriction=0\ncollider={{shape='box',half_extents=[5,0.6,4]}}\n[[entities.components]]\ntype='emergent.locomotion'\nprofile='assets/locomotion/plane.locomotion.toml'\n", at[0], at[1], at[2])
}

/// A controlled airport: a 900 m runway landed northbound (threshold z=150), a parallel taxiway at
/// x=80, a holding position by the threshold, and five stands off an apron taxilane at x=100.
fn airport(planes: &str) -> (Fixture, Engine, HashMap<String, EntityId>) {
    let root = std::env::temp_dir().join(format!(
        "emergent-airport-{}-{}",
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
        root.join("assets/point.entity.toml"),
        "schema_version = 1\nid = 'point'\nkind = 'entity'\n",
    )
    .unwrap();
    fs::write(
        root.join("assets/locomotion/plane.locomotion.toml"),
        "schema_version=1\nid='plane'\nkind='locomotion'\ntype='fixed_wing_physics'\n",
    )
    .unwrap();
    let mut scene = String::from("schema_version=1\nid='main'\n[[entities]]\nid='ground'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[0,-1,0]\n[entities.physics]\nbody='static'\nfriction=0.8\ncollider={shape='box',half_extents=[6000,1,6000]}\n");
    scene += &item(
        "field",
        [0.0, 0.0, -300.0],
        &format!(
            "[[entities.components]]\ntype='emergent.zone'\nshape='circle'\nradius=2500\n{}",
            tag("aeroway='aerodrome',pattern_height=300")
        ),
    );
    scene += &item(
        "runway",
        [0.0, 0.0, -300.0],
        &format!(
            "[[entities.components]]\ntype='emergent.zone'\nshape='rect'\nwidth=30\ndepth=900\n{}",
            tag("aeroway='runway'")
        ),
    );
    scene += &item("tower", [150.0, 0.0, -300.0], &tag("aeroway='tower'"));
    scene += &item(
        "t1",
        [80.0, 0.0, 150.0],
        &format!("{}{}", waypoint(&["t2", "hold"]), tag("aeroway='taxiway'")),
    );
    scene += &item(
        "t2",
        [80.0, 0.0, -300.0],
        &format!("{}{}", waypoint(&["t3"]), tag("aeroway='taxiway'")),
    );
    scene += &item(
        "t3",
        [80.0, 0.0, -750.0],
        &format!("{}{}", waypoint(&[]), tag("aeroway='taxiway'")),
    );
    scene += &item(
        "hold",
        [40.0, 0.0, 190.0],
        &format!("{}{}", waypoint(&[]), tag("aeroway='holding_position'")),
    );
    // An apron taxilane at x=100 in front of the stands, joined to the taxiway at both ends.
    for (i, z) in [100.0f32, 0.0, -100.0, -200.0, -300.0].iter().enumerate() {
        let next = if i < 4 {
            format!("lane{}", i + 1)
        } else {
            "t2".into()
        };
        let links: Vec<&str> = if i == 0 {
            vec![next.as_str(), "t1"]
        } else {
            vec![next.as_str()]
        };
        scene += &item(
            &format!("lane{i}"),
            [100.0, 0.0, *z],
            &format!("{}{}", waypoint(&links), tag("aeroway='taxiway'")),
        );
        scene += &item(
            &format!("stand{i}"),
            [140.0, 0.0, *z],
            &format!(
                "{}{}",
                waypoint(&[&format!("lane{i}")]),
                tag("aeroway='parking_position'")
            ),
        );
    }
    scene += planes;
    fs::write(root.join("assets/scenes/main.scene.toml"), scene).unwrap();
    let definition = SceneDefinition::load(
        &Project::load(&root).unwrap(),
        "assets/scenes/main.scene.toml",
    )
    .unwrap();
    let mut engine = Engine::new(EngineConfig::default()).unwrap();
    let entities = definition.instantiate(engine.world_mut()).unwrap().entities;
    (Fixture(root), engine, entities)
}

fn order(engine: &mut Engine, plane: EntityId, goal: FlightGoal) {
    engine
        .world_mut()
        .component_mut::<LocomotionAgent>(plane)
        .unwrap()
        .unwrap()
        .flight = Some(goal);
}
fn status(engine: &Engine, plane: EntityId) -> (Option<Stage>, FlightTelemetry, Vec3) {
    let world = engine.world();
    let agent = world.component::<LocomotionAgent>(plane).unwrap().unwrap();
    (
        agent.dispatch().map(|d| d.stage),
        agent.flight_telemetry().unwrap_or_default(),
        world.global_transform(plane).unwrap().translation(),
    )
}
/// On the runway or low over it (the tower's occupancy rule).
fn on_runway(p: Vec3) -> bool {
    p.z < 170.0 && p.z > -770.0 && p.x.abs() < 30.0 && p.y < 30.0
}

#[test]
fn a_departure_taxis_out_holds_short_and_takes_off_along_the_centreline() {
    let (_f, mut engine, e) = airport(&plane("p", [140.0, 0.62, 100.0], 90.0));
    let (p, field) = (e["p"], e["field"]);
    order(
        &mut engine,
        p,
        FlightGoal::Depart {
            airport: field,
            altitude: 200.0,
        },
    );
    let (mut stages, mut worst_offset) = (Vec::<Stage>::new(), 0.0f32);
    for _ in 0..(240 * 60) {
        engine.run_ticks(&mut Idle, 1).unwrap();
        let (stage, t, at) = status(&engine, p);
        if let Some(stage) = stage {
            if stages.last() != Some(&stage) {
                stages.push(stage);
            }
            if stage == Stage::TakeOff && !t.airborne {
                worst_offset = worst_offset.max(at.x.abs());
            }
        }
        if stage == Some(Stage::Departed) && t.altitude > 190.0 {
            break;
        }
    }
    assert_eq!(
        stages,
        [
            Stage::TaxiOut,
            Stage::HoldShort,
            Stage::LineUp,
            Stage::TakeOff,
            Stage::Departed
        ]
    );
    assert!(
        worst_offset < 6.0,
        "rolls on the centreline: {worst_offset} m off"
    );
}

#[test]
fn five_arrivals_are_sequenced_land_in_turn_and_park_on_stands() {
    // Five aircraft inbound from different directions at 400-600 m, all asking at once.
    let starts = [
        [2500.0f32, 400.0, -2000.0],
        [-2500.0, 450.0, -1500.0],
        [0.0, 500.0, -4000.0],
        [3000.0, 550.0, 1500.0],
        [-3000.0, 600.0, 1000.0],
    ]
    .map(|at| (at, at[0].atan2(at[2]).to_degrees())); // nose towards the field
    let planes: String = starts
        .iter()
        .enumerate()
        .map(|(i, (at, yaw))| plane(&format!("a{i}"), *at, *yaw))
        .collect();
    let (_f, mut engine, e) = airport(&planes);
    let field = e["field"];
    let ids: Vec<EntityId> = (0..5).map(|i| e[&format!("a{i}")]).collect();
    // Air start: each already flying level at 40 m/s along its nose.
    for (&id, (_, yaw)) in ids.iter().zip(starts) {
        let yaw = yaw.to_radians();
        // One tick of m·v/dt along the nose: 1 t to 40 m/s at 60 Hz.
        engine
            .world_mut()
            .add_force(
                id,
                Vec3::new(-yaw.sin(), 0.0, -yaw.cos()) * (1000.0 * 40.0 * 60.0),
            )
            .unwrap();
        order(&mut engine, id, FlightGoal::Arrive { airport: field });
    }
    let (mut landed_order, mut most_on_runway, mut most_in_circuit, mut held) =
        (Vec::new(), 0usize, 0usize, false);
    let trace = std::env::var_os("FLIGHT_TRACE").is_some();
    for second in 0..1500 {
        engine.run_ticks(&mut Idle, 60).unwrap();
        let states: Vec<_> = ids.iter().map(|&id| status(&engine, id)).collect();
        most_on_runway = most_on_runway.max(
            states
                .iter()
                .filter(|(_, t, p)| on_runway(*p) && t.phase != FlightPhase::Taxi)
                .count(),
        );
        most_in_circuit = most_in_circuit.max(
            states
                .iter()
                .filter(|(s, _, _)| *s == Some(Stage::Circuit))
                .count(),
        );
        held |= states.iter().any(|(s, _, _)| *s == Some(Stage::Holding));
        for (i, (stage, t, _)) in states.iter().enumerate() {
            if t.landed && !landed_order.contains(&i)
                || (*stage == Some(Stage::Vacating) && !landed_order.contains(&i))
            {
                landed_order.push(i);
            }
        }
        if trace && second % 10 == 0 {
            let line: Vec<String> = states
                .iter()
                .map(|(s, t, p)| format!("{s:?}/{:?} ({:.0},{:.0},{:.0})", t.phase, p.x, p.y, p.z))
                .collect();
            println!("{second:>4}s {}", line.join(" | "));
        }
        if states.iter().all(|(s, _, _)| *s == Some(Stage::Parked)) {
            break;
        }
    }
    let states: Vec<_> = ids.iter().map(|&id| status(&engine, id)).collect();
    let go_arounds: u32 = states.iter().map(|(_, t, _)| t.go_arounds).sum();
    assert!(
        states.iter().all(|(s, _, _)| *s == Some(Stage::Parked)),
        "all parked: {:?}",
        states
            .iter()
            .map(|(s, t, p)| (s, t.phase, *p))
            .collect::<Vec<_>>()
    );
    assert_eq!(landed_order.len(), 5);
    assert!(held, "the fifth waited over the holding fix");
    assert!(most_in_circuit <= emergent::tower::CIRCUIT_CAPACITY);
    assert!(most_on_runway <= 1, "one aircraft on the runway at a time");
    assert_eq!(go_arounds, 0, "spacing needed no go-arounds");
    // Each on its own stand, east of the taxiway.
    let mut stands: Vec<i32> = states
        .iter()
        .map(|(_, _, p)| (p.z / 50.0).round() as i32)
        .collect();
    stands.sort_unstable();
    stands.dedup();
    assert_eq!(
        stands.len(),
        5,
        "five different stands: {:?}",
        states.iter().map(|(_, _, p)| *p).collect::<Vec<_>>()
    );
    assert!(states.iter().all(|(_, _, p)| p.x > 120.0));
}

#[test]
fn departures_and_arrivals_share_the_runway_without_conflict() {
    // Two aircraft parked on stands want to leave while three arrive.
    let mut planes =
        plane("d0", [140.0, 0.62, 100.0], 90.0) + &plane("d1", [140.0, 0.62, 0.0], 90.0);
    let arrivals = [
        [2500.0f32, 400.0, -2000.0],
        [-2500.0, 450.0, -1500.0],
        [0.0, 500.0, -4000.0],
    ];
    for (i, at) in arrivals.iter().enumerate() {
        planes += &plane(&format!("a{i}"), *at, at[0].atan2(at[2]).to_degrees());
    }
    let (_f, mut engine, e) = airport(&planes);
    let field = e["field"];
    for (i, at) in arrivals.iter().enumerate() {
        let id = e[&format!("a{i}")];
        let yaw = at[0].atan2(at[2]);
        engine
            .world_mut()
            .add_force(
                id,
                Vec3::new(-yaw.sin(), 0.0, -yaw.cos()) * (1000.0 * 40.0 * 60.0),
            )
            .unwrap();
        order(&mut engine, id, FlightGoal::Arrive { airport: field });
    }
    for d in ["d0", "d1"] {
        order(
            &mut engine,
            e[d],
            FlightGoal::Depart {
                airport: field,
                altitude: 600.0,
            },
        );
    }
    let ids: Vec<(String, EntityId)> = ["d0", "d1", "a0", "a1", "a2"]
        .iter()
        .map(|n| (n.to_string(), e[*n]))
        .collect();
    let mut most_on_runway = 0;
    for _ in 0..1200 {
        engine.run_ticks(&mut Idle, 60).unwrap();
        let states: Vec<_> = ids.iter().map(|(_, id)| status(&engine, *id)).collect();
        most_on_runway = most_on_runway.max(
            states
                .iter()
                .filter(|(_, t, p)| on_runway(*p) && t.phase != FlightPhase::Taxi)
                .count(),
        );
        let done = states.iter().zip(&ids).all(|((s, t, _), (name, _))| {
            if name.starts_with('d') {
                *s == Some(Stage::Departed) && t.altitude > 300.0
            } else {
                *s == Some(Stage::Parked)
            }
        });
        if done {
            break;
        }
    }
    let states: Vec<_> = ids
        .iter()
        .map(|(name, id)| (name.clone(), status(&engine, *id)))
        .collect();
    for (name, (stage, t, p)) in &states {
        let expected = if name.starts_with('d') {
            Stage::Departed
        } else {
            Stage::Parked
        };
        assert_eq!(*stage, Some(expected), "{name}: {:?} at {p}", t.phase);
    }
    assert!(most_on_runway <= 1, "one aircraft on the runway at a time");
    assert_eq!(
        states
            .iter()
            .map(|(_, (_, t, _))| t.go_arounds)
            .sum::<u32>(),
        0
    );
}

#[test]
fn a_runway_tagged_circuit_right_is_flown_on_the_right() {
    // Untowered: a plain landing on a runway tagged circuit=right (landing north, so right is east, +X).
    let (_f, mut engine, e) = airport(&plane("p", [0.0, 0.62, 100.0], 0.0));
    let runway = e["runway"];
    let mut values = engine
        .world()
        .component::<emergent::Metadata>(runway)
        .unwrap()
        .unwrap()
        .clone();
    values
        .0
        .push(("circuit".into(), emergent::MetaValue::Text("right".into())));
    engine.world_mut().insert_component(runway, values).unwrap();
    order(
        &mut engine,
        e["p"],
        FlightGoal::Land {
            zone: runway,
            pattern_height: 300.0,
        },
    );
    let mut downwind_x = Vec::new();
    for _ in 0..400 {
        engine.run_ticks(&mut Idle, 60).unwrap();
        let (_, t, p) = status(&engine, e["p"]);
        if t.phase == FlightPhase::Downwind {
            downwind_x.push(p.x);
        }
        if t.landed {
            break;
        }
    }
    assert!(
        !downwind_x.is_empty() && downwind_x.iter().all(|&x| x > 200.0),
        "downwind east of the runway: {downwind_x:?}"
    );
    assert!(status(&engine, e["p"]).1.landed);
}

#[test]
fn a_brain_departs_then_arrives_back_via_the_tower() {
    // Depart to 400 m, then come back: taxi, take off, circuit, land, vacate, park.
    let brain = "[[entities.components]]\ntype='emergent.brain'\nbrain='assets/brains/sortie.brain.toml'\nmax_speed=60\n";
    let (f, _, _) = airport("");
    fs::create_dir_all(f.0.join("assets/brains")).unwrap();
    fs::write(f.0.join("assets/brains/sortie.brain.toml"), "schema_version=1\nid='sortie'\nkind='brain'\n[[nodes]]\nid='seq'\ntype='sequence'\nloop=false\n[[nodes]]\nid='out'\nparent='seq'\ntype='depart_airport'\ngroup='airports'\nheight=400\n[[nodes]]\nid='back'\nparent='seq'\ntype='arrive_airport'\ngroup='airports'\n").unwrap();
    let scene = fs::read_to_string(f.0.join("assets/scenes/main.scene.toml")).unwrap()
        .replacen("aeroway='aerodrome',pattern_height=300}\n", "aeroway='aerodrome',pattern_height=300}\n[[entities.components]]\ntype='emergent.group'\nname='airports'\n", 1)
        + &plane("p", [140.0, 0.62, 100.0], 90.0) + brain;
    fs::write(f.0.join("assets/scenes/main.scene.toml"), scene).unwrap();
    let definition = SceneDefinition::load(
        &Project::load(&f.0).unwrap(),
        "assets/scenes/main.scene.toml",
    )
    .unwrap();
    let mut engine = Engine::new(EngineConfig::default()).unwrap();
    let p = definition.instantiate(engine.world_mut()).unwrap().entities["p"];
    let (mut stages, mut highest) = (Vec::<Stage>::new(), 0.0f32);
    for _ in 0..900 {
        engine.run_ticks(&mut Idle, 60).unwrap();
        let (stage, t, _) = status(&engine, p);
        highest = highest.max(t.altitude);
        if let Some(stage) = stage.filter(|s| stages.last() != Some(s)) {
            stages.push(stage);
        }
        if stage == Some(Stage::Parked) && stages.contains(&Stage::Circuit) {
            break;
        }
    }
    assert!(highest > 390.0, "climbed out to 400 m: {highest}");
    // Holding short is momentary with the runway free, so it can fall between one-second samples.
    for stage in [
        Stage::TaxiOut,
        Stage::LineUp,
        Stage::TakeOff,
        Stage::Departed,
        Stage::Circuit,
        Stage::Vacating,
        Stage::Parked,
    ] {
        assert!(stages.contains(&stage), "{stage:?} in {stages:?}");
    }
}

#[test]
fn the_tower_lands_into_the_wind() {
    // 6 m/s from the south is a tailwind for the northbound (forward) direction: land southbound.
    let (_f, mut engine, e) = airport(&format!(
        "{}[wind]\nspeed=6\nfrom=180\n",
        plane(
            "p",
            [-2500.0, 400.0, -1500.0],
            (-2500.0f32).atan2(-1500.0).to_degrees()
        )
    ));
    let p = e["p"];
    let yaw = (-2500.0f32).atan2(-1500.0);
    engine
        .world_mut()
        .add_force(
            p,
            Vec3::new(-yaw.sin(), 0.0, -yaw.cos()) * (1000.0 * 40.0 * 60.0),
        )
        .unwrap();
    order(
        &mut engine,
        p,
        FlightGoal::Arrive {
            airport: e["field"],
        },
    );
    let mut touchdown_heading = None;
    let mut last = FlightTelemetry::default();
    for _ in 0..(600 * 60) {
        engine.run_ticks(&mut Idle, 1).unwrap();
        let (stage, t, _) = status(&engine, p);
        if last.airborne && t.phase == FlightPhase::Rollout && touchdown_heading.is_none() {
            touchdown_heading = Some(t.heading);
        }
        last = t;
        if stage == Some(Stage::Parked) {
            break;
        }
    }
    let heading = touchdown_heading.expect("landed");
    assert!(
        (heading - 180.0).abs() < 20.0,
        "landed southbound, into the wind: {heading}°"
    );
    assert_eq!(status(&engine, p).0, Some(Stage::Parked));
}

#[test]
fn airport_views_show_the_active_direction_and_circuit_for_the_map() {
    let (_f, engine, e) = airport("[wind]\nspeed=6\nfrom=180\n");
    let profile = emergent::LocomotionProfile::from_source(
        "p.locomotion.toml",
        "schema_version=1\nid='p'\nkind='locomotion'\ntype='fixed_wing_physics'\n",
    )
    .unwrap();
    let emergent::locomotion::Model::FixedWingPhysics(params) = profile.model else {
        unreachable!()
    };
    let views = emergent::tower::airport_views(engine.world(), &params);
    assert_eq!(views.len(), 1);
    let airport = &views[0];
    assert!(airport.towered && airport.aerodrome == Some(e["field"]));
    let runway = &airport.runways[0];
    assert!(runway.active);
    assert!(
        (runway.runway.heading.to_degrees() - 180.0).abs() < 0.5,
        "southbound into the wind"
    );
    let circuit = runway.circuit.unwrap();
    // Southbound threshold is the runway's north end (z = -750); a left-hand circuit is then east.
    assert!((circuit.threshold.z + 750.0).abs() < 1.0, "{circuit:?}");
    assert!(
        circuit.entry.x > 300.0 && circuit.base_turn.x > 300.0,
        "{circuit:?}"
    );
    assert!(circuit.final_turn.x.abs() < 1.0 && circuit.final_turn.z < -750.0);
    // The holding fix is 3 km out on the extended centreline, before the threshold.
    assert!((airport.holding_fix.unwrap().z + 3750.0).abs() < 1.0);
}
