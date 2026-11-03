//! Locomotion: how an object carries out what its brain intends, within physical limits.
//!
//! A profile (`*.locomotion.toml`, attached with `emergent.locomotion`) names a type and its
//! tuning. Each fixed tick, after brains and before physics, every agent turns its latest
//! [`Intent`] into motion with a kinematic model (bicycle model for wheels, point mass with
//! tilt-limited thrust for drones, coordinated turns for aircraft, gravity and drag for
//! projectiles) and publishes the activity behind it (steer angle, throttle, brake, bank,
//! rotor and thrust levels) for visuals. Models move the transform directly, so profiles are
//! rejected on dynamic physics bodies. `DronePhysics` is an unpowered boundary:
//! physics owns its pose, external code may apply force/torque, and pilot intent
//! is not executed until the powered pilot is delivered.
use std::time::Duration;

use bevy_ecs::component::Component;
use serde::{Deserialize, Serialize};

use crate::brain::Intent;
use crate::component::{
    validate_fields, ComponentField, ComponentRegistry, ComponentValues, FieldKind,
};
use crate::plugin::{Hook, LocomotionModel, Params};
use crate::{EngineError, EntityId, Project, Quat, Transform, Vec3, World};

const GRAVITY: f32 = 9.81;
const MAX_FILE_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize)]
pub struct LocomotionType {
    pub id: &'static str,
    pub label: &'static str,
    pub category: &'static str,
    /// `available`, `boundary` (loadable but unpowered), or `planned`.
    pub status: &'static str,
    pub description: &'static str,
    /// Activities this type carries out, shown in the editor.
    pub manoeuvres: &'static [&'static str],
    /// Schematic the editor draws (wheels, rotors, wing, projectile, body).
    pub rig: &'static str,
    pub fields: Vec<ComponentField>,
}

pub const CATEGORIES: &[&str] = &[
    "Ground",
    "Legged",
    "Rotorcraft & VTOL",
    "Fixed wing",
    "Projectiles & propulsion",
    "Water",
];

/// Every locomotion type: the single source for loading, the editor library and AI prompts.
pub fn type_catalogue() -> Vec<LocomotionType> {
    use FieldKind::*;
    let f = ComponentField::new;
    let num = |name, label, min, max, default: f64, unit: &'static str| {
        let field = f(name, label, Number).range(min, max).default(default);
        if unit.is_empty() {
            field
        } else {
            field.unit(unit)
        }
    };
    let floor = || {
        num(
            "ground_height",
            "Ground height",
            -10000.0,
            10000.0,
            0.0,
            "m",
        )
        .help("Lands or stops at this height.")
    };
    let t = |id, label, category, description, manoeuvres, rig, fields| LocomotionType {
        id,
        label,
        category,
        status: "available",
        description,
        manoeuvres,
        rig,
        fields,
    };
    let planned = |id, label, category, description, rig| LocomotionType {
        id,
        label,
        category,
        status: "planned",
        description,
        manoeuvres: &[],
        rig,
        fields: vec![],
    };
    let c = CATEGORIES;
    vec![
        t("slide", "Slide", c[0], "A point mass pushed by force: accelerates towards the wanted velocity, brakes to a stop and turns to face where it goes.", &["move to", "steer", "brake"], "body", vec![
            num("max_speed", "Max speed", 0.0, 200.0, 4.0, "m/s"), num("acceleration", "Acceleration", 0.0, 1000.0, 8.0, "m/s²"), num("braking", "Braking", 0.0, 1000.0, 12.0, "m/s²"),
            num("turn_rate", "Turn rate", 0.0, 3600.0, 360.0, "°/s").help("How fast it turns to face its movement; 0 never turns."), f("ground", "Stay on the ground", Bool).default(true),
        ]),
        t("wheeled", "Wheeled", c[0], "A car or cart (bicycle model): front wheels steer within a lock angle, the body follows its wheels, throttle and brakes set the speed. Cannot turn tighter than wheelbase ÷ tan(max steer).", &["move to", "steer", "brake", "boost"], "wheels", vec![
            num("max_speed", "Max speed", 0.0, 200.0, 12.0, "m/s"), num("acceleration", "Acceleration", 0.0, 100.0, 4.0, "m/s²"), num("braking", "Braking", 0.0, 100.0, 8.0, "m/s²"),
            num("wheelbase", "Wheelbase", 0.1, 50.0, 2.5, "m"), num("max_steer", "Max steer", 1.0, 80.0, 35.0, "°"), num("steer_rate", "Steer rate", 1.0, 1000.0, 120.0, "°/s"),
            num("rolling_resistance", "Rolling resistance", 0.0, 50.0, 0.5, "m/s²").help("Slows the car when coasting."),
        ]),
        planned("tracked", "Tracked / skid-steer", c[0], "Tanks and diggers that turn on the spot.", "wheels"),
        planned("hovercraft", "Hovercraft", c[0], "Drifts on a cushion of air; slides wide in turns.", "body"),
        planned("biped_ik", "Biped (IK walk)", c[1], "Two legs placed with inverse kinematics; needs skeletons and animation.", "legs"),
        planned("quadruped", "Quadruped", c[1], "Four legs with gait patterns.", "legs"),
        planned("crawler", "Crawler", c[1], "Many legs; climbs slopes.", "legs"),
        t("drone", "Drone (multirotor)", c[2], "Tilts to move, holds altitude and hovers in place, yaws to face its travel. Horizontal acceleration is limited by the tilt angle.", &["move to", "hover", "climb", "orbit point", "brake"], "rotors", vec![
            num("max_speed", "Max speed", 0.0, 200.0, 10.0, "m/s"), num("max_climb", "Max climb", 0.0, 100.0, 4.0, "m/s"), num("max_tilt", "Max tilt", 1.0, 80.0, 30.0, "°"),
            num("yaw_rate", "Yaw rate", 0.0, 3600.0, 180.0, "°/s"), num("response", "Response time", 0.05, 10.0, 0.6, "s").help("How quickly it reaches the wanted velocity."), floor(),
        ]),
        LocomotionType {
            id: "drone_physics", label: "Drone (physics boundary)", category: c[2],
            status: "boundary", description: "Unpowered physics boundary. Requires a dynamic body and the physics feature. Forces and torques are supplied by game code; brain flight commands and powered preview are not implemented yet.",
            manoeuvres: &[], rig: "rotors", fields: vec![],
        },
        planned("helicopter", "Helicopter", c[2], "Collective, cyclic and tail rotor.", "rotors"),
        planned("tilt_rotor", "Tilt-rotor", c[2], "Hovers like a helicopter, cruises like a plane.", "rotors"),
        t("fixed_wing", "Fixed wing", c[3], "An aircraft: never slower than its stall speed, banks to turn (turn rate = g·tan(bank) ÷ speed), climbs and descends within limits. Hover makes it circle where it is.", &["move to", "orbit point", "climb", "loiter", "boost"], "wing", vec![
            num("min_speed", "Stall speed", 1.0, 500.0, 18.0, "m/s"), num("cruise_speed", "Cruise speed", 1.0, 500.0, 28.0, "m/s"), num("max_speed", "Max speed", 1.0, 1000.0, 45.0, "m/s"),
            num("acceleration", "Acceleration", 0.0, 100.0, 3.0, "m/s²"), num("max_bank", "Max bank", 1.0, 85.0, 45.0, "°"), num("bank_rate", "Roll rate", 1.0, 720.0, 60.0, "°/s"),
            num("max_climb", "Max climb", 0.0, 200.0, 6.0, "m/s"), num("max_descent", "Max descent", 0.0, 200.0, 8.0, "m/s"), floor(),
        ]),
        t("fixed_wing_physics", "Fixed wing (physics)", c[3], "A rigid-body aircraft flown by lift, drag, thrust and control-surface torques (needs the physics feature, a dynamic box body with friction 0; the pilot models the wheels). Rolls to lift-off speed, rotates, climbs on excess power, holds compass heading, altitude and speed, flies to points; max bank 0 allows aerobatics in manual flight.", &["take off", "climb", "hold heading", "hold altitude", "fly to", "manual"], "wing", vec![
            num("liftoff_speed", "Lift-off speed", 1.0, 300.0, 22.0, "m/s").help("Airspeed over the wing at which lift equals weight at the rotation angle."),
            num("cruise_speed", "Cruise speed", 1.0, 500.0, 40.0, "m/s"), num("max_speed", "Max speed", 1.0, 1000.0, 60.0, "m/s").help("Level-flight top speed at full throttle."),
            num("thrust_to_weight", "Thrust ÷ weight", 0.01, 5.0, 0.35, "").help("Full-throttle thrust as a fraction of weight: sets how fast it accelerates and climbs."),
            num("rotate_pitch", "Rotation angle", 1.0, 30.0, 8.0, "°").help("Nose-up angle held to lift off."), num("stall_angle", "Stall angle", 2.0, 40.0, 15.0, "°"),
            num("max_bank", "Max bank", 0.0, 85.0, 45.0, "°").help("0 removes the limit: manual flight can roll and loop."),
            num("roll_rate", "Roll rate", 1.0, 720.0, 90.0, "°/s"), num("pitch_rate", "Pitch rate", 1.0, 360.0, 30.0, "°/s"), num("yaw_rate", "Yaw rate", 1.0, 360.0, 20.0, "°/s"),
            num("max_climb", "Max climb", 0.1, 100.0, 5.0, "m/s"), num("max_descent", "Max descent", 0.1, 100.0, 4.0, "m/s"),
        ]),
        t("ballistic", "Ballistic", c[4], "Launched once at a set speed, then gravity and drag take over. Aims to hit the brain's target on a low or high arc when it can reach it.", &["launch", "ballistic arc"], "projectile", vec![
            num("launch_speed", "Launch speed", 0.1, 5000.0, 25.0, "m/s"), f("arc", "Arc", Choice).options(&["low", "high"]).default("low"),
            num("gravity_scale", "Gravity scale", 0.0, 10.0, 1.0, ""), num("drag", "Drag", 0.0, 10.0, 0.0, "1/s"), floor(),
        ]),
        t("rocket", "Rocket", c[4], "Thrusts along its nose while fuel lasts, turning towards the target within its turn rate; then coasts ballistically.", &["burn", "steer to target", "ballistic arc"], "projectile", vec![
            num("thrust", "Thrust", 0.0, 10000.0, 30.0, "m/s²"), num("burn_time", "Burn time", 0.0, 3600.0, 3.0, "s"), num("turn_rate", "Turn rate", 0.0, 3600.0, 90.0, "°/s"),
            num("launch_angle", "Launch angle", -90.0, 90.0, 60.0, "°").help("Initial nose-up angle."), num("gravity_scale", "Gravity scale", 0.0, 10.0, 1.0, ""), num("drag", "Drag", 0.0, 10.0, 0.05, "1/s"), floor(),
        ]),
        planned("guided_missile", "Guided missile", c[4], "Proportional navigation onto a moving target.", "projectile"),
        planned("bomb", "Bomb drop", c[4], "Solves the release point from speed and height.", "projectile"),
        planned("boat", "Boat", c[5], "Planes on water; rudder steering.", "body"),
        planned("submarine", "Submarine", c[5], "Dives and surfaces with ballast.", "body"),
    ]
}

