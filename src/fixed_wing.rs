//! Physical fixed-wing aircraft: aerodynamic plant, wheel/ground model and autopilot.
//!
//! Pure maths over the committed body state: [`Pilot::step`] returns a world force and torque
//! that the locomotion tick queues for the next physics step. Nothing here writes a pose.
//! Conventions, phases and stages: `docs/design/fixed-wing-physics.md`.
use crate::control::{Pid, PidConfig, PidInput};
use crate::{EntityId, Quat, Transform, Vec3};
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::time::Duration;

const G: f32 = 9.81;
/// Cambered wing: lift is zero a little below a level attitude.
const ZERO_LIFT_ANGLE: f32 = -3.0 * PI / 180.0;
/// Induced drag at lift-off speed, as a fraction of weight (sets the "slow flight costs power" curve).
const INDUCED_AT_LIFTOFF: f32 = 0.08;
const ROLLING_RESISTANCE: f32 = 0.02;
const MAX_BRAKING: f32 = 0.5;
/// Autopilot turns when max bank is 0 (aerobatic, unlimited for manual flight).
const AUTOPILOT_BANK: f32 = 60.0 * PI / 180.0;
/// Bank used for circuit turns (pattern legs and the turn to final).
const CIRCUIT_BANK: f32 = 25.0 * PI / 180.0;
/// Turn rate (rad/s) commanded per radian of heading error.
const HEADING_GAIN: f32 = 0.5;
/// Pitch attitude limit for non-aerobatic aircraft (max bank > 0) in manual flight.
const NORMAL_PITCH_LIMIT: f32 = 30.0 * PI / 180.0;
/// Throttle change per second.
const SPOOL_RATE: f32 = 1.0;
/// Seconds for a control surface to bring a body rate to its command (before authority limits).
const RATE_RESPONSE: f32 = 0.15;
/// Seconds of continuous airborne flight before the climb-out counts as "taken off".
const LIFTOFF_DWELL: f32 = 0.5;
/// Height above the take-off point before the autopilot turns towards its goal.
const SAFE_TURN_HEIGHT: f32 = 50.0;
/// Rotation starts a little before lift-off speed (Vr), so the wing reaches its rotation angle near V_lo.
const ROTATE_FRACTION: f32 = 0.9;
/// Final approach glide path.
const GLIDE_SLOPE: f32 = 3.0 * PI / 180.0;
/// Approach speed as a multiple of lift-off speed.
const APPROACH_FACTOR: f32 = 1.3;
/// Wheel height above the runway at which the flare starts, metres.
const FLARE_HEIGHT: f32 = 6.0;
const TAXI_SPEED: f32 = 8.0;

/// Tuning from a `fixed_wing_physics` profile (runtime units: m, s, rad).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FixedWingParams {
    /// Airspeed over the wing at which lift equals weight at the rotation angle.
    pub liftoff_speed: f32,
    pub cruise_speed: f32,
    /// Level-flight top speed at full throttle (sets parasitic drag).
    pub max_speed: f32,
    /// Full-throttle thrust as a fraction of weight.
    pub thrust_to_weight: f32,
    /// Nose-up angle held to lift off (and the angle of attack where lift = weight at lift-off speed).
    pub rotate_pitch: f32,
    pub stall_angle: f32,
    /// 0 means unlimited: manual flight can roll and loop (aerobatics).
    pub max_bank: f32,
    pub roll_rate: f32,
    pub pitch_rate: f32,
    pub yaw_rate: f32,
    pub max_climb: f32,
    pub max_descent: f32,
}

impl FixedWingParams {
    /// Parasitic and induced drag constants per unit weight: D/W = kp·V² + ki·(L/W)²/V².
    fn drag(&self) -> (f32, f32) {
        let induced = INDUCED_AT_LIFTOFF * self.liftoff_speed * self.liftoff_speed;
        let induced_at_max = induced / (self.max_speed * self.max_speed);
        let parasitic =
            ((self.thrust_to_weight - induced_at_max) / (self.max_speed * self.max_speed)).max(0.0);
        (parasitic, induced)
    }

    /// Drag ÷ weight in steady flight at `speed` with load factor `load`.
    fn drag_ratio(&self, speed: f32, load: f32) -> f32 {
        let (kp, ki) = self.drag();
        kp * speed * speed + ki * load * load / (speed * speed).max(1.0)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(self.liftoff_speed < self.cruise_speed && self.cruise_speed <= self.max_speed) {
            return Err(
                "physical fixed wing needs lift-off speed < cruise speed ≤ max speed".into(),
            );
        }
        if self.rotate_pitch >= self.stall_angle {
            return Err("rotation pitch must be below the stall angle".into());
        }
        let needed = INDUCED_AT_LIFTOFF * (self.liftoff_speed / self.max_speed).powi(2);
        if self.thrust_to_weight <= needed {
            return Err(format!(
                "thrust-to-weight must exceed {needed:.3} to reach max speed at all"
            ));
        }
        Ok(())
    }

    /// Lift as a multiple of weight·(V/V_lo)²: 1 at the rotation angle, linear to stall, 60% past it.
    fn lift_coefficient(&self, alpha: f32) -> (f32, bool) {
        let linear = |a: f32| (a - ZERO_LIFT_ANGLE) / (self.rotate_pitch - ZERO_LIFT_ANGLE);
        if alpha > self.stall_angle {
            (0.6 * linear(self.stall_angle), true)
        } else if alpha < -self.stall_angle {
            (0.6 * linear(-self.stall_angle), true)
        } else {
            (linear(alpha), false)
        }
    }

    /// Angle of attack that makes `load` × weight of lift at speed `v`.
    fn alpha_for(&self, load: f32, v: f32) -> f32 {
        let q = (v / self.liftoff_speed).powi(2).max(0.05);
        (ZERO_LIFT_ANGLE + load / q * (self.rotate_pitch - ZERO_LIFT_ANGLE))
            .min(self.stall_angle * 0.85)
    }

    fn approach_speed(&self) -> f32 {
        APPROACH_FACTOR * self.liftoff_speed
    }

    /// Nose-up attitude held on final: the angle of attack for approach speed, less the glide angle.
    fn approach_pitch(&self) -> f32 {
        self.alpha_for(1.0, self.approach_speed()) - GLIDE_SLOPE
    }

    fn bank_limit(&self) -> f32 {
        if self.max_bank > 0.0 {
            self.max_bank
        } else {
            AUTOPILOT_BANK
        }
    }
}

