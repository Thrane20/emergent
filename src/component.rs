//! Authored component types: durable type IDs and field schemas shared by the scene
//! loader, the IDE inspector and tools. Runtime state stays with the owning subsystem;
//! nothing here is keyed by or serialises SlotMap handles.
use serde::Serialize;

/// The value shape of one authored field.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind {
    Number,
    /// Linear RGB, three channels in 0..=1.
    Colour,
    Bool,
    /// Three numbers; `min`/`max` bound each axis.
    Vector3,
    /// The instance ID of another object in the same scene.
    Entity,
    /// Instance IDs in order (waypoints).
    Entities,
    /// Free text up to 64 characters (a group name).
    Text,
    /// One of the field's `options`.
    Choice,
    /// Path of a `.brain.toml` asset.
    Brain,
    /// Path of a `.locomotion.toml` asset.
    Locomotion,
    /// Ordered paths of `.brain.toml` assets.
    Brains,
    /// Path of a `.audio.toml` clip asset.
    Audio,
    /// A table of up to 32 name = value pairs; values are text, numbers or true/false.
    Pairs,
}

#[derive(Clone, Debug, Serialize)]
pub struct ComponentField {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<&'static str>,
    /// Object variants (for a light: its type) that accept this field; empty means all.
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    pub variants: &'static [&'static str],
    /// Allowed values of a `Choice` field.
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    pub options: &'static [&'static str],
    /// Value used when the field is not authored. `None` means it comes from the asset or is required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<toml::Value>,
    pub help: &'static str,
}

