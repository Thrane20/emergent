//! Airports described by scene-map tags (`aeroway=*`) and the control tower that turns
//! `Arrive`/`Depart` orders into the concrete goals and gates each aircraft's pilot flies.
//! See `docs/design/airport-operations.md`.
// The tower only runs with the physics feature (physical aircraft); the model is always built.
#![cfg_attr(not(feature = "physics"), allow(dead_code))]
use std::collections::HashMap;
use std::sync::Arc;

use bevy_ecs::prelude::Resource;

use crate::fixed_wing::{
    bearing_direction, FlightGoal, FlightPhase, FlightTelemetry, Gates, PatternFix, Runway,
};
use crate::locomotion::{LocomotionAgent, Model};
use crate::taxi::{NodeKind, TaxiGraph};
use crate::{EngineError, EntityId, MetaValue, Metadata, Vec3, World};

/// Aircraft flying the circuit at once; later arrivals hold over the holding fix.
pub const CIRCUIT_CAPACITY: usize = 4;
/// Height between aircraft stacked over the holding fix.
const STACK_SPACING: f32 = 150.0;
/// Approach distance kept between successive arrivals (≈70 s at circuit speed: enough for the
/// one ahead to land and vacate). A follower turns base once its remaining path to the threshold
/// (downwind position + base leg) exceeds the leader's by this much.
const SEPARATION: f32 = 2000.0;
/// On downwind, slow to approach speed when the one ahead is this close.
const SLOW_DISTANCE: f32 = 1200.0;
/// A departure waits while an arrival on final is within this distance of the threshold.
const DEPARTURE_GAP: f32 = 3000.0;
/// A taxi route point counts as passed within this distance.
const ROUTE_REACH: f32 = 12.0;
/// Distance of the default holding fix out along the extended centreline.
const HOLDING_FIX_DISTANCE: f32 = 3000.0;
const DEFAULT_PATTERN_HEIGHT: f32 = 300.0;

/// One airport's items, by role. Geometry is read live from the items each tick.
#[derive(Clone, Debug, Default)]
pub struct Airport {
    /// The `aeroway=aerodrome` zone; `None` for the implicit airport of an untagged scene.
    pub aerodrome: Option<EntityId>,
    pub pattern_height: f32,
    /// Size relative to a real airport (`scale` tag on the aerodrome, default 1). Circuits,
    /// spacing and holding distances shrink with it; aircraft speed still sets turn size.
    pub scale: f32,
    /// An `aeroway=tower` item makes the airport controlled (sequenced).
    pub towered: bool,
    pub runways: Vec<EntityId>,
    pub taxiways: Vec<EntityId>,
    pub holding_positions: Vec<EntityId>,
    pub stands: Vec<EntityId>,
    pub holding_fixes: Vec<EntityId>,
}

impl Airport {
    fn contains(&self, id: EntityId) -> bool {
        self.aerodrome == Some(id)
            || [
                &self.runways,
                &self.taxiways,
                &self.holding_positions,
                &self.stands,
                &self.holding_fixes,
            ]
            .iter()
            .any(|items| items.contains(&id))
    }
}

fn text<'a>(world: &'a World, id: EntityId, key: &str) -> Option<&'a str> {
    match world.component::<Metadata>(id).ok()??.get(key)? {
        MetaValue::Text(value) => Some(value.as_str()),
        _ => None,
    }
}
fn number(world: &World, id: EntityId, key: &str) -> Option<f32> {
    match world.component::<Metadata>(id).ok()??.get(key)? {
        MetaValue::Number(value) => Some(*value as f32),
        _ => None,
    }
}
/// An aerodrome's `scale` tag, kept to 0.01–10.
fn aerodrome_scale(world: &World, aerodrome: EntityId) -> f32 {
    number(world, aerodrome, "scale").map_or(1.0, |s| s.clamp(0.01, 10.0))
}
/// The scale of the aerodrome containing `at` (1 outside any).
fn scale_at(world: &World, at: Vec3) -> f32 {
    let value = MetaValue::Text("aerodrome".into());
    world
        .zones_at(at)
        .find(|&zone| {
            world
                .component::<Metadata>(zone)
                .ok()
                .flatten()
                .and_then(|m| m.get("aeroway"))
                == Some(&value)
        })
        .map_or(1.0, |zone| aerodrome_scale(world, zone))
}
fn position(world: &World, id: EntityId) -> Option<Vec3> {
    world.global_transform(id).ok().map(|t| t.translation())
}

