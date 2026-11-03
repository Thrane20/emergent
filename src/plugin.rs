//! Plugins: reusable component types and their runtime, compiled into a game as
//! ordinary crates and registered with the engine at startup (static registration,
//! see docs/design/plugins.md). A plugin owns the `<plugin id>.` component namespace.
use crate::brain::{Intent, NodeRole, NodeSpec};
use crate::component::{
    segment, ComponentField, ComponentRegistry, ComponentType, ComponentValues, FieldKind,
};
use crate::locomotion::{Activity, LocomotionType};
use crate::{EngineContext, LoadedScene, Vec3, World};

/// A plugin compiled into the game. Implementations are usually a `static` unit struct.
pub trait Plugin: Sync {
    /// Durable identity and component-ID prefix: two or more `.`-separated segments,
    /// e.g. `acme.beans`. The first segment may not be `emergent` or `project`.
    fn id(&self) -> &'static str;

    /// Component types this plugin adds. Each ID must be `<plugin id>.<name>`.
    fn components(&self) -> Vec<ComponentType> {
        Vec::new()
    }

    /// Steering nodes this plugin adds to the brain editor and runtime.
    fn brain_nodes(&self) -> Vec<&'static dyn BrainNode> {
        Vec::new()
    }

    /// Kinematic locomotion types this plugin adds.
    fn locomotion_types(&self) -> Vec<&'static dyn LocomotionModel> {
        Vec::new()
    }

    /// Builds runtime state for this plugin's components in a freshly loaded scene.
    /// `loaded.components` lists every non-engine component; take only your own IDs.
    /// Return `None` when the scene uses none of them.
    fn attach(
        &self,
        _loaded: &LoadedScene,
        _world: &mut World,
    ) -> Result<Option<Box<dyn PluginRuntime>>, String> {
        Ok(None)
    }
}

/// Per-scene state of one plugin, advanced once per fixed tick after the game's own update.
pub trait PluginRuntime {
    fn fixed_update(&mut self, context: &mut EngineContext<'_>) -> Result<(), String>;
}

/// Most number/bool fields a plugin brain node or locomotion type may declare.
pub const MAX_PARAMS: usize = 16;
/// A plugin node's or type's number and bool fields (bools as 0/1), in declared order,
/// read once at load so the per-tick call does no field lookups.
pub type Params = [f32; MAX_PARAMS];

/// What a plugin steering node sees of its agent this tick.
#[derive(Clone, Copy, Debug)]
pub struct BrainInput {
    pub position: Vec3,
    pub velocity: Vec3,
    /// Where the agent started.
    pub home: Vec3,
    /// The brain's max speed (m/s).
    pub max_speed: f32,
    pub dt: f32,
    /// Position and velocity of the node's `target` object, or the nearest member of its
    /// `group`, when the node declares those fields and one resolves.
    pub target: Option<(Vec3, Vec3)>,
}

/// A steering leaf. Its spec's ID must be `<plugin id>.<name>`; its fields may be
/// numbers, bools, `target` (Entity) and `group` (Text). Weight and label are added.
pub trait BrainNode: Sync {
    fn spec(&self) -> NodeSpec;
    /// Desired velocity this tick, or `None` when the node is inactive.
    /// Runs per agent per tick: no allocation.
    fn steer(&self, params: &Params, input: &BrainInput) -> Option<Vec3>;
}

/// A kinematic mover. Its spec's ID must be `<plugin id>.<name>`; fields are numbers or bools.
pub trait LocomotionModel: Sync {
    fn spec(&self) -> LocomotionType;
    /// Moves one agent by `dt` towards `intent`; returns its new position. `velocity`
    /// persists per agent. `activity.yaw` sets the object's heading (radians from -Z
    /// towards -X); fill the rest for visuals. Runs per agent per tick: no allocation.
    fn advance(
        &self,
        params: &Params,
        intent: &Intent,
        velocity: &mut Vec3,
        activity: &mut Activity,
        position: Vec3,
        dt: f32,
    ) -> Vec3;
}

/// A `'static` plugin implementation stored in engine data; compares by address.
pub struct Hook<T: ?Sized + 'static>(pub(crate) &'static T);

impl<T: ?Sized> Clone for Hook<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized> Copy for Hook<T> {}
impl<T: ?Sized> PartialEq for Hook<T> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self.0, other.0)
    }
}
impl<T: ?Sized> std::fmt::Debug for Hook<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("plugin hook")
    }
}

/// Field names the engine handles itself rather than packing into `Params`.
const ENGINE_FIELDS: [&str; 4] = ["weight", "label", "target", "group"];

pub(crate) fn pack(fields: &[ComponentField], values: &ComponentValues) -> Params {
    let mut params = [0.0; MAX_PARAMS];
    let packed = fields.iter().filter(|f| !ENGINE_FIELDS.contains(&f.name));
    for (slot, field) in params.iter_mut().zip(packed) {
        *slot = match field.kind {
            FieldKind::Bool => f32::from(u8::from(values.bool(field.name).unwrap_or(false))),
            _ => values.number(field.name).unwrap_or_default(),
        };
    }
    params
}