/// Built-in types plus the plugin types in `registry`.
pub fn type_catalogue_with(registry: &ComponentRegistry) -> Vec<LocomotionType> {
    let mut types = type_catalogue();
    types.extend(registry.locomotion.iter().map(|(spec, _)| spec.clone()));
    types
}

pub fn export_type_catalogue() -> Result<String, toml::ser::Error> {
    export_type_catalogue_with(&ComponentRegistry::builtin())
}

/// Type catalogue export including the project's plugin types.
pub fn export_type_catalogue_with(
    registry: &ComponentRegistry,
) -> Result<String, toml::ser::Error> {
    #[derive(Serialize)]
    struct Export {
        categories: &'static [&'static str],
        types: Vec<LocomotionType>,
    }
    toml::to_string(&Export {
        categories: CATEGORIES,
        types: type_catalogue_with(registry),
    })
}

#[derive(Deserialize)]
struct ProfileFile {
    schema_version: u32,
    id: String,
    name: Option<String>,
    kind: Option<String>,
    description: Option<String>,
    template: Option<String>,
    runtime_status: Option<String>,
    #[serde(rename = "type")]
    model: String,
    #[serde(flatten)]
    fields: toml::Table,
}

/// The tuning of one model, read from a validated profile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Model {
    /// A plugin's kinematic mover with its packed fields.
    Plugin {
        model: Hook<dyn LocomotionModel>,
        params: Params,
    },
    /// Unpowered physical body boundary; external code supplies forces/torques.
    DronePhysics,
    /// Rigid-body aircraft flown by its pilot's forces and torques.
    FixedWingPhysics(crate::fixed_wing::FixedWingParams),
    Slide {
        max_speed: f32,
        acceleration: f32,
        braking: f32,
        turn_rate: f32,
        ground: bool,
    },
    Wheeled {
        max_speed: f32,
        acceleration: f32,
        braking: f32,
        wheelbase: f32,
        max_steer: f32,
        steer_rate: f32,
        rolling: f32,
    },
    Drone {
        max_speed: f32,
        max_climb: f32,
        max_tilt: f32,
        yaw_rate: f32,
        response: f32,
        floor: f32,
    },
    FixedWing {
        min_speed: f32,
        cruise: f32,
        max_speed: f32,
        acceleration: f32,
        max_bank: f32,
        bank_rate: f32,
        max_climb: f32,
        max_descent: f32,
        floor: f32,
    },
    Ballistic {
        launch_speed: f32,
        high: bool,
        gravity: f32,
        drag: f32,
        floor: f32,
    },
    Rocket {
        thrust: f32,
        burn: f32,
        turn_rate: f32,
        launch_angle: f32,
        gravity: f32,
        drag: f32,
        floor: f32,
    },
}

impl Model {
    pub fn is_physical(self) -> bool {
        matches!(self, Self::DronePhysics | Self::FixedWingPhysics(_))
    }

    pub(crate) fn validate_physics(
        self,
        body: Option<crate::PhysicsBody>,
    ) -> Result<(), crate::PhysicsError> {
        if self.is_physical() {
            if !cfg!(feature = "physics") {
                return Err(crate::PhysicsError::FeatureDisabled);
            }
            if !body.is_some_and(|body| body.body == crate::BodyKind::Dynamic) {
                return Err(crate::PhysicsError::PhysicalLocomotionRequiresDynamicBody);
            }
            // The pilot models the wheels, so contact must not add friction of its own.
            if matches!(self, Self::FixedWingPhysics(_))
                && !body
                    .is_some_and(|body| body.friction == 0.0 && body.collider.as_box().is_some())
            {
                return Err(crate::PhysicsError::AircraftBody);
            }
        } else if body.is_some_and(|body| body.body == crate::BodyKind::Dynamic) {
            return Err(crate::PhysicsError::LocomotionConflict);
        }
        Ok(())
    }
}

/// A loaded, validated locomotion profile.
#[derive(Clone, Debug)]
pub struct LocomotionProfile {
    pub path: String,
    pub name: String,
    pub type_id: &'static str,
    pub model: Model,
}

fn identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}

impl LocomotionProfile {
    pub fn load(project: &Project, path: &str) -> Result<Self, String> {
        Self::load_with(project, path, &ComponentRegistry::builtin())
    }

