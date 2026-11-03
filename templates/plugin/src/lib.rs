//! An Emergent plugin. The game registers [`PLUGIN`] through its generated
//! `src/generated/plugins.rs`. Every ID this plugin adds starts with its plugin id.
//!
//! Add component types in `components`, steering nodes in `brain_nodes` and movers in
//! `locomotion_types`; see `emergent::plugin` for the traits.
use emergent::plugin::registry;
use emergent::{
    ComponentField, ComponentType, EngineContext, EntityId, FieldKind, LoadedScene, Plugin,
    PluginRuntime, Vec3, World,
};

/// The plugin the game registers.
pub static PLUGIN: ThisPlugin = ThisPlugin;

const ID: &str = "template.plugin";
const DRIFT: &str = "template.plugin.drift";

pub struct ThisPlugin;

impl Plugin for ThisPlugin {
    fn id(&self) -> &'static str {
        ID
    }

    fn components(&self) -> Vec<ComponentType> {
        vec![ComponentType {
            id: DRIFT,
            label: "Drift",
            description: "Moves the object at a constant velocity.",
            applies_to: "any",
            fields: vec![
                ComponentField::new("velocity", "Velocity", FieldKind::Vector3)
                    .range(-100.0, 100.0)
                    .unit("m/s"),
            ],
        }]
    }

    fn attach(
        &self,
        loaded: &LoadedScene,
        _world: &mut World,
    ) -> Result<Option<Box<dyn PluginRuntime>>, String> {
        let registry = registry(&[&PLUGIN])?;
        let schema = registry.get(DRIFT).ok_or("drift is registered")?;
        let drifting: Vec<_> = loaded
            .components
            .iter()
            .filter(|component| component.type_id == DRIFT)
            .map(|component| {
                let velocity = schema.values(&component.fields).vector3("velocity");
                (component.entity, Vec3::from(velocity.unwrap_or_default()))
            })
            .collect();
        Ok((!drifting.is_empty()).then(|| Box::new(Drift(drifting)) as Box<dyn PluginRuntime>))
    }
}

/// Runtime of every `drift` component in one scene.
struct Drift(Vec<(EntityId, Vec3)>);

impl PluginRuntime for Drift {
    fn fixed_update(&mut self, context: &mut EngineContext<'_>) -> Result<(), String> {
        let dt = context.fixed_delta.as_secs_f32();
        for &(entity, velocity) in &self.0 {
            let mut transform = *context
                .world
                .local_transform(entity)
                .map_err(|e| format!("{DRIFT}: {e}"))?;
            transform.translation += velocity * dt;
            context
                .world
                .set_local_transform(entity, transform)
                .map_err(|e| format!("{DRIFT}: {e}"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_inside_its_namespace() {
        let registry = registry(&[&PLUGIN]).unwrap();
        assert!(registry.get(DRIFT).is_some());
    }
}