fn check_fields(id: &str, fields: &[ComponentField], brain: bool) -> Result<(), String> {
    let mut count = 0;
    for field in fields {
        match (field.name, field.kind) {
            ("target", FieldKind::Entity) | ("group", FieldKind::Text) if brain => {}
            ("weight" | "label", _) if brain => {
                return Err(format!(
                    "{id}: field '{}' is added by the engine",
                    field.name
                ))
            }
            (_, FieldKind::Number | FieldKind::Bool) => count += 1,
            (name, kind) => {
                return Err(format!(
                    "{id}: field '{name}' has unsupported kind {kind:?}"
                ))
            }
        }
    }
    if count > MAX_PARAMS {
        return Err(format!("{id}: more than {MAX_PARAMS} number/bool fields"));
    }
    Ok(())
}

fn namespaced(plugin: &str, id: &str) -> bool {
    id.strip_prefix(plugin)
        .and_then(|rest| rest.strip_prefix('.'))
        .is_some_and(segment)
}

fn register_brain_node(
    registry: &mut ComponentRegistry,
    plugin: &str,
    node: &'static dyn BrainNode,
) -> Result<(), String> {
    let spec = node.spec();
    let id = spec.id;
    if !namespaced(plugin, id) {
        return Err(format!("brain node '{id}' must look like {plugin}.my_node"));
    }
    if spec.role != NodeRole::Steering || spec.status != "available" {
        return Err(format!(
            "brain node '{id}' must be an available Steering node"
        ));
    }
    if !crate::brain::CATEGORIES.contains(&spec.category) {
        return Err(format!(
            "brain node '{id}': unknown category '{}'",
            spec.category
        ));
    }
    check_fields(id, &spec.fields, true)?;
    if registry.brain_nodes.iter().any(|(s, _)| s.id == id) {
        return Err(format!("brain node '{id}' is already registered"));
    }
    registry.brain_nodes.push((spec, Hook(node)));
    Ok(())
}

fn register_locomotion(
    registry: &mut ComponentRegistry,
    plugin: &str,
    model: &'static dyn LocomotionModel,
) -> Result<(), String> {
    let spec = model.spec();
    let id = spec.id;
    if !namespaced(plugin, id) {
        return Err(format!(
            "locomotion type '{id}' must look like {plugin}.my_type"
        ));
    }
    if spec.status != "available" {
        return Err(format!("locomotion type '{id}' must be available"));
    }
    if !crate::locomotion::CATEGORIES.contains(&spec.category) {
        return Err(format!(
            "locomotion type '{id}': unknown category '{}'",
            spec.category
        ));
    }
    check_fields(id, &spec.fields, false)?;
    if registry.locomotion.iter().any(|(s, _)| s.id == id) {
        return Err(format!("locomotion type '{id}' is already registered"));
    }
    registry.locomotion.push((spec, Hook(model)));
    Ok(())
}

/// Whether `id` is a valid plugin id: `publisher.name`, lowercase segments.
pub(crate) fn valid_id(id: &str) -> bool {
    let first = id.split('.').next().unwrap_or_default();
    id.contains('.') && !matches!(first, "emergent" | "project") && id.split('.').all(segment)
}

/// Engine built-ins plus every plugin's types. Fails on a bad or duplicate plugin id
/// or a type outside its plugin's namespace.
pub fn registry(plugins: &[&dyn Plugin]) -> Result<ComponentRegistry, String> {
    let mut registry = ComponentRegistry::builtin();
    for (index, plugin) in plugins.iter().enumerate() {
        let id = plugin.id();
        if !valid_id(id) {
            return Err(format!(
                "plugin id '{id}' must look like publisher.name (lowercase, not emergent or project)"
            ));
        }
        if plugins[..index].iter().any(|other| other.id() == id) {
            return Err(format!("plugin '{id}' is registered twice"));
        }
        let context = |e: String| format!("plugin '{id}': {e}");
        for component in plugin.components() {
            registry.register_in(id, component).map_err(context)?;
        }
        for node in plugin.brain_nodes() {
            register_brain_node(&mut registry, id, node).map_err(context)?;
        }
        for model in plugin.locomotion_types() {
            register_locomotion(&mut registry, id, model).map_err(context)?;
        }
    }
    Ok(registry)
}

/// Runtime state of every plugin used by one loaded scene.
#[derive(Default)]
pub struct PluginRuntimes {
    runtimes: Vec<(&'static str, Box<dyn PluginRuntime>)>,
}

impl PluginRuntimes {
    pub fn attach(
        plugins: &[&dyn Plugin],
        loaded: &LoadedScene,
        world: &mut World,
    ) -> Result<Self, String> {
        let mut runtimes = Vec::with_capacity(plugins.len());
        for plugin in plugins {
            if let Some(runtime) = plugin
                .attach(loaded, world)
                .map_err(|e| format!("plugin '{}': {e}", plugin.id()))?
            {
                runtimes.push((plugin.id(), runtime));
            }
        }
        Ok(Self { runtimes })
    }

    /// Advances each runtime. A failing plugin stops the game instead of continuing
    /// with half-updated state; the error names the plugin.
    pub fn fixed_update(&mut self, context: &mut EngineContext<'_>) -> Result<(), String> {
        for (id, runtime) in &mut self.runtimes {
            runtime
                .fixed_update(context)
                .map_err(|e| format!("plugin '{id}': {e}"))?;
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.runtimes.is_empty()
    }
}