    /// Loads with the project's registry so plugin types resolve.
    pub fn load_with(
        project: &Project,
        path: &str,
        registry: &ComponentRegistry,
    ) -> Result<Self, String> {
        let source = project
            .read_text_asset(path)
            .map_err(|e| format!("{path}: {e}"))?;
        Self::from_source_with(path, &source, registry)
    }

    pub fn from_source(path: &str, source: &str) -> Result<Self, String> {
        Self::from_source_with(path, source, &ComponentRegistry::builtin())
    }

    pub fn from_source_with(
        path: &str,
        source: &str,
        registry: &ComponentRegistry,
    ) -> Result<Self, String> {
        let fail = |e: String| format!("{path}: {e}");
        if source.len() > MAX_FILE_BYTES {
            return Err(fail("locomotion file exceeds 64 KiB".into()));
        }
        let file: ProfileFile = toml::from_str(source).map_err(|e| fail(e.to_string()))?;
        let _metadata = (file.template, file.runtime_status, file.description);
        if file.schema_version != 1 || !identifier(&file.id) {
            return Err(fail("requires schema_version = 1 and a valid id".into()));
        }
        if file.kind.as_deref().is_some_and(|k| k != "locomotion") {
            return Err(fail("kind must be locomotion".into()));
        }
        let catalogue = type_catalogue_with(registry);
        let spec = catalogue
            .iter()
            .find(|t| t.id == file.model)
            .ok_or_else(|| fail(format!("unknown locomotion type '{}'", file.model)))?;
        if spec.status != "available" && spec.status != "boundary" {
            return Err(fail(format!(
                "{} is planned, not available yet",
                spec.label
            )));
        }
        if matches!(spec.id, "drone_physics" | "fixed_wing_physics") && !cfg!(feature = "physics") {
            return Err(fail(format!(
                "{} requires the optional physics feature",
                spec.id
            )));
        }
        validate_fields(spec.id, &spec.fields, &file.fields, false).map_err(fail)?;
        let v = ComponentValues::new(&spec.fields, &file.fields);
        let n = |name| v.number(name).unwrap_or_default();
        let deg = |name| n(name).to_radians();
        let model = match spec.id {
            "drone_physics" => Model::DronePhysics,
            "fixed_wing_physics" => {
                let params = crate::fixed_wing::FixedWingParams {
                    liftoff_speed: n("liftoff_speed"),
                    cruise_speed: n("cruise_speed"),
                    max_speed: n("max_speed"),
                    thrust_to_weight: n("thrust_to_weight"),
                    rotate_pitch: deg("rotate_pitch"),
                    stall_angle: deg("stall_angle"),
                    max_bank: deg("max_bank"),
                    roll_rate: deg("roll_rate"),
                    pitch_rate: deg("pitch_rate"),
                    yaw_rate: deg("yaw_rate"),
                    max_climb: n("max_climb"),
                    max_descent: n("max_descent"),
                };
                params.validate().map_err(fail)?;
                Model::FixedWingPhysics(params)
            }
            "slide" => Model::Slide {
                max_speed: n("max_speed"),
                acceleration: n("acceleration"),
                braking: n("braking"),
                turn_rate: deg("turn_rate"),
                ground: v.bool("ground").unwrap_or(true),
            },
            "wheeled" => Model::Wheeled {
                max_speed: n("max_speed"),
                acceleration: n("acceleration"),
                braking: n("braking"),
                wheelbase: n("wheelbase"),
                max_steer: deg("max_steer"),
                steer_rate: deg("steer_rate"),
                rolling: n("rolling_resistance"),
            },
            "drone" => Model::Drone {
                max_speed: n("max_speed"),
                max_climb: n("max_climb"),
                max_tilt: deg("max_tilt"),
                yaw_rate: deg("yaw_rate"),
                response: n("response"),
                floor: n("ground_height"),
            },
            "fixed_wing" => {
                let (min_speed, cruise, max_speed) =
                    (n("min_speed"), n("cruise_speed"), n("max_speed"));
                if !(min_speed <= cruise && cruise <= max_speed) {
                    return Err(fail(
                        "fixed wing needs stall speed ≤ cruise speed ≤ max speed".into(),
                    ));
                }
                Model::FixedWing {
                    min_speed,
                    cruise,
                    max_speed,
                    acceleration: n("acceleration"),
                    max_bank: deg("max_bank"),
                    bank_rate: deg("bank_rate"),
                    max_climb: n("max_climb"),
                    max_descent: n("max_descent"),
                    floor: n("ground_height"),
                }
            }
            "ballistic" => Model::Ballistic {
                launch_speed: n("launch_speed"),
                high: v.entity("arc") == Some("high"),
                gravity: n("gravity_scale") * GRAVITY,
                drag: n("drag"),
                floor: n("ground_height"),
            },
            "rocket" => Model::Rocket {
                thrust: n("thrust"),
                burn: n("burn_time"),
                turn_rate: deg("turn_rate"),
                launch_angle: deg("launch_angle"),
                gravity: n("gravity_scale") * GRAVITY,
                drag: n("drag"),
                floor: n("ground_height"),
            },
            other => match registry.locomotion.iter().find(|(s, _)| s.id == other) {
                Some(&(_, model)) => Model::Plugin {
                    model,
                    params: crate::plugin::pack(&spec.fields, &v),
                },
                None => unreachable!("catalogue type without a model: {other}"),
            },
        };
        Ok(Self {
            path: path.into(),
            name: file.name.unwrap_or(file.id),
            type_id: spec.id,
            model,
        })
    }
}

/// What the object is doing to move, for visuals, sound and the editor.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Activity {
    pub speed: f32,
    /// Front-wheel angle, radians (positive = left).
    pub steer: f32,
    pub throttle: f32,
    pub brake: f32,
    /// Roll, radians (positive = right wing down).
    pub bank: f32,
    /// Nose up, radians.
    pub pitch: f32,
    /// Heading, radians, measured from -Z towards -X (right-handed yaw about +Y).
    pub yaw: f32,
    /// Rotor demand, 0..1 (drones).
    pub rotor: f32,
    /// Engine thrust, 0..1 (rockets).
    pub thrust: f32,
    /// Remaining fuel, 0..1.
    pub fuel: f32,
    /// Projectile has come to rest on the ground.
    pub landed: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct State {
    started: bool,
    velocity: Vec3,
    /// Unit direction of travel for projectiles.
    nose: Vec3,
    burned: f32,
    hold: Option<Vec3>,
    /// Heading taken from the authored rotation on the first tick.
    oriented: bool,
}

/// Runtime locomotion of one object.
#[derive(Component, Debug)]
pub struct LocomotionAgent {
    pub model: Model,
    /// Set by the object's brain each tick (or by game code).
    pub intent: Intent,
    activity: Activity,
    state: State,
    // Keep physical snapshots out of every kinematic agent's inline layout.
    physical_state: Option<Box<crate::PhysicsBodyState>>,
    /// What a physical aircraft is asked to do; None follows the brain's intent.
    pub flight: Option<crate::fixed_wing::FlightGoal>,
    pilot: Option<Box<crate::fixed_wing::Pilot>>,
    /// The control tower's instruction for an Arrive/Depart order, refreshed each tick.
    pub(crate) dispatch: Option<crate::tower::Dispatch>,
}

impl LocomotionAgent {
    pub fn new(model: Model) -> Self {
        Self {
            model,
            intent: Intent::default(),
            activity: Activity::default(),
            state: State::default(),
            physical_state: None,
            flight: None,
            pilot: None,
            dispatch: None,
        }
    }

    /// The control tower's current instruction (stage, sequence number, gates) for an Arrive or Depart order.
    pub fn dispatch(&self) -> Option<crate::tower::Dispatch> {
        self.dispatch
    }

