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
    airport_with(&standard_layout(), planes)
}

fn airport_with(layout: &str, planes: &str) -> (Fixture, Engine, HashMap<String, EntityId>) {
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
    // A slow miniature aircraft for 0.1-scale airfields.
    fs::write(
        root.join("assets/locomotion/mini.locomotion.toml"),
        "schema_version=1\nid='mini'\nkind='locomotion'\ntype='fixed_wing_physics'\nliftoff_speed=6\ncruise_speed=9\nmax_speed=14\nthrust_to_weight=0.4\nmax_climb=2\nmax_descent=2\n",
    )
    .unwrap();
    let mut scene = String::from("schema_version=1\nid='main'\n[[entities]]\nid='ground'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[0,-1,0]\n[entities.physics]\nbody='static'\nfriction=0.8\ncollider={shape='box',half_extents=[6000,1,6000]}\n");
    scene += layout;
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

fn standard_layout() -> String {
    let mut scene = String::new();
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
    scene
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

fn zone_rect(id: &str, at: [f32; 3], width: f32, depth: f32, tags: &str) -> String {
    item(
        id,
        at,
        &format!(
            "[[entities.components]]\ntype='emergent.zone'\nshape='rect'\nwidth={width}\ndepth={depth}\n{}",
            tag(tags)
        ),
    )
}
/// A two-point taxiway line, linked only to itself.
fn line(id: &str, a: [f32; 2], b: [f32; 2]) -> String {
    let end = format!("{id}b");
    item(
        &format!("{id}a"),
        [a[0], 0.0, a[1]],
        &format!("{}{}", waypoint(&[&end]), tag("aeroway='taxiway'")),
    ) + &item(
        &end,
        [b[0], 0.0, b[1]],
        &format!("{}{}", waypoint(&[]), tag("aeroway='taxiway'")),
    )
}

/// The same runway drawn the newer way: a threshold box at its north end only (so it is landed
/// southbound, +Z, from 20 m in), taxiway lines drawn separately and never linked to each other,
/// box stands and a box tower.
fn drawn_layout() -> String {
    let mut scene = item(
        "field",
        [0.0, 0.0, -300.0],
        &format!(
            "[[entities.components]]\ntype='emergent.zone'\nshape='circle'\nradius=2500\n{}",
            tag("aeroway='aerodrome',pattern_height=300")
        ),
    );
    scene += &zone_rect(
        "runway",
        [0.0, 0.0, -300.0],
        30.0,
        900.0,
        "aeroway='runway'",
    );
    // Outer edge at z = -730: 20 m in from the north (-Z) end.
    scene += &zone_rect(
        "threshold",
        [0.0, 0.0, -700.0],
        30.0,
        60.0,
        "aeroway='threshold'",
    );
    scene += &zone_rect("tower", [150.0, 0.0, -300.0], 10.0, 10.0, "aeroway='tower'");
    // Parallel taxiway and an apron taxilane, joined only by a cross line drawn over both, and a
    // stub whose end stops 10 m short of the parallel taxiway.
    scene += &line("par", [80.0, 150.0], [80.0, -760.0]);
    scene += &line("apron", [100.0, 100.0], [100.0, -300.0]);
    scene += &line("cross", [60.0, -320.0], [120.0, -320.0]);
    scene += &line("stub", [40.0, -780.0], [70.0, -760.0]);
    scene += &item(
        "hold",
        [40.0, 0.0, -790.0],
        &format!("{}{}", waypoint(&[]), tag("aeroway='holding_position'")),
    );
    for (i, z) in [100.0f32, 0.0, -100.0].iter().enumerate() {
        scene += &zone_rect(
            &format!("stand{i}"),
            [140.0, 0.0, *z],
            30.0,
            30.0,
            "aeroway='parking_position'",
        );
    }
    scene
}

#[test]
fn a_threshold_box_sets_the_landing_end_and_the_usable_runway() {
    let (_f, engine, e) = airport_with(&drawn_layout(), "");
    let world = engine.world();
    let tower = emergent::tower::airports(world);
    let (zone, runway, reverse) = emergent::tower::active_runway(world, &tower[0], None).unwrap();
    assert_eq!(zone, e["runway"]);
    assert!(
        reverse,
        "only the south end has a threshold: landed northbound"
    );
    assert!(
        (runway.length - 880.0).abs() < 0.1,
        "usable length {}",
        runway.length
    );
    let direction = emergent::fixed_wing::bearing_direction(runway.heading);
    let threshold = runway.center - direction * (runway.length / 2.0);
    assert!(
        (threshold.z + 730.0).abs() < 0.1 && threshold.x.abs() < 0.1,
        "{threshold:?}"
    );
    assert!(direction.z > 0.99, "southbound: {direction:?}");
}

#[test]
fn unconnected_taxiway_lines_route_a_departure_from_a_box_stand_to_the_threshold() {
    let (_f, mut engine, e) = airport_with(&drawn_layout(), &plane("p", [140.0, 0.62, 0.0], 90.0));
    let (p, field) = (e["p"], e["field"]);
    order(
        &mut engine,
        p,
        FlightGoal::Depart {
            airport: field,
            altitude: 200.0,
        },
    );
    let (mut stages, mut roll_start) = (Vec::<Stage>::new(), None::<Vec3>);
    for _ in 0..(300 * 60) {
        engine.run_ticks(&mut Idle, 1).unwrap();
        let (stage, t, at) = status(&engine, p);
        if let Some(stage) = stage {
            if stages.last() != Some(&stage) {
                stages.push(stage);
                if stage == Stage::TakeOff {
                    roll_start = Some(at);
                }
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
    let start = roll_start.unwrap();
    assert!(
        start.z > -735.0 && start.z < -600.0 && start.x.abs() < 10.0,
        "rolls from the threshold box: {start:?}"
    );
}

#[test]
fn an_arrival_lands_past_the_threshold_box_and_parks_on_a_box_stand() {
    let (_f, mut engine, e) =
        airport_with(&drawn_layout(), &plane("a", [2500.0, 400.0, -2500.0], 45.0));
    let (a, field) = (e["a"], e["field"]);
    let yaw = 45f32.to_radians();
    engine
        .world_mut()
        .add_force(
            a,
            Vec3::new(-yaw.sin(), 0.0, -yaw.cos()) * (1000.0 * 40.0 * 60.0),
        )
        .unwrap();
    order(&mut engine, a, FlightGoal::Arrive { airport: field });
    let mut touchdown = None::<Vec3>;
    for _ in 0..(900 * 60) {
        engine.run_ticks(&mut Idle, 1).unwrap();
        let (stage, t, at) = status(&engine, a);
        if touchdown.is_none() && t.landed {
            touchdown = Some(at);
        }
        if stage == Some(Stage::Parked) {
            break;
        }
    }
    let (stage, _, at) = status(&engine, a);
    assert_eq!(stage, Some(Stage::Parked), "parked at {at:?}");
    let touchdown = touchdown.expect("landed");
    assert!(
        touchdown.z > -730.0,
        "touched down past the threshold box: {touchdown:?}"
    );
    assert!(
        [100.0f32, 0.0, -100.0]
            .iter()
            .any(|z| (at.z - z).abs() < 12.0)
            && (at.x - 140.0).abs() < 12.0,
        "on a box stand: {at:?}"
    );
}

/// A pattern fix: a waypoint tagged `aeroway=circuit` with its leg, courses and height.
fn fix(
    id: &str,
    at: [f32; 2],
    next: Option<&str>,
    leg: &str,
    height: f32,
    entry: Option<[f32; 2]>,
    exit: Option<[f32; 2]>,
) -> String {
    let mut component = String::from("[[entities.components]]\ntype='emergent.waypoint'\n");
    if let Some(next) = next {
        component += &format!("links=['{next}']\n");
    }
    component += &format!("altitude={height}\n");
    if let Some([a, b]) = entry {
        component += &format!("entry_from={a}\nentry_to={b}\n");
    }
    if let Some([a, b]) = exit {
        component += &format!("exit_from={a}\nexit_to={b}\n");
    }
    item(
        id,
        [at[0], 0.0, at[1]],
        &(component + &tag(&format!("aeroway='circuit',leg='{leg}'"))),
    )
}

/// The drawn airport plus a hand-drawn pattern on the west (−X) side, opposite the generated
/// left-hand circuit (east, +X, for this southbound +Z landing). Downwind runs north (course 0°),
/// base east (90°), final south (180°) into the threshold box.
fn patterned_layout() -> String {
    drawn_layout()
        + &fix("p0", [-1500.0, 0.0], Some("p1"), "entry", 300.0, None, None)
        + &fix("p1", [-800.0, 200.0], Some("p2"), "downwind", 300.0, Some([350.0, 10.0]), None)
        + &fix("p2", [-800.0, -1800.0], Some("p3"), "downwind", 200.0, Some([350.0, 10.0]), Some([80.0, 100.0]))
        // The base leg, one turn radius past the downwind end.
        + &fix("p3", [-300.0, -2100.0], Some("p4"), "base", 100.0, Some([80.0, 100.0]), None)
        // On the extended centreline and the glide path, 970 m before the threshold box.
        + &fix("p4", [0.0, -1700.0], None, "final", 60.0, Some([170.0, 190.0]), None)
}

#[test]
fn a_drawn_traffic_pattern_is_flown_through_its_fixes_on_their_courses() {
    let (_f, mut engine, e) = airport_with(
        &patterned_layout(),
        &plane("a", [-2500.0, 350.0, 1500.0], 120.0),
    );
    let (a, field) = (e["a"], e["field"]);
    let yaw = 120f32.to_radians();
    engine
        .world_mut()
        .add_force(
            a,
            Vec3::new(-yaw.sin(), 0.0, -yaw.cos()) * (1000.0 * 40.0 * 60.0),
        )
        .unwrap();
    order(&mut engine, a, FlightGoal::Arrive { airport: field });
    let fixes = [
        (-800.0f32, 200.0f32),
        (-800.0, -1800.0),
        (-300.0, -2100.0),
        (0.0, -1700.0),
    ];
    let mut closest = [f32::MAX; 4];
    let mut heading_at = [0.0f32; 4];
    let (mut phases, mut touchdown, mut east_of_runway) = (Vec::new(), None::<Vec3>, false);
    let trace = std::env::var_os("FLIGHT_TRACE").is_some();
    for tick in 0..(900 * 60) {
        engine.run_ticks(&mut Idle, 1).unwrap();
        let (stage, t, at) = status(&engine, a);
        if phases.last() != Some(&t.phase) {
            phases.push(t.phase);
        }
        for (i, (x, z)) in fixes.iter().enumerate() {
            let d = Vec3::new(at.x - x, 0.0, at.z - z).length();
            if d < closest[i] {
                closest[i] = d;
                heading_at[i] = t.heading;
            }
        }
        // The generated circuit would fly downwind 400+ m east; the drawn one stays west.
        east_of_runway |= t.airborne && at.x > 200.0 && at.y > 100.0;
        if touchdown.is_none() && t.landed {
            touchdown = Some(at);
        }
        if trace && tick % 600 == 0 {
            println!(
                "{:>4}s {stage:?}/{:?} ({:.0},{:.0},{:.0}) hdg {:.0}",
                tick / 60,
                t.phase,
                at.x,
                at.y,
                at.z,
                t.heading
            );
        }
        if stage == Some(Stage::Parked) {
            break;
        }
    }
    let (stage, t, at) = status(&engine, a);
    assert_eq!(
        stage,
        Some(Stage::Parked),
        "parked: {at:?}, phases {phases:?}"
    );
    assert_eq!(t.go_arounds, 0, "phases {phases:?}");
    assert!(
        !east_of_runway,
        "flew the drawn west-side pattern, not the generated circuit"
    );
    for (i, d) in closest.iter().enumerate() {
        assert!(*d < 250.0, "crossed fix {i} (closest {d:.0} m)");
    }
    let within = |h: f32, from: f32, to: f32| {
        (h - from).rem_euclid(360.0) <= (to - from).rem_euclid(360.0) + 15.0
            || (from - h).rem_euclid(360.0) <= 15.0
    };
    assert!(
        within(heading_at[1], 350.0, 10.0) || within(heading_at[1], 80.0, 100.0),
        "downwind end crossed northbound or turning onto base: {heading_at:?}"
    );
    assert!(
        within(heading_at[2], 80.0, 100.0),
        "base fix crossed eastbound: {heading_at:?}"
    );
    assert!(
        within(heading_at[3], 170.0, 190.0),
        "final fix crossed southbound: {heading_at:?}"
    );
    for phase in [FlightPhase::Downwind, FlightPhase::Base, FlightPhase::Final] {
        assert!(phases.contains(&phase), "{phase:?} in {phases:?}");
    }
    assert!(touchdown.expect("landed").z > -730.0);
}

#[test]
fn the_map_overlay_leaves_out_the_generated_circuit_when_a_pattern_is_drawn() {
    let profile = emergent::LocomotionProfile::from_source(
        "p.locomotion.toml",
        "schema_version=1\nid='p'\nkind='locomotion'\ntype='fixed_wing_physics'\n",
    )
    .unwrap();
    let emergent::locomotion::Model::FixedWingPhysics(params) = profile.model else {
        unreachable!()
    };
    for (layout, drawn) in [(drawn_layout(), false), (patterned_layout(), true)] {
        let (_f, engine, _) = airport_with(&layout, "");
        let views = emergent::tower::airport_views(engine.world(), &params);
        let active = views[0].runways.iter().find(|r| r.active).unwrap();
        assert_eq!(active.circuit.is_none(), drawn);
    }
}

fn mini_plane(id: &str, at: [f32; 3], yaw: f32) -> String {
    format!("[[entities]]\nid='{id}'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[{},{},{}]\nrotation_degrees=[0,{yaw},0]\n[entities.physics]\nbody='dynamic'\nmass=20\nfriction=0\ncollider={{shape='box',half_extents=[1.2,0.25,1]}}\n[[entities.components]]\ntype='emergent.locomotion'\nprofile='assets/locomotion/mini.locomotion.toml'\n", at[0], at[1], at[2])
}

/// A 0.1-scale airfield: a 90 × 8 m runway landed north (−Z) from z = 15, a parallel taxiway at
/// x = 15, a box stand and tower. Its aerodrome carries `scale=0.1`.
fn miniature_layout() -> String {
    let mut scene = item(
        "field",
        [0.0, 0.0, -30.0],
        &format!(
            "[[entities.components]]\ntype='emergent.zone'\nshape='circle'\nradius=300\n{}",
            tag("aeroway='aerodrome',scale=0.1")
        ),
    );
    scene += &zone_rect("runway", [0.0, 0.0, -30.0], 8.0, 90.0, "aeroway='runway'");
    scene += &zone_rect("tower", [30.0, 0.0, -30.0], 3.0, 3.0, "aeroway='tower'");
    scene += &line("par", [15.0, 18.0], [15.0, -78.0]);
    scene += &line("link", [15.0, 18.0], [6.0, 18.0]);
    scene += &zone_rect(
        "stand0",
        [25.0, 0.0, -40.0],
        6.0,
        6.0,
        "aeroway='parking_position'",
    );
    scene
}

#[test]
fn a_slow_aircraft_flies_a_miniature_circuit_lands_and_parks() {
    let (_f, mut engine, e) = airport_with(
        &miniature_layout(),
        &mini_plane("a", [120.0, 40.0, 120.0], 45.0),
    );
    let (a, field) = (e["a"], e["field"]);
    let yaw = 45f32.to_radians();
    // Air start at 8 m/s along the nose (20 kg).
    engine
        .world_mut()
        .add_force(
            a,
            Vec3::new(-yaw.sin(), 0.0, -yaw.cos()) * (20.0 * 8.0 * 60.0),
        )
        .unwrap();
    order(&mut engine, a, FlightGoal::Arrive { airport: field });
    let (mut widest, mut highest, mut joined) = (0.0f32, 0.0f32, false);
    let trace = std::env::var_os("FLIGHT_TRACE").is_some();
    for tick in 0..(600 * 60) {
        engine.run_ticks(&mut Idle, 1).unwrap();
        let (stage, t, at) = status(&engine, a);
        joined |= matches!(t.phase, FlightPhase::Downwind);
        if joined && t.airborne {
            widest = widest.max(at.x.abs());
            highest = highest.max(at.y);
        }
        if trace && tick % 300 == 0 {
            println!(
                "{:>4}s {stage:?}/{:?} ({:.0},{:.0},{:.0}) {:.1} m/s",
                tick / 60,
                t.phase,
                at.x,
                at.y,
                at.z,
                t.airspeed
            );
        }
        if stage == Some(Stage::Parked) {
            break;
        }
    }
    let (stage, t, at) = status(&engine, a);
    assert_eq!(stage, Some(Stage::Parked), "parked: {at:?} {:?}", t.phase);
    assert_eq!(t.go_arounds, 0);
    assert!(joined);
    // A real-scale circuit would be at least 400 m out and 300 m up.
    assert!(
        widest < 120.0,
        "circuit stayed near the field: {widest:.0} m out"
    );
    assert!(highest < 45.0, "circuit height is scaled: {highest:.0} m");
    assert!(
        (at.x - 25.0).abs() < 6.0 && (at.z + 40.0).abs() < 6.0,
        "on the stand: {at:?}"
    );
}
