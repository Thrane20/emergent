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

use crate::component::{validate_fields, ComponentField, ComponentValues, FieldKind};
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
        node("priority", "Priority", c[0], Composite, "Uses the first child that wants to move; later children only act when earlier ones are idle.", vec![]),
        node("selector", "Selector", c[0], Composite, "Uses the first child that is active (its conditions pass), even if it holds still.", vec![]),
        node("sequence", "Sequence", c[0], Composite, "Runs children one after another; moves on when the current child finishes (arrives or becomes inactive).", vec![f("loop", "Loop", Bool).default(true)]),
        node("utility", "Utility", c[0], Composite, "Uses the child with the highest score (a Score node's value, otherwise its weight).", vec![]),
        planned("random_choice", "Random choice", c[0], Composite, "Picks a child at random and keeps it for a while."),
        planned("repeat", "Repeat", c[0], Decorator, "Runs its child a number of times."),
        planned("invert", "Invert", c[0], Decorator, "Active when its child is not."),
        node("within_distance", "Within distance", c[1], Decorator, "Child is active only while the target is closer than the distance.", vec![target(), target_group(), num("distance", "Distance", 0.0, 10000.0, 5.0, "m")]),
        node("outside_distance", "Outside distance", c[1], Decorator, "Child is active only while the target is farther than the distance (or absent).", vec![target(), target_group(), num("distance", "Distance", 0.0, 10000.0, 5.0, "m")]),
        node("in_view", "In view", c[1], Decorator, "Child is active while the target is inside the view cone around the current heading.", vec![target(), target_group(), num("angle", "Half angle", 0.0, 180.0, 60.0, "°"), num("distance", "Range", 0.0, 10000.0, 15.0, "m")]),
        node("neighbours_at_least", "Neighbours at least", c[1], Decorator, "Child is active while enough group members are nearby.", vec![group("Members to count."), num("radius", "Radius", 0.0, 1000.0, 5.0, "m"), num("count", "Count", 1.0, 1024.0, 3.0, "")]),
        node("chance", "Chance", c[1], Decorator, "Re-rolls every interval; the child is active with this probability.", vec![num("probability", "Probability", 0.0, 1.0, 0.5, ""), num("interval", "Interval", 0.05, 3600.0, 2.0, "s")]),
        node("duty_cycle", "Duty cycle", c[1], Decorator, "Child is active for 'on' seconds, then rests for 'off' seconds.", vec![num("on", "On", 0.0, 3600.0, 2.0, "s"), num("off", "Off", 0.0, 3600.0, 2.0, "s")]),
        node("score_distance", "Score: distance", c[1], Decorator, "For Utility: scores 1 when the target is at 'near', falling to 0 at 'far' (or the reverse).", vec![target(), target_group(), num("near", "Near", 0.0, 10000.0, 0.0, "m"), num("far", "Far", 0.0, 10000.0, 20.0, "m"), f("invert", "Prefer far", Bool).default(false)]),
        node("score_crowding", "Score: crowding", c[1], Decorator, "For Utility: scores how crowded it is, from 0 to 1 at 'count' neighbours.", vec![group("Members to count."), num("radius", "Radius", 0.0, 1000.0, 5.0, "m"), num("count", "Full at", 1.0, 1024.0, 5.0, "")]),
        planned("line_of_sight", "Line of sight", c[1], Decorator, "Needs physics ray queries."),
        planned("stat_threshold", "Stat threshold", c[1], Decorator, "Needs object stats such as health."),
        node("seek", "Seek", c[2], Steering, "Heads straight for the target at full speed.", vec![target(), target_group()]),
        node("flee", "Flee", c[2], Steering, "Runs directly away from the target while it is within the panic distance (0 = always).", vec![target(), target_group(), num("panic_distance", "Panic distance", 0.0, 10000.0, 10.0, "m")]),
        node("arrive", "Arrive", c[2], Steering, "Like seek, but slows inside the slowing radius and finishes inside the stop radius.", vec![target(), target_group(), num("slow_radius", "Slowing radius", 0.0, 1000.0, 3.0, "m"), num("stop_radius", "Stop radius", 0.0, 1000.0, 0.5, "m")]),
        node("pursue", "Pursue", c[2], Steering, "Seeks where the target will be, predicting from its velocity.", vec![target(), target_group(), num("prediction", "Look ahead", 0.0, 10.0, 1.0, "s")]),
        node("evade", "Evade", c[2], Steering, "Flees from where the target will be, while it is within the panic distance.", vec![target(), target_group(), num("prediction", "Look ahead", 0.0, 10.0, 1.0, "s"), num("panic_distance", "Panic distance", 0.0, 10000.0, 10.0, "m")]),
        planned("hide", "Hide", c[2], Steering, "Needs obstacle queries."),
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
        node("speed_limit", "Speed limit", c[7], Decorator, "Caps its child's speed.", vec![num("max_speed", "Max speed", 0.0, 100.0, 1.0, "m/s")]),
        planned("jump", "Jump", c[7], Steering, "Needs ground contact queries."),
        planned("signal", "Signal", c[7], Steering, "Needs an event system."),
        node("brain", "Saved brain", c[8], Reference, "Embeds another saved brain here.", vec![f("brain", "Brain", Brain)]),
    ];
    for node in &mut nodes {
        node.fields.push(
            num("weight", "Weight", 0.0, 100.0, 1.0, "").help("Blend weight or Utility score."),
        );
        node.fields
            .push(f("label", "Label", Text).help("Shown in the editor."));
    }
    nodes
}