    /// Phase, airspeed, heading, attitude and forces of a physical aircraft, after its last tick.
    pub fn flight_telemetry(&self) -> Option<crate::fixed_wing::FlightTelemetry> {
        self.pilot.as_ref().map(|pilot| pilot.telemetry())
    }

    pub fn activity(&self) -> Activity {
        self.activity
    }

    /// Last synchronized physical observation, never inferred from commanded intent.
    pub fn physics_state(&self) -> Option<crate::PhysicsBodyState> {
        self.physical_state.as_deref().copied()
    }

    #[cfg(feature = "physics")]
    fn observe_physics(&mut self, state: Option<crate::PhysicsBodyState>) {
        if state.is_none()
            || self.physical_state.as_ref().map(|s| s.revision) != state.map(|s| s.revision)
        {
            self.state = State::default();
            self.activity = Activity::default();
        }
        match (self.physical_state.as_mut(), state) {
            (Some(previous), Some(current)) => **previous = current,
            (None, Some(current)) => self.physical_state = Some(Box::new(current)),
            (_, None) => self.physical_state = None,
        }
        if let Some(state) = state {
            self.state.velocity = state.linear_velocity;
            self.activity.speed = state.linear_velocity.length();
            let (yaw, pitch, roll) = state.transform.rotation.to_euler(crate::EulerRot::YXZ);
            self.activity.yaw = yaw;
            self.activity.pitch = pitch;
            self.activity.bank = -roll;
        }
    }
}

/// One wheel child of a locomotion object.
#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub id: EntityId,
    /// Front wheels turn with the steering angle; all wheels roll.
    pub steers: bool,
    /// Authored local rotation, so any wheel model orientation works.
    pub rest: Quat,
}

/// Wheel children a locomotion object turns and rolls each tick (see the wheel rig brief).
#[derive(Component, Debug)]
pub struct WheelRig {
    wheels: Vec<Wheel>,
    radius: f32,
    /// Accumulated roll, radians, wrapped to ±π.
    roll: f32,
}

impl WheelRig {
    pub fn new(wheels: Vec<Wheel>, radius: f32) -> Self {
        Self {
            wheels,
            radius: radius.max(0.01),
            roll: 0.0,
        }
    }
    pub fn wheels(&self) -> &[Wheel] {
        &self.wheels
    }
    /// Roll angle, radians: negative rolls the top of the wheel forward (towards -Z).
    pub fn roll(&self) -> f32 {
        self.roll
    }
}

/// Turns and rolls the wheel children from the car's activity. Stale (deleted) wheels are skipped.
fn drive_wheels(
    world: &mut World,
    id: EntityId,
    activity: Activity,
    seconds: f32,
) -> Result<(), EngineError> {
    let Some(mut rig) = world.component_mut::<WheelRig>(id)? else {
        return Ok(());
    };
    rig.roll = wrap(rig.roll - activity.speed * seconds / rig.radius);
    let (roll, count) = (rig.roll, rig.wheels.len());
    let steer = Quat::from_rotation_y(activity.steer);
    for i in 0..count {
        let Some(wheel) = world.component::<WheelRig>(id)?.map(|rig| rig.wheels[i]) else {
            break;
        };
        let Ok(mut local) = world.local_transform(wheel.id).copied() else {
            continue;
        };
        let turn = if wheel.steers { steer } else { Quat::IDENTITY };
        local.rotation = turn * Quat::from_rotation_x(roll) * wheel.rest;
        world.set_local_transform(wheel.id, local)?;
    }
    Ok(())
}

fn forward(yaw: f32) -> Vec3 {
    Vec3::new(-yaw.sin(), 0.0, -yaw.cos())
}

fn yaw_of(direction: Vec3) -> Option<f32> {
    let flat = Vec3::new(direction.x, 0.0, direction.z);
    (flat.length_squared() > 1e-8).then(|| (-flat.x).atan2(-flat.z))
}