/// Every airport in the scene, from `aeroway` tags. Items belong to the aerodrome zone they sit
/// in (else the nearest aerodrome); with no aerodrome, all of them form one implicit airport.
pub fn airports(world: &World) -> Vec<Airport> {
    let tagged = |kind: &str| -> Vec<EntityId> {
        let value = MetaValue::Text(kind.into());
        world.items_where("aeroway", &value).collect()
    };
    let aerodromes = tagged("aerodrome");
    let mut list: Vec<Airport> = if aerodromes.is_empty() {
        vec![Airport {
            pattern_height: DEFAULT_PATTERN_HEIGHT,
            scale: 1.0,
            ..Airport::default()
        }]
    } else {
        aerodromes
            .iter()
            .map(|&id| {
                let scale = aerodrome_scale(world, id);
                Airport {
                    aerodrome: Some(id),
                    // Full-size metres, shrunk with the rest of the airport.
                    pattern_height: number(world, id, "pattern_height")
                        .unwrap_or(DEFAULT_PATTERN_HEIGHT)
                        * scale,
                    scale,
                    ..Airport::default()
                }
            })
            .collect()
    };
    let owner = |id: EntityId| -> usize {
        if aerodromes.is_empty() {
            return 0;
        }
        let Some(at) = position(world, id) else {
            return 0;
        };
        if let Some(index) = world
            .zones_at(at)
            .find_map(|zone| aerodromes.iter().position(|&a| a == zone))
        {
            return index;
        }
        (0..aerodromes.len())
            .min_by(|&a, &b| {
                let d = |i: usize| {
                    position(world, aerodromes[i]).map_or(f32::MAX, |p| p.distance_squared(at))
                };
                d(a).total_cmp(&d(b))
            })
            .unwrap_or(0)
    };
    for (kind, pick) in [
        ("runway", 0usize),
        ("taxiway", 1),
        ("holding_position", 2),
        ("parking_position", 3),
        ("holding_fix", 4),
        ("tower", 5),
    ] {
        for id in tagged(kind) {
            let airport = &mut list[owner(id)];
            match pick {
                0 => airport.runways.push(id),
                1 => airport.taxiways.push(id),
                2 => airport.holding_positions.push(id),
                3 => airport.stands.push(id),
                4 => airport.holding_fixes.push(id),
                _ => airport.towered = true,
            }
        }
    }
    // An untagged scene has no airport (the implicit one exists only when it has items).
    if aerodromes.is_empty()
        && list[0].runways.is_empty()
        && list[0].taxiways.is_empty()
        && list[0].stands.is_empty()
        && list[0].holding_positions.is_empty()
        && !list[0].towered
    {
        list.clear();
    }
    list
}

/// A rectangular zone as a runway: landed along its local −Z (or +Z when `reverse`), with the
/// circuit side from its `circuit=right` tag. A threshold box at the landing end moves the
/// threshold to the box's outer edge (landings aim from there, take-offs line up there).
/// None if it is gone or not a rectangle.
pub fn runway_geometry(world: &World, zone: EntityId, reverse: bool) -> Option<Runway> {
    let mut runway = drawn_runway(world, zone, reverse)?;
    let forward = drawn_runway(world, zone, false)?;
    let start = thresholds(world, &forward)
        .filter(|&(end, _, _)| end == reverse)
        .map(|(_, start, _)| start)
        .min_by(f32::total_cmp);
    if let Some(start) = start {
        runway.center += bearing_direction(runway.heading) * (start / 2.0);
        runway.length -= start;
    }
    Some(runway)
}

/// `aeroway=threshold` items on a runway: for each, the landing end it opens (true: the reverse
/// end) and how far its outer edge lies in from that end (at most half the runway).
fn thresholds<'a>(
    world: &'a World,
    forward: &'a Runway,
) -> impl Iterator<Item = (bool, f32, EntityId)> + 'a {
    let value = MetaValue::Text("threshold".into());
    let items: Vec<EntityId> = world.items_where("aeroway", &value).collect();
    items.into_iter().filter_map(move |id| {
        let transform = world.global_transform(id).ok()?;
        let matrix = transform.matrix();
        let center = transform.translation();
        let (along, across) = runway_frame(forward, center);
        let margin = forward.scale;
        if across.abs() > forward.width / 2.0 + 30.0 * margin
            || along < -50.0 * margin
            || along > forward.length + 50.0 * margin
        {
            return None;
        }
        // The box's extent along the runway, from its corners.
        let corners: Vec<Vec3> = match world
            .component::<crate::Zone>(id)
            .ok()
            .flatten()
            .map(|z| z.shape)
        {
            Some(crate::ZoneShape::Rect { width, depth }) => {
                [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                    .iter()
                    .map(|&(x, z)| {
                        matrix.transform_point3(Vec3::new(x * width / 2.0, 0.0, z * depth / 2.0))
                    })
                    .collect()
            }
            Some(crate::ZoneShape::Circle { radius }) => {
                let direction = bearing_direction(forward.heading);
                vec![center - direction * radius, center + direction * radius]
            }
            None => vec![center],
        };
        let alongs = corners.iter().map(|&p| runway_frame(forward, p).0);
        let half = forward.length / 2.0;
        Some(if along <= half {
            (false, alongs.fold(f32::MAX, f32::min).clamp(0.0, half), id)
        } else {
            (
                true,
                (forward.length - alongs.fold(f32::MIN, f32::max)).clamp(0.0, half),
                id,
            )
        })
    })
}

