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
                !v.is_empty() && v.len() <= 64 && v.iter().all(|id| text(id, 128))
            }),
            FieldKind::Text => text(value, 64),
            FieldKind::Choice => value.as_str().is_some_and(|s| field.options.contains(&s)),
            FieldKind::Brain => value
                .as_str()
                .is_some_and(|s| s.ends_with(".brain.toml") && s.len() <= 512),
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
                    FieldKind::Entities => "a list of 1-64 instance IDs".to_string(),
                    FieldKind::Text => "1-64 printable characters".to_string(),
                    FieldKind::Choice => format!("one of {}", field.options.join(", ")),
                    FieldKind::Brain => "the path of a .brain.toml asset".to_string(),
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
}

/// Durable ID of the built-in light component.
pub const LIGHT: &str = "emergent.light";
pub const CHARACTER_CONTROLLER: &str = "emergent.character_controller";
pub const SPIN: &str = "emergent.spin";
pub const FOLLOW: &str = "emergent.follow";
pub const GROUP: &str = "emergent.group";
pub const BRAIN: &str = "emergent.brain";

impl ComponentRegistry {
    pub fn builtin() -> Self {
        use FieldKind::*;
        let f = ComponentField::new;
        Self {
            types: vec![
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
                    id: CHARACTER_CONTROLLER,
                    label: "Character controller",
                    description: "WASD or arrow keys move the object; Space jumps. A dynamic physics body is pushed and keeps falling; otherwise the object slides.",
                    applies_to: "any",
                    fields: vec![
                        f("speed", "Speed", Number).range(0.0, 100.0).unit("m/s").default(4.0).help("Horizontal movement speed."),
                        f("jump_speed", "Jump speed", Number).range(0.0, 50.0).unit("m/s").default(5.0).help("Upward speed of a jump; 0 turns jumping off. Needs a dynamic physics body."),
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
                        f("max_speed", "Max speed", Number).range(0.0, 100.0).unit("m/s").default(3.0),
                        f("acceleration", "Acceleration", Number).range(0.0, 1000.0).unit("m/s²").default(8.0).help("How quickly it can change velocity; lower turns more smoothly."),
                        f("planar", "Ground only", Bool).default(false).help("Ignore vertical steering (objects without physics that walk on the ground)."),
                        f("face_movement", "Face movement", Bool).default(true).help("Turn to face the direction of travel (not for dynamic physics bodies)."),
                    ],
                },
            ],
        }
    }

    /// Adds a project-defined type. IDs must start with `project.` and be unique.
    pub fn register(&mut self, component: ComponentType) -> Result<(), String> {
        let id = component.id;
        let valid_id = id.strip_prefix("project.").is_some_and(|rest| {
            !rest.is_empty()
                && rest
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        });
        if !valid_id {
            return Err(format!(
                "component type '{id}' must look like project.my_type"
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