/// Node catalogue export for the editor library, inspector forms and AI prompts.
pub fn export_node_catalogue() -> Result<String, toml::ser::Error> {
    #[derive(Serialize)]
    struct Export {
        categories: &'static [&'static str],
        nodes: Vec<NodeSpec>,
    }
    toml::to_string(&Export {
        categories: CATEGORIES,
        nodes: node_catalogue(),
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
        let source = project
            .read_text_asset(path)
            .map_err(|e| format!("{path}: {e}"))?;
        Self::from_source(project, path, &source)
    }

    /// Validates a candidate (editor or AI) against the catalogue and the project's other brains.
    pub fn from_source(project: &Project, path: &str, source: &str) -> Result<Self, String> {
        let catalogue = node_catalogue();
        let mut stack = vec![path.to_string()];
        let mut definition = Self {
            path: path.into(),
            name: String::new(),
            nodes: Vec::new(),
        };
        definition.name = parse_into(
            project,
            &catalogue,
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
        );
        if needs_group && values.entity("group").is_none() {
            return Err(format!("node '{}' ({}) needs a group", node.id, spec.label));
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
            let (_, root) = parse_into(project, catalogue, &source, stack, out, depth)
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
    StayWithin {
        radius: f32,
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
        let nodes = definition
            .nodes
            .iter()
            .map(|def| {
                let spec = catalogue
                    .iter()
                    .find(|s| s.id == def.kind)
                    .expect("validated node");
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
        Self { nodes, objects }
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
    pub(crate) brain: Arc<CompiledBrain>,
    settings: BrainSettings,
    state: Vec<NodeState>,
    velocity: Vec3,
    home: Option<Vec3>,
    rng: u64,
}

impl BrainAgent {
    /// `seed` makes wander and chance replayable; scenes derive it from the instance ID.
    pub fn new(brain: Arc<CompiledBrain>, settings: BrainSettings, seed: u64) -> Self {
        Self {
            state: vec![NodeState::default(); brain.nodes.len()],
            brain,
            settings,
            velocity: Vec3::ZERO,
            home: None,
            rng: seed | 1,
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
    pub group: Option<u32>,
}

/// Committed positions for this tick plus reused buffers; owned by the world.
#[derive(Default)]
pub(crate) struct BrainScratch {
    pub entries: Vec<Entry>,
    pub index: HashMap<EntityId, usize>,
    pub last: HashMap<EntityId, Vec3>,
    pub moves: Vec<(EntityId, Vec3, bool, bool)>,
}

impl BrainScratch {
    fn get(&self, id: EntityId) -> Option<&Entry> {
        self.index.get(&id).map(|&i| &self.entries[i])
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

#[derive(Clone, Copy)]
struct Out {
    velocity: Vec3,
    score: f32,
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

fn resolve(node: &Node, ctx: &Ctx) -> Option<(Vec3, Vec3)> {
    if let Some(object) = node.target.object {
        return ctx.snapshot.get(object).map(|e| (e.position, e.velocity));
    }
    node.target
        .group
        .and_then(|g| ctx.snapshot.nearest(g, ctx.position, ctx.id))
        .map(|e| (e.position, e.velocity))
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
    let out = |velocity: Vec3| {
        Some(Out {
            velocity,
            score: node.weight,
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
            let mut sum = None::<Vec3>;
            for i in 0..node.children.len() {
                if let Some(o) = child(state, rng, i) {
                    let w = brain.nodes[node.children[i] as usize].weight;
                    sum = Some(sum.unwrap_or_default() + o.velocity * w);
                }
            }
            sum.and_then(|v| out(v.clamp_length_max(ctx.speed)))
        }
        Op::Priority => {
            let mut idle = None;
            for i in 0..node.children.len() {
                if let Some(o) = child(state, rng, i) {
                    if o.velocity.length_squared() > 1e-6 {
                        return out(o.velocity);
                    }
                    idle = Some(o.velocity);
                }
            }
            idle.and_then(out)
        }
        Op::Selector => (0..node.children.len())
            .find_map(|i| child(state, rng, i))
            .and_then(|o| out(o.velocity)),
        Op::Sequence { looping } => {
            let count = node.children.len() as u32;
            // At most one advance per child per tick: bounded and never spins.
            for _ in 0..count {
                let current = state[n].index;
                if current >= count {
                    return None;
                }
                if let Some(o) = child(state, rng, current as usize) {
                    return out(o.velocity);
                }
                let next = current + 1;
                state[n].index = if next >= count && *looping { 0 } else { next };
            }
            None
        }
        Op::Utility => (0..node.children.len())
            .filter_map(|i| child(state, rng, i))
            .max_by(|a, b| a.score.total_cmp(&b.score))
            .and_then(|o| out(o.velocity)),
        Op::Within { distance, inside } => {
            let near =
                resolve(node, ctx).is_some_and(|(p, _)| p.distance(ctx.position) < *distance);
            if near == *inside {
                child(state, rng, 0).and_then(|o| out(o.velocity))
            } else {
                None
            }
        }
        Op::InView { cos, distance } => {
            let (p, _) = resolve(node, ctx)?;
            let offset = p - ctx.position;
            let seen = offset.length() <= *distance
                && offset
                    .try_normalize()
                    .is_some_and(|d| d.dot(heading(ctx.velocity)) >= *cos);
            if seen {
                child(state, rng, 0).and_then(|o| out(o.velocity))
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
                child(state, rng, 0).and_then(|o| out(o.velocity))
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
                child(state, rng, 0).and_then(|o| out(o.velocity))
            } else {
                None
            }
        }
        Op::Duty { on, off } => {
            let s = &mut state[n];
            s.timer = (s.timer + ctx.dt) % (on + off).max(1e-3);
            if s.timer < *on {
                child(state, rng, 0).and_then(|o| out(o.velocity))
            } else {
                None
            }
        }
        Op::ScoreDistance { near, far, invert } => {
            let (p, _) = resolve(node, ctx)?;
            let d = p.distance(ctx.position);
            let t = ((d - near) / (far - near).max(1e-3)).clamp(0.0, 1.0);
            let score = if *invert { t } else { 1.0 - t } * node.weight;
            child(state, rng, 0).map(|o| Out {
                velocity: o.velocity,
                score,
            })
        }
        Op::ScoreCrowding {
            group,
            radius,
            count,
        } => {
            let crowd = neighbours(*group, *radius).count() as f32;
            let score = (crowd / count.max(1.0)).min(1.0) * node.weight;
            child(state, rng, 0).map(|o| Out {
                velocity: o.velocity,
                score,
            })
        }
        Op::SpeedLimit { max } => child(state, rng, 0).map(|o| Out {
            velocity: o.velocity.clamp_length_max(*max),
            score: o.score,
        }),
        Op::Seek => {
            let (p, _) = resolve(node, ctx)?;
            out((p - ctx.position).try_normalize()? * ctx.speed)
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
            arrive(ctx, p, *slow, *stop).and_then(out)
        }
        Op::Pursue { prediction } => {
            let (p, v) = resolve(node, ctx)?;
            let lead = (p.distance(ctx.position) / ctx.speed.max(1e-3)).min(*prediction);
            out((p + v * lead - ctx.position).try_normalize()? * ctx.speed)
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
        Op::Hold => out(Vec3::ZERO),
        Op::ReturnHome { slow, stop } => arrive(ctx, ctx.home, *slow, *stop).and_then(out),
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
                        return out(arrive(ctx, p, *radius * 2.0, 0.0).unwrap_or_default());
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
    if scratch.entries.is_empty() {
        world.restore_brain_scratch(scratch);
        return Ok(());
    }
    for entry in &mut scratch.entries {
        entry.velocity = scratch
            .last
            .get(&entry.id)
            .map_or(Vec3::ZERO, |last| (entry.position - *last) / seconds);
    }
    let mut moves = std::mem::take(&mut scratch.moves);
    moves.clear();
    let snapshot = &scratch;
    world.for_each_brain_agent(|id, agent| {
        let Some(me) = snapshot.get(id) else { return };
        let home = *agent.home.get_or_insert(me.position);
        let ctx = Ctx {
            id,
            position: me.position,
            velocity: agent.velocity,
            home,
            speed: agent.settings.max_speed,
            dt: seconds,
            snapshot,
        };
        let brain = Arc::clone(&agent.brain);
        let mut rng = agent.rng;
        let mut desired = eval(&brain, 0, &ctx, &mut agent.state, &mut rng)
            .map_or(Vec3::ZERO, |o| o.velocity)
            .clamp_length_max(agent.settings.max_speed);
        agent.rng = rng;
        if agent.settings.planar {
            desired.y = 0.0;
        }
        let change =
            (desired - agent.velocity).clamp_length_max(agent.settings.acceleration * seconds);
        agent.velocity += change;
        moves.push((
            id,
            agent.velocity,
            agent.settings.planar,
            agent.settings.face_movement,
        ));
    });
    for &(id, velocity, _, face) in &moves {
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
    world.restore_brain_scratch(scratch);
    Ok(())
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
}

/// Positions per recorded frame: agents first, then targets.
#[derive(Clone, Debug, Default)]
pub struct PreviewTrace {
    pub agents: usize,
    pub targets: usize,
    pub frame_seconds: f32,
    pub frames: Vec<Vec<Vec3>>,
}

/// Runs the brain headlessly on a flat arena through the normal engine tick.
pub fn simulate(
    definition: &BrainDefinition,
    config: &PreviewConfig,
) -> Result<PreviewTrace, Box<dyn std::error::Error>> {
    if config.agents == 0
        || config.agents > 256
        || config.targets > 32
        || config.ticks > 60 * 120
        || config.every == 0
    {
        return Err("preview supports 1-256 agents, 0-32 targets and up to 2 minutes".into());
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
        },
    };
    engine.run_ticks(&mut preview, config.ticks)?;
    Ok(preview.trace)
}