/// The runway zone exactly as drawn, ignoring threshold boxes.
fn drawn_runway(world: &World, zone: EntityId, reverse: bool) -> Option<Runway> {
    let crate::ZoneShape::Rect { width, depth } = world.component::<crate::Zone>(zone).ok()??.shape
    else {
        return None;
    };
    let transform = world.global_transform(zone).ok()?;
    let (scale, rotation, center) = transform.matrix().to_scale_rotation_translation();
    let heading = crate::fixed_wing::compass(rotation * Vec3::NEG_Z);
    Some(Runway {
        center,
        heading: if reverse {
            (heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        } else {
            heading
        },
        length: depth * scale.z,
        width: width * scale.x,
        right_circuit: text(world, zone, "circuit") == Some("right"),
        scale: scale_at(world, center),
    })
}

/// Tailwind (m/s) above which the tower turns a `direction=both` runway round.
const TAILWIND_LIMIT: f32 = 2.0;

/// The runway in use: one tagged `active=true`, else the first. Landing direction from its
/// `direction` tag: `forward` (local −Z) or `reverse` (+Z) are fixed; `both` (the default) lands
/// into the wind, turning round only when the current direction has more than 2 m/s of tailwind.
pub fn active_runway(
    world: &World,
    airport: &Airport,
    previous: Option<bool>,
) -> Option<(EntityId, Runway, bool)> {
    let chosen = airport
        .runways
        .iter()
        .copied()
        .find(|&id| {
            matches!(
                world
                    .component::<Metadata>(id)
                    .ok()
                    .flatten()
                    .and_then(|m| m.get("active")),
                Some(MetaValue::Bool(true))
            )
        })
        .or_else(|| airport.runways.first().copied())?;
    // Threshold boxes open the ends they sit at; without any, the `direction` tag decides.
    let drawn = drawn_runway(world, chosen, false)?;
    let (mut forward_end, mut reverse_end) = (false, false);
    for (end, _, _) in thresholds(world, &drawn) {
        if end {
            reverse_end = true;
        } else {
            forward_end = true;
        }
    }
    let fixed = match (forward_end, reverse_end) {
        (true, false) => Some(false),
        (false, true) => Some(true),
        (true, true) => None,
        (false, false) => match text(world, chosen, "direction") {
            Some("forward") => Some(false),
            Some("reverse") => Some(true),
            _ => None,
        },
    };
    let reverse = match fixed {
        Some(reverse) => reverse,
        None => {
            let wind = world
                .resource::<crate::Wind>()
                .map_or(Vec3::ZERO, |w| w.velocity);
            let forward = drawn;
            // Positive: blowing along the forward landing direction, i.e. a tailwind for it.
            let tailwind = wind.dot(bearing_direction(forward.heading));
            match previous {
                Some(false) if tailwind > TAILWIND_LIMIT => true,
                Some(true) if -tailwind > TAILWIND_LIMIT => false,
                Some(current) => current,
                None => tailwind > 0.0,
            }
        }
    };
    runway_geometry(world, chosen, reverse).map(|runway| (chosen, runway, reverse))
}

/// Where an aircraft is in its order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Arrival waiting over the holding fix (circuit full).
    Holding,
    /// Arrival flying the circuit to land.
    Circuit,
    /// Landed: taxiing off the runway to a stand.
    Vacating,
    /// Order finished on a stand (arrival) or where it stopped.
    Parked,
    /// Departure taxiing to the holding position.
    TaxiOut,
    /// Departure waiting for the runway.
    HoldShort,
    /// Departure entering the runway and lining up.
    LineUp,
    /// Departure rolling and climbing out.
    TakeOff,
    /// Departure clear of the runway; holding runway heading at its altitude.
    Departed,
    /// The airport, runway or route can't be used: the aircraft holds where it is.
    Unavailable,
}

/// What the tower tells one aircraft this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dispatch {
    /// The concrete goal the pilot flies.
    pub goal: FlightGoal,
    /// The runway for a landing goal (direction and circuit side decided by the tower).
    pub runway: Option<Runway>,
    pub gates: Gates,
    pub stage: Stage,
    /// Arrival sequence number (1 lands next), while in the sequence.
    pub number: Option<usize>,
}

struct Flight {
    order: FlightGoal,
    airport: usize,
    stage: Stage,
    route: Vec<Vec3>,
    next: usize,
    stand: Option<EntityId>,
    go_arounds: u32,
}

#[derive(Default)]
struct Traffic {
    /// Arrival sequence (first lands next).
    arrivals: Vec<EntityId>,
    /// Departure queue at the holding position.
    departures: Vec<EntityId>,
    /// Landing direction in use (true: the runway's reverse end), kept for hysteresis.
    reverse: Option<bool>,
}

/// One aircraft as seen this tick.
#[derive(Clone, Copy)]
struct Seen {
    id: EntityId,
    order: Option<FlightGoal>,
    position: Vec3,
    telemetry: FlightTelemetry,
}

/// Tower state: airports, per-aircraft progress and per-airport sequences. A world resource.
#[derive(Default, Resource)]
pub struct Tower {
    generation: Option<u64>,
    airports: Vec<Airport>,
    flights: HashMap<EntityId, Flight>,
    traffic: Vec<Traffic>,
    seen: Vec<Seen>,
    /// Ground network per airport, rebuilt with the airports.
    graphs: Vec<TaxiGraph>,
}

