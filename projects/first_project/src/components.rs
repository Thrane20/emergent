//! Project-defined component types. To add one: describe its fields in `registry`, then
//! build its runtime in `ProjectComponents::attach` and advance it in `fixed_update`.
//! Scenes attach it with `[[entities.components]] type = "project.<name>"`.
use std::time::Duration;

use emergent::{ComponentField, ComponentRegistry, ComponentType, FieldKind, LoadedScene, World};

use crate::bounce::{PeriodicPop, DEFAULT_POP_IMPULSE, DEFAULT_POP_INTERVAL};

pub const JUMPING_BEAN: &str = "project.jumping_bean";

/// Engine built-ins plus this project's types; used for loading, validation and the IDE export.
pub fn registry() -> ComponentRegistry {
    let mut registry = ComponentRegistry::builtin();
    registry
        .register(ComponentType {
            id: JUMPING_BEAN,
            label: "Jumping bean",
            description: "Pops a dynamic physics body upwards at a fixed interval.",
            applies_to: "any",
            fields: vec![
                ComponentField::new("interval", "Interval", FieldKind::Number)
                    .range(0.1, 600.0)
                    .unit("s")
                    .default(f64::from(DEFAULT_POP_INTERVAL.as_secs_f32()))
                    .help("Time between pops."),
                ComponentField::new("impulse", "Impulse", FieldKind::Number)
                    .range(0.0, 10000.0)
                    .unit("N·s")
                    .default(f64::from(DEFAULT_POP_IMPULSE))
                    .help("Upward impulse of each pop."),
            ],
        })
        .expect("project component types are valid and unique");
    registry
}

/// Runtime state for the project components in a loaded scene.
#[derive(Default)]
pub struct ProjectComponents {
    jumping_beans: Vec<PeriodicPop>,
}

impl ProjectComponents {
    pub fn attach(loaded: &LoadedScene) -> Result<Self, String> {
        let registry = registry();
        let mut components = Self::default();
        for component in &loaded.components {
            let schema = registry
                .get(&component.type_id)
                .ok_or_else(|| format!("no runtime for component '{}'", component.type_id))?;
            let values = schema.values(&component.fields);
            match schema.id {
                JUMPING_BEAN => components.jumping_beans.push(PeriodicPop::with(
                    component.entity,
                    Duration::from_secs_f32(values.number("interval").unwrap_or_default()),
                    values.number("impulse").unwrap_or_default(),
                )),
                other => return Err(format!("no runtime for component '{other}'")),
            }
        }
        Ok(components)
    }

    pub fn fixed_update(&mut self, world: &mut World, fixed_delta: Duration) -> Result<(), String> {
        for bean in &mut self.jumping_beans {
            bean.update(world, fixed_delta)
                .map_err(|e| format!("{JUMPING_BEAN} needs a dynamic physics body: {e}"))?;
        }
        Ok(())
    }
}