fn wrap(angle: f32) -> f32 {
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

fn approach(value: f32, target: f32, step: f32) -> f32 {
    value + (target - value).clamp(-step, step)
}

/// Launch angle to hit a point `d` metres away horizontally and `h` metres higher at `speed`.
fn launch_angle(d: f32, h: f32, speed: f32, gravity: f32, high: bool) -> Option<f32> {
    if gravity <= 0.0 || d < 1e-3 {
        return None;
    }
    let s2 = speed * speed;
    let root = s2 * s2 - gravity * (gravity * d * d + 2.0 * h * s2);
    (root >= 0.0).then(|| {
        let sign = if high { 1.0 } else { -1.0 };
        ((s2 + sign * root.sqrt()) / (gravity * d)).atan()
    })
}

/// Advances one agent by `dt`; returns the new position. Pure: no world access.
fn advance(agent: &mut LocomotionAgent, position: Vec3, dt: f32) -> Vec3 {
    let intent = agent.intent;
    let a = &mut agent.activity;
    let s = &mut agent.state;
    let wanted = intent.velocity;
    match agent.model {
        Model::DronePhysics | Model::FixedWingPhysics(_) => position,
        Model::Plugin { model, params } => {
            model
                .0
                .advance(&params, &intent, &mut s.velocity, a, position, dt)
        }
        Model::Slide {
            max_speed,
            acceleration,
            braking,
            turn_rate,
            ground,
        } => {
            let mut target = if intent.brake || intent.hover {
                Vec3::ZERO
            } else {
                wanted.clamp_length_max(max_speed)
            };
            if intent.boost && target != Vec3::ZERO {
                target = target.normalize() * max_speed;
            }
            if ground {
                target.y = 0.0;
            }
            let slowing = target.length() < s.velocity.length();
            let limit = if slowing { braking } else { acceleration } * dt;
            s.velocity += (target - s.velocity).clamp_length_max(limit);
            if let Some(goal) =
                yaw_of(s.velocity).filter(|_| s.velocity.length() > 0.05 && turn_rate > 0.0)
            {
                a.yaw += wrap(goal - a.yaw).clamp(-turn_rate * dt, turn_rate * dt);
            }
            a.speed = s.velocity.length();
            a.throttle = if slowing {
                0.0
            } else {
                (target.length() / max_speed.max(1e-3)).min(1.0)
            };
            a.brake = if slowing && (intent.brake || target == Vec3::ZERO) {
                1.0
            } else {
                0.0
            };
            position + s.velocity * dt
        }
        Model::Wheeled {
            max_speed,
            acceleration,
            braking,
            wheelbase,
            max_steer,
            steer_rate,
            rolling,
        } => {
            let flat = Vec3::new(wanted.x, 0.0, wanted.z);
            let mut speed_goal = if intent.brake || intent.hover {
                0.0
            } else {
                flat.length().min(max_speed)
            };
            if intent.boost && speed_goal > 0.0 {
                speed_goal = max_speed;
            }
            // Pure pursuit: steer towards a point a speed-dependent distance ahead.
            let mut steer_goal = 0.0;
            if let Some(goal) = yaw_of(flat).filter(|_| speed_goal > 0.0) {
                let error = wrap(goal - a.yaw);
                let lookahead = (a.speed * 1.2).max(wheelbase * 1.5);
                steer_goal = (2.0 * wheelbase * error.sin() / lookahead)
                    .atan()
                    .clamp(-max_steer, max_steer);
                // Facing away: slow right down and keep the lock on until it comes round.
                if error.abs() > std::f32::consts::FRAC_PI_2 {
                    speed_goal = speed_goal.min(max_speed * 0.3);
                    steer_goal = max_steer.copysign(error);
                }
            }
            a.steer = approach(a.steer, steer_goal, steer_rate * dt);
            let speed = a.speed;
            let (throttle, brake) = if speed_goal > speed + 0.05 {
                (1.0, 0.0)
            } else if speed_goal < speed - 0.05 {
                (0.0, 1.0)
            } else {
                (speed_goal / max_speed.max(1e-3), 0.0)
            };
            a.speed = if throttle > 0.0 && brake == 0.0 && speed_goal > speed {
                (speed + acceleration * dt).min(speed_goal)
            } else if brake > 0.0 {
                (speed - braking * dt).max(speed_goal)
            } else {
                (speed - rolling * dt).max(speed_goal.min(speed))
            };
            a.throttle = throttle;
            a.brake = brake;
            a.yaw = wrap(a.yaw + a.speed / wheelbase * a.steer.tan() * dt);
            s.velocity = forward(a.yaw) * a.speed;
            position + s.velocity * dt
        }
        Model::Drone {
            max_speed,
            max_climb,
            max_tilt,
            yaw_rate,
            response,
            floor,
        } => {
            if s.hold.is_none() || !(intent.hover || intent.brake) {
                s.hold = (intent.hover || intent.brake).then_some(position);
            }
            let mut goal = Vec3::new(wanted.x, 0.0, wanted.z).clamp_length_max(max_speed);
            if intent.boost && goal != Vec3::ZERO {
                goal = goal.normalize() * max_speed;
            }
            let height = intent.altitude.or(s.hold.map(|h| h.y));
            goal.y = match height {
                Some(h) => ((h - position.y) * 1.5).clamp(-max_climb, max_climb),
                None => wanted.y.clamp(-max_climb, max_climb),
            };
            if let Some(hold) = s.hold {
                goal.x = (hold.x - position.x) * 1.5;
                goal.z = (hold.z - position.z) * 1.5;
            }
            let mut accel = (goal - s.velocity) / response.max(0.05);
            let horizontal =
                Vec3::new(accel.x, 0.0, accel.z).clamp_length_max(GRAVITY * max_tilt.tan());
            accel = Vec3::new(
                horizontal.x,
                accel.y.clamp(-GRAVITY * 0.8, GRAVITY * 0.8),
                horizontal.z,
            );
            s.velocity += accel * dt;
            let mut next = position + s.velocity * dt;
            if next.y < floor {
                next.y = floor;
                s.velocity.y = s.velocity.y.max(0.0);
            }
            if let Some(face) = yaw_of(s.velocity)
                .filter(|_| Vec3::new(s.velocity.x, 0.0, s.velocity.z).length() > 0.5)
            {
                a.yaw += wrap(face - a.yaw).clamp(-yaw_rate * dt, yaw_rate * dt);
            }
            let ahead = forward(a.yaw);
            let right = ahead.cross(Vec3::Y);
            a.pitch = -(horizontal.dot(ahead) / GRAVITY).atan();
            a.bank = (horizontal.dot(right) / GRAVITY).atan();
            a.rotor = ((GRAVITY + accel.y) / (2.0 * GRAVITY)
                + horizontal.length() / (4.0 * GRAVITY))
                .clamp(0.0, 1.0);
            a.speed = s.velocity.length();
            a.throttle = a.rotor;
            next
        }
        Model::FixedWing {
            min_speed,
            cruise,
            max_speed,
            acceleration,
            max_bank,
            bank_rate,
            max_climb,
            max_descent,
            floor,
        } => {
            if !s.started {
                a.speed = cruise;
                s.started = true;
            }
            let flat = Vec3::new(wanted.x, 0.0, wanted.z);
            let loiter = intent.hover || flat.length() < 0.1;
            let speed_goal = if intent.boost {
                max_speed
            } else if intent.brake {
                min_speed
            } else if loiter {
                cruise
            } else {
                flat.length().clamp(min_speed, max_speed)
            };
            a.speed = approach(a.speed, speed_goal, acceleration * dt).clamp(min_speed, max_speed);
            let bank_goal = if loiter {
                max_bank * 0.8
            } else {
                let error = yaw_of(flat).map_or(0.0, |goal| wrap(goal - a.yaw));
                // Positive bank (right wing down) turns right, i.e. decreases yaw.
                (-error * 1.5).clamp(-max_bank, max_bank)
            };
            a.bank = approach(a.bank, bank_goal, bank_rate * dt);
            a.yaw = wrap(a.yaw - GRAVITY * a.bank.tan() / a.speed.max(1.0) * dt);
            let climb_goal = match intent.altitude {
                Some(h) => (h - position.y) * 0.5,
                None if loiter => 0.0,
                None => wanted.y,
            };
            let climb = climb_goal
                .clamp(-max_descent, max_climb)
                .clamp(-a.speed * 0.5, a.speed * 0.5);
            a.pitch = (climb / a.speed.max(1e-3)).asin();
            a.throttle = (a.speed - min_speed) / (max_speed - min_speed).max(1e-3);
            s.velocity = forward(a.yaw) * (a.speed * a.speed - climb * climb).max(0.0).sqrt()
                + Vec3::Y * climb;
            let mut next = position + s.velocity * dt;
            next.y = next.y.max(floor);
            next
        }
        Model::Ballistic {
            launch_speed,
            high,
            gravity,
            drag,
            floor,
        } => {
            if !s.started {
                s.started = true;
                let aim = intent.target.and_then(|t| {
                    let offset = t - position;
                    let flat = Vec3::new(offset.x, 0.0, offset.z);
                    let angle = launch_angle(flat.length(), offset.y, launch_speed, gravity, high)?;
                    Some(flat.normalize() * angle.cos() + Vec3::Y * angle.sin())
                });
                let direction = aim
                    .or_else(|| {
                        intent.target.and_then(|t| {
                            let flat = Vec3::new(t.x - position.x, 0.0, t.z - position.z)
                                .try_normalize()?;
                            Some((flat + Vec3::Y).normalize())
                        })
                    })
                    .or_else(|| wanted.try_normalize())
                    .unwrap_or_else(|| (forward(a.yaw) + Vec3::Y).normalize());
                s.velocity = direction * launch_speed;
            }
            if a.landed {
                return position;
            }
            s.velocity.y -= gravity * dt;
            s.velocity -= s.velocity * (drag * dt).min(1.0);
            let mut next = position + s.velocity * dt;
            if next.y <= floor && s.velocity.y < 0.0 {
                next.y = floor;
                s.velocity = Vec3::ZERO;
                a.landed = true;
            }
            if let Some(yaw) = yaw_of(s.velocity) {
                a.yaw = yaw;
            }
            a.pitch = s
                .velocity
                .y
                .atan2(Vec3::new(s.velocity.x, 0.0, s.velocity.z).length());
            a.speed = s.velocity.length();
            next
        }
        Model::Rocket {
            thrust,
            burn,
            turn_rate,
            launch_angle: angle,
            gravity,
            drag,
            floor,
        } => {
            if !s.started {
                s.started = true;
                let heading = intent
                    .target
                    .and_then(|t| yaw_of(t - position))
                    .unwrap_or(a.yaw);
                a.yaw = heading;
                s.nose = forward(heading) * angle.cos() + Vec3::Y * angle.sin();
            }
            if a.landed {
                return position;
            }
            // Turn the nose towards the target (or the wanted direction) within the turn rate.
            let goal = intent
                .target
                .map(|t| t - position)
                .or(Some(wanted))
                .and_then(Vec3::try_normalize);
            if let Some(goal) = goal.filter(|_| s.burned > 0.25) {
                let between = s.nose.angle_between(goal);
                let step = (turn_rate * dt).min(between);
                if between > 1e-4 {
                    let axis = s.nose.cross(goal).try_normalize().unwrap_or(Vec3::Y);
                    s.nose = (Quat::from_axis_angle(axis, step) * s.nose).normalize();
                }
            }
            let burning = s.burned < burn;
            if burning {
                s.burned += dt;
                s.velocity += s.nose * thrust * dt;
            }
            s.velocity.y -= gravity * dt;
            s.velocity -= s.velocity * (drag * dt).min(1.0);
            let mut next = position + s.velocity * dt;
            if next.y <= floor && s.velocity.y < 0.0 && s.burned > 0.1 {
                next.y = floor;
                s.velocity = Vec3::ZERO;
                a.landed = true;
            }
            let along = if burning { s.nose } else { s.velocity };
            if let Some(yaw) = yaw_of(along) {
                a.yaw = yaw;
            }
            a.pitch = along.y.atan2(Vec3::new(along.x, 0.0, along.z).length());
            a.thrust = if burning && !a.landed { 1.0 } else { 0.0 };
            a.fuel = if burn > 0.0 {
                (1.0 - s.burned / burn).max(0.0)
            } else {
                0.0
            };
            a.speed = s.velocity.length();
            next
        }
    }
}

fn orientation(activity: &Activity) -> Quat {
    Quat::from_rotation_y(activity.yaw)
        * Quat::from_rotation_x(activity.pitch)
        * Quat::from_rotation_z(-activity.bank)
}

/// Flies one physical aircraft: goal (or brain intent) → pilot → queued force and torque.
#[cfg(feature = "physics")]
fn fly(
    world: &mut World,
    id: EntityId,
    params: &crate::fixed_wing::FixedWingParams,
    state: crate::PhysicsBodyState,
    dt: Duration,
) -> Result<(), EngineError> {
    use crate::fixed_wing::{BodyInput, FlightGoal};
    let grounded = world.is_grounded(id)?;
    let half_extents = world
        .component::<crate::PhysicsBody>(id)?
        .and_then(|body| body.collider.as_box())
        .map_or(Vec3::splat(0.5), |collider| {
            Vec3::from_array(collider.half_extents) * state.transform.scale
        });
    // A landing zone is resolved by handle every tick; a deleted or non-rectangular zone is None.
    // Tower orders (Arrive, Depart) fly the tower's dispatch instead: its goal, runway and gates.
    let (order, dispatch) = world
        .component::<LocomotionAgent>(id)?
        .map_or((None, None), |agent| {
            (agent.flight.or(agent.intent.flight), agent.dispatch)
        });
    let (tower_goal, runway, gates) = match (order, dispatch) {
        (Some(FlightGoal::Arrive { .. } | FlightGoal::Depart { .. }), Some(d)) => {
            (Some(d.goal), d.runway, d.gates)
        }
        (Some(FlightGoal::Arrive { .. } | FlightGoal::Depart { .. }), None) => {
            (Some(FlightGoal::Park), None, Default::default())
        }
        (Some(FlightGoal::Land { zone, .. }), _) => (
            None,
            crate::tower::runway_geometry(world, zone, false),
            Default::default(),
        ),
        _ => (None, None, Default::default()),
    };
    let wind = world
        .resource::<crate::Wind>()
        .map_or(Vec3::ZERO, |w| w.velocity);
    let Some(mut agent) = world.component_mut::<LocomotionAgent>(id)? else {
        return Ok(());
    };
    let position = state.transform.translation;
    // Brain adapter: a moving intent flies there (taking off first); brake/hover parks on the ground.
    // Game code's goal wins, then a brain's flight order (Take off, Land), then its movement.
    let goal = tower_goal
        .or(agent.flight)
        .or(agent.intent.flight)
        .unwrap_or_else(|| {
            let intent = agent.intent;
            let altitude = intent.altitude.unwrap_or(position.y.max(100.0));
            match intent.target.or_else(|| {
                (intent.velocity.length_squared() > 1e-4).then(|| position + intent.velocity * 10.0)
            }) {
                Some(point) if !(intent.brake || intent.hover) => FlightGoal::FlyTo {
                    point,
                    altitude,
                    speed: params.cruise_speed,
                },
                _ => FlightGoal::Park,
            }
        });
    let body = BodyInput {
        transform: state.transform,
        velocity: state.linear_velocity,
        angular_velocity: state.angular_velocity,
        mass: state.mass,
        half_extents,
        grounded,
        revision: state.revision,
        wind,
    };
    let output = agent
        .pilot
        .get_or_insert_with(Default::default)
        .step(params, goal, runway, gates, &body, dt);
    let telemetry = agent.flight_telemetry().unwrap_or_default();
    agent.activity.throttle = telemetry.throttle;
    agent.activity.brake = telemetry.brake;
    agent.activity.landed = !telemetry.airborne;
    world
        .add_force(id, output.force)
        .map_err(EngineError::Physics)?;
    world
        .add_torque(id, output.torque)
        .map_err(EngineError::Physics)?;
    Ok(())
}

/// One fixed tick of every locomotion agent, after brains.
pub(crate) fn step(world: &mut World, dt: Duration) -> Result<(), EngineError> {
    // Air traffic control first: it turns Arrive/Depart orders into this tick's goals.
    #[cfg(feature = "physics")]
    crate::tower::step(world)?;
    let mut ids = world.take_locomotion_ids();
    let seconds = dt.as_secs_f32();
    let result = (|| {
        for &id in &ids {
            let Some(model) = world
                .component::<LocomotionAgent>(id)?
                .map(|agent| agent.model)
            else {
                continue;
            };
            model
                .validate_physics(world.component::<crate::PhysicsBody>(id)?.copied())
                .map_err(EngineError::Physics)?;
            if model.is_physical() {
                if world.component::<crate::CharacterController>(id)?.is_some()
                    || world.component::<crate::Follow>(id)?.is_some()
                    || world.component::<crate::Spin>(id)?.is_some()
                    || world.component::<crate::LookAt>(id)?.is_some()
                    || world.component::<crate::FollowCamera>(id)?.is_some()
                {
                    return Err(EngineError::Physics(
                        crate::PhysicsError::LocomotionConflict,
                    ));
                }
                let scale = world.local_transform(id)?.scale;
                if scale.min_element() <= 0.0 || scale.x != scale.y || scale.x != scale.z {
                    return Err(EngineError::Physics(
                        crate::PhysicsError::UnsupportedTransform,
                    ));
                }
                #[cfg(feature = "physics")]
                {
                    let state = world.physics_body_state(id)?;
                    if let Some(mut agent) = world.component_mut::<LocomotionAgent>(id)? {
                        agent.observe_physics(state);
                    }
                    if let (Model::FixedWingPhysics(params), Some(state)) = (model, state) {
                        fly(world, id, &params, state, dt)?;
                    }
                }
                continue;
            }
            let global = *world.global_transform(id)?;
            let Some(mut agent) = world.component_mut::<LocomotionAgent>(id)? else {
                continue;
            };
            if !agent.state.oriented {
                agent.state.oriented = true;
                agent.activity.yaw =
                    yaw_of(global.matrix().transform_vector3(Vec3::NEG_Z)).unwrap_or(0.0);
            }
            agent.physical_state = None;
            let next = advance(&mut agent, global.translation(), seconds);
            let rotation = orientation(&agent.activity);
            let activity = agent.activity;
            let mut local = world.local_from_global_position(id, next)?;
            // Rotation is written in the parent's space; locomotion on nested objects turns approximately.
            local.rotation = rotation;
            world.set_local_transform(id, local)?;
            drive_wheels(world, id, activity, seconds)?;
        }
        Ok(())
    })();
    ids.clear();
    world.restore_locomotion_ids(ids);
    result
}

/// Publish committed physics observations before post-fixed observers run.
#[cfg(feature = "physics")]
pub(crate) fn observe_physics(world: &mut World) -> Result<(), EngineError> {
    let mut ids = world.take_locomotion_ids();
    let result = (|| {
        for &id in &ids {
            if !world
                .component::<LocomotionAgent>(id)?
                .is_some_and(|agent| agent.model.is_physical())
            {
                continue;
            }
            let state = world.physics_body_state(id)?;
            if let Some(mut agent) = world.component_mut::<LocomotionAgent>(id)? {
                agent.observe_physics(state);
            }
        }
        Ok(())
    })();
    ids.clear();
    world.restore_locomotion_ids(ids);
    result
}

/// Scripted courses the editor preview drives a profile along.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Course {
    /// A loop of four waypoints.
    Waypoints,
    /// Circle a point at 12 m.
    Orbit,
    /// Go 20 m ahead, then brake (or hover).
    Stop,
    /// Launch at a target 40 m away.
    Launch,
    /// Physical aircraft: take off, fly a circuit and land on a runway zone (others: waypoints).
    Circuit,
}

