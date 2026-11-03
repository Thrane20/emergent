//! Brains: saved trees of steering nodes attached to scene objects with `emergent.brain`.
//!
//! A brain asset is a flat `[[nodes]]` list (one root, `parent` links, sibling order = list
//! order). Loading validates every node against the catalogue, inlines referenced brains
//! (cycles rejected) and bounds the result to `MAX_NODES` nodes and `MAX_DEPTH` levels, so
//! evaluation recursion is bounded. Each fixed tick, before other motion: build one snapshot
//! of committed positions, evaluate every agent's tree against it (order independent, one
//! tick behind, like `Follow`), then apply all moves.
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bevy_ecs::component::Component;
use serde::{Deserialize, Serialize};

use crate::component::{
    validate_fields, ComponentField, ComponentRegistry, ComponentValues, FieldKind,
};
use crate::plugin::{BrainInput, BrainNode, Hook, Params};
use crate::{EngineError, EntityId, Project, Quat, Vec3, World};

pub const MAX_NODES: usize = 256;
pub const MAX_DEPTH: usize = 32;
const MAX_FILE_BYTES: usize = 128 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole {
    /// Combines any number of children.
    Composite,
    /// Gates, scores or limits exactly one child.
    Decorator,
    /// Outputs a desired velocity; no children.
    Steering,
    /// Embeds another saved brain.
    Reference,
}

#[derive(Clone, Debug, Serialize)]
pub struct NodeSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub category: &'static str,
    pub role: NodeRole,
    /// `available` or `planned`; planned nodes are listed but rejected on load.
    pub status: &'static str,
    pub description: &'static str,
    pub fields: Vec<ComponentField>,
}

/// Library headings, in display order.
pub const CATEGORIES: &[&str] = &[
    "Composition",
    "Evaluation & senses",
    "Seek & flee",
    "Wander & idle",
    "Paths & patrol",
    "Flocking & formation",
    "Avoidance & bounds",
    "Battle",
    "Locomotion",
    "Look & aim",
    "Actions",
    "Saved brains",
];

/// Every node type: the single source for loading, the editor library and AI prompts.
pub fn node_catalogue() -> Vec<NodeSpec> {
    use FieldKind::*;
    use NodeRole::*;
    let f = ComponentField::new;
    let target = || f("target", "Target object", Entity).help("An object in the scene.");
    let group = |help| f("group", "Group", Text).help(help);
    let target_group =
        || group("Use the nearest member of this group when no target object is set.");
    let num = |name, label, min, max, default: f64, unit: &'static str| {
        let field = f(name, label, Number).range(min, max).default(default);
        if unit.is_empty() {
            field
        } else {
            field.unit(unit)
        }
    };
    let node = |id, label, category, role, description, fields: Vec<ComponentField>| NodeSpec {
        id,
        label,
        category,
        role,
        status: "available",
        description,
        fields,
    };
    let planned = |id, label, category, role, description| NodeSpec {
        status: "planned",
        ..node(id, label, category, role, description, vec![])
    };
    let c = CATEGORIES;
    let mut nodes = vec![
        node("blend", "Blend", c[0], Composite, "Adds the children's steering, each scaled by its weight.", vec![]),
        node("priority", "Priority", c[0], Composite, "Uses the first child that wants to move (or to brake or hover); later children only act when earlier ones are idle.", vec![]),
        node("selector", "Selector", c[0], Composite, "Uses the first child that is active (its conditions pass), even if it holds still.", vec![]),
        node("sequence", "Sequence", c[0], Composite, "Runs children one after another; moves on when the current child finishes (arrives or becomes inactive).", vec![f("loop", "Loop", Bool).default(true)]),
        node("utility", "Utility", c[0], Composite, "Uses the child with the highest score (a Score node's value, otherwise its weight).", vec![]),
        planned("random_choice", "Random choice", c[0], Composite, "Picks a child at random and keeps it for a while."),
        planned("repeat", "Repeat", c[0], Decorator, "Runs its child a number of times."),
        planned("invert", "Invert", c[0], Decorator, "Active when its child is not."),
        node("within_distance", "Within distance", c[1], Decorator, "Child is active only while the target is closer than the distance.", vec![target(), target_group(), num("distance", "Distance", 0.0, 10000.0, 5.0, "m")]),
        node("outside_distance", "Outside distance", c[1], Decorator, "Child is active only while the target is farther than the distance (or absent).", vec![target(), target_group(), num("distance", "Distance", 0.0, 10000.0, 5.0, "m")]),
        node("in_view", "In view", c[1], Decorator, "Child is active while the target is inside the view cone around the current heading (and, optionally, not hidden behind an obstacle).", vec![target(), target_group(), num("angle", "Half angle", 0.0, 180.0, 60.0, "°"), num("distance", "Range", 0.0, 10000.0, 15.0, "m"), f("line_of_sight", "Needs line of sight", Bool).default(false).help("Obstacles (objects with box physics colliders) block the view.")]),
        node("neighbours_at_least", "Neighbours at least", c[1], Decorator, "Child is active while enough group members are nearby.", vec![group("Members to count."), num("radius", "Radius", 0.0, 1000.0, 5.0, "m"), num("count", "Count", 1.0, 1024.0, 3.0, "")]),
        node("chance", "Chance", c[1], Decorator, "Re-rolls every interval; the child is active with this probability.", vec![num("probability", "Probability", 0.0, 1.0, 0.5, ""), num("interval", "Interval", 0.05, 3600.0, 2.0, "s")]),
        node("duty_cycle", "Duty cycle", c[1], Decorator, "Child is active for 'on' seconds, then rests for 'off' seconds.", vec![num("on", "On", 0.0, 3600.0, 2.0, "s"), num("off", "Off", 0.0, 3600.0, 2.0, "s")]),
        node("score_distance", "Score: distance", c[1], Decorator, "For Utility: scores 1 when the target is at 'near', falling to 0 at 'far' (or the reverse).", vec![target(), target_group(), num("near", "Near", 0.0, 10000.0, 0.0, "m"), num("far", "Far", 0.0, 10000.0, 20.0, "m"), f("invert", "Prefer far", Bool).default(false)]),
        node("score_crowding", "Score: crowding", c[1], Decorator, "For Utility: scores how crowded it is, from 0 to 1 at 'count' neighbours.", vec![group("Members to count."), num("radius", "Radius", 0.0, 1000.0, 5.0, "m"), num("count", "Full at", 1.0, 1024.0, 5.0, "")]),
        node("line_of_sight", "Line of sight", c[1], Decorator, "Child is active while nothing solid stands between this object and the target (objects with box physics colliders block sight).", vec![target(), target_group(), num("distance", "Range", 0.0, 10000.0, 30.0, "m"), f("invert", "When hidden", Bool).default(false).help("Active while the target cannot be seen instead.")]),
        planned("stat_threshold", "Stat threshold", c[1], Decorator, "Needs object stats such as health."),
        node("seek", "Seek", c[2], Steering, "Heads straight for the target at full speed.", vec![target(), target_group()]),
        node("flee", "Flee", c[2], Steering, "Runs directly away from the target while it is within the panic distance (0 = always).", vec![target(), target_group(), num("panic_distance", "Panic distance", 0.0, 10000.0, 10.0, "m")]),
        node("arrive", "Arrive", c[2], Steering, "Like seek, but slows inside the slowing radius and finishes inside the stop radius.", vec![target(), target_group(), num("slow_radius", "Slowing radius", 0.0, 1000.0, 3.0, "m"), num("stop_radius", "Stop radius", 0.0, 1000.0, 0.5, "m")]),
        node("pursue", "Pursue", c[2], Steering, "Seeks where the target will be, predicting from its velocity.", vec![target(), target_group(), num("prediction", "Look ahead", 0.0, 10.0, 1.0, "s")]),
        node("evade", "Evade", c[2], Steering, "Flees from where the target will be, while it is within the panic distance.", vec![target(), target_group(), num("prediction", "Look ahead", 0.0, 10.0, 1.0, "s"), num("panic_distance", "Panic distance", 0.0, 10000.0, 10.0, "m")]),
        node("hide", "Hide", c[2], Steering, "Moves to the far side of the nearest cover from the threat, then stays hidden. Cover is any object with a box physics collider no bigger than the max cover size. Inactive when no cover is in reach.", vec![target(), group("Threats; hides from the nearest member."), num("search_radius", "Search radius", 0.0, 1000.0, 15.0, "m"), num("margin", "Distance behind cover", 0.0, 100.0, 1.0, "m"), num("max_cover", "Max cover size", 0.1, 1000.0, 5.0, "m").help("Bigger obstacles (the ground, large walls) are not used as cover."), num("panic_distance", "Panic distance", 0.0, 10000.0, 0.0, "m").help("Only hide while the threat is this close (0 = always).")]),
        node("wander", "Wander", c[3], Steering, "Meanders: a target point drifts randomly on a sphere ahead of the object.", vec![num("radius", "Circle radius", 0.0, 100.0, 1.0, "m"), num("distance", "Circle distance", 0.0, 100.0, 2.0, "m"), num("jitter", "Jitter", 0.0, 100.0, 2.0, "m/s")]),
        node("hold", "Hold position", c[3], Steering, "Stands still (active, zero velocity).", vec![]),
        node("return_home", "Return home", c[3], Steering, "Arrives back where the object started.", vec![num("slow_radius", "Slowing radius", 0.0, 1000.0, 2.0, "m"), num("stop_radius", "Stop radius", 0.0, 1000.0, 0.3, "m")]),
        node("orbit", "Orbit", c[3], Steering, "Circles the target (or home) at a radius, in the horizontal plane.", vec![target(), target_group(), num("radius", "Radius", 0.0, 1000.0, 4.0, "m"), f("clockwise", "Clockwise", Bool).default(false)]),
        planned("drift", "Drift", c[3], Steering, "Slow ambient motion from a noise field."),
        node("patrol", "Patrol", c[4], Steering, "Visits waypoints in order: loop, ping-pong, or once (then finishes).", vec![f("waypoints", "Waypoints", Entities).help("Objects to visit, in order. Point markers work well."), f("mode", "Mode", Choice).options(&["loop", "ping_pong", "once"]).default("loop"), num("arrive_radius", "Arrive radius", 0.05, 100.0, 0.5, "m")]),
        node("follow_target", "Follow target", c[4], Steering, "Keeps a set distance behind the target.", vec![target(), target_group(), num("distance", "Distance", 0.0, 1000.0, 2.0, "m")]),
        planned("pathfind", "Pathfind", c[4], Steering, "Needs a navigation mesh."),
        node("separation", "Separation", c[5], Steering, "Steers away from group members that are too close, harder the closer they are.", vec![group("Neighbours."), num("radius", "Radius", 0.0, 1000.0, 2.0, "m")]),
        node("alignment", "Alignment", c[5], Steering, "Matches the average heading of nearby group members.", vec![group("Neighbours."), num("radius", "Radius", 0.0, 1000.0, 5.0, "m")]),
        node("cohesion", "Cohesion", c[5], Steering, "Steers towards the centre of nearby group members.", vec![group("Neighbours."), num("radius", "Radius", 0.0, 1000.0, 5.0, "m")]),
        node("flock", "Flock", c[5], Steering, "Separation, alignment and cohesion together.", vec![group("Neighbours."), num("radius", "Radius", 0.0, 1000.0, 5.0, "m"), num("separation", "Separation", 0.0, 100.0, 1.5, ""), num("alignment", "Alignment", 0.0, 100.0, 1.0, ""), num("cohesion", "Cohesion", 0.0, 100.0, 1.0, "")]),
        node("follow_leader", "Follow leader", c[5], Steering, "Arrives at a point behind the leader and steps out of its way.", vec![target(), target_group(), num("distance", "Distance behind", 0.0, 1000.0, 2.0, "m")]),
        node("formation", "Formation", c[5], Steering, "Takes a slot around the leader by position in the members group: line, wedge or ring.", vec![target(), target_group(), f("members", "Members group", Text).help("Formation members; slots follow their order in the scene."), f("shape", "Shape", Choice).options(&["line", "wedge", "ring"]).default("wedge"), num("spacing", "Spacing", 0.1, 100.0, 2.0, "m")]),
        planned("queue", "Queue", c[5], Steering, "Single file behind the one in front."),
        node("avoid", "Avoid", c[6], Steering, "Moves away from the target or group members inside the radius.", vec![target(), target_group(), num("radius", "Radius", 0.0, 1000.0, 3.0, "m")]),
        node("stay_within", "Stay within", c[6], Steering, "Returns towards the centre (target or home) when outside the radius.", vec![target(), target_group(), num("radius", "Radius", 0.0, 10000.0, 10.0, "m")]),
        planned("obstacle_avoidance", "Obstacle avoidance", c[6], Steering, "Needs collider ray queries."),
        planned("wall_follow", "Wall follow", c[6], Steering, "Needs collider queries."),
        node("attack", "Attack", c[7], Steering, "Charges the nearest enemy (leading a moving one) and holds once within striking range.", vec![target(), group("Enemies; the nearest member is attacked."), num("range", "Strike range", 0.0, 1000.0, 1.5, "m"), num("prediction", "Look ahead", 0.0, 10.0, 0.5, "s")]),
        node("defend", "Defend", c[7], Steering, "Guards the target (or home): steps between it and the nearest threat inside the alert radius, otherwise stays close.", vec![target(), group("Protect the nearest member of this group."), f("threats", "Threats group", Text), num("radius", "Guard radius", 0.0, 1000.0, 2.0, "m"), num("alert", "Alert radius", 0.0, 10000.0, 10.0, "m")]),
        node("hold_ground", "Hold ground", c[7], Steering, "Holds its starting position; if pushed out of the radius it walks back.", vec![num("radius", "Radius", 0.0, 1000.0, 0.5, "m")]),
        node("retreat", "Retreat", c[7], Steering, "Falls back from threats inside the panic distance towards a rally point (target, or home). Inactive when no threat is near.", vec![target(), group("Rally on the nearest member of this group."), f("threats", "Threats group", Text), num("panic_distance", "Panic distance", 0.0, 10000.0, 8.0, "m")]),
        node("flank", "Flank", c[7], Steering, "Swings wide to the enemy's side, then closes in.", vec![target(), group("Enemies; the nearest member is flanked."), num("offset", "Flank distance", 0.0, 1000.0, 4.0, "m"), f("side", "Side", Choice).options(&["nearest", "left", "right"]).default("nearest")]),
        node("encircle", "Encircle", c[7], Steering, "Takes a slot on a ring around the enemy, spread by position in the members group.", vec![target(), group("Enemies; the nearest member is encircled."), f("members", "Members group", Text).help("Who shares the ring; slots follow their order in the scene."), num("radius", "Ring radius", 0.0, 1000.0, 4.0, "m")]),
        node("kite", "Kite", c[7], Steering, "Skirmishes: backs off inside the minimum distance, closes beyond the maximum, and circles in between.", vec![target(), group("Enemies; the nearest member is kited."), num("min_distance", "Minimum distance", 0.0, 1000.0, 4.0, "m"), num("max_distance", "Maximum distance", 0.0, 1000.0, 8.0, "m")]),
        node("regroup", "Regroup", c[7], Steering, "Returns to the centre of its allies when it has strayed farther than the radius.", vec![group("Allies."), num("radius", "Radius", 0.0, 1000.0, 4.0, "m")]),
        node("outnumbered", "Outnumbered", c[7], Decorator, "Child is active while enemies near it outnumber its allies by the ratio.", vec![f("allies", "Allies group", Text), f("enemies", "Enemies group", Text), num("radius", "Radius", 0.0, 10000.0, 8.0, "m"), num("ratio", "Ratio", 0.1, 100.0, 1.5, "")]),
        node("brake", "Brake", c[8], Steering, "Stops: a car brakes, a walker halts, a drone hovers in place, a plane slows to its minimum speed.", vec![]),
        node("hover", "Hover", c[8], Steering, "Holds position in the air (drones); a plane circles where it is; ground objects stop.", vec![]),
        node("boost", "Boost", c[8], Decorator, "Its child moves at the locomotion's maximum (full throttle).", vec![]),
        node("altitude", "Altitude", c[8], Decorator, "Its child flies at this world-space height. Enable Finish on arrival to advance a Sequence when the object's centre reaches the height within tolerance; otherwise holds altitude while the child is active.", vec![num("height", "Height", -10000.0, 100000.0, 10.0, "m"), f("finish_on_arrival", "Finish on arrival", Bool).default(false), num("arrival_radius", "Arrival tolerance", 0.01, 100.0, 0.1, "m")]),
        node("take_off", "Take off", c[8], Steering, "Physical aircraft: roll, lift off and climb to this world height, then hold it. Finish at height advances a Sequence once within 10 m of it. Other movers ignore it.", vec![num("height", "Height", -10000.0, 100000.0, 150.0, "m"), f("finish_at_height", "Finish at height", Bool).default(true)]),
        node("depart_airport", "Depart airport", c[8], Steering, "Physical aircraft: ask the airport's tower to depart — taxi to the holding position, wait for the runway, line up, take off and climb to the height. The target is the aerodrome (or any of its items), or the nearest member of the group. Finish at height advances a Sequence once within 10 m of it.", vec![target(), group("Airports; the nearest is used."), num("height", "Height", -10000.0, 100000.0, 300.0, "m"), f("finish_at_height", "Finish at height", Bool).default(true)]),
        node("arrive_airport", "Arrive at airport", c[8], Steering, "Physical aircraft: ask the airport's tower to land — it joins the sequence (holding if the circuit is full), lands on the active runway, vacates and parks on a free stand. The target is the aerodrome (or any of its items), or the nearest member of the group.", vec![target(), group("Airports; the nearest is used.")]),
        node("land", "Land", c[8], Steering, "Physical aircraft: fly a left-hand circuit and land on the target zone (a rectangular map zone; or the nearest member of the group), then stop. Inactive without a zone. Other movers ignore it.", vec![target(), group("Zones to land on; the nearest is used."), num("pattern_height", "Circuit height", 30.0, 3000.0, 300.0, "m").help("Height above the runway for the downwind leg.")]),
        node("look_at", "Look at", c[9], Steering, "Faces the target (or the nearest of a group) while moving however the movement nodes say. Only turns the object.", vec![target(), target_group(), f("offset", "Aim offset", Vector3).range(-1000.0, 1000.0).unit("m").default(toml::Value::Array(vec![0.0.into(), 0.0.into(), 0.0.into()]))]),
        node("look_along", "Look along movement", c[9], Steering, "Faces the direction it is moving (the usual default, made explicit for a stack).", vec![]),
        node("scan", "Scan", c[9], Steering, "Sweeps its gaze left and right around the direction it is moving (or its start heading).", vec![num("angle", "Sweep", 0.0, 180.0, 60.0, "°"), num("period", "Period", 0.1, 600.0, 4.0, "s")]),
        node("signal", "Signal", c[10], Steering, "Sends a named signal (fire, alarm, …) other brains and game code receive next tick; then waits for the cooldown. Does not move the object.", vec![f("name", "Signal", Text), target(), target_group(), num("cooldown", "Cooldown", 0.0, 3600.0, 1.0, "s")]),
        node("on_signal", "On signal", c[1], Decorator, "Child is active for a while after a signal with this name arrives (from a group, within a radius, if set). Signals aimed at someone else are ignored.", vec![f("name", "Signal", Text), group("Only from members of this group."), num("radius", "Radius", 0.0, 100000.0, 0.0, "m").help("0 hears it anywhere."), num("hold", "Hold", 0.0, 3600.0, 0.5, "s")]),
        node("speed_limit", "Speed limit", c[10], Decorator, "Caps its child's speed.", vec![num("max_speed", "Max speed", 0.0, 100.0, 1.0, "m/s")]),
        planned("jump", "Jump", c[10], Steering, "Needs ground contact queries."),
        node("brain", "Saved brain", c[11], Reference, "Embeds another saved brain here.", vec![f("brain", "Brain", Brain)]),
    ];
    for node in &mut nodes {
        add_common_fields(node);
    }
    nodes
}