/// What the aircraft is asked to do. Persistent until replaced; repeating the same goal keeps
/// controller history.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlightGoal {
    /// On the ground: idle, brakes on. In the air there is nowhere to park, so it holds.
    Park,
    /// Roll along the current heading, rotate at lift-off speed, climb to `altitude` (world Y), then hold.
    TakeOff { altitude: f32 },
    /// Hold a compass heading (degrees), altitude (world Y) and airspeed. Takes off first if on the ground.
    Hold {
        heading: f32,
        altitude: f32,
        speed: f32,
    },
    /// Fly to a point at `altitude` and circle it. Takes off first if on the ground.
    FlyTo {
        point: Vec3,
        altitude: f32,
        speed: f32,
    },
    /// Land on a rectangular map zone: take off first if needed, join a left-hand circuit at
    /// `pattern_height` above the runway, fly downwind, base and final, flare, touch down and stop.
    Land { zone: EntityId, pattern_height: f32 },
    /// Drive on the ground to a point at taxi speed; `stop` brakes to a halt there, otherwise it
    /// keeps rolling (a route point the caller replaces with the next).
    Taxi { point: Vec3, stop: bool },
    /// Take off along a runway's centreline (from wherever on it the aircraft is lined up) and
    /// climb to `altitude`, then hold the runway heading.
    TakeOffFrom { runway: Runway, altitude: f32 },
    /// Direct control.
    Manual(Sticks),
    /// Order for the control tower: land at this airport (its aerodrome zone, or any of its items),
    /// vacate and park on a stand. The tower turns it into the goals above each tick.
    Arrive { airport: EntityId },
    /// Order for the control tower: taxi to the holding position, wait for the runway, take off and
    /// climb to `altitude`, then hold the runway heading.
    Depart { airport: EntityId, altitude: f32 },
}

/// Pilot inputs: roll/pitch/yaw −1..1 as a fraction of the profile's rates, throttle and brake 0..1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sticks {
    pub roll: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub throttle: f32,
    pub brake: f32,
}

/// A landing zone in world space: a rectangle landed along `heading`, its threshold at the near end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Runway {
    pub center: Vec3,
    /// Compass radians of the landing direction (the zone's local −Z).
    pub heading: f32,
    pub length: f32,
    pub width: f32,
    /// Fly the circuit on the right of the landing direction (default left).
    pub right_circuit: bool,
    /// The airport's size relative to a real one (its aerodrome's `scale` tag, default 1):
    /// the circuit's minimum spacing and the approach gates shrink with it.
    pub scale: f32,
}

/// One fix of a drawn traffic pattern: cross `point` (its `y` is the world altitude) on a course
/// within `entry`, leave on one within `exit`. Courses are compass radians, clockwise from the
/// first to the second; `None` allows any. `leg` is the circuit phase flown towards the fix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PatternFix {
    pub point: Vec3,
    pub entry: Option<(f32, f32)>,
    pub exit: Option<(f32, f32)>,
    pub leg: FlightPhase,
}

/// Whether a compass course lies in a clockwise range.
fn in_range(course: f32, (from, to): (f32, f32)) -> bool {
    (course - from).rem_euclid(TAU) <= (to - from).rem_euclid(TAU) + 1e-4
}
/// The course in a range closest to `course`.
fn clamp_course(course: f32, range: (f32, f32)) -> f32 {
    if in_range(course, range) {
        course
    } else if wrap(course - range.0).abs() <= wrap(course - range.1).abs() {
        range.0
    } else {
        range.1
    }
}

/// Progress through a drawn pattern.
#[derive(Clone, Copy, Debug, Default)]
struct PatternNav {
    /// Next fix to cross.
    fix: usize,
    /// Committed course to cross it on (chosen on starting it).
    course: Option<f32>,
    /// Flying out to line up again before the fix (approached too close or off course).
    lead: bool,
    /// Loops flown to line up on this fix; after two it is flown straight at.
    tries: u8,
    /// Leaving the last fix on its exit course, from where.
    leaving: Option<(Vec3, f32, f32)>,
    /// This tick's heading and altitude.
    command: (f32, f32),
}

/// What the tower allows this tick (all open for a plain `Land` with no tower).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gates {
    /// Leave downwind for base; until then downwind is extended.
    pub may_turn_base: bool,
    /// Land; without it the aircraft goes around 1 km from the threshold.
    pub cleared_to_land: bool,
    /// Fly the circuit at approach speed to keep spacing behind the aircraft ahead.
    pub slow: bool,
}

impl Default for Gates {
    fn default() -> Self {
        Self {
            may_turn_base: true,
            cleared_to_land: true,
            slow: false,
        }
    }
}

/// Flight phases. The numeric order is part of the preview trace format (`as u8`); append new ones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum FlightPhase {
    #[default]
    Parked,
    Taxi,
    TakeoffRoll,
    Rotate,
    /// Taken off: climbing out.
    Climb,
    Cruise,
    PatternEntry,
    Downwind,
    Base,
    Final,
    Flare,
    Rollout,
    GoAround,
    Manual,
}

/// What the aircraft is doing, for game code, brains, the editor and tests.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlightTelemetry {
    pub phase: FlightPhase,
    pub airborne: bool,
    /// Speed over the wing: velocity along the aircraft's −Z, m/s.
    pub airspeed: f32,
    pub ground_speed: f32,
    /// Compass degrees: −Z is 0, +X 90, +Z 180, −X 270.
    pub heading: f32,
    pub altitude: f32,
    pub climb_rate: f32,
    /// Degrees, nose up positive.
    pub pitch: f32,
    /// Degrees, right wing down positive.
    pub bank: f32,
    /// Angle of attack, degrees.
    pub alpha: f32,
    /// Sideslip, degrees (air from the right positive).
    pub sideslip: f32,
    pub throttle: f32,
    pub brake: f32,
    pub lift: f32,
    pub thrust: f32,
    pub drag: f32,
    pub stalled: bool,
    /// On final: metres above (+) or below (−) the glide path.
    pub glide_error: f32,
    /// Landings abandoned and flown again.
    pub go_arounds: u32,
    /// The landing goal finished: stopped on the runway.
    pub landed: bool,
}

/// Committed physical observation the pilot flies from.
#[derive(Clone, Copy, Debug)]
pub struct BodyInput {
    pub transform: Transform,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub mass: f32,
    /// Box collider half extents in metres (after scale), for inertia.
    pub half_extents: Vec3,
    pub grounded: bool,
    pub revision: u64,
    /// Air velocity (uniform wind), m/s; aerodynamics use the velocity relative to it.
    pub wind: Vec3,
}

/// World-space force (N) and torque (N·m) for the next physics step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotOutput {
    pub force: Vec3,
    pub torque: Vec3,
}

/// Compass bearing in radians (0..TAU) of a world direction.
pub fn compass(direction: Vec3) -> f32 {
    direction.x.atan2(-direction.z).rem_euclid(TAU)
}