#[derive(Clone, Debug, Default)]
pub struct LocomotionTrace {
    pub frame_seconds: f32,
    /// Points the course aims at (waypoints, orbit centre, stop point, target).
    pub course: Vec<Vec3>,
    /// Position and activity per frame.
    pub frames: Vec<(Vec3, Activity)>,
    /// Flight phase per frame (physical aircraft only; empty otherwise).
    pub phases: Vec<crate::fixed_wing::FlightPhase>,
}

/// Runs a profile along a course in the normal engine tick.
pub fn simulate(
    profile: &LocomotionProfile,
    course: Course,
    seconds: f32,
) -> Result<LocomotionTrace, Box<dyn std::error::Error>> {
    if let Model::FixedWingPhysics(params) = profile.model {
        #[cfg(feature = "physics")]
        return simulate_aircraft(params, course, seconds);
        #[cfg(not(feature = "physics"))]
        {
            let _ = params;
            return Err("fixed_wing_physics previews need the physics feature".into());
        }
    }
    if profile.model.is_physical() {
        return Err(
            "drone_physics is an unpowered boundary; powered flight preview is not implemented yet"
                .into(),
        );
    }
    if !(0.1..=120.0).contains(&seconds) {
        return Err("preview runs for 0.1-120 seconds".into());
    }
    let mut engine = crate::Engine::new(crate::EngineConfig {
        fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
        ..crate::EngineConfig::default()
    })?;
    let flying = matches!(profile.model, Model::Drone { .. } | Model::FixedWing { .. });
    let altitude = match profile.model {
        Model::FixedWing { .. } => 25.0,
        Model::Drone { .. } => 6.0,
        _ => 0.0,
    };
    let scale = if matches!(profile.model, Model::FixedWing { .. }) {
        8.0
    } else {
        1.0
    };
    let points = match course {
        Course::Waypoints | Course::Circuit => vec![
            Vec3::new(15.0, altitude, -15.0) * Vec3::new(scale, 1.0, scale),
            Vec3::new(15.0, altitude, 15.0) * Vec3::new(scale, 1.0, scale),
            Vec3::new(-15.0, altitude, 15.0) * Vec3::new(scale, 1.0, scale),
            Vec3::new(-15.0, altitude, -15.0) * Vec3::new(scale, 1.0, scale),
        ],
        Course::Orbit => vec![Vec3::new(0.0, altitude, 0.0)],
        Course::Stop => vec![Vec3::new(0.0, altitude, -20.0 * scale)],
        Course::Launch => vec![Vec3::new(40.0, 0.0, 0.0)],
    };
    let start = match profile.model {
        Model::FixedWing { .. } => Vec3::new(-15.0 * scale, altitude, 0.0),
        _ => Vec3::ZERO,
    };
    let world = engine.world_mut();
    let id = world.spawn("locomotion preview");
    world.set_local_transform(id, Transform::from_translation(start))?;
    world.insert_component(id, LocomotionAgent::new(profile.model))?;
    struct Driver {
        id: EntityId,
        course: Course,
        points: Vec<Vec3>,
        next: usize,
        /// The Stop course keeps braking once the mark has been reached.
        stopped: bool,
        reach: f32,
        /// Speed the course asks for: cruise for aircraft, "as fast as you can" otherwise.
        speed: f32,
        flying: bool,
        altitude: f32,
        trace: LocomotionTrace,
    }
    impl crate::Game for Driver {
        fn fixed_update(&mut self, context: &mut crate::EngineContext<'_>) {
            let Ok(position) = context
                .world
                .global_transform(self.id)
                .map(|t| t.translation())
            else {
                return;
            };
            let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
            let mut intent = Intent {
                altitude: self.flying.then_some(self.altitude),
                ..Intent::default()
            };
            match self.course {
                Course::Waypoints | Course::Circuit => {
                    if flat(self.points[self.next] - position).length() < self.reach {
                        self.next = (self.next + 1) % self.points.len();
                    }
                    let target = self.points[self.next];
                    intent.velocity = flat(target - position).normalize_or_zero() * self.speed;
                    intent.target = Some(target);
                }
                Course::Orbit => {
                    let centre = self.points[0];
                    let offset = flat(position - centre);
                    let radial = offset.try_normalize().unwrap_or(Vec3::X);
                    let tangent = radial.cross(Vec3::Y);
                    let radius = 12.0 * self.reach / 3.0;
                    intent.velocity = (tangent
                        + radial * ((radius - offset.length()) / radius).clamp(-1.0, 1.0))
                    .normalize_or_zero()
                        * self.speed;
                    intent.target = Some(centre);
                }
                Course::Stop => {
                    let target = self.points[0];
                    let gap = flat(target - position);
                    self.stopped |= gap.length() < self.reach;
                    if self.stopped {
                        intent.brake = true;
                        intent.hover = true;
                    } else {
                        intent.velocity = gap.normalize_or_zero() * self.speed;
                    }
                    intent.target = Some(target);
                }
                Course::Launch => intent.target = Some(self.points[0]),
            }
            if let Ok(Some(mut agent)) = context.world.component_mut::<LocomotionAgent>(self.id) {
                agent.intent = intent;
            }
        }
        fn after_fixed_update(&mut self, context: &mut crate::EngineContext<'_>) {
            if context.tick.is_multiple_of(2) {
                let position = context
                    .world
                    .global_transform(self.id)
                    .map_or(Vec3::ZERO, |t| t.translation());
                let activity = context
                    .world
                    .component::<LocomotionAgent>(self.id)
                    .ok()
                    .flatten()
                    .map(LocomotionAgent::activity)
                    .unwrap_or_default();
                self.trace.frames.push((position, activity));
            }
        }
    }
    let mut driver = Driver {
        id,
        course,
        reach: if matches!(profile.model, Model::FixedWing { .. }) {
            30.0
        } else {
            3.0
        },
        speed: match profile.model {
            Model::FixedWing { cruise, .. } => cruise,
            _ => 1000.0,
        },
        points: points.clone(),
        next: 0,
        stopped: false,
        flying,
        altitude,
        trace: LocomotionTrace {
            frame_seconds: 2.0 / 60.0,
            course: points,
            frames: Vec::new(),
            phases: Vec::new(),
        },
    };
    engine.run_ticks(&mut driver, (seconds * 60.0) as u64)?;
    Ok(driver.trace)
}