/// Weight and label, which every node carries.
fn add_common_fields(node: &mut NodeSpec) {
    node.fields.push(
        ComponentField::new("weight", "Weight", FieldKind::Number)
            .range(0.0, 100.0)
            .default(1.0)
            .help("Blend weight or Utility score."),
    );
    node.fields
        .push(ComponentField::new("label", "Label", FieldKind::Text).help("Shown in the editor."));
}

/// Built-in nodes plus the plugin nodes in `registry`.
pub fn node_catalogue_with(registry: &ComponentRegistry) -> Vec<NodeSpec> {
    let mut nodes = node_catalogue();
    nodes.extend(registry.brain_nodes.iter().map(|(spec, _)| {
        let mut spec = spec.clone();
        add_common_fields(&mut spec);
        spec
    }));
    nodes
}

/// Node catalogue export for the editor library, inspector forms and AI prompts.
pub fn export_node_catalogue() -> Result<String, toml::ser::Error> {
    export_node_catalogue_with(&ComponentRegistry::builtin())
}

/// Node catalogue export including the project's plugin nodes.
pub fn export_node_catalogue_with(
    registry: &ComponentRegistry,
) -> Result<String, toml::ser::Error> {
    #[derive(Serialize)]
    struct Export {
        categories: &'static [&'static str],
        nodes: Vec<NodeSpec>,
    }
    toml::to_string(&Export {
        categories: CATEGORIES,
        nodes: node_catalogue_with(registry),
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BrainFile {
    schema_version: u32,
    id: String,
    name: Option<String>,
    kind: Option<String>,
    description: Option<String>,
    template: Option<String>,
    runtime_status: Option<String>,
    #[serde(default)]
    nodes: Vec<NodeFile>,
}

#[derive(Clone, Deserialize)]
struct NodeFile {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    parent: Option<String>,
    #[serde(flatten)]
    fields: toml::Table,
}

/// One validated node; names (targets, groups, waypoints) are resolved per scene.
#[derive(Clone, Debug)]
struct NodeDef {
    kind: &'static str,
    plugin: Option<Hook<dyn BrainNode>>,
    fields: toml::Table,
    children: Vec<usize>,
}

/// A loaded, validated brain with referenced brains inlined. Scene-independent.
#[derive(Clone, Debug)]
pub struct BrainDefinition {
    pub path: String,
    pub name: String,
    nodes: Vec<NodeDef>,
}

fn identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}

impl BrainDefinition {
    pub fn load(project: &Project, path: &str) -> Result<Self, String> {
        Self::load_with(project, path, &ComponentRegistry::builtin())
    }

    /// Loads with the project's registry so plugin nodes resolve.
    pub fn load_with(
        project: &Project,
        path: &str,
        registry: &ComponentRegistry,
    ) -> Result<Self, String> {
        let source = project
            .read_text_asset(path)
            .map_err(|e| format!("{path}: {e}"))?;
        Self::from_source_with(project, path, &source, registry)
    }

    /// Validates a candidate (editor or AI) against the catalogue and the project's other brains.
    pub fn from_source(project: &Project, path: &str, source: &str) -> Result<Self, String> {
        Self::from_source_with(project, path, source, &ComponentRegistry::builtin())
    }

    pub fn from_source_with(
        project: &Project,
        path: &str,
        source: &str,
        registry: &ComponentRegistry,
    ) -> Result<Self, String> {
        let catalogue = node_catalogue_with(registry);
        let mut stack = vec![path.to_string()];
        let mut definition = Self {
            path: path.into(),
            name: String::new(),
            nodes: Vec::new(),
        };
        definition.name = parse_into(
            project,
            &catalogue,
            registry,
            source,
            &mut stack,
            &mut definition.nodes,
            1,
        )
        .map_err(|e| format!("{path}: {e}"))?
        .0;
        Ok(definition)
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Instance IDs this brain names (targets and waypoints), for scene validation.
    pub fn object_references(&self) -> Vec<&str> {
        let mut ids = Vec::new();
        for node in &self.nodes {
            ids.extend(node.fields.get("target").and_then(toml::Value::as_str));
            if let Some(list) = node.fields.get("waypoints").and_then(toml::Value::as_array) {
                ids.extend(list.iter().filter_map(toml::Value::as_str));
            }
        }
        ids
    }

    /// Whether any node says where to look, so the brain turns its object.
    pub fn looks(&self) -> bool {
        self.nodes
            .iter()
            .any(|node| matches!(node.kind, "look_at" | "look_along" | "scan"))
    }

    /// Group names this brain reads.
    pub fn group_names(&self) -> impl Iterator<Item = &str> {
        self.nodes.iter().flat_map(|node| {
            ["group", "members"]
                .into_iter()
                .filter_map(|name| node.fields.get(name).and_then(toml::Value::as_str))
        })
    }
}

/// Parses one brain file into `out`, inlining references. Returns (name, root index).
fn parse_into(
    project: &Project,
    catalogue: &[NodeSpec],
    registry: &ComponentRegistry,
    source: &str,
    stack: &mut Vec<String>,
    out: &mut Vec<NodeDef>,
    base_depth: usize,
) -> Result<(String, usize), String> {
    if source.len() > MAX_FILE_BYTES {
        return Err("brain file exceeds 128 KiB".into());
    }
    let file: BrainFile = toml::from_str(source).map_err(|e| e.to_string())?;
    let _metadata = (file.template, file.runtime_status, file.description);
    if file.schema_version != 1 || !identifier(&file.id) {
        return Err("requires schema_version = 1 and a valid id".into());
    }
    if file.kind.as_deref().is_some_and(|k| k != "brain") {
        return Err("kind must be brain".into());
    }
    let name = file.name.unwrap_or(file.id);
    let mut index = HashMap::with_capacity(file.nodes.len());
    for (i, node) in file.nodes.iter().enumerate() {
        if !identifier(&node.id) || index.insert(node.id.as_str(), i).is_some() {
            return Err(format!("invalid or duplicate node id '{}'", node.id));
        }
    }
    let roots: Vec<_> = file.nodes.iter().filter(|n| n.parent.is_none()).collect();
    if roots.len() != 1 {
        return Err(format!(
            "needs exactly one root node (no parent); found {}",
            roots.len()
        ));
    }
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); file.nodes.len()];
    for (i, node) in file.nodes.iter().enumerate() {
        if let Some(parent) = &node.parent {
            let p = *index
                .get(parent.as_str())
                .ok_or_else(|| format!("node '{}' has unknown parent '{parent}'", node.id))?;
            children[p].push(i);
        }
    }
    // Depth-first from the root with an explicit stack; parent cycles leave nodes unreached.
    let root = index[roots[0].id.as_str()];
    let mut work = vec![(root, base_depth)];
    let mut order = Vec::new();
    while let Some((i, depth)) = work.pop() {
        if depth > MAX_DEPTH {
            return Err(format!("tree deeper than {MAX_DEPTH} levels"));
        }
        order.push((i, depth));
        for &child in children[i].iter().rev() {
            work.push((child, depth + 1));
        }
    }
    if order.len() != file.nodes.len() {
        return Err("every node must descend from the root (parent cycle or orphan)".into());
    }
    for &(i, _) in &order {
        let node = &file.nodes[i];
        let spec = catalogue
            .iter()
            .find(|spec| spec.id == node.kind)
            .ok_or_else(|| format!("node '{}': unknown type '{}'", node.id, node.kind))?;
        if spec.status != "available" {
            return Err(format!(
                "node '{}': {} is planned, not available yet",
                node.id, spec.label
            ));
        }
        validate_fields(
            &format!("node '{}' ({})", node.id, spec.id),
            &spec.fields,
            &node.fields,
            false,
        )?;
        let count = children[i].len();
        let arity_ok = match spec.role {
            NodeRole::Composite => count >= 1,
            NodeRole::Decorator => count == 1,
            NodeRole::Steering | NodeRole::Reference => count == 0,
        };
        if !arity_ok {
            return Err(format!(
                "node '{}' ({}) {}",
                node.id,
                spec.label,
                match spec.role {
                    NodeRole::Composite => "needs at least one child",
                    NodeRole::Decorator => "needs exactly one child",
                    _ => "cannot have children",
                }
            ));
        }
        let values = ComponentValues::new(&spec.fields, &node.fields);
        let needs_target = matches!(
            spec.id,
            "within_distance"
                | "in_view"
                | "score_distance"
                | "seek"
                | "flee"
                | "arrive"
                | "pursue"
                | "evade"
                | "follow_target"
                | "follow_leader"
                | "formation"
                | "attack"
                | "line_of_sight"
                | "hide"
                | "flank"
                | "encircle"
                | "kite"
        );
        if needs_target && values.entity("target").is_none() && values.entity("group").is_none() {
            return Err(format!(
                "node '{}' ({}) needs a target object or group",
                node.id, spec.label
            ));
        }
        let needs_group = matches!(
            spec.id,
            "neighbours_at_least"
                | "score_crowding"
                | "separation"
                | "alignment"
                | "cohesion"
                | "flock"
                | "regroup"
        );
        if needs_group && values.entity("group").is_none() {
            return Err(format!("node '{}' ({}) needs a group", node.id, spec.label));
        }
        for (kind, field) in [
            ("defend", "threats"),
            ("retreat", "threats"),
            ("encircle", "members"),
            ("outnumbered", "allies"),
            ("outnumbered", "enemies"),
        ] {
            if spec.id == kind && values.entity(field).is_none() {
                return Err(format!("node '{}' ({}) needs {field}", node.id, spec.label));
            }
        }
        if spec.id == "kite" && values.number("min_distance") > values.number("max_distance") {
            return Err(format!(
                "node '{}' (Kite) needs minimum ≤ maximum distance",
                node.id
            ));
        }
        if matches!(spec.id, "signal" | "on_signal") && values.entity("name").is_none() {
            return Err(format!(
                "node '{}' ({}) needs a signal name",
                node.id, spec.label
            ));
        }
        if spec.id == "look_at"
            && values.entity("target").is_none()
            && values.entity("group").is_none()
        {
            return Err(format!(
                "node '{}' (Look at) needs a target object or group",
                node.id
            ));
        }
        if spec.id == "formation" && values.entity("members").is_none() {
            return Err(format!(
                "node '{}' (Formation) needs a members group",
                node.id
            ));
        }
        if spec.id == "patrol" && values.entities("waypoints").is_empty() {
            return Err(format!("node '{}' (Patrol) needs waypoints", node.id));
        }
    }
    // Emit in pre-order, inlining references; indices in `out` are global.
    let mut map = vec![usize::MAX; file.nodes.len()];
    for &(i, depth) in &order {
        let node = &file.nodes[i];
        let spec = catalogue
            .iter()
            .find(|s| s.id == node.kind)
            .expect("validated above");
        let at = if spec.role == NodeRole::Reference {
            let target = node.fields["brain"].as_str().expect("validated brain path");
            if stack.iter().any(|p| p == target) {
                return Err(format!(
                    "brain reference cycle: {} -> {target}",
                    stack.join(" -> ")
                ));
            }
            let source = project
                .read_text_asset(target)
                .map_err(|e| format!("node '{}': {target}: {e}", node.id))?;
            stack.push(target.to_string());
            let (_, root) = parse_into(project, catalogue, registry, &source, stack, out, depth)
                .map_err(|e| format!("in {target}: {e}"))?;
            stack.pop();
            // The reference node's weight and label apply to the embedded root.
            for key in ["weight", "label"] {
                if let Some(value) = node.fields.get(key) {
                    out[root].fields.insert(key.into(), value.clone());
                }
            }
            root
        } else {
            out.push(NodeDef {
                kind: spec.id,
                plugin: registry
                    .brain_nodes
                    .iter()
                    .find(|(s, _)| s.id == spec.id)
                    .map(|&(_, hook)| hook),
                fields: node.fields.clone(),
                children: Vec::new(),
            });
            out.len() - 1
        };
        if out.len() > MAX_NODES {
            return Err(format!(
                "brain exceeds {MAX_NODES} nodes including referenced brains"
            ));
        }
        map[i] = at;
    }
    for (i, list) in children.iter().enumerate() {
        out[map[i]].children = list.iter().map(|&c| map[c]).collect();
    }
    Ok((name, map[root]))
}