impl Tower {
    /// The airports the tower knows (rebuilt when the map's items change).
    pub fn airports(&self) -> &[Airport] {
        &self.airports
    }
    /// Arrival sequence at an airport, first to land first.
    pub fn arrivals(&self, airport: usize) -> &[EntityId] {
        self.traffic.get(airport).map_or(&[], |t| &t.arrivals)
    }
}

fn order_of(agent: &LocomotionAgent) -> Option<FlightGoal> {
    agent
        .flight
        .or(agent.intent.flight)
        .filter(|goal| matches!(goal, FlightGoal::Arrive { .. } | FlightGoal::Depart { .. }))
}

/// Runway frame: (along from the threshold, across from the centreline).
fn runway_frame(runway: &Runway, at: Vec3) -> (f32, f32) {
    let direction = bearing_direction(runway.heading);
    let threshold = runway.center - direction * (runway.length / 2.0);
    let relative = at - threshold;
    (
        relative.dot(direction),
        relative.dot(bearing_direction(
            runway.heading - std::f32::consts::FRAC_PI_2,
        )),
    )
}

/// On the runway or low over it (below 30 m), with a little margin.
fn occupies(runway: &Runway, seen: &Seen) -> bool {
    let (along, across) = runway_frame(runway, seen.position);
    let s = runway.scale;
    along > -20.0 * s
        && along < runway.length + 20.0 * s
        && across.abs() < runway.width / 2.0 + 15.0 * s
        && seen.position.y - runway.center.y < (30.0 * s).max(8.0)
}

/// One fixed tick of air traffic control, after brains and before locomotion.
pub(crate) fn step(world: &mut World) -> Result<(), EngineError> {
    let mut tower = match world.resource_mut::<Tower>() {
        Some(mut tower) => std::mem::take(&mut *tower),
        None => Tower::default(),
    };
    let result = tower.control(world);
    world.insert_resource(tower);
    result
}

impl Tower {
    fn control(&mut self, world: &mut World) -> Result<(), EngineError> {
        let generation = world
            .resource::<crate::map::MapGeneration>()
            .map_or(0, |g| g.0);
        if self.generation != Some(generation) {
            // Airport items changed: rebuild them and start sequencing afresh.
            self.generation = Some(generation);
            self.airports = airports(world);
            self.graphs = self
                .airports
                .iter()
                .map(|a| {
                    TaxiGraph::build(world, &a.taxiways, &a.stands, &a.holding_positions, a.scale)
                })
                .collect();
            self.flights.clear();
            self.traffic.clear();
            self.traffic
                .resize_with(self.airports.len(), Traffic::default);
        }

        // Every physical aircraft (orders or not: any of them can occupy a runway).
        self.seen.clear();
        let ids = world.take_locomotion_ids();
        for &id in &ids {
            let Some(agent) = world.component::<LocomotionAgent>(id)? else {
                continue;
            };
            if !matches!(agent.model, Model::FixedWingPhysics(_)) {
                continue;
            }
            let Some(at) = position(world, id) else {
                continue;
            };
            // No telemetry until the body's first physics step: wait a tick rather than guess.
            let Some(telemetry) = agent.flight_telemetry() else {
                continue;
            };
            self.seen.push(Seen {
                id,
                order: order_of(agent),
                position: at,
                telemetry,
            });
        }
        world.restore_locomotion_ids(ids);

        // Forget aircraft that are gone or whose order changed.
        let seen = &self.seen;
        self.flights.retain(|id, flight| {
            seen.iter()
                .any(|s| s.id == *id && s.order == Some(flight.order))
        });
        let flights = &self.flights;
        for traffic in &mut self.traffic {
            traffic.arrivals.retain(|id| flights.contains_key(id));
            traffic.departures.retain(|id| flights.contains_key(id));
        }

        for i in 0..self.seen.len() {
            let s = self.seen[i];
            let Some(order) = s.order else { continue };
            if !self.flights.contains_key(&s.id) {
                let target = match order {
                    FlightGoal::Arrive { airport } | FlightGoal::Depart { airport, .. } => airport,
                    _ => unreachable!("orders are arrivals and departures"),
                };
                let airport = self.airports.iter().position(|a| a.contains(target));
                let stage = match (airport, order) {
                    (None, _) => Stage::Unavailable,
                    (_, FlightGoal::Arrive { .. }) if s.telemetry.airborne => Stage::Holding,
                    (_, FlightGoal::Arrive { .. }) => Stage::Vacating,
                    (_, _) if s.telemetry.airborne => Stage::Departed,
                    _ => Stage::TaxiOut,
                };
                self.flights.insert(
                    s.id,
                    Flight {
                        order,
                        airport: airport.unwrap_or(0),
                        stage,
                        route: Vec::new(),
                        next: 0,
                        stand: None,
                        go_arounds: s.telemetry.go_arounds,
                    },
                );
            }
        }

        for airport in 0..self.airports.len() {
            self.control_airport(world, airport)?;
        }
        Ok(())
    }