/// Unit horizontal direction of a compass bearing in radians.
pub fn bearing_direction(heading: f32) -> Vec3 {
    Vec3::new(heading.sin(), 0.0, -heading.cos())
}

/// Heading to fly so the ground track is `track` in `wind` at `airspeed`: turn into the
/// crosswind by asin(crosswind ÷ airspeed), at most 30°.
fn crab(track: f32, wind: Vec3, airspeed: f32) -> f32 {
    let crosswind = wind.dot(bearing_direction(track + FRAC_PI_2));
    (track - (crosswind / airspeed).clamp(-0.5, 0.5).asin()).rem_euclid(TAU)
}

fn wrap(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

fn pid(kp: f32, ki: f32, limit: f32, min: f32, max: f32) -> Pid {
    Pid::new(PidConfig {
        kp,
        ki,
        kd: 0.0,
        integral_limit: limit,
        output_min: min,
        output_max: max,
        derivative_filter_seconds: 0.0,
        tracking_gain: 0.0,
    })
    .expect("constant pilot gains are valid")
}

/// Per-aircraft autopilot state. Owned by the aircraft's locomotion agent.
#[derive(Clone, Debug)]
pub struct Pilot {
    phase: FlightPhase,
    throttle: f32,
    /// Climb-rate error → pitch correction (rad).
    climb: Pid,
    /// Airspeed error → throttle correction.
    speed: Pid,
    /// Final approach: descent-rate error → throttle correction.
    glide: Pid,
    revision: Option<u64>,
    goal: Option<FlightGoal>,
    runway_heading: f32,
    ground_altitude: f32,
    airborne_for: f32,
    /// The current goal has been completed on the ground (landed or arrived); don't start again.
    done: bool,
    go_arounds: u32,
    gates: Gates,
    telemetry: FlightTelemetry,
    nav: PatternNav,
    /// A drawn pattern is being flown this tick.
    nav_active: bool,
}

impl Default for Pilot {
    fn default() -> Self {
        Self {
            phase: FlightPhase::Parked,
            throttle: 0.0,
            climb: pid(0.08, 0.04, 0.15, -0.2, 0.2),
            speed: pid(0.15, 0.05, 0.4, -1.0, 1.0),
            glide: pid(0.25, 0.08, 0.3, -1.0, 1.0),
            revision: None,
            goal: None,
            runway_heading: 0.0,
            ground_altitude: 0.0,
            airborne_for: 0.0,
            done: false,
            go_arounds: 0,
            gates: Gates::default(),
            telemetry: FlightTelemetry::default(),
            nav: PatternNav::default(),
            nav_active: false,
        }
    }
}

/// How the air loops hold height.
enum Vertical {
    /// Pitch to reach and hold a world altitude.
    Altitude(f32),
    /// Full power at the best climb the speed allows.
    ClimbOut,
    /// Fixed attitude (rad) with the throttle given: final approach and flare.
    PitchPower { pitch: f32, throttle: f32 },
}

/// One tick's air command.
struct Air {
    heading: f32,
    bank_limit: f32,
    speed: f32,
    vertical: Vertical,
}

/// Runway frame for the circuit: along the landing direction from the threshold, and to its left.
struct Circuit {
    heading: f32,
    direction: Vec3,
    /// Unit vector to the circuit side of the centreline (left of the landing direction for a
    /// left-hand circuit), and +1 (left) / −1 (right) for turn directions.
    side: Vec3,
    sign: f32,
    threshold: Vec3,
    /// Distance from the threshold to the touchdown aim point.
    aim: f32,
    length: f32,
    /// Downwind distance from the centreline; also how far out base leg is flown.
    spacing: f32,
    turn_radius: f32,
    elevation: f32,
    pattern_altitude: f32,
    /// The airport's scale (see [`Runway::scale`]).
    scale: f32,
}

impl Circuit {
    fn new(p: &FixedWingParams, runway: &Runway, pattern_height: f32) -> Self {
        let direction = bearing_direction(runway.heading);
        let speed = p.approach_speed() * 1.15;
        let turn_radius = speed * speed / (G * CIRCUIT_BANK.tan());
        let spacing = (3.0 * turn_radius).max(400.0 * runway.scale);
        let threshold = runway.center - direction * (runway.length / 2.0);
        Self {
            heading: runway.heading,
            direction,
            side: bearing_direction(
                runway.heading
                    + if runway.right_circuit {
                        FRAC_PI_2
                    } else {
                        -FRAC_PI_2
                    },
            ),
            sign: if runway.right_circuit { -1.0 } else { 1.0 },
            threshold,
            aim: (runway.length * 0.15).min(150.0 * runway.scale),
            length: runway.length,
            spacing,
            turn_radius,
            elevation: runway.center.y,
            pattern_altitude: runway.center.y + pattern_height,
            scale: runway.scale,
        }
    }
    /// (along, out to the circuit side) of a world position relative to the threshold.
    fn frame(&self, position: Vec3) -> (f32, f32) {
        let relative = position - self.threshold;
        (relative.dot(self.direction), relative.dot(self.side))
    }
    /// Where the circuit is joined: the upwind end of the downwind leg.
    fn entry(&self) -> Vec3 {
        self.threshold + self.direction * self.length + self.side * self.spacing
    }
    /// Glide-path altitude (wheels) at an along-runway position.
    fn glide_altitude(&self, along: f32) -> f32 {
        self.elevation + GLIDE_SLOPE.tan() * (self.aim - along).max(0.0)
    }
}

/// Where an aircraft with these parameters flies the circuit for a runway: for maps and overlays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CircuitOutline {
    pub threshold: Vec3,
    pub far_end: Vec3,
    /// Joining point: the upwind end of the downwind leg.
    pub entry: Vec3,
    /// Nominal turn onto base (aircraft extend downwind beyond it when told to, or when high).
    pub base_turn: Vec3,
    /// Turn onto final, on the extended centreline.
    pub final_turn: Vec3,
    /// Touchdown aim point.
    pub aim: Vec3,
    pub pattern_altitude: f32,
}

pub fn circuit_outline(
    p: &FixedWingParams,
    runway: &Runway,
    pattern_height: f32,
) -> CircuitOutline {
    let c = Circuit::new(p, runway, pattern_height);
    CircuitOutline {
        threshold: c.threshold,
        far_end: c.threshold + c.direction * c.length,
        entry: c.entry(),
        base_turn: c.threshold - c.direction * c.spacing + c.side * c.spacing,
        final_turn: c.threshold - c.direction * c.spacing,
        aim: c.threshold + c.direction * c.aim,
        pattern_altitude: c.pattern_altitude,
    }
}