/// Per-scene identities: group names to small IDs, shared by `Group` and brains.
#[derive(Clone, Debug, Default, bevy_ecs::resource::Resource)]
pub struct GroupNames(pub Vec<String>);

impl GroupNames {
    pub fn id(&mut self, name: &str) -> u32 {
        match self.0.iter().position(|n| n == name) {
            Some(i) => i as u32,
            None => {
                self.0.push(name.to_string());
                (self.0.len() - 1) as u32
            }
        }
    }
}

/// Membership used by brain targets and neighbourhoods (`emergent.group`).
#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub struct Group(pub u32);

#[derive(Clone, Copy, Debug, Default)]
struct Target {
    object: Option<EntityId>,
    group: Option<u32>,
}

#[derive(Clone, Debug)]
enum Op {
    Blend,
    Priority,
    Selector,
    Sequence {
        looping: bool,
    },
    Utility,
    Within {
        distance: f32,
        inside: bool,
    },
    InView {
        cos: f32,
        distance: f32,
        sight: bool,
    },
    LineOfSight {
        distance: f32,
        invert: bool,
    },
    Hide {
        search: f32,
        margin: f32,
        max_cover: f32,
        panic: f32,
    },
    Neighbours {
        group: u32,
        radius: f32,
        count: usize,
    },
    Chance {
        probability: f32,
        interval: f32,
    },
    Duty {
        on: f32,
        off: f32,
    },
    ScoreDistance {
        near: f32,
        far: f32,
        invert: bool,
    },
    ScoreCrowding {
        group: u32,
        radius: f32,
        count: f32,
    },
    SpeedLimit {
        max: f32,
    },
    Seek,
    Flee {
        panic: f32,
    },
    Arrive {
        slow: f32,
        stop: f32,
    },
    Pursue {
        prediction: f32,
    },
    Evade {
        prediction: f32,
        panic: f32,
    },
    Wander {
        radius: f32,
        distance: f32,
        jitter: f32,
    },
    Hold,
    LookAt {
        offset: Vec3,
    },
    LookAlong,
    Scan {
        angle: f32,
        period: f32,
    },
    Signal {
        name: u32,
        cooldown: f32,
    },
    OnSignal {
        name: u32,
        from: Option<u32>,
        radius: f32,
        hold: f32,
    },
    Brake,
    Hover,
    Boost,
    TakeOff {
        height: f32,
        finish: bool,
    },
    Land {
        pattern: f32,
    },
    DepartAirport {
        height: f32,
        finish: bool,
    },
    ArriveAirport,
    Altitude {
        height: f32,
        finish_on_arrival: bool,
        arrival_radius: f32,
    },
    ReturnHome {
        slow: f32,
        stop: f32,
    },
    Orbit {
        radius: f32,
        clockwise: bool,
    },
    Patrol {
        points: Vec<Option<EntityId>>,
        mode: u8,
        radius: f32,
    },
    FollowTarget {
        distance: f32,
    },
    Separation {
        group: u32,
        radius: f32,
    },
    Alignment {
        group: u32,
        radius: f32,
    },
    Cohesion {
        group: u32,
        radius: f32,
    },
    Flock {
        group: u32,
        radius: f32,
        s: f32,
        a: f32,
        c: f32,
    },
    FollowLeader {
        distance: f32,
    },
    Formation {
        members: u32,
        shape: u8,
        spacing: f32,
    },
    Avoid {
        radius: f32,
    },
    Attack {
        range: f32,
        prediction: f32,
    },
    Defend {
        threats: u32,
        radius: f32,
        alert: f32,
    },
    HoldGround {
        radius: f32,
    },
    Retreat {
        threats: u32,
        panic: f32,
    },
    Flank {
        offset: f32,
        side: i8,
    },
    Encircle {
        members: u32,
        radius: f32,
    },
    Kite {
        min: f32,
        max: f32,
    },
    Regroup {
        group: u32,
        radius: f32,
    },
    Outnumbered {
        allies: u32,
        enemies: u32,
        radius: f32,
        ratio: f32,
    },
    StayWithin {
        radius: f32,
    },
    Plugin {
        plugin: Hook<dyn BrainNode>,
        params: Params,
    },
}