    fn control_airport(&mut self, world: &mut World, index: usize) -> Result<(), EngineError> {
        let active = active_runway(world, &self.airports[index], self.traffic[index].reverse);
        if let Some((_, _, reverse)) = active {
            self.traffic[index].reverse = Some(reverse);
        }
        let towered = self.airports[index].towered;
        let pattern_height = self.airports[index].pattern_height;
        let telemetry = |seen: &[Seen], id: EntityId| seen.iter().find(|s| s.id == id).copied();

        // Arrivals: keep the sequence, send go-arounds to the back.
        // In engine order (not hash order), so runs replay identically.
        let flights = &self.flights;
        let ids: Vec<EntityId> = self
            .seen
            .iter()
            .map(|s| s.id)
            .filter(|id| flights.get(id).is_some_and(|f| f.airport == index))
            .collect();
        for &id in &ids {
            let Some(s) = telemetry(&self.seen, id) else {
                continue;
            };
            let flight = self.flights.get_mut(&id).expect("listed above");
            if active.is_none() && flight.stage != Stage::Parked {
                flight.stage = Stage::Unavailable;
            }
            let traffic = &mut self.traffic[index];
            match flight.stage {
                Stage::Holding | Stage::Circuit => {
                    if s.telemetry.go_arounds > flight.go_arounds {
                        flight.go_arounds = s.telemetry.go_arounds;
                        traffic.arrivals.retain(|&other| other != id);
                    }
                    if !traffic.arrivals.contains(&id) {
                        traffic.arrivals.push(id);
                    }
                    if s.telemetry.landed {
                        flight.stage = Stage::Vacating;
                        traffic.arrivals.retain(|&other| other != id);
                    }
                }
                _ => traffic.arrivals.retain(|&other| other != id),
            }
        }

        let Some((zone, runway, _)) = active else {
            for &id in &ids {
                write(
                    world,
                    id,
                    Dispatch {
                        goal: FlightGoal::Park,
                        runway: None,
                        gates: Gates::default(),
                        stage: Stage::Unavailable,
                        number: None,
                    },
                )?;
            }
            return Ok(());
        };
        let elevation = runway.center.y;
        let direction = bearing_direction(runway.heading);
        let threshold = runway.center - direction * (runway.length / 2.0);
        let runway_busy = |seen: &[Seen], except: EntityId| {
            seen.iter().any(|s| s.id != except && occupies(&runway, s))
        };
        let fix = self.airports[index]
            .holding_fixes
            .first()
            .and_then(|&id| position(world, id))
            .unwrap_or(threshold - direction * HOLDING_FIX_DISTANCE * runway.scale);

        // Arrival gates, in sequence order.
        let arrivals = self.traffic[index].arrivals.clone();
        for (number, &id) in arrivals.iter().enumerate() {
            let Some(s) = telemetry(&self.seen, id) else {
                continue;
            };
            let leader = number
                .checked_sub(1)
                .and_then(|n| telemetry(&self.seen, arrivals[n]));
            let flight = self.flights.get_mut(&id).expect("in sequence");
            let dispatch = if number < CIRCUIT_CAPACITY || !towered {
                flight.stage = Stage::Circuit;
                // "Turn base when the traffic ahead is far enough down final": compare remaining paths.
                let (along, across) = runway_frame(&runway, s.position);
                let my_path = (-along).max(0.0) + across.abs();
                let leader_close = leader.is_none_or(|l| {
                    let (l_along, l_across) = runway_frame(&runway, l.position);
                    let remaining = match l.telemetry.phase {
                        FlightPhase::Base => (-l_along).max(0.0) + l_across.abs(),
                        FlightPhase::Final | FlightPhase::Flare => (-l_along).max(0.0),
                        FlightPhase::Rollout | FlightPhase::Parked | FlightPhase::Taxi => 0.0,
                        _ => return false,
                    };
                    my_path - remaining >= SEPARATION * runway.scale
                });
                let gates = if towered {
                    Gates {
                        may_turn_base: leader_close,
                        cleared_to_land: !runway_busy(&self.seen, id),
                        slow: leader.is_some_and(|l| {
                            l.telemetry.airborne
                                && l.position.distance(s.position) < SLOW_DISTANCE * runway.scale
                        }) && matches!(
                            s.telemetry.phase,
                            FlightPhase::PatternEntry | FlightPhase::Downwind
                        ),
                    }
                } else {
                    Gates::default()
                };
                Dispatch {
                    goal: FlightGoal::Land {
                        zone,
                        pattern_height,
                    },
                    runway: Some(runway),
                    gates,
                    stage: Stage::Circuit,
                    number: Some(number + 1),
                }
            } else {
                flight.stage = Stage::Holding;
                let level = (number + 1 - CIRCUIT_CAPACITY) as f32;
                Dispatch {
                    goal: FlightGoal::FlyTo {
                        point: fix,
                        altitude: elevation + pattern_height + STACK_SPACING * runway.scale * level,
                        speed: 0.0,
                    },
                    runway: None,
                    gates: Gates::default(),
                    stage: Stage::Holding,
                    number: Some(number + 1),
                }
            };
            write(world, id, dispatch)?;
        }

        // Ground movements and departures.
        let arrival_near = self.seen.iter().any(|s| {
            arrivals.contains(&s.id)
                && matches!(
                    s.telemetry.phase,
                    FlightPhase::Base
                        | FlightPhase::Final
                        | FlightPhase::Flare
                        | FlightPhase::Rollout
                )
                && runway_frame(&runway, s.position).0 > -DEPARTURE_GAP * runway.scale
        });
        for &id in &ids {
            let Some(s) = telemetry(&self.seen, id) else {
                continue;
            };
            let stage = self.flights[&id].stage;
            let dispatch = match stage {
                Stage::Holding | Stage::Circuit => continue,
                Stage::Unavailable | Stage::Parked => Dispatch {
                    goal: FlightGoal::Park,
                    runway: None,
                    gates: Gates::default(),
                    stage,
                    number: None,
                },
                Stage::Vacating | Stage::TaxiOut => {
                    if self.flights[&id].route.is_empty() {
                        let route = if stage == Stage::Vacating {
                            self.route_to_stand(index, id, s.position, &runway)
                        } else {
                            self.route_to_runway(index, s.position, threshold)
                        };
                        let flight = self.flights.get_mut(&id).expect("listed");
                        flight.route = route;
                        flight.next = 0;
                        if flight.route.is_empty() {
                            flight.stage = if stage == Stage::Vacating {
                                Stage::Parked
                            } else {
                                Stage::Unavailable
                            };
                            continue;
                        }
                    }
                    let flight = self.flights.get_mut(&id).expect("listed");
                    let last = flight.route.len() - 1;
                    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
                    while flight.next < last
                        && flat(flight.route[flight.next] - s.position).length()
                            < (ROUTE_REACH * runway.scale).max(3.0)
                    {
                        flight.next += 1;
                    }
                    let point = flight.route[flight.next];
                    if flight.next == last
                        && flat(point - s.position).length() < (8.0 * runway.scale).max(5.0)
                        && s.telemetry.phase == FlightPhase::Parked
                    {
                        flight.stage = if stage == Stage::Vacating {
                            Stage::Parked
                        } else {
                            Stage::HoldShort
                        };
                        if flight.stage == Stage::HoldShort {
                            self.traffic[index].departures.push(id);
                        }
                    }
                    Dispatch {
                        goal: FlightGoal::Taxi {
                            point,
                            stop: flight.next == last,
                        },
                        runway: None,
                        gates: Gates::default(),
                        stage,
                        number: None,
                    }
                }
                Stage::HoldShort => {
                    let first = self.traffic[index].departures.first() == Some(&id);
                    let clear = !towered || (!runway_busy(&self.seen, id) && !arrival_near);
                    if first && clear {
                        let flight = self.flights.get_mut(&id).expect("listed");
                        flight.stage = Stage::LineUp;
                        flight.route = vec![
                            threshold + direction * 15.0 * runway.scale,
                            threshold + direction * 120.0 * runway.scale,
                        ];
                        flight.next = 0;
                    }
                    Dispatch {
                        goal: FlightGoal::Park,
                        runway: None,
                        gates: Gates::default(),
                        stage: Stage::HoldShort,
                        number: None,
                    }
                }
                Stage::LineUp => {
                    let (along, _) = runway_frame(&runway, s.position);
                    let aligned = ((s.telemetry.heading.to_radians() - runway.heading
                        + std::f32::consts::PI)
                        .rem_euclid(std::f32::consts::TAU)
                        - std::f32::consts::PI)
                        .abs()
                        < 15f32.to_radians();
                    let flight = self.flights.get_mut(&id).expect("listed");
                    if along > 60.0 * runway.scale && aligned {
                        flight.stage = Stage::TakeOff;
                    } else if along > 10.0 * runway.scale {
                        flight.next = 1;
                    }
                    Dispatch {
                        goal: FlightGoal::Taxi {
                            point: flight.route[flight.next],
                            stop: false,
                        },
                        runway: None,
                        gates: Gates::default(),
                        stage: Stage::LineUp,
                        number: None,
                    }
                }
                Stage::TakeOff | Stage::Departed => {
                    let FlightGoal::Depart { altitude, .. } = self.flights[&id].order else {
                        continue;
                    };
                    let (along, _) = runway_frame(&runway, s.position);
                    if stage == Stage::TakeOff
                        && s.telemetry.airborne
                        && (along > runway.length || s.position.y > elevation + 50.0 * runway.scale)
                    {
                        let flight = self.flights.get_mut(&id).expect("listed");
                        flight.stage = Stage::Departed;
                        self.traffic[index].departures.retain(|&other| other != id);
                    }
                    Dispatch {
                        goal: FlightGoal::TakeOffFrom { runway, altitude },
                        runway: None,
                        gates: Gates::default(),
                        stage: self.flights[&id].stage,
                        number: None,
                    }
                }
            };
            write(world, id, dispatch)?;
        }
        Ok(())
    }