impl Pilot {
    pub fn telemetry(&self) -> FlightTelemetry {
        self.telemetry
    }
    pub fn phase(&self) -> FlightPhase {
        self.phase
    }

    fn reset(&mut self) {
        let (phase, go_arounds) = (self.phase, self.go_arounds);
        *self = Self::default();
        self.phase = phase;
        self.go_arounds = go_arounds;
    }

    /// One fixed tick: phase logic, autopilot loops and the plant. `runway` is the landing goal's
    /// zone resolved this tick (None if there is no landing goal or the zone is gone); `pattern`,
    /// when drawn for that runway end, replaces the generated circuit up to final.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        p: &FixedWingParams,
        goal: FlightGoal,
        runway: Option<Runway>,
        pattern: Option<&[PatternFix]>,
        gates: Gates,
        body: &BodyInput,
        dt: Duration,
    ) -> PilotOutput {
        let seconds = dt.as_secs_f32();
        self.gates = gates;
        if self.revision != Some(body.revision) {
            // A rebuilt body has no flight history; work out the phase again from scratch.
            if self.revision.is_some() {
                self.reset();
            }
            self.phase = if body.grounded {
                FlightPhase::Parked
            } else {
                FlightPhase::Cruise
            };
            self.revision = Some(body.revision);
        }

        // Body frame: forward −Z, right +X, up +Y.
        let rotation = body.transform.rotation;
        let inverse = rotation.inverse();
        let (forward, right, up) = (
            rotation * Vec3::NEG_Z,
            rotation * Vec3::X,
            rotation * Vec3::Y,
        );
        let position = body.transform.translation;
        // Aerodynamics see the air (`v`); wheels, taxiing and tracks see the ground (`ground`).
        let ground = body.velocity;
        let v = ground - body.wind;
        let local = inverse * v;
        let (along, rise, side) = (-local.z, local.y, local.x);
        let ground_local = inverse * ground;
        let (rolling, sliding) = (-ground_local.z, ground_local.x);
        let airspeed2 = along * along + rise * rise;
        let airspeed = airspeed2.sqrt();
        let alpha = if airspeed > 0.5 {
            (-rise).atan2(along)
        } else {
            0.0
        };
        let beta = if v.length() > 0.5 {
            side.atan2(along.abs().max(0.1))
        } else {
            0.0
        };
        let pitch = forward.y.clamp(-1.0, 1.0).asin();
        let bank = (-right.y).atan2(up.y);
        let heading = compass(forward);
        let rates = inverse * body.angular_velocity;
        let (roll_rate, pitch_rate, yaw_rate) = (-rates.z, rates.x, -rates.y);
        let wheels = position.y - body.half_extents.y;

        // Goal changes restart the loops; a repeated goal keeps their history.
        if self.goal != Some(goal) {
            self.climb.reset();
            self.speed.reset();
            self.glide.reset();
            self.goal = Some(goal);
            self.done = false;
        }
        let circuit = match goal {
            FlightGoal::Land { pattern_height, .. } => {
                runway.map(|runway| Circuit::new(p, &runway, pattern_height))
            }
            _ => None,
        };
        let pattern = pattern.filter(|fixes| !fixes.is_empty() && circuit.is_some());
        self.nav_active = pattern.is_some();
        self.advance_phase(
            p,
            goal,
            circuit.as_ref(),
            pattern,
            body,
            position,
            heading,
            rolling,
            airspeed,
            seconds,
        );

        // Commands: body rates (rad/s), throttle and brake.
        let (cmd_p, cmd_q, cmd_r, throttle_cmd, brake) = match self.phase {
            FlightPhase::Parked => (-2.0 * roll_rate, 0.0, 0.0, 0.0, 1.0),
            FlightPhase::Manual => {
                let FlightGoal::Manual(s) = goal else {
                    unreachable!("manual phase follows a manual goal")
                };
                let mut cp = s.roll.clamp(-1.0, 1.0) * p.roll_rate;
                let mut cq = s.pitch.clamp(-1.0, 1.0) * p.pitch_rate;
                if p.max_bank > 0.0 {
                    // Normal category: ease off approaching max bank, and stop pitching past ±30°.
                    cp = cp.clamp(3.0 * (-p.max_bank - bank), 3.0 * (p.max_bank - bank));
                    if pitch >= NORMAL_PITCH_LIMIT && cq > 0.0
                        || pitch <= -NORMAL_PITCH_LIMIT && cq < 0.0
                    {
                        cq = 0.0;
                    }
                }
                (
                    cp,
                    cq,
                    s.yaw.clamp(-1.0, 1.0) * p.yaw_rate,
                    s.throttle.clamp(0.0, 1.0),
                    s.brake.clamp(0.0, 1.0),
                )
            }
            FlightPhase::TakeoffRoll
            | FlightPhase::Rotate
            | FlightPhase::Rollout
            | FlightPhase::Taxi => {
                let (pitch_goal, steer_to, throttle, brake) = match self.phase {
                    FlightPhase::Rotate => (p.rotate_pitch, self.runway_heading, 1.0, 0.0),
                    FlightPhase::TakeoffRoll => {
                        // From a runway: steer back onto its centreline, not just along the heading.
                        let steer = match goal {
                            FlightGoal::TakeOffFrom { runway, .. } => {
                                let left = bearing_direction(runway.heading - FRAC_PI_2);
                                let offset = (position - runway.center).dot(left);
                                let drift = ground.dot(left);
                                runway.heading
                                    + ((offset + 3.0 * drift) / 40.0).atan().clamp(-0.3, 0.3)
                            }
                            _ => self.runway_heading,
                        };
                        (0.0, steer, 1.0, 0.0)
                    }
                    FlightPhase::Rollout => {
                        // Lower the nose, then brake harder as it comes down.
                        let brake = if pitch < 2f32.to_radians() { 1.0 } else { 0.3 };
                        (0.0, self.runway_heading, 0.0, brake)
                    }
                    _ => {
                        let FlightGoal::Taxi { point, stop } = goal else {
                            unreachable!("taxi phase follows a taxi goal")
                        };
                        let to = Vec3::new(point.x - position.x, 0.0, point.z - position.z);
                        // Small, slow aircraft taxi slower so they can make their turns.
                        let taxi_speed = TAXI_SPEED.min(0.4 * p.liftoff_speed);
                        let wanted = if stop {
                            taxi_speed.min(0.3 * to.length())
                        } else {
                            taxi_speed
                        };
                        let throttle = (0.15 * (wanted - rolling) + 0.05).clamp(0.0, 0.4);
                        let brake = (0.4 * (rolling - wanted)).clamp(0.0, 1.0);
                        (0.0, compass(to), throttle, brake)
                    }
                };
                (
                    -3.0 * bank,
                    (2.5 * (pitch_goal - pitch)).clamp(-p.pitch_rate, p.pitch_rate),
                    (2.0 * wrap(steer_to - heading)).clamp(-0.5, 0.5),
                    throttle,
                    brake,
                )
            }
            _ => {
                let air = self.air(
                    p,
                    goal,
                    circuit.as_ref(),
                    position,
                    ground,
                    heading,
                    airspeed,
                    wheels,
                    dt,
                );
                // Air legs are ground tracks: crab into a crosswind so the track holds.
                let air = Air {
                    heading: crab(air.heading, body.wind, airspeed.max(p.liftoff_speed)),
                    ..air
                };
                let speed = airspeed.max(p.liftoff_speed * 0.8);
                // Command a turn rate, then the bank that gives it at this speed: a fixed
                // heading→bank gain turns g/V faster when slow and outruns the roll loop (wing rock).
                let turn_rate_goal = HEADING_GAIN * wrap(air.heading - heading);
                let bank_goal = (turn_rate_goal * speed / G)
                    .atan()
                    .clamp(-air.bank_limit, air.bank_limit);
                let load = 1.0 / bank.cos().max(0.3);
                let (pitch_goal, climb_goal, fixed_throttle) = match air.vertical {
                    Vertical::PitchPower { pitch, throttle } => (pitch, None, Some(throttle)),
                    Vertical::ClimbOut | Vertical::Altitude(_) => {
                        let climb_goal = match air.vertical {
                            // Keep a stall margin: give up climb rate below ~1.25 × lift-off speed.
                            Vertical::ClimbOut => {
                                p.max_climb
                                    * ((airspeed - 1.05 * p.liftoff_speed)
                                        / (0.2 * p.liftoff_speed))
                                        .clamp(0.0, 1.0)
                            }
                            Vertical::Altitude(altitude) => {
                                (0.3 * (altitude - position.y)).clamp(-p.max_descent, p.max_climb)
                            }
                            Vertical::PitchPower { .. } => unreachable!(),
                        };
                        // Climb only as fast as spare thrust allows, or the climb eats the airspeed.
                        let sustainable =
                            0.8 * speed * (p.thrust_to_weight - p.drag_ratio(speed, load));
                        let climb_goal = climb_goal.min(sustainable.max(0.0));
                        let path = (climb_goal / speed).clamp(-1.0, 1.0).asin();
                        let correction = self
                            .climb
                            .step(input(climb_goal, v.y, 0.0), dt)
                            .map_or(0.0, |o| o.output);
                        (
                            path + p.alpha_for(load, speed) + correction,
                            Some(climb_goal),
                            matches!(air.vertical, Vertical::ClimbOut).then_some(1.0),
                        )
                    }
                };
                let pitch_goal = pitch_goal.clamp(-0.45, 0.45);
                let turn = G * bank.tan().clamp(-3.0, 3.0) / speed;
                let pitch_change = 2.5 * (pitch_goal - pitch);
                let (sin_b, cos_b) = bank.sin_cos();
                let cp = (3.0 * (bank_goal - bank)).clamp(-p.roll_rate, p.roll_rate);
                let cq = (pitch_change * cos_b + turn * pitch.cos() * sin_b)
                    .clamp(-p.pitch_rate, p.pitch_rate);
                let cr = (-pitch_change * sin_b + turn * pitch.cos() * cos_b + 1.5 * beta)
                    .clamp(-p.yaw_rate, p.yaw_rate);
                let throttle = fixed_throttle.unwrap_or_else(|| {
                    let climb = climb_goal.unwrap_or(0.0);
                    let feedforward =
                        (p.drag_ratio(air.speed, load) + climb / air.speed) / p.thrust_to_weight;
                    self.speed
                        .step(input(air.speed, along, feedforward), dt)
                        .map_or(feedforward, |o| o.output)
                        .clamp(0.0, 1.0)
                });
                (cp, cq, cr, throttle, 0.0)
            }
        };
        self.throttle +=
            (throttle_cmd - self.throttle).clamp(-SPOOL_RATE * seconds, SPOOL_RATE * seconds);

        // Plant: forces.
        let weight = body.mass * G;
        let dynamic = airspeed2 / (p.liftoff_speed * p.liftoff_speed);
        let (cl, stalled) = p.lift_coefficient(alpha);
        let lift = weight * dynamic * cl;
        let symmetric = v - right * right.dot(v);
        let lift_direction = if airspeed > 0.5 {
            right.cross(symmetric).normalize_or_zero()
        } else {
            up
        };
        let (kp, ki) = p.drag();
        let drag = weight
            * (kp * airspeed2
                + ki * (lift / weight).powi(2) / airspeed2.max(1.0)
                + if stalled { 0.1 * dynamic } else { 0.0 });
        let thrust = self.throttle * p.thrust_to_weight * weight;
        let mut force = lift_direction * lift - v.normalize_or_zero() * drag + forward * thrust
            - right * (weight * 0.5 * side * v.length() / (p.liftoff_speed * p.liftoff_speed));
        if body.grounded {
            // Wheels: rolling resistance and brakes oppose rolling but never reverse it; tyres stop sideways slip.
            let stop = body.mass * rolling.abs() / seconds;
            let resist = ((ROLLING_RESISTANCE + MAX_BRAKING * brake) * weight).min(stop);
            force -= forward * resist * rolling.signum();
            force -=
                right * (body.mass * sliding / seconds * 0.5).clamp(-0.8 * weight, 0.8 * weight);
        }

        // Plant: control surfaces track the commanded rates within authority ∝ dynamic pressure.
        let authority = dynamic.min(2.0);
        let wash = 0.3 * self.throttle;
        let stall_factor = if stalled { 0.4 } else { 1.0 };
        let mut auth_p = p.roll_rate / 0.4 * authority;
        let auth_q = p.pitch_rate / 0.4 * (authority + wash) * stall_factor;
        let mut auth_r = p.yaw_rate / 0.4 * (authority + wash);
        if body.grounded {
            // Nosewheel steering works at taxi speed; the ground keeps the wings level.
            if rolling.abs() > 0.3 {
                auth_r = auth_r.max(1.5);
            }
            auth_p = auth_p.max(2.0);
        }
        let track =
            |cmd: f32, rate: f32, limit: f32| ((cmd - rate) / RATE_RESPONSE).clamp(-limit, limit);
        let mut acc_q = track(cmd_q, pitch_rate, auth_q);
        if stalled && alpha > 0.0 {
            acc_q -= 2.0 * authority.max(0.3);
        }
        let acc_r = track(cmd_r, yaw_rate, auth_r) + 2.0 * beta * authority; // weathervane
        let acc_p = track(cmd_p, roll_rate, auth_p);
        let (hx, hy, hz) = (
            body.half_extents.x,
            body.half_extents.y,
            body.half_extents.z,
        );
        let inertia =
            body.mass / 3.0 * Vec3::new(hy * hy + hz * hz, hx * hx + hz * hz, hx * hx + hy * hy);
        let mut torque = rotation * (inertia * Vec3::new(acc_q, -acc_r, -acc_p));
        if body.grounded && cmd_q > 0.0 {
            // A box rests on its tail edge to rotate; real main gear sits just behind the centre of
            // mass. Cancel the weight still on the wheels about that edge so rotation needs only the elevator.
            let on_wheels = (weight - lift).max(0.0);
            torque += right * (on_wheels * hz * (3.0 * cmd_q / p.pitch_rate).min(1.0));
        }

        let glide_error = match (&circuit, self.phase) {
            (Some(c), FlightPhase::Final | FlightPhase::Flare) => {
                wheels - c.glide_altitude(c.frame(position).0)
            }
            _ => 0.0,
        };
        self.telemetry = FlightTelemetry {
            phase: self.phase,
            airborne: !body.grounded,
            airspeed: along,
            ground_speed: Vec3::new(ground.x, 0.0, ground.z).length(),
            heading: heading.to_degrees(),
            altitude: position.y,
            climb_rate: v.y,
            pitch: pitch.to_degrees(),
            bank: bank.to_degrees(),
            alpha: alpha.to_degrees(),
            sideslip: beta.to_degrees(),
            throttle: self.throttle,
            brake,
            lift,
            thrust,
            drag,
            stalled,
            glide_error,
            go_arounds: self.go_arounds,
            landed: self.done && matches!(goal, FlightGoal::Land { .. }),
        };
        PilotOutput { force, torque }
    }

    /// Steers through a drawn pattern, setting the phase to the leg being flown and `nav.command`.
    /// Returns false once past the last fix (time for final).
    fn fly_pattern(
        &mut self,
        fixes: &[PatternFix],
        c: &Circuit,
        position: Vec3,
        heading: f32,
    ) -> bool {
        let radius = c.turn_radius;
        let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
        let nav = &mut self.nav;
        if let Some((from, course, altitude)) = nav.leaving {
            // Leave on the exit course until the turn onto it is done; downwind is extended
            // while the tower holds the turn onto base.
            let held = self.phase == FlightPhase::Downwind
                && fixes
                    .get(nav.fix)
                    .is_some_and(|f| f.leg == FlightPhase::Base)
                && !self.gates.may_turn_base;
            let turning = wrap(course - heading).abs() > 5f32.to_radians();
            if held || turning || flat(position - from).length() < 0.5 * radius {
                nav.command = (course, altitude);
                return true;
            }
            nav.leaving = None;
        }
        let Some(fix) = fixes.get(nav.fix).copied() else {
            return false;
        };
        self.phase = fix.leg;
        let to = flat(fix.point - position);
        let mut passed = false;
        // A fix that can't be made on its course (drawn too tight to turn) is flown straight at.
        match fix.entry.filter(|_| nav.tries < 2) {
            None => {
                // Any course: fly straight at it.
                nav.command = (compass(to), fix.point.y);
                passed = to.length() < (0.8 * radius).max(60.0);
            }
            Some(range) => {
                let fresh = nav.course.is_none();
                let course = *nav
                    .course
                    .get_or_insert_with(|| clamp_course(compass(to), range));
                let direction = bearing_direction(course);
                let relative = flat(position - fix.point);
                let (along, across) = (
                    relative.dot(direction),
                    relative.dot(bearing_direction(course + FRAC_PI_2)),
                );
                // Too close to turn onto the course in time: fly out and come back. Turning
                // through an angle takes about the radius times it, plus room to settle.
                let room = radius * (wrap(course - heading).abs() + 0.5);
                if fresh && -along < room {
                    nav.lead = true;
                }
                if along >= 0.0 && !nav.lead {
                    let tolerance = 10f32.to_radians();
                    let on_course = in_range(heading, (range.0 - tolerance, range.1 + tolerance));
                    if across.abs() < 2.0 * radius && on_course {
                        passed = true;
                    } else {
                        nav.lead = true;
                        nav.tries += 1;
                    }
                }
                if nav.lead {
                    // Far enough back to turn round and settle on the course before the fix.
                    let lead = fix.point - direction * (5.0 * radius);
                    let to_lead = flat(lead - position);
                    if to_lead.length() < 1.5 * radius {
                        nav.lead = false;
                    }
                    nav.command = (compass(to_lead), fix.point.y);
                } else {
                    // Track the line through the fix on the committed course.
                    let correction = (across / (1.5 * radius)).atan().clamp(-1.0, 1.0);
                    nav.command = ((course - correction).rem_euclid(TAU), fix.point.y);
                }
            }
        }
        if passed {
            let next = fixes.get(nav.fix + 1);
            nav.fix += 1;
            nav.course = None;
            nav.lead = false;
            nav.tries = 0;
            nav.leaving = fix.exit.map(|range| {
                let towards = next.map_or(heading, |n| compass(flat(n.point - fix.point)));
                let course = clamp_course(towards, range);
                (fix.point, course, next.map_or(fix.point.y, |n| n.point.y))
            });
            if let Some((_, course, altitude)) = nav.leaving {
                nav.command = (course, altitude);
            }
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn advance_phase(
        &mut self,
        p: &FixedWingParams,
        goal: FlightGoal,
        circuit: Option<&Circuit>,
        pattern: Option<&[PatternFix]>,
        body: &BodyInput,
        position: Vec3,
        heading: f32,
        along: f32,
        airspeed: f32,
        seconds: f32,
    ) {
        use FlightPhase::*;
        let landing = matches!(goal, FlightGoal::Land { .. });
        let flying_goal = !self.done
            && !matches!(
                goal,
                FlightGoal::Park
                    | FlightGoal::Manual(_)
                    | FlightGoal::Taxi { .. }
                    | FlightGoal::Arrive { .. }
                    | FlightGoal::Depart { .. }
            );
        // Climb out along the runway's course (a track: the air legs crab as needed).
        let go_around = |pilot: &mut Self, course: f32| {
            pilot.go_arounds += 1;
            pilot.phase = GoAround;
            pilot.runway_heading = course;
            pilot.ground_altitude = position.y;
        };
        match (self.phase, goal) {
            (_, FlightGoal::Manual(_)) => self.phase = Manual,
            (Manual, _) if body.grounded && airspeed < 1.0 => self.phase = Parked,
            (Manual, _) if !body.grounded => self.phase = Cruise,
            (Parked, FlightGoal::Taxi { .. }) if !self.done => self.phase = Taxi,
            (Taxi, FlightGoal::Taxi { point, stop }) => {
                if stop
                    && Vec3::new(point.x - position.x, 0.0, point.z - position.z).length() < 5.0
                    && along.abs() < 0.5
                {
                    self.phase = Parked;
                    self.done = true;
                }
            }
            (Taxi, _) => self.phase = Parked,
            (Parked, _) if flying_goal => {
                self.phase = TakeoffRoll;
                self.runway_heading = match goal {
                    FlightGoal::TakeOffFrom { runway, .. } => runway.heading,
                    _ => heading,
                };
                self.ground_altitude = position.y;
            }
            (TakeoffRoll, _) if !flying_goal => self.phase = Parked,
            (TakeoffRoll, _) if airspeed >= ROTATE_FRACTION * p.liftoff_speed => {
                self.phase = Rotate
            }
            (Rotate, _) => {
                self.airborne_for = if body.grounded {
                    0.0
                } else {
                    self.airborne_for + seconds
                };
                if self.airborne_for >= LIFTOFF_DWELL {
                    self.phase = Climb;
                }
            }
            (Climb, _) if position.y - self.ground_altitude >= SAFE_TURN_HEIGHT => {
                self.phase = Cruise
            }
            (Cruise | Climb, FlightGoal::Park) if body.grounded && airspeed < 1.0 => {
                self.phase = Parked
            }
            // Landing: no zone (deleted, or not a rectangle) means hold where it is.
            (PatternEntry | Downwind | Base | Final | Flare | GoAround, _)
                if landing && circuit.is_none() =>
            {
                self.phase = Cruise
            }
            (PatternEntry | Downwind | Base | Final | Flare | GoAround, _) if !landing => {
                self.phase = Cruise
            }
            (Cruise, _) if landing && circuit.is_some() => {
                self.phase = PatternEntry;
                self.nav = PatternNav::default();
            }
            (GoAround, _) => {
                let c = circuit.expect("checked above");
                if position.y >= c.pattern_altitude - 30.0 * c.scale
                    && position.y - self.ground_altitude >= SAFE_TURN_HEIGHT * c.scale
                {
                    self.phase = PatternEntry;
                    // Rejoin a drawn pattern at its downwind leg.
                    self.nav = PatternNav {
                        fix: pattern
                            .and_then(|fixes| fixes.iter().position(|f| f.leg == Downwind))
                            .unwrap_or(0),
                        ..PatternNav::default()
                    };
                }
            }
            // A drawn pattern: fly its fixes, then the final approach below.
            (PatternEntry | Downwind | Base, _) if pattern.is_some() => {
                let c = circuit.expect("checked above");
                if !self.fly_pattern(pattern.expect("checked"), c, position, heading) {
                    self.phase = Final;
                }
            }
            (PatternEntry, _) => {
                let c = circuit.expect("checked above");
                let to_entry = c.entry() - position;
                if Vec3::new(to_entry.x, 0.0, to_entry.z).length() < 2.0 * c.turn_radius {
                    self.phase = Downwind;
                }
            }
            (Downwind, _) => {
                // Extend downwind until the height left can be lost on base (pilots call it "extending").
                let c = circuit.expect("checked above");
                let a = c.frame(position).0;
                let wheels = position.y - body.half_extents.y;
                let base_descent = c.spacing / (p.approach_speed() * 1.15) * p.max_descent * 0.7;
                if self.gates.may_turn_base
                    && a < -c.spacing + c.turn_radius
                    && wheels <= c.glide_altitude(a) + 30.0 * c.scale + base_descent
                {
                    self.phase = Base;
                }
            }
            (Base, _) => {
                let c = circuit.expect("checked above");
                // Lead the turn by the radius over the ground: a tailwind on base widens it.
                let ground_speed = Vec3::new(body.velocity.x, 0.0, body.velocity.z).length();
                let ground_radius = ground_speed * ground_speed / (G * CIRCUIT_BANK.tan());
                if c.frame(position).1 < c.turn_radius.max(ground_radius) * 0.8 {
                    self.phase = Final;
                }
            }
            (Final, _) => {
                let c = circuit.expect("checked above");
                let (a, s) = c.frame(position);
                let wheels = position.y - body.half_extents.y;
                let height = wheels - c.elevation;
                let error = wheels - c.glide_altitude(a);
                // Stabilised-approach gate: below 60 m it must be on the glide path, and once lined up,
                // on the centreline.
                // Gates shrink with a miniature airport, but not below what a small aircraft needs.
                let lined_up = wrap(c.heading - heading).abs() < 30f32.to_radians();
                let unstable = height < (60.0 * c.scale).max(15.0)
                    && ((lined_up && s.abs() > (40.0 * c.scale).max(8.0))
                        || error.abs() > (20.0 * c.scale).max(5.0));
                // No landing clearance by 1 km out: go around (the tower re-sequences it).
                let uncleared = !self.gates.cleared_to_land && a > -1000.0 * c.scale;
                if unstable
                    || uncleared
                    || airspeed < 1.05 * p.liftoff_speed
                    || a > c.aim + 0.4 * c.length
                {
                    go_around(self, c.heading);
                } else if height < FLARE_HEIGHT && a > -50.0 * c.scale {
                    self.phase = Flare;
                }
            }
            (Flare, _) => {
                let c = circuit.expect("checked above");
                if body.grounded {
                    self.phase = Rollout;
                    self.runway_heading = c.heading;
                } else if c.frame(position).0 > c.aim + 0.5 * c.length {
                    // Floating down the runway: there won't be room to stop.
                    go_around(self, c.heading);
                }
            }
            (Rollout, _) if along.abs() < 0.5 => {
                self.phase = Parked;
                self.done = true;
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn air(
        &mut self,
        p: &FixedWingParams,
        goal: FlightGoal,
        circuit: Option<&Circuit>,
        position: Vec3,
        v: Vec3,
        heading: f32,
        airspeed: f32,
        wheels: f32,
        dt: Duration,
    ) -> Air {
        use FlightPhase::*;
        let climbing = self.phase == Climb;
        let cruise = |heading: f32, altitude: f32, speed: f32| Air {
            heading: heading.rem_euclid(TAU),
            bank_limit: p.bank_limit(),
            speed: speed.max(p.liftoff_speed * 1.1),
            vertical: if climbing && position.y < altitude - 10.0 {
                Vertical::ClimbOut
            } else {
                Vertical::Altitude(altitude)
            },
        };
        let runway = self.runway_heading;
        if let (Some(c), PatternEntry | Downwind | Base | Final | Flare | GoAround) =
            (circuit, self.phase)
        {
            let (a, s) = c.frame(position);
            let circuit_speed = p.approach_speed() * if self.gates.slow { 1.0 } else { 1.15 };
            let bank_limit = p.bank_limit().min(CIRCUIT_BANK);
            let leg = |heading: f32, altitude: f32| Air {
                heading: heading.rem_euclid(TAU),
                bank_limit,
                speed: circuit_speed,
                vertical: Vertical::Altitude(altitude),
            };
            return match self.phase {
                GoAround => Air {
                    heading: runway,
                    bank_limit,
                    speed: circuit_speed,
                    vertical: Vertical::ClimbOut,
                },
                PatternEntry | Downwind | Base if self.nav_active => {
                    leg(self.nav.command.0, self.nav.command.1)
                }
                PatternEntry => {
                    let to = c.entry() - position;
                    leg(compass(Vec3::new(to.x, 0.0, to.z)), c.pattern_altitude)
                }
                // Downwind runs opposite the landing direction with the runway on the circuit's inside.
                // Descending from abeam the threshold towards a little above the glide path.
                Downwind => leg(
                    c.heading + PI
                        - c.sign
                            * ((s - c.spacing) / (400.0 * c.scale))
                                .atan()
                                .clamp(-0.5, 0.5),
                    if a < 0.0 {
                        c.pattern_altitude.min(c.glide_altitude(a) + 30.0 * c.scale)
                    } else {
                        c.pattern_altitude
                    },
                ),
                // Base flies towards the centreline, holding wherever downwind was extended to, down onto the glide path.
                Base => leg(
                    c.heading + c.sign * FRAC_PI_2,
                    c.pattern_altitude.min(c.glide_altitude(a)),
                ),
                _ => {
                    // Final: track the centreline; nose slightly up and power on the glide path.
                    let drift = v.dot(c.side);
                    let correction = c.sign
                        * ((s + 4.0 * drift) / (100.0 * c.scale).max(10.0))
                            .atan()
                            .clamp(-0.5, 0.5);
                    let approach = p.approach_speed();
                    if self.phase == Flare {
                        let height = (wheels - c.elevation).max(0.0);
                        let pitch = p.approach_pitch()
                            + 5f32.to_radians() * (1.0 - height / FLARE_HEIGHT).clamp(0.0, 1.0);
                        return Air {
                            heading: (c.heading + correction).rem_euclid(TAU),
                            bank_limit: 5f32.to_radians(),
                            speed: approach,
                            vertical: Vertical::PitchPower {
                                pitch,
                                throttle: 0.0,
                            },
                        };
                    }
                    let ground_speed = Vec3::new(v.x, 0.0, v.z).length();
                    let error = wheels - c.glide_altitude(a);
                    let wanted = (-ground_speed * GLIDE_SLOPE.tan() - 0.2 * error)
                        .clamp(-p.max_descent, 1.0);
                    let feedforward =
                        (p.drag_ratio(approach, 1.0) + wanted / approach) / p.thrust_to_weight;
                    let throttle = self
                        .glide
                        .step(input(wanted, v.y, feedforward), dt)
                        .map_or(feedforward, |o| o.output)
                        .clamp(0.0, 1.0);
                    // Speed protection: lower the nose when slow, raise it when fast.
                    let pitch = p.approach_pitch() - 0.02 * (approach - airspeed);
                    Air {
                        heading: (c.heading + correction).rem_euclid(TAU),
                        // Circuit bank for the turn onto final, then gentle corrections once lined up.
                        bank_limit: if wrap(c.heading - heading).abs() > 20f32.to_radians() {
                            bank_limit
                        } else {
                            bank_limit.min(15f32.to_radians())
                        },
                        speed: approach,
                        vertical: Vertical::PitchPower { pitch, throttle },
                    }
                }
            };
        }
        match goal {
            FlightGoal::TakeOff { altitude } | FlightGoal::TakeOffFrom { altitude, .. } => {
                cruise(runway, altitude, p.cruise_speed)
            }
            FlightGoal::Hold {
                heading: compass_degrees,
                altitude,
                speed,
            } => cruise(
                if climbing {
                    runway
                } else {
                    compass_degrees.to_radians()
                },
                altitude,
                speed,
            ),
            FlightGoal::FlyTo {
                point,
                altitude,
                speed,
            } => {
                let to = Vec3::new(point.x - position.x, 0.0, point.z - position.z);
                let distance = to.length();
                let radius = (airspeed.max(p.liftoff_speed).powi(2)
                    / (G * (p.bank_limit() * 0.7).tan()))
                .max(10.0);
                // Inside the circle, fly its tangent (clockwise), pulled in or out by the radius error.
                let bearing = if distance > 1e-3 {
                    compass(to)
                } else {
                    heading
                };
                let course = if distance < radius * 1.5 {
                    bearing - FRAC_PI_2 + ((distance - radius) / radius).clamp(-1.0, 1.0) * 0.6
                } else {
                    bearing
                };
                cruise(if climbing { runway } else { course }, altitude, speed)
            }
            // Landing without a usable zone, Park in the air, or after a manual hand-back: hold.
            _ => cruise(
                heading,
                if climbing {
                    position.y + 100.0
                } else {
                    position.y
                },
                p.cruise_speed,
            ),
        }
    }
}

fn input(setpoint: f32, measurement: f32, feedforward: f32) -> PidInput {
    PidInput {
        setpoint,
        measurement,
        feedforward,
        measurement_rate: None,
        achieved_output: None,
    }
}

/// Body orientation for a compass heading (degrees), pitch and bank (radians); handy for tests and spawning.
pub fn attitude(heading_degrees: f32, pitch: f32, bank: f32) -> Quat {
    Quat::from_rotation_y(-heading_degrees.to_radians())
        * Quat::from_rotation_x(pitch)
        * Quat::from_rotation_z(-bank)
}

#[cfg(test)]
mod pattern_tests {
    use super::*;

    #[test]
    fn course_ranges_wrap_through_north() {
        let deg = f32::to_radians;
        let north = (deg(350.0), deg(10.0));
        assert!(in_range(deg(355.0), north) && in_range(deg(5.0), north) && in_range(0.0, north));
        assert!(!in_range(deg(180.0), north) && !in_range(deg(20.0), north));
        assert!((clamp_course(deg(30.0), north) - deg(10.0)).abs() < 1e-4);
        assert!((clamp_course(deg(300.0), north) - deg(350.0)).abs() < 1e-4);
        assert!((clamp_course(deg(5.0), north) - deg(5.0)).abs() < 1e-4);
    }
}