#[derive(Clone, Debug)]
struct Node {
    op: Op,
    weight: f32,
    target: Target,
    children: Vec<u32>,
}

/// A brain resolved against one scene's objects and groups; shared by every agent using it.
#[derive(Debug)]
pub struct CompiledBrain {
    nodes: Vec<Node>,
    /// Objects named by targets and waypoints; included in every tick's snapshot.
    pub(crate) objects: Vec<EntityId>,
    /// Whether any node tests sight or looks for cover, so the snapshot collects obstacles.
    pub(crate) needs_obstacles: bool,
}

impl CompiledBrain {
    /// `object` resolves instance IDs; unknown IDs are inactive (`None`), which only the
    /// editor preview allows — scenes reject them at load.
    pub fn compile(
        definition: &BrainDefinition,
        object: &dyn Fn(&str) -> Option<EntityId>,
        groups: &mut GroupNames,
    ) -> Self {
        let catalogue = node_catalogue();
        let mut objects = Vec::new();
        let nodes: Vec<Node> = definition
            .nodes
            .iter()
            .map(|def| {
                let plugin_spec = def.plugin.map(|hook| {
                    let mut spec = hook.0.spec();
                    add_common_fields(&mut spec);
                    spec
                });
                let spec = plugin_spec.as_ref().unwrap_or_else(|| {
                    catalogue
                        .iter()
                        .find(|s| s.id == def.kind)
                        .expect("validated node")
                });
                let v = ComponentValues::new(&spec.fields, &def.fields);
                let n = |name| v.number(name).unwrap_or_default();
                let target = Target {
                    object: v.entity("target").and_then(object),
                    group: v.entity("group").map(|s| groups.id(s)),
                };
                let mut group = |name| v.entity(name).map(|s| groups.id(s)).unwrap_or(u32::MAX);
                objects.extend(target.object);
                let op = match def.kind {
                    "blend" => Op::Blend,
                    "priority" => Op::Priority,
                    "selector" => Op::Selector,
                    "sequence" => Op::Sequence {
                        looping: v.bool("loop").unwrap_or(true),
                    },
                    "utility" => Op::Utility,
                    "within_distance" => Op::Within {
                        distance: n("distance"),
                        inside: true,
                    },
                    "outside_distance" => Op::Within {
                        distance: n("distance"),
                        inside: false,
                    },
                    "in_view" => Op::InView {
                        cos: n("angle").to_radians().cos(),
                        distance: n("distance"),
                        sight: v.bool("line_of_sight").unwrap_or(false),
                    },
                    "line_of_sight" => Op::LineOfSight {
                        distance: n("distance"),
                        invert: v.bool("invert").unwrap_or(false),
                    },
                    "hide" => Op::Hide {
                        search: n("search_radius"),
                        margin: n("margin"),
                        max_cover: n("max_cover"),
                        panic: n("panic_distance"),
                    },
                    "neighbours_at_least" => Op::Neighbours {
                        group: group("group"),
                        radius: n("radius"),
                        count: n("count") as usize,
                    },
                    "chance" => Op::Chance {
                        probability: n("probability"),
                        interval: n("interval"),
                    },
                    "duty_cycle" => Op::Duty {
                        on: n("on"),
                        off: n("off"),
                    },
                    "score_distance" => Op::ScoreDistance {
                        near: n("near"),
                        far: n("far"),
                        invert: v.bool("invert").unwrap_or(false),
                    },
                    "score_crowding" => Op::ScoreCrowding {
                        group: group("group"),
                        radius: n("radius"),
                        count: n("count"),
                    },
                    "speed_limit" => Op::SpeedLimit {
                        max: n("max_speed"),
                    },
                    "seek" => Op::Seek,
                    "flee" => Op::Flee {
                        panic: n("panic_distance"),
                    },
                    "arrive" => Op::Arrive {
                        slow: n("slow_radius"),
                        stop: n("stop_radius"),
                    },
                    "pursue" => Op::Pursue {
                        prediction: n("prediction"),
                    },
                    "evade" => Op::Evade {
                        prediction: n("prediction"),
                        panic: n("panic_distance"),
                    },
                    "wander" => Op::Wander {
                        radius: n("radius"),
                        distance: n("distance"),
                        jitter: n("jitter"),
                    },
                    "hold" => Op::Hold,
                    "look_at" => Op::LookAt {
                        offset: Vec3::from_array(v.vector3("offset").unwrap_or_default()),
                    },
                    "look_along" => Op::LookAlong,
                    "scan" => Op::Scan {
                        angle: n("angle").to_radians(),
                        period: n("period"),
                    },
                    "signal" => Op::Signal {
                        name: group("name"),
                        cooldown: n("cooldown"),
                    },
                    "on_signal" => Op::OnSignal {
                        name: group("name"),
                        from: v.entity("group").map(|_| group("group")),
                        radius: n("radius"),
                        hold: n("hold"),
                    },
                    "brake" => Op::Brake,
                    "hover" => Op::Hover,
                    "boost" => Op::Boost,
                    "take_off" => Op::TakeOff {
                        height: n("height"),
                        finish: v.bool("finish_at_height").unwrap_or(true),
                    },
                    "land" => Op::Land {
                        pattern: n("pattern_height"),
                    },
                    "depart_airport" => Op::DepartAirport {
                        height: n("height"),
                        finish: v.bool("finish_at_height").unwrap_or(true),
                    },
                    "arrive_airport" => Op::ArriveAirport,
                    "altitude" => Op::Altitude {
                        height: n("height"),
                        finish_on_arrival: v.bool("finish_on_arrival").unwrap_or(false),
                        arrival_radius: n("arrival_radius"),
                    },
                    "return_home" => Op::ReturnHome {
                        slow: n("slow_radius"),
                        stop: n("stop_radius"),
                    },
                    "orbit" => Op::Orbit {
                        radius: n("radius"),
                        clockwise: v.bool("clockwise").unwrap_or(false),
                    },
                    "patrol" => {
                        let points: Vec<_> =
                            v.entities("waypoints").into_iter().map(object).collect();
                        objects.extend(points.iter().flatten());
                        Op::Patrol {
                            points,
                            mode: match v.entity("mode") {
                                Some("ping_pong") => 1,
                                Some("once") => 2,
                                _ => 0,
                            },
                            radius: n("arrive_radius"),
                        }
                    }
                    "follow_target" => Op::FollowTarget {
                        distance: n("distance"),
                    },
                    "separation" => Op::Separation {
                        group: group("group"),
                        radius: n("radius"),
                    },
                    "alignment" => Op::Alignment {
                        group: group("group"),
                        radius: n("radius"),
                    },
                    "cohesion" => Op::Cohesion {
                        group: group("group"),
                        radius: n("radius"),
                    },
                    "flock" => Op::Flock {
                        group: group("group"),
                        radius: n("radius"),
                        s: n("separation"),
                        a: n("alignment"),
                        c: n("cohesion"),
                    },
                    "follow_leader" => Op::FollowLeader {
                        distance: n("distance"),
                    },
                    "formation" => Op::Formation {
                        members: group("members"),
                        shape: match v.entity("shape") {
                            Some("line") => 0,
                            Some("ring") => 2,
                            _ => 1,
                        },
                        spacing: n("spacing"),
                    },
                    "avoid" => Op::Avoid {
                        radius: n("radius"),
                    },
                    "stay_within" => Op::StayWithin {
                        radius: n("radius"),
                    },
                    "attack" => Op::Attack {
                        range: n("range"),
                        prediction: n("prediction"),
                    },
                    "defend" => Op::Defend {
                        threats: group("threats"),
                        radius: n("radius"),
                        alert: n("alert"),
                    },
                    "hold_ground" => Op::HoldGround {
                        radius: n("radius"),
                    },
                    "retreat" => Op::Retreat {
                        threats: group("threats"),
                        panic: n("panic_distance"),
                    },
                    "flank" => Op::Flank {
                        offset: n("offset"),
                        side: match v.entity("side") {
                            Some("left") => -1,
                            Some("right") => 1,
                            _ => 0,
                        },
                    },
                    "encircle" => Op::Encircle {
                        members: group("members"),
                        radius: n("radius"),
                    },
                    "kite" => Op::Kite {
                        min: n("min_distance"),
                        max: n("max_distance"),
                    },
                    "regroup" => Op::Regroup {
                        group: group("group"),
                        radius: n("radius"),
                    },
                    "outnumbered" => Op::Outnumbered {
                        allies: group("allies"),
                        enemies: group("enemies"),
                        radius: n("radius"),
                        ratio: n("ratio"),
                    },
                    _ if def.plugin.is_some() => Op::Plugin {
                        plugin: def.plugin.expect("checked"),
                        params: crate::plugin::pack(&spec.fields, &v),
                    },
                    other => unreachable!("catalogue node without runtime: {other}"),
                };
                Node {
                    op,
                    weight: n("weight"),
                    target,
                    children: def.children.iter().map(|&c| c as u32).collect(),
                }
            })
            .collect();
        let needs_obstacles = nodes.iter().any(|node: &Node| {
            matches!(
                node.op,
                Op::LineOfSight { .. } | Op::Hide { .. } | Op::InView { sight: true, .. }
            )
        });
        Self {
            nodes,
            objects,
            needs_obstacles,
        }
    }
}

/// Movement settings from the `emergent.brain` component.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrainSettings {
    pub max_speed: f32,
    /// Maximum change of velocity per second; smooths turns.
    pub acceleration: f32,
    /// Ignore the vertical part of steering (ground agents without physics).
    pub planar: bool,
    /// Turn to face the direction of travel (objects without a dynamic body).
    pub face_movement: bool,
    /// How fast a brain's look turns the object, radians per second (0 = instantly).
    pub turn_rate: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct NodeState {
    index: u32,
    direction: i8,
    timer: f32,
    value: f32,
    vector: Vec3,
}