    /// Off the runway at the nearest taxiway point, then the taxiways to the nearest free stand.
    fn route_to_stand(
        &mut self,
        index: usize,
        id: EntityId,
        at: Vec3,
        runway: &Runway,
    ) -> Vec<Vec3> {
        let off_runway = |p: Vec3| {
            let (along, across) = runway_frame(runway, p);
            let s = runway.scale;
            !(along > -20.0 * s
                && along < runway.length + 20.0 * s
                && across.abs() < runway.width / 2.0 + 5.0 * s)
        };
        let graph = &self.graphs[index];
        let Some(exit) = graph.nearest(at, |_, p, kind| kind == NodeKind::Taxi && off_runway(p))
        else {
            return Vec::new();
        };
        let exit_at = graph.nodes[exit as usize].0;
        let taken: Vec<EntityId> = self
            .flights
            .iter()
            .filter(|(&other, _)| other != id)
            .filter_map(|(_, f)| f.stand)
            .collect();
        let stand = self.airports[index]
            .stands
            .iter()
            .filter(|item| !taken.contains(item))
            .filter_map(|&item| graph.anchors.get(&item).map(|&node| (item, node)))
            .min_by(|a, b| {
                let d = |node: u32| graph.nodes[node as usize].0.distance_squared(exit_at);
                d(a.1).total_cmp(&d(b.1))
            });
        if let Some(flight) = self.flights.get_mut(&id) {
            flight.stand = stand.map(|(item, _)| item);
        }
        let Some((_, stand)) = stand else {
            return vec![exit_at];
        };
        self.route(index, exit, stand)
    }