/// Flies a physical aircraft over a 6 km field with a 900 × 30 m runway zone, by forces only.
/// Waypoints: a 2.4 km square at 150 m; Orbit: circle a point; Launch: take off and climb;
/// Stop: taxi 150 m and stop; Circuit: take off, fly the circuit and land on the runway.
#[cfg(feature = "physics")]
fn simulate_aircraft(
    params: crate::fixed_wing::FixedWingParams,
    course: Course,
    seconds: f32,
) -> Result<LocomotionTrace, Box<dyn std::error::Error>> {
    use crate::fixed_wing::FlightGoal;
    if !(0.1..=300.0).contains(&seconds) {
        return Err("aircraft previews run for 0.1-300 seconds".into());
    }
    let mut engine = crate::Engine::new(crate::EngineConfig {
        fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
        ..crate::EngineConfig::default()
    })?;
    let world = engine.world_mut();
    let body = |kind,
                half_extents: [f32; 3],
                friction,
                mass|
     -> Result<crate::PhysicsBody, crate::PhysicsError> {
        let mut body = crate::PhysicsBody::new(
            kind,
            crate::BoxCollider {
                center: [0.0; 3],
                half_extents,
            },
        )?;
        body.friction = friction;
        body.mass = mass;
        Ok(body)
    };
    let ground = world.spawn("preview ground");
    world.set_local_transform(
        ground,
        Transform::from_translation(Vec3::new(0.0, -1.0, 0.0)),
    )?;
    world.insert_component(
        ground,
        body(crate::BodyKind::Static, [3000.0, 1.0, 3000.0], 0.8, 1.0)?,
    )?;
    let runway = world.spawn("preview runway");
    world.set_local_transform(
        runway,
        Transform::from_translation(Vec3::new(0.0, 0.0, -300.0)),
    )?;
    world.insert_component(
        runway,
        crate::Zone {
            shape: crate::ZoneShape::Rect {
                width: 30.0,
                depth: 900.0,
            },
            height: 0.0,
            cost: 1.0,
            layers: Vec::new(),
        },
    )?;
    // A 1 t light aircraft, 10 m span and 8 m long, parked near the runway threshold.
    let id = world.spawn("aircraft preview");
    world.set_local_transform(id, Transform::from_translation(Vec3::new(0.0, 0.62, 100.0)))?;
    world.insert_component(
        id,
        body(crate::BodyKind::Dynamic, [5.0, 0.6, 4.0], 0.0, 1000.0)?,
    )?;
    world.insert_component(id, LocomotionAgent::new(Model::FixedWingPhysics(params)))?;
    let square = [
        Vec3::new(1200.0, 150.0, -1500.0),
        Vec3::new(1200.0, 150.0, 900.0),
        Vec3::new(-1200.0, 150.0, 900.0),
        Vec3::new(-1200.0, 150.0, -1500.0),
    ];
    let points = match course {
        Course::Waypoints => square.to_vec(),
        Course::Orbit => vec![Vec3::new(0.0, 150.0, -1500.0)],
        Course::Launch => vec![Vec3::new(0.0, 150.0, -1000.0)],
        Course::Stop => vec![Vec3::new(0.0, 0.0, -50.0)],
        // Runway threshold and far end.
        Course::Circuit => vec![Vec3::new(0.0, 0.0, 150.0), Vec3::new(0.0, 0.0, -750.0)],
    };
    struct Pilot {
        id: EntityId,
        runway: EntityId,
        course: Course,
        points: Vec<Vec3>,
        next: usize,
        trace: LocomotionTrace,
    }
    impl crate::Game for Pilot {
        fn fixed_update(&mut self, context: &mut crate::EngineContext<'_>) {
            let position = context
                .world
                .global_transform(self.id)
                .map_or(Vec3::ZERO, |t| t.translation());
            let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
            let goal = match self.course {
                Course::Waypoints => {
                    if flat(self.points[self.next] - position).length() < 150.0 {
                        self.next = (self.next + 1) % self.points.len();
                    }
                    FlightGoal::FlyTo {
                        point: self.points[self.next],
                        altitude: 150.0,
                        speed: 0.0,
                    }
                }
                Course::Orbit => FlightGoal::FlyTo {
                    point: self.points[0],
                    altitude: 150.0,
                    speed: 0.0,
                },
                Course::Launch => FlightGoal::TakeOff { altitude: 150.0 },
                Course::Stop => FlightGoal::Taxi {
                    stop: true,
                    point: self.points[0],
                },
                Course::Circuit => FlightGoal::Land {
                    zone: self.runway,
                    pattern_height: 300.0,
                },
            };
            if let Ok(Some(mut agent)) = context.world.component_mut::<LocomotionAgent>(self.id) {
                // Speed 0 asks for the profile's cruise speed (the pilot raises it to its minimum).
                agent.flight = Some(match goal {
                    FlightGoal::FlyTo {
                        point, altitude, ..
                    } => FlightGoal::FlyTo {
                        point,
                        altitude,
                        speed: match agent.model {
                            Model::FixedWingPhysics(p) => p.cruise_speed,
                            _ => 0.0,
                        },
                    },
                    other => other,
                });
            }
        }
        fn after_fixed_update(&mut self, context: &mut crate::EngineContext<'_>) {
            // 10 frames a second keeps a five-minute circuit small.
            if context.tick.is_multiple_of(6) {
                let position = context
                    .world
                    .global_transform(self.id)
                    .map_or(Vec3::ZERO, |t| t.translation());
                let agent = context
                    .world
                    .component::<LocomotionAgent>(self.id)
                    .ok()
                    .flatten();
                let activity = agent.map(LocomotionAgent::activity).unwrap_or_default();
                let phase = agent
                    .and_then(LocomotionAgent::flight_telemetry)
                    .map(|t| t.phase)
                    .unwrap_or_default();
                self.trace.frames.push((position, activity));
                self.trace.phases.push(phase);
            }
        }
    }
    let mut pilot = Pilot {
        id,
        runway,
        course,
        points: points.clone(),
        next: 0,
        trace: LocomotionTrace {
            frame_seconds: 6.0 / 60.0,
            course: points,
            frames: Vec::new(),
            phases: Vec::new(),
        },
    };
    engine.run_ticks(&mut pilot, (seconds * 60.0) as u64)?;
    Ok(pilot.trace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "physics")]
    #[test]
    fn physical_reconstruction_resets_history_but_ordinary_observation_preserves_it() {
        let mut agent = LocomotionAgent::new(Model::DronePhysics);
        agent.state.hold = Some(Vec3::ONE);
        agent.observe_physics(None);
        assert_eq!(
            agent.state.hold, None,
            "entering physics mode has no inherited motion history"
        );
        let mut state = crate::PhysicsBodyState {
            transform: Transform::IDENTITY,
            linear_velocity: Vec3::X,
            angular_velocity: Vec3::ZERO,
            mass: 1.0,
            gravity: crate::PHYSICS_GRAVITY,
            revision: 1,
        };
        agent.observe_physics(Some(state));
        agent.state.hold = Some(Vec3::ONE);
        agent.activity.rotor = 0.5;
        state.linear_velocity = Vec3::Z;
        agent.observe_physics(Some(state));
        assert_eq!(agent.state.hold, Some(Vec3::ONE));
        assert_eq!(agent.activity.rotor, 0.5);
        assert_eq!(agent.state.velocity, Vec3::Z);
        state.revision += 1;
        agent.observe_physics(Some(state));
        assert_eq!(agent.state.hold, None);
        assert_eq!(agent.activity.rotor, 0.0);
        agent.observe_physics(None);
        assert_eq!(agent.physics_state(), None);
        assert_eq!(agent.state.velocity, Vec3::ZERO);
    }

    #[test]
    fn launch_angles_reach_the_target_and_refuse_the_unreachable() {
        let low = launch_angle(40.0, 0.0, 25.0, GRAVITY, false).unwrap();
        let high = launch_angle(40.0, 0.0, 25.0, GRAVITY, true).unwrap();
        assert!(
            low < high && (low + high - std::f32::consts::FRAC_PI_2).abs() < 1e-3,
            "complementary arcs on flat ground"
        );
        let range = 25.0_f32.powi(2) * (2.0 * low).sin() / GRAVITY;
        assert!((range - 40.0).abs() < 0.05, "{range}");
        assert!(launch_angle(400.0, 0.0, 25.0, GRAVITY, false).is_none());
    }

    #[test]
    fn heading_conventions_round_trip() {
        for yaw in [-2.5_f32, -0.5, 0.0, 1.0, 3.0] {
            assert!((yaw_of(forward(yaw)).unwrap() - yaw).abs() < 1e-5);
            let rotated = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
            assert!(
                rotated.abs_diff_eq(forward(yaw), 1e-5),
                "orientation matches forward()"
            );
        }
    }
}