/// Runtime brain of one object.
#[derive(Component, Debug)]
pub struct BrainAgent {
    /// The stack: the main brain first, then any further brains; each keeps its own state.
    pub(crate) layers: Vec<Layer>,
    settings: BrainSettings,
    velocity: Vec3,
    home: Option<Vec3>,
}

#[derive(Debug)]
pub(crate) struct Layer {
    pub(crate) brain: Arc<CompiledBrain>,
    state: Vec<NodeState>,
    rng: u64,
}

impl BrainAgent {
    /// `seed` makes wander and chance replayable; scenes derive it from the instance ID.
    pub fn new(brain: Arc<CompiledBrain>, settings: BrainSettings, seed: u64) -> Self {
        Self::stacked(vec![brain], settings, seed)
    }

    /// Several brains on one object, run together each tick and merged by channel.
    pub fn stacked(brains: Vec<Arc<CompiledBrain>>, settings: BrainSettings, seed: u64) -> Self {
        let layers = brains
            .into_iter()
            .enumerate()
            .map(|(i, brain)| Layer {
                state: vec![NodeState::default(); brain.nodes.len()],
                brain,
                rng: (seed ^ (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)) | 1,
            })
            .collect();
        Self {
            layers,
            settings,
            velocity: Vec3::ZERO,
            home: None,
        }
    }

    pub fn velocity(&self) -> Vec3 {
        self.velocity
    }
}

/// Stable 64-bit seed from a string (FNV-1a).
pub fn seed_from(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry {
    pub id: EntityId,
    pub position: Vec3,
    pub velocity: Vec3,
    pub measured_velocity: Option<Vec3>,
    pub group: Option<u32>,
}

/// Committed positions for this tick plus reused buffers; owned by the world.
#[derive(Default)]
pub(crate) struct BrainScratch {
    pub entries: Vec<Entry>,
    pub index: HashMap<EntityId, usize>,
    pub last: HashMap<EntityId, Vec3>,
    pub moves: Vec<(EntityId, Vec3, bool, bool, Intent)>,
    pub obstacles: Vec<Obstacle>,
    /// Signals sent last tick (readable now) and this tick (readable next tick).
    pub signals: Vec<Signal>,
    pub sent: Vec<Signal>,
    /// Looks to apply after all movement this tick: object, look, turn rate.
    pub looks: Vec<(EntityId, Look, f32)>,
}

/// A solid box (a box physics collider) that blocks sight and offers cover.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Obstacle {
    pub id: EntityId,
    /// World to box-local (the collider's centre at the origin, scale included).
    pub inverse: crate::Mat4,
    pub half: Vec3,
    pub centre: Vec3,
    /// Radius of the box's footprint on the ground plane.
    pub radius: f32,
}

impl Obstacle {
    pub(crate) fn new(
        id: EntityId,
        model: crate::Mat4,
        collider: crate::BoxCollider,
    ) -> Option<Self> {
        let model = model * crate::Mat4::from_translation(Vec3::from_array(collider.center));
        let inverse = model.inverse();
        let half = Vec3::from_array(collider.half_extents);
        if !inverse.is_finite() || !half.is_finite() {
            return None;
        }
        let corner =
            |x: f32, z: f32| model.transform_vector3(Vec3::new(x * half.x, 0.0, z * half.z));
        let radius = [corner(1.0, 1.0), corner(1.0, -1.0)]
            .iter()
            .map(|v| Vec3::new(v.x, 0.0, v.z).length())
            .fold(0.0, f32::max);
        Some(Self {
            id,
            inverse,
            half,
            centre: model.transform_point3(Vec3::ZERO),
            radius,
        })
    }

    /// Whether the segment from `a` to `b` passes through the box (slab test in box space).
    fn blocks(&self, a: Vec3, b: Vec3) -> bool {
        let (o, d) = (
            self.inverse.transform_point3(a),
            self.inverse.transform_vector3(b - a),
        );
        let (mut near, mut far) = (0.0_f32, 1.0_f32);
        for axis in 0..3 {
            if d[axis].abs() < 1e-9 {
                if o[axis].abs() > self.half[axis] {
                    return false;
                }
                continue;
            }
            let t1 = (-self.half[axis] - o[axis]) / d[axis];
            let t2 = (self.half[axis] - o[axis]) / d[axis];
            near = near.max(t1.min(t2));
            far = far.min(t1.max(t2));
            if near > far {
                return false;
            }
        }
        true
    }
}

impl BrainScratch {
    fn get(&self, id: EntityId) -> Option<&Entry> {
        self.index.get(&id).map(|&i| &self.entries[i])
    }
    /// Nothing solid between `a` and `b`; the two objects themselves never block.
    fn clear(&self, a: Vec3, b: Vec3, ignore: [EntityId; 2]) -> bool {
        !self
            .obstacles
            .iter()
            .any(|o| !ignore.contains(&o.id) && o.blocks(a, b))
    }
    fn nearest(&self, group: u32, from: Vec3, except: EntityId) -> Option<&Entry> {
        self.entries
            .iter()
            .filter(|e| e.group == Some(group) && e.id != except)
            .min_by(|a, b| {
                a.position
                    .distance_squared(from)
                    .total_cmp(&b.position.distance_squared(from))
            })
    }
}

struct Ctx<'a> {
    id: EntityId,
    position: Vec3,
    velocity: Vec3,
    home: Vec3,
    speed: f32,
    dt: f32,
    snapshot: &'a BrainScratch,
}

/// What a brain asks of its object this tick. Objects with a locomotion profile interpret
/// it within their limits (a car brakes, a drone hovers); others just follow the velocity.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Intent {
    pub velocity: Vec3,
    /// Where it is heading (seek/arrive/attack target, waypoint), when known.
    pub target: Option<Vec3>,
    pub brake: bool,
    pub hover: bool,
    pub boost: bool,
    /// Requested height above the world origin.
    pub altitude: Option<f32>,
    /// A flight order for a physical aircraft (Take off, Land nodes); other movers ignore it.
    pub flight: Option<crate::fixed_wing::FlightGoal>,
}

/// What a brain wants the object to face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Look {
    Point(Vec3),
    Direction(Vec3),
}

/// A signal sent by a brain: visible to every brain and to game code on the next tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Signal {
    pub from: EntityId,
    /// Index into the scene's name interner ([`GroupNames`]).
    pub name: u32,
    pub target: Option<EntityId>,
    /// Sender's position when it was sent.
    pub position: Vec3,
}

#[derive(Clone, Copy, Default)]
struct Extra {
    target: Option<Vec3>,
    brake: bool,
    hover: bool,
    boost: bool,
    altitude: Option<f32>,
    flight: Option<crate::fixed_wing::FlightGoal>,
    look: Option<Look>,
    signal: Option<(u32, Option<EntityId>)>,
}

impl Extra {
    /// Blend: flags combine; the first child's target and altitude win.
    fn merge(self, other: Self) -> Self {
        Self {
            target: self.target.or(other.target),
            brake: self.brake || other.brake,
            hover: self.hover || other.hover,
            boost: self.boost || other.boost,
            altitude: self.altitude.or(other.altitude),
            flight: self.flight.or(other.flight),
            look: self.look.or(other.look),
            signal: self.signal.or(other.signal),
        }
    }
}

#[derive(Clone, Copy)]
struct Out {
    velocity: Vec3,
    score: f32,
    extra: Extra,
}

fn random(rng: &mut u64) -> f32 {
    // xorshift64*: replayable per agent, no allocation.
    *rng ^= *rng >> 12;
    *rng ^= *rng << 25;
    *rng ^= *rng >> 27;
    ((rng.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 40) as f32) / (1u64 << 24) as f32
}

fn heading(velocity: Vec3) -> Vec3 {
    velocity.try_normalize().unwrap_or(Vec3::NEG_Z)
}

fn target<'a>(node: &Node, ctx: &Ctx<'a>) -> Option<&'a Entry> {
    if let Some(object) = node.target.object {
        return ctx.snapshot.get(object);
    }
    node.target
        .group
        .and_then(|g| ctx.snapshot.nearest(g, ctx.position, ctx.id))
}

fn resolve(node: &Node, ctx: &Ctx) -> Option<(Vec3, Vec3)> {
    target(node, ctx).map(|e| (e.position, e.velocity))
}

fn arrive(ctx: &Ctx, point: Vec3, slow: f32, stop: f32) -> Option<Vec3> {
    let offset = point - ctx.position;
    let distance = offset.length();
    if distance <= stop {
        return None;
    }
    let speed = if slow > 0.0 {
        ctx.speed * (distance / slow).min(1.0)
    } else {
        ctx.speed
    };
    Some(offset / distance * speed)
}

