//! Airports described by scene-map tags (`aeroway=*`) and the control tower that turns
//! `Arrive`/`Depart` orders into the concrete goals and gates each aircraft's pilot flies.
//! See `docs/design/airport-operations.md`.
// The tower only runs with the physics feature (physical aircraft); the model is always built.
#![cfg_attr(not(feature = "physics"), allow(dead_code))]
use std::collections::HashMap;

use bevy_ecs::prelude::Resource;

use crate::fixed_wing::{
    bearing_direction, FlightGoal, FlightPhase, FlightTelemetry, Gates, Runway,
};
use crate::locomotion::{LocomotionAgent, Model};
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
            ..Airport::default()
        }]
    } else {
        aerodromes
            .iter()
            .map(|&id| Airport {
                aerodrome: Some(id),
                pattern_height: number(world, id, "pattern_height")
                    .unwrap_or(DEFAULT_PATTERN_HEIGHT),
                ..Airport::default()
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
/// circuit side from its `circuit=right` tag. None if it is gone or not a rectangle.
pub fn runway_geometry(world: &World, zone: EntityId, reverse: bool) -> Option<Runway> {
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
    let reverse = match text(world, chosen, "direction") {
        Some("forward") => false,
        Some("reverse") => true,
        _ => {
            let wind = world
                .resource::<crate::Wind>()
                .map_or(Vec3::ZERO, |w| w.velocity);
            let forward = runway_geometry(world, chosen, false)?;
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
    path: Vec<EntityId>,
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
    along > -20.0
        && along < runway.length + 20.0
        && across.abs() < runway.width / 2.0 + 15.0
        && seen.position.y - runway.center.y < 30.0
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
            .unwrap_or(threshold - direction * HOLDING_FIX_DISTANCE);

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
                    my_path - remaining >= SEPARATION
                });
                let gates = if towered {
                    Gates {
                        may_turn_base: leader_close,
                        cleared_to_land: !runway_busy(&self.seen, id),
                        slow: leader.is_some_and(|l| {
                            l.telemetry.airborne && l.position.distance(s.position) < SLOW_DISTANCE
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
                        altitude: elevation + pattern_height + STACK_SPACING * level,
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
                && runway_frame(&runway, s.position).0 > -DEPARTURE_GAP
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
                            self.route_to_stand(world, index, id, s.position, &runway)
                        } else {
                            self.route_to_runway(world, index, s.position, threshold)
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
                        && flat(flight.route[flight.next] - s.position).length() < ROUTE_REACH
                    {
                        flight.next += 1;
                    }
                    let point = flight.route[flight.next];
                    if flight.next == last
                        && flat(point - s.position).length() < 8.0
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
                        flight.route =
                            vec![threshold + direction * 15.0, threshold + direction * 120.0];
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
                    if along > 60.0 && aligned {
                        flight.stage = Stage::TakeOff;
                    } else if along > 10.0 {
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
                        && (along > runway.length || s.position.y > elevation + 50.0)
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
        world: &mut World,
        index: usize,
        id: EntityId,
        at: Vec3,
        runway: &Runway,
    ) -> Vec<Vec3> {
        let airport = &self.airports[index];
        let off_runway = |p: Vec3| {
            let (along, across) = runway_frame(runway, p);
            !(along > -20.0
                && along < runway.length + 20.0
                && across.abs() < runway.width / 2.0 + 5.0)
        };
        let nearest = |items: &[EntityId], to: Vec3, filter: &dyn Fn(EntityId, Vec3) -> bool| {
            items
                .iter()
                .filter_map(|&item| {
                    position(world, item)
                        .filter(|&p| filter(item, p))
                        .map(|p| (item, p))
                })
                .min_by(|a, b| {
                    a.1.distance_squared(to)
                        .total_cmp(&b.1.distance_squared(to))
                })
        };
        let Some((exit, exit_at)) = nearest(&airport.taxiways, at, &|_, p| off_runway(p)) else {
            return Vec::new();
        };
        let taken: Vec<EntityId> = self
            .flights
            .iter()
            .filter(|(&other, _)| other != id)
            .filter_map(|(_, f)| f.stand)
            .collect();
        let stand = nearest(&airport.stands, exit_at, &|item, _| !taken.contains(&item));
        if let Some(flight) = self.flights.get_mut(&id) {
            flight.stand = stand.map(|(item, _)| item);
        }
        let Some((stand, _)) = stand else {
            return vec![exit_at];
        };
        self.route(world, exit, stand)
    }

    /// From the nearest network point to the holding position nearest the threshold.
    fn route_to_runway(
        &mut self,
        world: &mut World,
        index: usize,
        at: Vec3,
        threshold: Vec3,
    ) -> Vec<Vec3> {
        let airport = &self.airports[index];
        let network: Vec<EntityId> = airport
            .taxiways
            .iter()
            .chain(&airport.stands)
            .chain(&airport.holding_positions)
            .copied()
            .collect();
        let closest = |items: &[EntityId], to: Vec3| {
            items
                .iter()
                .filter_map(|&i| position(world, i).map(|p| (i, p)))
                .min_by(|a, b| {
                    a.1.distance_squared(to)
                        .total_cmp(&b.1.distance_squared(to))
                })
        };
        let (Some((start, _)), Some((goal, _))) = (
            closest(&network, at),
            closest(
                if airport.holding_positions.is_empty() {
                    &airport.taxiways
                } else {
                    &airport.holding_positions
                },
                threshold,
            ),
        ) else {
            return Vec::new();
        };
        self.route(world, start, goal)
    }

    /// Waypoint positions along the cheapest network route (or straight there if unlinked).
    fn route(&mut self, world: &mut World, from: EntityId, to: EntityId) -> Vec<Vec3> {
        let mut path = std::mem::take(&mut self.path);
        let points = match world.waypoint_path(from, to, None, &mut path) {
            Ok(cost) if cost.is_finite() => {
                path.iter().filter_map(|&w| position(world, w)).collect()
            }
            _ => [from, to]
                .iter()
                .filter_map(|&w| position(world, w))
                .collect(),
        };
        self.path = path;
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
                    let circuit = in_use.then(|| {
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
                                .unwrap_or(threshold - direction * HOLDING_FIX_DISTANCE),
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