    /// From the nearest network point to the holding position nearest the threshold.
    fn route_to_runway(&mut self, index: usize, at: Vec3, threshold: Vec3) -> Vec<Vec3> {
        let graph = &self.graphs[index];
        let goal_kind = if self.airports[index].holding_positions.is_empty() {
            NodeKind::Taxi
        } else {
            NodeKind::Holding
        };
        let (Some(start), Some(goal)) = (
            graph.nearest(at, |_, _, _| true),
            graph.nearest(threshold, |_, _, kind| kind == goal_kind),
        ) else {
            return Vec::new();
        };
        self.route(index, start, goal)
    }

    /// Positions along the cheapest network route (or straight there if unconnected).
    fn route(&mut self, index: usize, from: u32, to: u32) -> Vec<Vec3> {
        let graph = &mut self.graphs[index];
        let mut points = Vec::new();
        if !graph.route(from, to, &mut points) {
            points = vec![graph.nodes[from as usize].0, graph.nodes[to as usize].0];
        }
        points
    }
}

fn write(world: &mut World, id: EntityId, dispatch: Dispatch) -> Result<(), EngineError> {
    if let Some(mut agent) = world.component_mut::<LocomotionAgent>(id)? {
        agent.dispatch = Some(dispatch);
    }
    Ok(())
}

/// One runway as drawn on a map: its active landing direction and, when in use, the circuit.
#[derive(Clone, Debug, PartialEq)]
pub struct RunwayView {
    pub zone: EntityId,
    pub runway: Runway,
    pub active: bool,
    /// Present for the runway in use.
    pub circuit: Option<crate::fixed_wing::CircuitOutline>,
}

/// An airport as drawn on a map.
#[derive(Clone, Debug, PartialEq)]
pub struct AirportView {
    pub aerodrome: Option<EntityId>,
    pub towered: bool,
    pub runways: Vec<RunwayView>,
    pub holding_fix: Option<Vec3>,
}

/// Every airport with its runways, the direction in use (from tags and the current wind) and the
/// circuit an aircraft with `params` would fly. Uses the same geometry as the pilot and tower.
pub fn airport_views(
    world: &World,
    params: &crate::fixed_wing::FixedWingParams,
) -> Vec<AirportView> {
    let patterns = drawn_patterns(world);
    airports(world)
        .iter()
        .map(|airport| {
            let active = active_runway(world, airport, None);
            let mut holding_fix = None;
            let runways = airport
                .runways
                .iter()
                .filter_map(|&zone| {
                    let (runway, in_use) = match active {
                        Some((id, runway, _)) if id == zone => (runway, true),
                        _ => (runway_geometry(world, zone, false)?, false),
                    };
                    // A drawn pattern replaces the generated circuit; the map shows its waypoints.
                    let drawn = patterns.iter().any(|(z, h, _)| {
                        *z == zone
                            && bearing_direction(*h).dot(bearing_direction(runway.heading)) > 0.99
                    });
                    let circuit = (in_use && !drawn).then(|| {
                        crate::fixed_wing::circuit_outline(params, &runway, airport.pattern_height)
                    });
                    if in_use {
                        let direction = bearing_direction(runway.heading);
                        let threshold = runway.center - direction * (runway.length / 2.0);
                        holding_fix = Some(
                            airport
                                .holding_fixes
                                .first()
                                .and_then(|&id| position(world, id))
                                .unwrap_or(
                                    threshold - direction * HOLDING_FIX_DISTANCE * runway.scale,
                                ),
                        );
                    }
                    Some(RunwayView {
                        zone,
                        runway,
                        active: in_use,
                        circuit,
                    })
                })
                .collect();
            AirportView {
                aerodrome: airport.aerodrome,
                towered: airport.towered,
                runways,
                holding_fix,
            }
        })
        .collect()
}