/// Evaluates one node; `None` means inactive (failed condition, finished, no target).
fn eval(
    brain: &CompiledBrain,
    n: usize,
    ctx: &Ctx,
    state: &mut [NodeState],
    rng: &mut u64,
) -> Option<Out> {
    let node = &brain.nodes[n];
    let keep = |o: Out| Out {
        score: node.weight,
        ..o
    };
    let aim = |velocity: Vec3, target: Vec3| {
        Some(Out {
            velocity,
            score: node.weight,
            extra: Extra {
                target: Some(target),
                ..Extra::default()
            },
        })
    };
    let out = |velocity: Vec3| {
        Some(Out {
            velocity,
            score: node.weight,
            extra: Extra::default(),
        })
    };
    let child = |state: &mut [NodeState], rng: &mut u64, i: usize| {
        eval(brain, node.children[i] as usize, ctx, state, rng)
    };
    let neighbours = |group: u32, radius: f32| {
        ctx.snapshot.entries.iter().filter(move |e| {
            e.group == Some(group)
                && e.id != ctx.id
                && e.position.distance_squared(ctx.position) <= radius * radius
        })
    };
    let separation = |group, radius: f32| {
        neighbours(group, radius).fold(Vec3::ZERO, |sum, e| {
            let away = ctx.position - e.position;
            let d = away.length().max(1e-3);
            sum + away / d * (1.0 - d / radius.max(1e-3))
        }) * ctx.speed
    };
    let alignment = |group, radius| {
        let (sum, count) =
            neighbours(group, radius).fold((Vec3::ZERO, 0), |(s, c), e| (s + e.velocity, c + 1));
        if count == 0 {
            Vec3::ZERO
        } else {
            (sum / count as f32).clamp_length_max(ctx.speed)
        }
    };
    let cohesion = |group, radius| {
        let (sum, count) =
            neighbours(group, radius).fold((Vec3::ZERO, 0), |(s, c), e| (s + e.position, c + 1));
        if count == 0 {
            Vec3::ZERO
        } else {
            ((sum / count as f32) - ctx.position).clamp_length_max(1.0) * ctx.speed
        }
    };
    match &node.op {
        Op::Blend => {
            let mut sum = None::<(Vec3, Extra)>;
            for i in 0..node.children.len() {
                if let Some(o) = child(state, rng, i) {
                    let w = brain.nodes[node.children[i] as usize].weight;
                    let (v, extra) = sum.unwrap_or_default();
                    sum = Some((v + o.velocity * w, extra.merge(o.extra)));
                }
            }
            sum.map(|(v, extra)| Out {
                velocity: v.clamp_length_max(ctx.speed),
                score: node.weight,
                extra,
            })
        }
        Op::Priority => {
            // Per channel: movement from the first child that wants to move (or brake or
            // hover), look from the first that looks, signal from the first that sends one.
            let (mut moving, mut idle, mut look, mut signal) = (None, None, None, None);
            for i in 0..node.children.len() {
                if let Some(o) = child(state, rng, i) {
                    if moving.is_none()
                        && (o.velocity.length_squared() > 1e-6
                            || o.extra.brake
                            || o.extra.hover
                            || o.extra.flight.is_some())
                    {
                        moving = Some(o);
                    } else if idle.is_none() {
                        idle = Some(o);
                    }
                    look = look.or(o.extra.look);
                    signal = signal.or(o.extra.signal);
                    if moving.is_some() && look.is_some() && signal.is_some() {
                        break;
                    }
                }
            }
            moving.or(idle).map(|o| {
                let mut o = keep(o);
                o.extra.look = look;
                o.extra.signal = signal;
                o
            })
        }
        Op::Selector => (0..node.children.len())
            .find_map(|i| child(state, rng, i))
            .map(keep),
        Op::Sequence { looping } => {
            let count = node.children.len() as u32;
            // At most one advance per child per tick: bounded and never spins.
            for _ in 0..count {
                let current = state[n].index;
                if current >= count {
                    return None;
                }
                if let Some(o) = child(state, rng, current as usize) {
                    return Some(keep(o));
                }
                let next = current + 1;
                state[n].index = if next >= count && *looping { 0 } else { next };
            }
            None
        }
        Op::Utility => (0..node.children.len())
            .filter_map(|i| child(state, rng, i))
            .max_by(|a, b| a.score.total_cmp(&b.score))
            .map(keep),
        Op::Within { distance, inside } => {
            let near =
                resolve(node, ctx).is_some_and(|(p, _)| p.distance(ctx.position) < *distance);
            if near == *inside {
                child(state, rng, 0).map(keep)
            } else {
                None
            }
        }
        Op::InView {
            cos,
            distance,
            sight,
        } => {
            let seen_target = target(node, ctx)?;
            let p = seen_target.position;
            let offset = p - ctx.position;
            let seen = offset.length() <= *distance
                && offset
                    .try_normalize()
                    .is_some_and(|d| d.dot(heading(ctx.velocity)) >= *cos)
                && (!sight
                    || ctx
                        .snapshot
                        .clear(ctx.position, p, [ctx.id, seen_target.id]));
            if seen {
                child(state, rng, 0).map(keep)
            } else {
                None
            }
        }
        Op::Neighbours {
            group,
            radius,
            count,
        } => {
            if neighbours(*group, *radius).take(*count).count() >= *count {
                child(state, rng, 0).map(keep)
            } else {
                None
            }
        }
        Op::Chance {
            probability,
            interval,
        } => {
            let s = &mut state[n];
            s.timer -= ctx.dt;
            if s.timer <= 0.0 {
                s.timer = *interval;
                s.value = f32::from(u8::from(random(rng) < *probability));
            }
            if state[n].value > 0.5 {
                child(state, rng, 0).map(keep)
            } else {
                None
            }
        }
        Op::Duty { on, off } => {
            let s = &mut state[n];
            s.timer = (s.timer + ctx.dt) % (on + off).max(1e-3);
            if s.timer < *on {
                child(state, rng, 0).map(keep)
            } else {
                None
            }
        }
        Op::ScoreDistance { near, far, invert } => {
            let (p, _) = resolve(node, ctx)?;
            let d = p.distance(ctx.position);
            let t = ((d - near) / (far - near).max(1e-3)).clamp(0.0, 1.0);
            let score = if *invert { t } else { 1.0 - t } * node.weight;
            child(state, rng, 0).map(|o| Out { score, ..o })
        }
        Op::ScoreCrowding {
            group,
            radius,
            count,
        } => {
            let crowd = neighbours(*group, *radius).count() as f32;
            let score = (crowd / count.max(1.0)).min(1.0) * node.weight;
            child(state, rng, 0).map(|o| Out { score, ..o })
        }
        Op::SpeedLimit { max } => child(state, rng, 0).map(|o| Out {
            velocity: o.velocity.clamp_length_max(*max),
            ..o
        }),
        Op::LineOfSight { distance, invert } => {
            let visible = target(node, ctx).is_some_and(|t| {
                t.position.distance(ctx.position) <= *distance
                    && ctx.snapshot.clear(ctx.position, t.position, [ctx.id, t.id])
            });
            if visible != *invert {
                child(state, rng, 0).map(keep)
            } else {
                None
            }
        }
        Op::Hide {
            search,
            margin,
            max_cover,
            panic,
        } => {
            let threat = target(node, ctx)?;
            if *panic > 0.0 && threat.position.distance(ctx.position) > *panic {
                return None;
            }
            // Nearest hiding spot: just past each small enough obstacle, on the side away from the threat.
            let spot = ctx
                .snapshot
                .obstacles
                .iter()
                .filter(|o| o.id != ctx.id && o.id != threat.id && o.radius <= *max_cover)
                .filter(|o| o.centre.distance(ctx.position) <= *search)
                .filter_map(|o| {
                    let away = Vec3::new(
                        o.centre.x - threat.position.x,
                        0.0,
                        o.centre.z - threat.position.z,
                    )
                    .try_normalize()?;
                    let point = o.centre + away * (o.radius + *margin);
                    Some(Vec3::new(point.x, ctx.position.y, point.z))
                })
                .min_by(|a, b| {
                    a.distance_squared(ctx.position)
                        .total_cmp(&b.distance_squared(ctx.position))
                })?;
            out(arrive(ctx, spot, 2.0, 0.15).unwrap_or(Vec3::ZERO))
        }
        Op::Plugin { plugin, params } => out(plugin.0.steer(
            params,
            &BrainInput {
                position: ctx.position,
                velocity: ctx.velocity,
                home: ctx.home,
                max_speed: ctx.speed,
                dt: ctx.dt,
                target: resolve(node, ctx),
            },
        )?),
        Op::Seek => {
            let (p, _) = resolve(node, ctx)?;
            aim((p - ctx.position).try_normalize()? * ctx.speed, p)
        }
        Op::Flee { panic } => {
            let (p, _) = resolve(node, ctx)?;
            let away = ctx.position - p;
            if *panic > 0.0 && away.length() > *panic {
                return None;
            }
            out(away.try_normalize().unwrap_or(Vec3::X) * ctx.speed)
        }
        Op::Arrive { slow, stop } => {
            let (p, _) = resolve(node, ctx)?;
            arrive(ctx, p, *slow, *stop).and_then(|v| aim(v, p))
        }
        Op::Pursue { prediction } => {
            let (p, v) = resolve(node, ctx)?;
            let lead = (p.distance(ctx.position) / ctx.speed.max(1e-3)).min(*prediction);
            aim(
                (p + v * lead - ctx.position).try_normalize()? * ctx.speed,
                p + v * lead,
            )
        }
        Op::Evade { prediction, panic } => {
            let (p, v) = resolve(node, ctx)?;
            if *panic > 0.0 && p.distance(ctx.position) > *panic {
                return None;
            }
            let future = p + v * *prediction;
            out((ctx.position - future).try_normalize().unwrap_or(Vec3::X) * ctx.speed)
        }
        Op::Wander {
            radius,
            distance,
            jitter,
        } => {
            let s = &mut state[n];
            let step = Vec3::new(random(rng) - 0.5, random(rng) - 0.5, random(rng) - 0.5)
                * 2.0
                * *jitter
                * ctx.dt;
            s.vector = (s.vector + step).try_normalize().unwrap_or(Vec3::X) * *radius;
            let ahead = heading(ctx.velocity) * *distance + s.vector;
            out(ahead.try_normalize().unwrap_or(Vec3::NEG_Z) * ctx.speed)
        }
        Op::Hold => Some(Out {
            velocity: Vec3::ZERO,
            score: node.weight,
            extra: Extra {
                brake: true,
                ..Extra::default()
            },
        }),
        Op::LookAt { offset } => {
            let (p, _) = resolve(node, ctx)?;
            Some(Out {
                velocity: Vec3::ZERO,
                score: node.weight,
                extra: Extra {
                    look: Some(Look::Point(p + *offset)),
                    ..Extra::default()
                },
            })
        }
        Op::LookAlong => Some(Out {
            velocity: Vec3::ZERO,
            score: node.weight,
            extra: Extra {
                look: Some(Look::Direction(heading(ctx.velocity))),
                ..Extra::default()
            },
        }),
        Op::Scan { angle, period } => {
            let s = &mut state[n];
            s.timer = (s.timer + ctx.dt) % period.max(1e-3);
            let sweep = (s.timer / period.max(1e-3) * std::f32::consts::TAU).sin() * *angle;
            Some(Out {
                velocity: Vec3::ZERO,
                score: node.weight,
                extra: Extra {
                    look: Some(Look::Direction(
                        crate::Quat::from_rotation_y(sweep) * heading(ctx.velocity),
                    )),
                    ..Extra::default()
                },
            })
        }
        Op::Signal { name, cooldown } => {
            let s = &mut state[n];
            s.timer -= ctx.dt;
            if s.timer > 0.0 {
                return None;
            }
            s.timer = *cooldown;
            Some(Out {
                velocity: Vec3::ZERO,
                score: node.weight,
                extra: Extra {
                    signal: Some((*name, target(node, ctx).map(|t| t.id))),
                    ..Extra::default()
                },
            })
        }
        Op::OnSignal {
            name,
            from,
            radius,
            hold,
        } => {
            let heard = ctx.snapshot.signals.iter().any(|signal| {
                signal.name == *name
                    && signal.target.is_none_or(|t| t == ctx.id)
                    && signal.from != ctx.id
                    && from.is_none_or(|g| {
                        ctx.snapshot
                            .get(signal.from)
                            .is_some_and(|e| e.group == Some(g))
                    })
                    && (*radius <= 0.0 || signal.position.distance(ctx.position) <= *radius)
            });
            let s = &mut state[n];
            s.timer = if heard { *hold } else { s.timer - ctx.dt };
            if heard || s.timer > 0.0 {
                child(state, rng, 0).map(keep)
            } else {
                None
            }
        }
        Op::Brake | Op::Hover => Some(Out {
            velocity: Vec3::ZERO,
            score: node.weight,
            extra: Extra {
                brake: matches!(node.op, Op::Brake),
                hover: matches!(node.op, Op::Hover),
                ..Extra::default()
            },
        }),
        Op::TakeOff { height, finish } => {
            if *finish && ctx.position.y >= height - 10.0 {
                return None;
            }
            Some(Out {
                velocity: Vec3::ZERO,
                score: node.weight,
                extra: Extra {
                    flight: Some(crate::fixed_wing::FlightGoal::TakeOff { altitude: *height }),
                    altitude: Some(*height),
                    ..Extra::default()
                },
            })
        }
        Op::DepartAirport { height, finish } => {
            if *finish && ctx.position.y >= height - 10.0 {
                return None;
            }
            target(node, ctx).map(|airport| Out {
                velocity: Vec3::ZERO,
                score: node.weight,
                extra: Extra {
                    flight: Some(crate::fixed_wing::FlightGoal::Depart {
                        airport: airport.id,
                        altitude: *height,
                    }),
                    ..Extra::default()
                },
            })
        }
        Op::ArriveAirport => target(node, ctx).map(|airport| Out {
            velocity: Vec3::ZERO,
            score: node.weight,
            extra: Extra {
                flight: Some(crate::fixed_wing::FlightGoal::Arrive {
                    airport: airport.id,
                }),
                ..Extra::default()
            },
        }),
        Op::Land { pattern } => target(node, ctx).map(|zone| Out {
            velocity: Vec3::ZERO,
            score: node.weight,
            extra: Extra {
                flight: Some(crate::fixed_wing::FlightGoal::Land {
                    zone: zone.id,
                    pattern_height: *pattern,
                }),
                ..Extra::default()
            },
        }),
        Op::Boost => child(state, rng, 0).map(|o| Out {
            extra: Extra {
                boost: true,
                ..o.extra
            },
            ..keep(o)
        }),
        Op::Altitude {
            height,
            finish_on_arrival,
            arrival_radius,
        } => {
            if *finish_on_arrival && (ctx.position.y - height).abs() <= *arrival_radius {
                return None;
            }
            child(state, rng, 0).map(|o| Out {
                extra: Extra {
                    altitude: Some(*height),
                    ..o.extra
                },
                ..keep(o)
            })
        }
        Op::ReturnHome { slow, stop } => {
            arrive(ctx, ctx.home, *slow, *stop).and_then(|v| aim(v, ctx.home))
        }
        Op::Orbit { radius, clockwise } => {
            let centre = resolve(node, ctx).map_or(ctx.home, |(p, _)| p);
            let offset = Vec3::new(ctx.position.x - centre.x, 0.0, ctx.position.z - centre.z);
            let radial = offset.try_normalize().unwrap_or(Vec3::X);
            let tangent = if *clockwise {
                Vec3::Y.cross(radial)
            } else {
                radial.cross(Vec3::Y)
            };
            let correction = radial * (*radius - offset.length()).clamp(-1.0, 1.0);
            out((tangent + correction).try_normalize()? * ctx.speed)
        }
        Op::Patrol {
            points,
            mode,
            radius,
        } => {
            let count = points.len() as u32;
            let s = &mut state[n];
            if s.direction == 0 {
                s.direction = 1;
            }
            for _ in 0..=count {
                if s.index >= count {
                    return None;
                }
                let point = points[s.index as usize]
                    .and_then(|id| ctx.snapshot.get(id))
                    .map(|e| e.position);
                match point {
                    Some(p) if p.distance(ctx.position) > *radius => {
                        return aim(arrive(ctx, p, *radius * 2.0, 0.0).unwrap_or_default(), p);
                    }
                    _ => match mode {
                        0 => s.index = (s.index + 1) % count,
                        1 => {
                            if count > 1
                                && (s.index == 0 && s.direction < 0
                                    || s.index + 1 == count && s.direction > 0)
                            {
                                s.direction = -s.direction;
                            }
                            s.index = (s.index as i64 + i64::from(s.direction))
                                .clamp(0, i64::from(count) - 1)
                                as u32;
                        }
                        _ => s.index += 1,
                    },
                }
            }
            None
        }
        Op::FollowTarget { distance } => {
            let (p, _) = resolve(node, ctx)?;
            let behind = p + (ctx.position - p).try_normalize().unwrap_or(Vec3::Z) * *distance;
            out(arrive(ctx, behind, 2.0, 0.1).unwrap_or_default())
        }
        Op::Separation { group, radius } => out(separation(*group, *radius)),
        Op::Alignment { group, radius } => out(alignment(*group, *radius)),
        Op::Cohesion { group, radius } => out(cohesion(*group, *radius)),
        Op::Flock {
            group,
            radius,
            s,
            a,
            c,
        } => out((separation(*group, *radius * 0.5) * *s
            + alignment(*group, *radius) * *a
            + cohesion(*group, *radius) * *c)
            .clamp_length_max(ctx.speed)),
        Op::FollowLeader { distance } => {
            let (p, v) = resolve(node, ctx)?;
            let behind = p - heading(v) * *distance;
            out(arrive(ctx, behind, 2.0, 0.1).unwrap_or_default())
        }
        Op::Formation {
            members,
            shape,
            spacing,
        } => {
            let (p, v) = resolve(node, ctx)?;
            let slot = ctx
                .snapshot
                .entries
                .iter()
                .filter(|e| e.group == Some(*members))
                .position(|e| e.id == ctx.id)? as f32;
            let forward = heading(Vec3::new(v.x, 0.0, v.z));
            let right = forward.cross(Vec3::Y);
            let offset = match shape {
                0 => -forward * (slot + 1.0) * *spacing,
                2 => {
                    let total = ctx
                        .snapshot
                        .entries
                        .iter()
                        .filter(|e| e.group == Some(*members))
                        .count()
                        .max(1) as f32;
                    let angle = slot / total * std::f32::consts::TAU;
                    (forward * angle.cos() + right * angle.sin())
                        * *spacing
                        * (total / 3.0).max(1.0)
                }
                _ => {
                    let rank = (slot / 2.0).floor() + 1.0;
                    let side = if (slot as u32).is_multiple_of(2) {
                        1.0
                    } else {
                        -1.0
                    };
                    -forward * rank * *spacing + right * side * rank * *spacing
                }
            };
            out(arrive(ctx, p + offset, 2.0, 0.1).unwrap_or_default())
        }
        Op::Avoid { radius } => {
            let push = if let Some(object) = node.target.object {
                ctx.snapshot
                    .get(object)
                    .map(|e| vec![e.position])
                    .unwrap_or_default()
            } else {
                node.target
                    .group
                    .map(|g| neighbours(g, *radius).map(|e| e.position).collect())
                    .unwrap_or_default()
            };
            // shortcut: collects the few in-radius threats per evaluation; reuse a buffer if profiles show it.
            let away = push.iter().fold(Vec3::ZERO, |sum, p| {
                let d = ctx.position - *p;
                let len = d.length().max(1e-3);
                if len < *radius {
                    sum + d / len * (1.0 - len / radius.max(1e-3))
                } else {
                    sum
                }
            });
            if away == Vec3::ZERO {
                None
            } else {
                out(away.try_normalize()? * ctx.speed)
            }
        }
        Op::Attack { range, prediction } => {
            let (p, v) = resolve(node, ctx)?;
            let distance = p.distance(ctx.position);
            if distance <= *range {
                return out(Vec3::ZERO);
            }
            let lead = (distance / ctx.speed.max(1e-3)).min(*prediction);
            aim(
                (p + v * lead - ctx.position).try_normalize()? * ctx.speed,
                p + v * lead,
            )
        }
        Op::Defend {
            threats,
            radius,
            alert,
        } => {
            let protected = resolve(node, ctx).map_or(ctx.home, |(p, _)| p);
            let threat = ctx
                .snapshot
                .nearest(*threats, protected, ctx.id)
                .filter(|e| e.position.distance(protected) <= *alert);
            let post = match threat {
                Some(e) => {
                    protected
                        + (e.position - protected).try_normalize().unwrap_or(Vec3::X) * *radius
                }
                None if ctx.position.distance(protected) <= *radius => return out(Vec3::ZERO),
                None => protected,
            };
            out(
                arrive(ctx, post, 2.0, if threat.is_some() { 0.2 } else { *radius })
                    .unwrap_or_default(),
            )
        }
        Op::HoldGround { radius } => {
            if ctx.position.distance(ctx.home) <= *radius {
                return out(Vec3::ZERO);
            }
            out(arrive(ctx, ctx.home, 1.0, 0.05).unwrap_or_default())
        }
        Op::Retreat { threats, panic } => {
            let threat = ctx
                .snapshot
                .nearest(*threats, ctx.position, ctx.id)
                .filter(|e| e.position.distance(ctx.position) <= *panic)?;
            let away = (ctx.position - threat.position)
                .try_normalize()
                .unwrap_or(Vec3::X);
            let rally = resolve(node, ctx).map_or(ctx.home, |(p, _)| p);
            let toward = (rally - ctx.position).try_normalize().unwrap_or_default();
            out((away + toward * 0.75).try_normalize().unwrap_or(away) * ctx.speed)
        }
        Op::Flank { offset, side } => {
            let (p, v) = resolve(node, ctx)?;
            let forward = Vec3::new(v.x, 0.0, v.z)
                .try_normalize()
                .or_else(|| {
                    Vec3::new(p.x - ctx.position.x, 0.0, p.z - ctx.position.z).try_normalize()
                })
                .unwrap_or(Vec3::NEG_Z);
            let right = forward.cross(Vec3::Y);
            let sign = match side {
                0 if (ctx.position - p).dot(right) < 0.0 => -1.0,
                0 => 1.0,
                s => f32::from(*s),
            };
            let point = p + right * sign * *offset;
            // Swing out to the side first; once level with the enemy, close in.
            let aim = if ctx.position.distance(point) > 1.0
                && ctx.position.distance(p) > *offset * 0.75
            {
                point
            } else {
                p
            };
            out((aim - ctx.position).try_normalize()? * ctx.speed)
        }
        Op::Encircle { members, radius } => {
            let (p, _) = resolve(node, ctx)?;
            let ring = ctx
                .snapshot
                .entries
                .iter()
                .filter(|e| e.group == Some(*members));
            let total = ring.clone().count().max(1) as f32;
            let slot = ring.clone().position(|e| e.id == ctx.id)? as f32;
            let angle = slot / total * std::f32::consts::TAU;
            out(arrive(
                ctx,
                p + Vec3::new(angle.cos(), 0.0, angle.sin()) * *radius,
                2.0,
                0.2,
            )
            .unwrap_or_default())
        }
        Op::Kite { min, max } => {
            let (p, _) = resolve(node, ctx)?;
            let offset = p - ctx.position;
            let distance = offset.length();
            let toward = offset.try_normalize().unwrap_or(Vec3::NEG_Z);
            let direction = if distance < *min {
                -toward
            } else if distance > *max {
                toward
            } else {
                Vec3::Y.cross(toward) * 0.6
            };
            out(direction * ctx.speed)
        }
        Op::Regroup { group, radius } => {
            let (sum, count) = ctx
                .snapshot
                .entries
                .iter()
                .filter(|e| e.group == Some(*group) && e.id != ctx.id)
                .fold((Vec3::ZERO, 0), |(s, c), e| (s + e.position, c + 1));
            if count == 0 {
                return None;
            }
            let centre = sum / count as f32;
            if centre.distance(ctx.position) <= *radius {
                return None;
            }
            arrive(ctx, centre, *radius * 2.0, *radius * 0.5).and_then(out)
        }
        Op::Outnumbered {
            allies,
            enemies,
            radius,
            ratio,
        } => {
            let near = |g: u32| {
                ctx.snapshot
                    .entries
                    .iter()
                    .filter(|e| {
                        e.group == Some(g)
                            && e.id != ctx.id
                            && e.position.distance_squared(ctx.position) <= radius * radius
                    })
                    .count() as f32
            };
            let foes = near(*enemies);
            if foes > 0.0 && foes >= (near(*allies) + 1.0) * *ratio {
                child(state, rng, 0).map(keep)
            } else {
                None
            }
        }
        Op::StayWithin { radius } => {
            let centre = resolve(node, ctx).map_or(ctx.home, |(p, _)| p);
            let offset = centre - ctx.position;
            if offset.length() <= *radius {
                None
            } else {
                out(offset.try_normalize()? * ctx.speed)
            }
        }
    }
}