impl ComponentField {
    pub fn new(name: &'static str, label: &'static str, kind: FieldKind) -> Self {
        Self {
            name,
            label,
            kind,
            min: None,
            max: None,
            unit: None,
            variants: &[],
            options: &[],
            default: None,
            help: "",
        }
    }
    pub fn range(mut self, min: f32, max: f32) -> Self {
        (self.min, self.max) = (Some(min), Some(max));
        self
    }
    pub fn unit(mut self, unit: &'static str) -> Self {
        self.unit = Some(unit);
        self
    }
    pub fn variants(mut self, variants: &'static [&'static str]) -> Self {
        self.variants = variants;
        self
    }
    pub fn options(mut self, options: &'static [&'static str]) -> Self {
        self.options = options;
        self
    }
    pub fn default(mut self, value: impl Into<toml::Value>) -> Self {
        self.default = Some(value.into());
        self
    }
    pub fn help(mut self, help: &'static str) -> Self {
        self.help = help;
        self
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ComponentType {
    /// Durable ID written to scene files, e.g. `emergent.light`.
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    /// Object kind the type attaches to (`light`), or `any`.
    pub applies_to: &'static str,
    pub fields: Vec<ComponentField>,
}

/// One ID segment: non-empty lowercase ASCII, digits and `_`.
pub(crate) fn segment(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn number(value: &toml::Value) -> Option<f32> {
    match value {
        toml::Value::Float(n) => Some(*n as f32),
        toml::Value::Integer(n) => Some(*n as f32),
        _ => None,
    }
}

impl ComponentType {
    /// Checks names, value shapes and schema ranges; owners still validate semantics.
    pub fn validate(&self, fields: &toml::Table) -> Result<(), String> {
        validate_fields(self.id, &self.fields, fields, self.applies_to == "any")
    }

    /// Authored values with schema defaults, for a table that passed `validate`.
    pub fn values<'a>(&'a self, fields: &'a toml::Table) -> ComponentValues<'a> {
        ComponentValues::new(&self.fields, fields)
    }
}

/// Checks a table of authored values against a field schema. With `require_defaultless`,
/// fields that have no default must be present.
pub fn validate_fields(
    owner: &str,
    schema: &[ComponentField],
    fields: &toml::Table,
    require_defaultless: bool,
) -> Result<(), String> {
    let text = |value: &toml::Value, limit: usize| {
        value.as_str().is_some_and(|s| {
            !s.trim().is_empty() && s.len() <= limit && !s.chars().any(char::is_control)
        })
    };
    for (name, value) in fields {
        let field = schema
            .iter()
            .find(|field| field.name == name)
            .ok_or_else(|| format!("{owner} has no field '{name}'"))?;
        let in_range = |n: f32| {
            n.is_finite()
                && field.min.is_none_or(|min| n >= min)
                && field.max.is_none_or(|max| n <= max)
        };
        let triple = |check: &dyn Fn(f32) -> bool| {
            value
                .as_array()
                .is_some_and(|v| v.len() == 3 && v.iter().all(|c| number(c).is_some_and(check)))
        };
        let valid = match field.kind {
            FieldKind::Bool => value.is_bool(),
            FieldKind::Number => number(value).is_some_and(in_range),
            FieldKind::Colour => triple(&|n| (0.0..=1.0).contains(&n)),
            FieldKind::Vector3 => triple(&in_range),
            FieldKind::Entity => text(value, 128),
            FieldKind::Entities => value.as_array().is_some_and(|v| {
                (!v.is_empty()
                    || field
                        .default
                        .as_ref()
                        .and_then(toml::Value::as_array)
                        .is_some_and(Vec::is_empty))
                    && v.len() <= 64
                    && v.iter().all(|id| text(id, 128))
            }),
            FieldKind::Text => text(value, 64),
            FieldKind::Choice => value.as_str().is_some_and(|s| field.options.contains(&s)),
            FieldKind::Brain => value
                .as_str()
                .is_some_and(|s| s.ends_with(".brain.toml") && s.len() <= 512),
            FieldKind::Brains => value.as_array().is_some_and(|v| {
                v.len() <= 8
                    && v.iter().all(|path| {
                        path.as_str()
                            .is_some_and(|s| s.ends_with(".brain.toml") && s.len() <= 512)
                    })
            }),
            FieldKind::Locomotion => value
                .as_str()
                .is_some_and(|s| s.ends_with(".locomotion.toml") && s.len() <= 512),
            FieldKind::Audio => value
                .as_str()
                .is_some_and(|s| s.ends_with(".audio.toml") && s.len() <= 512),
            FieldKind::Pairs => value.as_table().is_some_and(|table| {
                table.len() <= crate::map::MAP_MAX_PAIRS
                    && table.iter().all(|(key, value)| {
                        !key.trim().is_empty()
                            && key.len() <= 64
                            && !key.chars().any(char::is_control)
                            && match value {
                                toml::Value::String(text) => {
                                    text.len() <= 256 && !text.chars().any(char::is_control)
                                }
                                toml::Value::Integer(_) | toml::Value::Boolean(_) => true,
                                toml::Value::Float(n) => n.is_finite(),
                                _ => false,
                            }
                    })
            }),
        };
        if !valid {
            let bounds = || {
                format!(
                    "{}..{}",
                    field.min.map_or("-inf".into(), |n| n.to_string()),
                    field.max.map_or("inf".into(), |n| n.to_string())
                )
            };
            return Err(format!(
                "{owner}.{name} must be {}",
                match field.kind {
                    FieldKind::Bool => "true or false".to_string(),
                    FieldKind::Colour => "three numbers in 0..1".to_string(),
                    FieldKind::Number => format!("a number in {}", bounds()),
                    FieldKind::Vector3 => format!("three numbers in {}", bounds()),
                    FieldKind::Entity => "the ID of an instance in this scene".to_string(),
                    FieldKind::Entities => if field.default.as_ref().and_then(toml::Value::as_array).is_some_and(Vec::is_empty) { "a list of 0-64 instance IDs" } else { "a list of 1-64 instance IDs" }.to_string(),
                    FieldKind::Text => "1-64 printable characters".to_string(),
                    FieldKind::Choice => format!("one of {}", field.options.join(", ")),
                    FieldKind::Brain => "the path of a .brain.toml asset".to_string(),
                    FieldKind::Locomotion => "the path of a .locomotion.toml asset".to_string(),
                    FieldKind::Brains => "a list of up to 8 .brain.toml paths".to_string(),
                    FieldKind::Audio => "the path of a .audio.toml asset".to_string(),
                    FieldKind::Pairs => "a table of up to 32 name = value pairs (names up to 64 characters; values text up to 256 characters, numbers or true/false)".to_string(),
                }
            ));
        }
    }
    if require_defaultless {
        for field in schema {
            if field.default.is_none() && !fields.contains_key(field.name) {
                return Err(format!("{owner} requires field '{}'", field.name));
            }
        }
    }
    Ok(())
}

/// Typed reads of a validated component table, falling back to schema defaults.
pub struct ComponentValues<'a> {
    schema: &'a [ComponentField],
    fields: &'a toml::Table,
}

impl<'a> ComponentValues<'a> {
    pub fn new(schema: &'a [ComponentField], fields: &'a toml::Table) -> Self {
        Self { schema, fields }
    }
    fn value(&self, name: &str) -> Option<&toml::Value> {
        self.fields.get(name).or_else(|| {
            self.schema
                .iter()
                .find(|f| f.name == name)
                .and_then(|f| f.default.as_ref())
        })
    }
    pub fn number(&self, name: &str) -> Option<f32> {
        self.value(name).and_then(number)
    }
    pub fn bool(&self, name: &str) -> Option<bool> {
        self.value(name).and_then(toml::Value::as_bool)
    }
    pub fn vector3(&self, name: &str) -> Option<[f32; 3]> {
        let values = self.value(name)?.as_array()?;
        Some([
            number(values.first()?)?,
            number(values.get(1)?)?,
            number(values.get(2)?)?,
        ])
    }
    /// Entity, text, choice and brain fields.
    pub fn entity(&self, name: &str) -> Option<&str> {
        self.value(name).and_then(toml::Value::as_str)
    }
    /// Pairs fields.
    pub fn table(&self, name: &str) -> Option<&toml::Table> {
        self.value(name).and_then(toml::Value::as_table)
    }
    pub fn entities(&self, name: &str) -> Vec<&str> {
        self.value(name)
            .and_then(toml::Value::as_array)
            .map(|v| v.iter().filter_map(toml::Value::as_str).collect())
            .unwrap_or_default()
    }
}

/// Known component types: engine built-ins plus any the project registers.
#[derive(Clone, Debug, Serialize)]
pub struct ComponentRegistry {
    types: Vec<ComponentType>,
    /// Plugin brain nodes and locomotion types; exported through their own catalogues.
    #[serde(skip)]
    pub(crate) brain_nodes: Vec<(
        crate::brain::NodeSpec,
        crate::plugin::Hook<dyn crate::plugin::BrainNode>,
    )>,
    #[serde(skip)]
    pub(crate) locomotion: Vec<(
        crate::locomotion::LocomotionType,
        crate::plugin::Hook<dyn crate::plugin::LocomotionModel>,
    )>,
}

/// Durable ID of the built-in light component.
pub const LIGHT: &str = "emergent.light";
pub const CHARACTER_CONTROLLER: &str = "emergent.character_controller";
pub const SPIN: &str = "emergent.spin";
pub const FOLLOW: &str = "emergent.follow";
pub const GROUP: &str = "emergent.group";
pub const BRAIN: &str = "emergent.brain";
pub const LOCOMOTION: &str = "emergent.locomotion";
pub const LOOK_AT: &str = "emergent.look_at";
pub const FOLLOW_CAMERA: &str = "emergent.follow_camera";
pub const AUDIO_SOURCE: &str = "emergent.audio_source";
pub const AUDIO_LISTENER: &str = "emergent.audio_listener";
pub const WAYPOINT: &str = "emergent.waypoint";
pub const ZONE: &str = "emergent.zone";
pub const METADATA: &str = "emergent.metadata";

impl ComponentRegistry {
    pub fn builtin() -> Self {
        use FieldKind::*;
        let f = ComponentField::new;
        Self {
            types: vec![
                ComponentType {id:"emergent.ui",label:"UI surface",description:"A rendered screen overlay or entity-local world panel.",applies_to:"any",fields:vec![
                    f("ui","UI asset",Text).help("Project-relative .ui.toml path."),
                    f("space","Space",Choice).options(&["screen","world"]).default("screen"),
                    f("width","Width",Number).range(0.0,1e6).default(0.0).help("Pixels for screen, metres for world. Zero: viewport or intrinsic aspect."),
                    f("height","Height",Number).range(0.0,1e6).default(0.0),
                    f("x","Screen X",Number).range(-1e6,1e6).default(0.0),f("y","Screen Y",Number).range(-1e6,1e6).default(0.0),
                ]},

                ComponentType {
                    id: LIGHT,
                    label: "Light",
                    description: "Colour, brightness and reach of a light. Its direction is the object's rotation.",
                    applies_to: "light",
                    fields: vec![
                        f("color", "Colour", Colour).help("Linear RGB."),
                        f("intensity", "Intensity", Number).range(0.0, 1000.0).help("Brightness multiplier."),
                        f("range", "Range", Number).range(0.01, 10000.0).unit("m").variants(&["point", "spot"]).help("Distance where the light fades to zero."),
                        f("inner_half_angle", "Inner cone", Number).range(0.0, 1.57).unit("rad").variants(&["spot"]).help("Full brightness inside this half-angle."),
                        f("outer_half_angle", "Outer cone", Number).range(0.0, 1.57).unit("rad").variants(&["spot"]).help("No light beyond this half-angle."),
                        f("casts_shadows", "Casts shadows", Bool).variants(&["directional", "point", "spot"]).help("Shadow maps cost GPU time."),
                    ],
                },
                ComponentType {
                    id:"emergent.mouse_look",label:"Mouse look",description:"First-person look on the active camera. Click Run window to capture; Escape releases.",applies_to:"camera",
                    fields:vec![f("sensitivity","Sensitivity",Number).range(0.001,5.0).unit("degrees/pixel").default(0.12),f("pitch_limit","Pitch limit",Number).range(1.0,89.0).unit("degrees").default(85.0)],
                },
                ComponentType {
                    id: CHARACTER_CONTROLLER,
                    label: "Character controller",
                    description: "WASD or arrow keys move the object; Space jumps. A dynamic physics body is pushed and keeps falling; otherwise the object slides.",
                    applies_to: "any",
                    fields: vec![
                        f("speed", "Speed", Number).range(0.0, 100.0).unit("m/s").default(4.0).help("Horizontal movement speed."),
                        f("jump_speed", "Jump speed", Number).range(0.0, 50.0).unit("m/s").default(5.0).help("Upward speed of a jump; 0 turns jumping off. Requires a dynamic body with supporting contact."),
                        f("camera_relative", "Relative to camera", Bool).default(true).help("W moves where the active camera looks. Off: W moves along world -Z."),
                        f("force", "Drive force", Number).range(0.0, 1_000_000.0).unit("N").default(0.0).help("Pushes a dynamic body: acceleration = force ÷ mass, minus friction × 9.81. Released keys coast under friction. Must exceed friction × mass × 9.81 to move. 0 = instant start and stop."),
                    ],
                },
                ComponentType {
                    id: SPIN,
                    label: "Spin",
                    description: "Rotates the object continuously around one of its own axes.",
                    applies_to: "any",
                    fields: vec![
                        f("axis", "Axis", Vector3).range(-1.0, 1.0).default(toml::Value::Array(vec![0.0.into(), 1.0.into(), 0.0.into()])).help("Local axis; it is normalised."),
                        f("degrees_per_second", "Speed", Number).range(-3600.0, 3600.0).unit("°/s").default(45.0).help("Negative spins the other way."),
                    ],
                },
                ComponentType {
                    id: FOLLOW,
                    label: "Follow",
                    description: "Moves the object towards another object until it is within the stop distance.",
                    applies_to: "any",
                    fields: vec![
                        f("target", "Target", Entity).help("The object to follow, in this scene."),
                        f("speed", "Speed", Number).range(0.0, 100.0).unit("m/s").default(2.0).help("Maximum approach speed."),
                        f("stop_distance", "Stop distance", Number).range(0.0, 1000.0).unit("m").default(1.0).help("Stops this far from the target."),
                    ],
                },
                ComponentType {
                    id: GROUP,
                    label: "Group",
                    description: "Membership that brains use for targets (nearest wolf) and neighbourhoods (flock with sheep).",
                    applies_to: "any",
                    fields: vec![f("name", "Group", Text).help("For example sheep, wolves or guards.")],
                },
                ComponentType {
                    id: BRAIN,
                    label: "Brain",
                    description: "Runs a saved brain from the Brain editor: its node tree steers this object every tick.",
                    applies_to: "any",
                    fields: vec![
                        f("brain", "Brain", Brain).help("A .brain.toml asset."),
                        f("stack", "Also run", Brains).default(toml::Value::Array(vec![])).help("Further brains run together with the main one: movement adds up, the first look wins, every signal is sent."),
                        f("max_speed", "Max speed", Number).range(0.0, 100.0).unit("m/s").default(3.0),
                        f("acceleration", "Acceleration", Number).range(0.0, 1000.0).unit("m/s²").default(8.0).help("How quickly it can change velocity; lower turns more smoothly."),
                        f("planar", "Ground only", Bool).default(false).help("Ignore vertical steering (objects without physics that walk on the ground)."),
                        f("face_movement", "Face movement", Bool).default(true).help("Turn to face the direction of travel (not for dynamic physics bodies)."),
                        f("turn_rate", "Turn rate", Number).range(0.0, 3600.0).unit("°/s").default(360.0).help("How fast a brain's look turns the object; 0 snaps."),
                    ],
                },
                ComponentType {
                    id: LOCOMOTION,
                    label: "Locomotion",
                    description: "How this object moves: a saved profile from the Locomotion editor (wheels, drone, aircraft, projectile …) carries out its brain's intent within physical limits.",
                    applies_to: "any",
                    fields: vec![
                        f("profile", "Profile", Locomotion).help("A .locomotion.toml asset."),
                        f("steering_wheels", "Steering wheels", Entities).default(toml::Value::Array(vec![])).help("Child objects that turn with the steering and roll (usually the front wheels)."),
                        f("rolling_wheels", "Rolling wheels", Entities).default(toml::Value::Array(vec![])).help("Child objects that only roll (usually the rear wheels)."),
                        f("wheel_radius", "Wheel radius", Number).range(0.01, 20.0).unit("m").default(0.35).help("Sets how fast the wheels roll for the ground speed."),
                    ],
                },
                ComponentType {
                    id: LOOK_AT,
                    label: "Look at",
                    description: "Turns the object to face a target every tick: a camera tracking a player, a turret, a head. Runs after movement.",
                    applies_to: "any",
                    fields: vec![
                        f("target", "Target", Entity).help("The object to face."),
                        f("offset", "Aim offset", Vector3).range(-1000.0, 1000.0).unit("m").default(toml::Value::Array(vec![0.0.into(), 0.0.into(), 0.0.into()])).help("Added to the target's position, e.g. aim above its feet."),
                        f("turn_rate", "Turn rate", Number).range(0.0, 3600.0).unit("°/s").default(0.0).help("0 turns instantly; lower values swing smoothly."),
                    ],
                },
                ComponentType {
                    id: FOLLOW_CAMERA,
                    label: "Follow camera",
                    description: "Keeps the object (usually a camera) at an offset from a target and smoothly follows it, looking at it by default: a chase or overhead camera.",
                    applies_to: "any",
                    fields: vec![
                        f("target", "Target", Entity).help("The object to follow."),
                        f("offset", "Offset", Vector3).range(-10000.0, 10000.0).unit("m").default(toml::Value::Array(vec![0.0.into(), 3.0.into(), 8.0.into()])).help("Behind: right, up, back from the target's heading. Fixed: world x, y, z."),
                        f("mode", "Mode", Choice).options(&["behind", "fixed"]).default("behind").help("Behind turns with the target; fixed keeps a world offset (top-down, isometric)."),
                        f("smoothing", "Smoothing", Number).range(0.0, 10.0).unit("s").default(0.3).help("Seconds to close most of the gap; 0 snaps."),
                        f("look_at", "Look at target", Bool).default(true),
                    ],
                },
                ComponentType {
                    id: AUDIO_SOURCE,
                    label: "Audio source",
                    description: "Plays a clip from this object (use a Point for a fixed spot), heard from the audio listener with distance falloff, stereo position and Doppler.",
                    applies_to: "any",
                    fields: vec![
                        f("clip", "Clip", Audio).help("A .audio.toml asset; its category picks the mix bus."),
                        f("mode", "Mode", Choice).options(&["loop", "one_shot"]).default("loop").help("Loop repeats between the clip's loop points; one shot plays once."),
                        f("autoplay", "Play on start", Bool).default(true),
                        f("volume", "Volume", Number).range(0.0, 4.0).default(1.0),
                        f("pitch", "Pitch", Number).range(0.1, 4.0).default(1.0).help("Playback speed; 2 is an octave up."),
                        f("spatial", "3D", Number).range(0.0, 1.0).default(1.0).help("1 is positioned in the world; 0 plays flat like music or UI."),
                        f("min_distance", "Full volume within", Number).range(0.01, 10000.0).unit("m").default(1.0),
                        f("max_distance", "Silent beyond", Number).range(0.02, 100000.0).unit("m").default(50.0).help("Must be larger than full volume within."),
                        f("rolloff", "Falloff", Choice).options(&["inverse", "linear"]).default("inverse").help("Inverse sounds natural; linear fades evenly."),
                        f("doppler", "Doppler", Number).range(0.0, 10.0).default(1.0).help("Pitch shift from motion toward or away from the listener; 0 turns it off."),
                        f("reverb", "Reverb send", Number).range(0.0, 1.0).default(0.3).help("How much of this sound feeds the listener's room reverb (heard only when the listener has a room)."),
                        f("play_on", "Play on signal", Text).default("").help("A brain signal name (e.g. fire): plays each time this object's brain starts sending it, or another brain sends it at this object. Usually with Play on start off."),
                    ],
                },
                ComponentType {
                    id: WAYPOINT,
                    label: "Waypoint",
                    description: "A point in a waypoint network. Links lead to other waypoints; games and brains find routes through the network, weighted by zone costs.",
                    applies_to: "any",
                    fields: vec![
                        f("links", "Links", Entities).default(toml::Value::Array(vec![])).help("Waypoints this one leads to."),
                        f("two_way", "Two-way links", Bool).default(true).help("Off: links can only be travelled from this waypoint."),
                        f("radius", "Arrival radius", Number).range(0.0, 1000.0).unit("m").default(1.0),
                    ],
                },
                ComponentType {
                    id: ZONE,
                    label: "Zone",
                    description: "A circular or rectangular area around the object. Games query which zones contain a point; its cost makes agents prefer or avoid it.",
                    applies_to: "any",
                    fields: vec![
                        f("shape", "Shape", Choice).options(&["circle", "rect"]).default("circle"),
                        f("radius", "Radius", Number).range(0.01, 100000.0).unit("m").default(5.0).help("Circle zones."),
                        f("width", "Width", Number).range(0.01, 100000.0).unit("m").default(10.0).help("Rectangle zones, along the object's X; turns with its rotation."),
                        f("depth", "Depth", Number).range(0.01, 100000.0).unit("m").default(10.0).help("Rectangle zones, along the object's Z."),
                        f("height", "Height", Number).range(0.0, 100000.0).unit("m").default(0.0).help("How far the zone reaches up from the object; 0 is unlimited."),
                        f("cost", "Travel cost", Number).range(0.01, 1000.0).default(1.0).help("1 is normal; below 1 is preferred (0.5 is half the cost); above 1 is avoided unless it saves enough distance."),
                        f("blocked", "Blocked", Bool).default(false).help("Never entered by the layers it applies to."),
                        f("layers", "Layers", Text).default("").help("Comma-separated, e.g. air, ground. Empty applies to everything."),
                    ],
                },
                ComponentType {
                    id: METADATA,
                    label: "Metadata",
                    description: "Name and value pairs (type = homebase, team = 2) that games query at runtime.",
                    applies_to: "any",
                    fields: vec![f("values", "Values", Pairs).default(toml::Value::Table(toml::Table::new())).help("Up to 32 pairs; values are text, numbers or true/false.")],
                },
                ComponentType {
                    id: AUDIO_LISTENER,
                    label: "Audio listener",
                    description: "Hears the scene from this object, usually the camera. Without one, the active camera listens.",
                    applies_to: "any",
                    fields: vec![
                        f("gain", "Volume", Number).range(0.0, 4.0).default(1.0).help("Master volume."),
                        f("reverb_room", "Reverb room", Number).range(0.0, 1.0).default(0.0).help("0 is no reverb; up to 1 for a large hall. Sources choose how much they send."),
                        f("reverb_damping", "Reverb damping", Number).range(0.0, 1.0).default(0.5).help("Higher sounds softer and more muffled."),
                        f("ducking", "Music ducking", Number).range(-40.0, 0.0).unit("dB").default(-9.0).help("Music drops by this much while speech plays; 0 turns it off."),
                    ],
                },
            ],
            brain_nodes: Vec::new(),
            locomotion: Vec::new(),
        }
    }

    /// Adds a project-defined type. IDs must start with `project.` and be unique.
    pub fn register(&mut self, component: ComponentType) -> Result<(), String> {
        self.register_in("project", component)
    }

    /// Adds a type owned by `namespace` (`project` or a plugin id); its ID must be
    /// `<namespace>.<name>` with `name` in lowercase ASCII, digits and `_`.
    pub(crate) fn register_in(
        &mut self,
        namespace: &str,
        component: ComponentType,
    ) -> Result<(), String> {
        let id = component.id;
        let valid_id = id
            .strip_prefix(namespace)
            .and_then(|rest| rest.strip_prefix('.'))
            .is_some_and(segment);
        if !valid_id {
            return Err(format!(
                "component type '{id}' must look like {namespace}.my_type"
            ));
        }
        if self.get(id).is_some() {
            return Err(format!("component type '{id}' is already registered"));
        }
        if component.applies_to != "any" {
            return Err(format!("project component '{id}' must apply to any object"));
        }
        self.types.push(component);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ComponentType> {
        self.types.iter().find(|t| t.id == id)
    }

    pub fn types(&self) -> &[ComponentType] {
        &self.types
    }

    /// Registry/schema export for editors and AI tools.
    pub fn export_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_schema_rejects_unknown_fields_wrong_shapes_and_out_of_range_values() {
        let registry = ComponentRegistry::builtin();
        let light = registry.get(LIGHT).unwrap();
        let table = |source: &str| source.parse::<toml::Table>().unwrap();
        assert!(light
            .validate(&table(
                "color = [1.0, 0.5, 0]\nintensity = 3\ncasts_shadows = true"
            ))
            .is_ok());
        assert!(light
            .validate(&table("colour = [1.0, 1.0, 1.0]"))
            .unwrap_err()
            .contains("no field"));
        assert!(light.validate(&table("color = [1.0, 1.0]")).is_err());
        assert!(light.validate(&table("color = [2.0, 1.0, 1.0]")).is_err());
        assert!(light.validate(&table("intensity = -1.0")).is_err());
        assert!(light.validate(&table("intensity = nan")).is_err());
        assert!(light.validate(&table("casts_shadows = 1")).is_err());
        let export = registry.export_toml().unwrap();
        assert!(export.contains("id = \"emergent.light\""));
        assert!(export.contains("variants = [\"spot\"]"));
    }

    #[test]
    fn explicit_empty_optional_entity_lists_match_defaults() {
        let registry = ComponentRegistry::builtin();
        let locomotion = registry.get("emergent.locomotion").unwrap();
        let check = |field: &str, value: toml::Value| {
            validate_fields(
                locomotion.id,
                &locomotion.fields,
                &toml::Table::from_iter([(field.into(), value)]),
                false,
            )
        };
        for field in ["steering_wheels", "rolling_wheels"] {
            assert!(check(field, toml::Value::Array(vec![])).is_ok());
            assert!(check(field, toml::Value::Array(vec![toml::Value::Integer(1)])).is_err());
            assert!(check(
                field,
                toml::Value::Array(vec![toml::Value::String("wheel".into()); 65])
            )
            .is_err());
        }
        let required = ComponentType {
            id: "project.required",
            label: "Required",
            description: "",
            applies_to: "any",
            fields: vec![ComponentField::new(
                "targets",
                "Targets",
                FieldKind::Entities,
            )],
        };
        assert!(required
            .validate(&"targets=[]".parse::<toml::Table>().unwrap())
            .is_err());
    }

    #[test]
    fn defaults_required_fields_vectors_and_project_registration() {
        let mut registry = ComponentRegistry::builtin();
        let table = |source: &str| source.parse::<toml::Table>().unwrap();
        let follow = registry.get(FOLLOW).unwrap();
        assert!(follow
            .validate(&table(""))
            .unwrap_err()
            .contains("requires field 'target'"));
        let authored = table("target = 'leader'");
        assert!(follow.validate(&authored).is_ok());
        let values = follow.values(&authored);
        assert_eq!(
            (values.entity("target"), values.number("speed")),
            (Some("leader"), Some(2.0))
        );
        let spin = registry.get(SPIN).unwrap();
        assert!(spin.validate(&table("axis = [0, 2, 0]")).is_err());
        assert_eq!(
            spin.values(&table("")).vector3("axis"),
            Some([0.0, 1.0, 0.0])
        );
        let custom = || ComponentType {
            id: "project.jumping_bean",
            label: "Jumping bean",
            description: "",
            applies_to: "any",
            fields: vec![ComponentField::new("height", "Height", FieldKind::Number).default(1.0)],
        };
        registry.register(custom()).unwrap();
        assert!(registry.register(custom()).unwrap_err().contains("already"));
        assert!(registry
            .register(ComponentType {
                id: "emergent.sneaky",
                ..custom()
            })
            .is_err());
        assert!(registry
            .export_toml()
            .unwrap()
            .contains("project.jumping_bean"));
    }
}