/// Drawn traffic patterns (`aeroway=circuit` waypoint paths), each attached to the runway end of
/// the threshold box nearest its last fix. Rebuilt when the map changes; shared with aircraft
/// without copying.
#[derive(Default, Resource)]
pub(crate) struct Patterns {
    generation: Option<u64>,
    /// (runway zone, landing heading, fixes).
    list: Vec<(EntityId, f32, Arc<[PatternFix]>)>,
}

/// The drawn pattern for landing on `zone` along `heading` (compass radians), if there is one.
pub(crate) fn pattern_for(
    world: &mut World,
    zone: EntityId,
    heading: f32,
) -> Option<Arc<[PatternFix]>> {
    let generation = world
        .resource::<crate::map::MapGeneration>()
        .map_or(0, |g| g.0);
    let stale = world
        .resource::<Patterns>()
        .is_none_or(|p| p.generation != Some(generation));
    if stale {
        let list = drawn_patterns(world);
        world.insert_resource(Patterns {
            generation: Some(generation),
            list,
        });
    }
    let patterns = world.resource::<Patterns>()?;
    patterns
        .list
        .iter()
        .find(|(z, h, _)| {
            *z == zone
                && crate::fixed_wing::bearing_direction(*h).dot(bearing_direction(heading)) > 0.99
        })
        .map(|(_, _, fixes)| fixes.clone())
}

/// Leg names on pattern waypoints (`leg` tag).
fn leg(world: &World, id: EntityId) -> Option<FlightPhase> {
    match text(world, id, "leg")? {
        "entry" => Some(FlightPhase::PatternEntry),
        "downwind" => Some(FlightPhase::Downwind),
        "base" => Some(FlightPhase::Base),
        "final" => Some(FlightPhase::Final),
        _ => None,
    }
}

/// Every drawn pattern, followed along its links from the point nothing links to.
pub fn drawn_patterns(world: &World) -> Vec<(EntityId, f32, Arc<[PatternFix]>)> {
    let value = MetaValue::Text("circuit".into());
    let points: Vec<EntityId> = world
        .items_where("aeroway", &value)
        .filter(|&id| matches!(world.component::<crate::Waypoint>(id), Ok(Some(_))))
        .collect();
    let next = |id: EntityId| -> Option<EntityId> {
        world
            .component::<crate::Waypoint>(id)
            .ok()??
            .links
            .iter()
            .copied()
            .find(|link| points.contains(link))
    };
    let runway_value = MetaValue::Text("runway".into());
    let runways: Vec<EntityId> = world.items_where("aeroway", &runway_value).collect();
    let airports = airports(world);
    let mut list = Vec::new();
    for &start in &points {
        if points.iter().any(|&other| next(other) == Some(start)) {
            continue;
        }
        let mut chain = vec![start];
        while let Some(following) = next(*chain.last().expect("non-empty")) {
            if chain.contains(&following) || chain.len() >= 64 {
                break;
            }
            chain.push(following);
        }
        let Some(last) = position(world, *chain.last().expect("non-empty")) else {
            continue;
        };
        // The runway end whose threshold box is nearest the last fix.
        let attached = runways
            .iter()
            .filter_map(|&zone| {
                let forward = drawn_runway(world, zone, false)?;
                thresholds(world, &forward)
                    .filter_map(|(end, _, id)| {
                        position(world, id).map(|at| (zone, end, at.distance_squared(last)))
                    })
                    .min_by(|a, b| a.2.total_cmp(&b.2))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2));
        let Some((zone, end, _)) = attached else {
            continue;
        };
        let Some(runway) = runway_geometry(world, zone, end) else {
            continue;
        };
        let pattern_height = airports
            .iter()
            .find(|a| a.runways.contains(&zone))
            .map_or(DEFAULT_PATTERN_HEIGHT, |a| a.pattern_height);
        let (mut altitude, mut current) =
            (runway.center.y + pattern_height, FlightPhase::PatternEntry);
        let fixes: Vec<PatternFix> = chain
            .iter()
            .filter_map(|&id| {
                let waypoint = world.component::<crate::Waypoint>(id).ok()??;
                if let Some(height) = waypoint.altitude {
                    altitude = runway.center.y + height;
                }
                current = leg(world, id).unwrap_or(current);
                let at = position(world, id)?;
                Some(PatternFix {
                    point: Vec3::new(at.x, altitude, at.z),
                    entry: waypoint.entry,
                    exit: waypoint.exit,
                    leg: current,
                })
            })
            .collect();
        list.push((zone, runway.heading, fixes.into()));
    }
    list
}