/// One fixed tick of every brain in the world.
pub(crate) fn step(world: &mut World, dt: Duration) -> Result<(), EngineError> {
    let mut scratch = world.take_brain_scratch();
    let seconds = dt.as_secs_f32();
    // Last tick's signals become readable; this tick's start empty.
    std::mem::swap(&mut scratch.signals, &mut scratch.sent);
    scratch.sent.clear();
    if let Some(mut published) = world.resource_mut::<PublishedSignals>() {
        published.0.clone_from(&scratch.signals);
    } else if !scratch.signals.is_empty() {
        world.insert_resource(PublishedSignals(scratch.signals.clone()));
    }
    if scratch.entries.is_empty() {
        world.restore_brain_scratch(scratch);
        return Ok(());
    }
    for entry in &mut scratch.entries {
        entry.velocity = entry.measured_velocity.unwrap_or_else(|| {
            scratch
                .last
                .get(&entry.id)
                .map_or(Vec3::ZERO, |last| (entry.position - *last) / seconds)
        });
    }
    let mut moves = std::mem::take(&mut scratch.moves);
    moves.clear();
    let mut sent = std::mem::take(&mut scratch.sent);
    let mut looks = std::mem::take(&mut scratch.looks);
    looks.clear();
    let snapshot = &scratch;
    world.for_each_brain_agent(|id, agent| {
        let Some(me) = snapshot.get(id) else { return };
        let home = *agent.home.get_or_insert(me.position);
        let ctx = Ctx {
            id,
            position: me.position,
            velocity: me.measured_velocity.unwrap_or(agent.velocity),
            home,
            speed: agent.settings.max_speed,
            dt: seconds,
            snapshot,
        };
        // Stacked brains: movement adds like a Blend, the first look wins, every signal is sent.
        let mut merged: Option<(Vec3, Extra)> = None;
        for layer in &mut agent.layers {
            let brain = Arc::clone(&layer.brain);
            let mut rng = layer.rng;
            let result = eval(&brain, 0, &ctx, &mut layer.state, &mut rng);
            layer.rng = rng;
            let Some(o) = result else { continue };
            if let Some((name, target)) = o.extra.signal {
                sent.push(Signal {
                    from: id,
                    name,
                    target,
                    position: me.position,
                });
            }
            merged = Some(match merged {
                Some((v, extra)) => (v + o.velocity, extra.merge(o.extra)),
                None => (o.velocity, o.extra),
            });
        }
        let (velocity, extra) = merged.unwrap_or_default();
        let mut desired = velocity.clamp_length_max(agent.settings.max_speed);
        if let Some(look) = extra.look {
            looks.push((id, look, agent.settings.turn_rate));
        }
        let intent = Intent {
            velocity: desired,
            target: extra.target,
            brake: extra.brake,
            hover: extra.hover,
            boost: extra.boost,
            altitude: extra.altitude,
            flight: extra.flight,
        };
        if agent.settings.planar {
            desired.y = 0.0;
        } else if let Some(height) = extra.altitude {
            desired.y =
                (height - me.position.y).clamp(-agent.settings.max_speed, agent.settings.max_speed);
        }
        let change =
            (desired - agent.velocity).clamp_length_max(agent.settings.acceleration * seconds);
        agent.velocity += change;
        moves.push((
            id,
            agent.velocity,
            agent.settings.planar,
            agent.settings.face_movement && extra.look.is_none(),
            intent,
        ));
    });
    for &(id, velocity, _, face, intent) in &moves {
        // A locomotion profile carries out the intent within its own limits.
        if let Some(mut locomotion) = world.component_mut::<crate::LocomotionAgent>(id)? {
            locomotion.intent = intent;
            continue;
        }
        let dynamic = matches!(world.component::<crate::PhysicsBody>(id)?,
            Some(body) if body.body == crate::BodyKind::Dynamic);
        #[cfg(feature = "physics")]
        if dynamic {
            world
                .drive_body(id, velocity, None)
                .map_err(EngineError::Physics)?;
            continue;
        }
        #[cfg(not(feature = "physics"))]
        let _ = dynamic;
        let position = world.global_transform(id)?.translation() + velocity * seconds;
        let mut local = world.local_from_global_position(id, position)?;
        if face {
            if let Some(direction) = velocity
                .try_normalize()
                .filter(|_| velocity.length() > 0.05)
            {
                // Rotation is set in the parent's space; brains on nested objects face approximately.
                local.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, direction);
            }
        }
        world.set_local_transform(id, local)?;
    }
    for entry in &scratch.entries {
        scratch.last.insert(entry.id, entry.position);
    }
    scratch.moves = moves;
    scratch.sent = sent;
    scratch.looks = looks;
    world.restore_brain_scratch(scratch);
    Ok(())
}

/// Signals sent on the previous tick, for game code ([`World::signals`]).
#[derive(Clone, Debug, Default, bevy_ecs::resource::Resource)]
pub struct PublishedSignals(pub Vec<Signal>);

/// Turns objects to face what their brains look at; runs after all other movement.
pub(crate) fn apply_looks(world: &mut World, dt: Duration) -> Result<(), EngineError> {
    let Some(mut scratch) = world.take_brain_scratch_only() else {
        return Ok(());
    };
    let result = (|| {
        if !scratch.looks.is_empty() {
            world.propagate_transforms();
        }
        for &(id, look, turn_rate) in &scratch.looks {
            if world
                .component::<crate::LocomotionAgent>(id)?
                .is_some_and(|agent| agent.model.is_physical())
            {
                return Err(EngineError::Physics(
                    crate::PhysicsError::LocomotionConflict,
                ));
            }
            let here = world.global_transform(id)?.translation();
            let direction = match look {
                Look::Point(point) => point - here,
                Look::Direction(direction) => direction,
            };
            let Some(goal) = crate::motion::look_rotation(direction) else {
                continue;
            };
            let mut local = *world.local_transform(id)?;
            // Written in the parent's space; nested objects aim approximately.
            local.rotation = if turn_rate > 0.0 {
                let angle = local.rotation.angle_between(goal);
                if angle > 1e-5 {
                    local
                        .rotation
                        .slerp(goal, (turn_rate * dt.as_secs_f32() / angle).min(1.0))
                } else {
                    goal
                }
            } else {
                goal
            };
            world.set_local_transform(id, local)?;
        }
        Ok(())
    })();
    scratch.looks.clear();
    world.restore_brain_scratch(scratch);
    result
}

impl World {
    /// Signals brains sent on the previous tick, with their names.
    pub fn signals(&self) -> impl Iterator<Item = (&Signal, &str)> {
        let names = self.resource::<GroupNames>();
        self.resource::<PublishedSignals>()
            .map(|s| s.0.as_slice())
            .unwrap_or_default()
            .iter()
            .map(move |signal| {
                let name = names
                    .and_then(|n| n.0.get(signal.name as usize))
                    .map_or("", String::as_str);
                (signal, name)
            })
    }
}

/// What the editor preview shows: agents running the brain among optional others.
#[derive(Clone, Debug)]
pub struct PreviewConfig {
    pub agents: usize,
    /// Group the agents belong to (so flocking nodes find each other).
    pub agent_group: String,
    pub targets: usize,
    pub target_group: String,
    /// Targets circle the arena at this speed (0 = stand still).
    pub target_speed: f32,
    pub ticks: u64,
    pub every: u64,
    pub seed: u64,
    pub settings: BrainSettings,
    /// Static box obstacles scattered across the arena (cover and sight blockers).
    pub obstacles: usize,
}

/// Positions per recorded frame: agents first, then targets.
#[derive(Clone, Debug, Default)]
pub struct PreviewTrace {
    pub agents: usize,
    pub targets: usize,
    pub frame_seconds: f32,
    pub frames: Vec<Vec<Vec3>>,
    /// Obstacle centres and footprint radii.
    pub obstacles: Vec<(Vec3, f32)>,
}

/// Runs the brain headlessly on a flat arena through the normal engine tick.
pub fn simulate(
    definition: &BrainDefinition,
    config: &PreviewConfig,
) -> Result<PreviewTrace, Box<dyn std::error::Error>> {
    if config.agents == 0
        || config.agents > 256
        || config.targets > 32
        || config.obstacles > 64
        || config.ticks > 60 * 120
        || config.every == 0
    {
        return Err(
            "preview supports 1-256 agents, 0-32 targets, 0-64 obstacles and up to 2 minutes"
                .into(),
        );
    }
    let mut engine = crate::Engine::new(crate::EngineConfig {
        fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
        ..crate::EngineConfig::default()
    })?;
    let world = engine.world_mut();
    let mut groups = GroupNames::default();
    let agent_group = groups.id(&config.agent_group);
    let target_group = groups.id(&config.target_group);
    let brain = Arc::new(CompiledBrain::compile(definition, &|_| None, &mut groups));
    let mut rng = config.seed | 1;
    let mut agents = Vec::new();
    for i in 0..config.agents {
        let id = world.spawn(format!("agent {i}"));
        let angle = random(&mut rng) * std::f32::consts::TAU;
        let r = 2.0 + random(&mut rng) * 8.0;
        world.set_local_transform(
            id,
            crate::Transform::from_translation(Vec3::new(angle.cos() * r, 0.0, angle.sin() * r)),
        )?;
        world.insert_component(id, Group(agent_group))?;
        world.insert_component(
            id,
            BrainAgent::new(
                Arc::clone(&brain),
                config.settings,
                config.seed ^ (i as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15),
            ),
        )?;
        agents.push(id);
    }
    // Obstacles are static boxes; they need the physics component, not a running physics step.
    let mut obstacles = Vec::new();
    for i in 0..config.obstacles {
        let id = world.spawn(format!("obstacle {i}"));
        let angle = random(&mut rng) * std::f32::consts::TAU;
        let r = 3.0 + random(&mut rng) * 7.0;
        let position = Vec3::new(angle.cos() * r, 0.0, angle.sin() * r);
        world.set_local_transform(id, crate::Transform::from_translation(position))?;
        let half = 0.6 + random(&mut rng) * 0.6;
        world.insert_component(
            id,
            crate::PhysicsBody::new(
                crate::BodyKind::Static,
                crate::BoxCollider {
                    center: [0.0; 3],
                    half_extents: [half, 1.0, half],
                },
            )?,
        )?;
        obstacles.push((position, half * std::f32::consts::SQRT_2));
    }
    let mut targets = Vec::new();
    for i in 0..config.targets {
        let id = world.spawn(format!("target {i}"));
        world.insert_component(id, Group(target_group))?;
        targets.push(id);
    }
    world.insert_resource(groups);
    struct Preview {
        agents: Vec<EntityId>,
        targets: Vec<EntityId>,
        speed: f32,
        every: u64,
        trace: PreviewTrace,
    }
    impl crate::Game for Preview {
        fn fixed_update(&mut self, context: &mut crate::EngineContext<'_>) {
            let t = context.tick as f32 / 60.0;
            let count = self.targets.len().max(1) as f32;
            for (i, &id) in self.targets.iter().enumerate() {
                let phase = i as f32 / count * std::f32::consts::TAU;
                let angle = phase + t * self.speed / 12.0;
                let position = Vec3::new(angle.cos() * 12.0, 0.0, angle.sin() * 12.0);
                let _ = context
                    .world
                    .set_local_transform(id, crate::Transform::from_translation(position));
            }
        }
        fn after_fixed_update(&mut self, context: &mut crate::EngineContext<'_>) {
            if context.tick.is_multiple_of(self.every) {
                let frame = self
                    .agents
                    .iter()
                    .chain(&self.targets)
                    .map(|&id| {
                        context
                            .world
                            .global_transform(id)
                            .map_or(Vec3::ZERO, |t| t.translation())
                    })
                    .collect();
                self.trace.frames.push(frame);
            }
        }
    }
    let mut preview = Preview {
        agents,
        targets,
        speed: config.target_speed,
        every: config.every,
        trace: PreviewTrace {
            agents: config.agents,
            targets: config.targets,
            frame_seconds: config.every as f32 / 60.0,
            frames: Vec::new(),
            obstacles,
        },
    };
    engine.run_ticks(&mut preview, config.ticks)?;
    Ok(preview.trace)
}
