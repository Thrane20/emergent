//! Scene map: waypoints, zones and name/value metadata placed in the scene, indexed for
//! runtime queries (zones at a point, travel cost along a segment, waypoint routes).
//!
//! Map items are ordinary entities with the components below; identity stays with
//! `EntityId`. The components are immutable, so every change goes through an insert,
//! which bumps a generation counter. [`refresh`] rebuilds the index only when that counter
//! moves, and otherwise just re-reads positions, so steady-state ticks don't allocate.

use crate::{EntityId, Vec3, World};
use bevy_ecs::{
    component::Component, lifecycle::HookContext, resource::Resource, world::DeferredWorld,
};
use glam::Vec2;
use std::collections::HashMap;

/// Most name/value pairs one object's metadata may hold.
pub const MAP_MAX_PAIRS: usize = 32;
/// Distinct zone layer names per world (each becomes a bit).
pub const MAP_MAX_LAYERS: usize = 32;
/// Grid cell size for the zone index, in metres.
const CELL: f32 = 16.0;
/// Segment costs are sampled about this often, in metres.
const SEGMENT_STEP: f32 = 1.0;
const MAX_SEGMENT_SAMPLES: usize = 4096;

#[derive(Debug, Default, Resource)]
pub(crate) struct MapGeneration(pub(crate) u64);

fn map_changed(mut world: DeferredWorld, _context: HookContext) {
    if let Some(mut generation) = world.get_resource_mut::<MapGeneration>() {
        generation.0 = generation.0.wrapping_add(1);
    }
}

/// A point in a waypoint network. Its position is the entity's.
#[derive(Clone, Component, Debug, PartialEq)]
#[component(immutable, on_insert = map_changed, on_discard = map_changed)]
pub struct Waypoint {
    /// Waypoints this one leads to.
    pub links: Vec<EntityId>,
    /// Links can be travelled both ways.
    pub two_way: bool,
    /// Arrival radius in metres.
    pub radius: f32,
    /// Compass courses (radians, clockwise from the first to the second) to cross this point
    /// on, and to leave it on. `None`: any course.
    pub entry: Option<(f32, f32)>,
    pub exit: Option<(f32, f32)>,
    /// Height to cross at; a traffic pattern measures it above the runway.
    pub altitude: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ZoneShape {
    Circle {
        radius: f32,
    },
    /// Width along the entity's local X, depth along its local Z; turns with its yaw.
    Rect {
        width: f32,
        depth: f32,
    },
}

/// An area of the scene, centred on its entity.
#[derive(Clone, Component, Debug, PartialEq)]
#[component(immutable, on_insert = map_changed, on_discard = map_changed)]
pub struct Zone {
    pub shape: ZoneShape,
    /// Vertical extent above the entity in metres; 0 is unbounded.
    pub height: f32,
    /// Travel cost multiplier: 1 normal, below 1 preferred, above 1 avoided,
    /// `f32::INFINITY` never entered.
    pub cost: f32,
    /// Layers the cost applies to (e.g. "air"); empty applies to every layer.
    pub layers: Vec<String>,
}

impl Zone {
    pub fn blocked(&self) -> bool {
        self.cost.is_infinite()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum MetaValue {
    Text(String),
    Number(f64),
    Bool(bool),
}

impl From<&str> for MetaValue {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}
impl From<f64> for MetaValue {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}
impl From<bool> for MetaValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

/// Name/value pairs on any object (`type = "homebase"`), queryable at runtime.
#[derive(Clone, Component, Debug, Default, PartialEq)]
#[component(immutable, on_insert = map_changed, on_discard = map_changed)]
pub struct Metadata(pub Vec<(String, MetaValue)>);

impl Metadata {
    pub fn get(&self, key: &str) -> Option<&MetaValue> {
        self.0
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapError {
    /// The entity is not a waypoint (or was deleted).
    NotAWaypoint(EntityId),
    /// No route avoids blocked zones.
    NoRoute,
}

impl std::fmt::Display for MapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAWaypoint(id) => write!(f, "{id:?} is not a waypoint"),
            Self::NoRoute => f.write_str("no route avoids the blocked zones"),
        }
    }
}

impl std::error::Error for MapError {}

/// A zone resolved to world space.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ZoneEntry {
    entity: EntityId,
    center: Vec2,
    base: f32,
    /// World X axis of the zone, projected on XZ and normalised.
    axis: Vec2,
    /// The component's shape; `shape` is it scaled to world size.
    local: ZoneShape,
    shape: ZoneShape,
    height: f32,
    cost: f32,
    /// Layer bits; 0 means every layer.
    mask: u32,
}

impl ZoneEntry {
    fn contains(&self, point: Vec3) -> bool {
        if point.y < self.base - 1e-3 || (self.height > 0.0 && point.y > self.base + self.height) {
            return false;
        }
        let offset = Vec2::new(point.x, point.z) - self.center;
        match self.shape {
            ZoneShape::Circle { radius } => offset.length_squared() <= radius * radius,
            ZoneShape::Rect { width, depth } => {
                let x = offset.dot(self.axis);
                let z = offset.dot(Vec2::new(-self.axis.y, self.axis.x));
                x.abs() <= width * 0.5 && z.abs() <= depth * 0.5
            }
        }
    }

    fn reach(&self) -> f32 {
        match self.shape {
            ZoneShape::Circle { radius } => radius,
            ZoneShape::Rect { width, depth } => (width * width + depth * depth).sqrt() * 0.5,
        }
    }

    fn applies(&self, layer: Option<u32>) -> bool {
        match layer {
            None => true,
            Some(bit) => self.mask == 0 || self.mask & bit != 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WaypointEntry {
    entity: EntityId,
    position: Vec3,
}

#[derive(Default)]
struct PathScratch {
    open: std::collections::BinaryHeap<Open>,
    cost: Vec<f32>,
    from: Vec<u32>,
}

#[derive(PartialEq)]
struct Open {
    estimate: f32,
    node: u32,
}
impl Eq for Open {}
impl Ord for Open {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reversed: BinaryHeap is a max-heap and A* wants the lowest estimate.
        other.estimate.total_cmp(&self.estimate)
    }
}
impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The world's map index. Owned by [`World`]; read through its map queries.
pub(crate) struct SceneMap {
    generation: u64,
    layers: Vec<String>,
    zones: Vec<ZoneEntry>,
    waypoints: Vec<WaypointEntry>,
    waypoint_index: HashMap<EntityId, u32>,
    /// Compressed adjacency: links of waypoint `i` are `edges[starts[i]..starts[i + 1]]`.
    starts: Vec<u32>,
    edges: Vec<u32>,
    tagged: Vec<EntityId>,
    grid: HashMap<(i32, i32), Vec<u32>>,
    /// Cheapest multiplier anywhere, for an admissible A* estimate.
    min_cost: f32,
    scratch: PathScratch,
    /// Edge costs per layer, parallel to `edges`; NaN until first needed. Cleared when a
    /// zone or waypoint moves or the map changes.
    edge_costs: Vec<(Option<u32>, Vec<f32>)>,
}

impl Default for SceneMap {
    fn default() -> Self {
        Self {
            generation: u64::MAX,
            layers: Vec::new(),
            zones: Vec::new(),
            waypoints: Vec::new(),
            waypoint_index: HashMap::new(),
            starts: vec![0],
            edges: Vec::new(),
            tagged: Vec::new(),
            grid: HashMap::new(),
            min_cost: 1.0,
            scratch: PathScratch::default(),
            edge_costs: Vec::new(),
        }
    }
}

fn cell(x: f32) -> i32 {
    (x / CELL).floor() as i32
}

impl SceneMap {
    fn layer_bit(&self, name: &str) -> Option<u32> {
        self.layers
            .iter()
            .position(|layer| layer == name)
            .map(|i| 1 << i)
    }

    fn layer_mask(&mut self, names: &[String]) -> u32 {
        let mut mask = 0;
        for name in names {
            let index = match self.layers.iter().position(|layer| layer == name) {
                Some(index) => index,
                None if self.layers.len() < MAP_MAX_LAYERS => {
                    self.layers.push(name.clone());
                    self.layers.len() - 1
                }
                // shortcut: beyond 32 layer names the extra ones apply to every layer; widen the mask if a game needs more.
                None => return 0,
            };
            mask |= 1 << index;
        }
        mask
    }

    fn rebuild_grid(&mut self) {
        for bucket in self.grid.values_mut() {
            bucket.clear();
        }
        for (index, zone) in self.zones.iter().enumerate() {
            let reach = zone.reach();
            for x in cell(zone.center.x - reach)..=cell(zone.center.x + reach) {
                for z in cell(zone.center.y - reach)..=cell(zone.center.y + reach) {
                    self.grid.entry((x, z)).or_default().push(index as u32);
                }
            }
        }
        self.min_cost = self
            .zones
            .iter()
            .fold(1.0f32, |least, zone| least.min(zone.cost));
    }

    fn zones_at(&self, point: Vec3) -> impl Iterator<Item = &ZoneEntry> + '_ {
        self.grid
            .get(&(cell(point.x), cell(point.z)))
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .map(|&index| &self.zones[index as usize])
            .filter(move |zone| zone.contains(point))
    }

    fn cost_at(&self, point: Vec3, layer: Option<u32>) -> f32 {
        self.zones_at(point)
            .filter(|zone| zone.applies(layer))
            .fold(1.0, |cost, zone| cost * zone.cost)
    }

    /// Length × average cost along a segment; infinite if it crosses a blocked zone.
    fn segment_cost(&self, a: Vec3, b: Vec3, layer: Option<u32>) -> f32 {
        let length = a.distance(b);
        if self.zones.is_empty() {
            return length;
        }
        let samples = ((length / SEGMENT_STEP).ceil() as usize).clamp(1, MAX_SEGMENT_SAMPLES);
        let mut total = 0.0;
        for i in 0..samples {
            // Midpoints of equal steps, so both ends count the same.
            let cost = self.cost_at(a.lerp(b, (i as f32 + 0.5) / samples as f32), layer);
            if cost.is_infinite() {
                return f32::INFINITY;
            }
            total += cost;
        }
        length * total / samples as f32
    }
}

/// Re-indexes map components after changes and re-reads positions. Runs every fixed tick
/// and after scene construction.
pub(crate) fn refresh(world: &mut World) {
    let mut map = world.take_scene_map();
    let generation = world
        .resource::<MapGeneration>()
        .map_or(0, |generation| generation.0);
    if generation != map.generation {
        map.generation = generation;
        map.zones.clear();
        map.waypoints.clear();
        map.waypoint_index.clear();
        map.tagged.clear();
        let mut zones = Vec::new();
        world.for_each_map_zone(|id, zone| zones.push((id, zone.clone())));
        for (entity, zone) in zones {
            let mask = map.layer_mask(&zone.layers);
            map.zones.push(ZoneEntry {
                entity,
                center: Vec2::ZERO,
                base: 0.0,
                axis: Vec2::X,
                local: zone.shape,
                shape: zone.shape,
                height: zone.height,
                cost: zone.cost,
                mask,
            });
        }
        let mut links = Vec::new();
        world.for_each_map_waypoint(|id, waypoint| links.push((id, waypoint.clone())));
        for (index, (entity, _)) in links.iter().enumerate() {
            map.waypoints.push(WaypointEntry {
                entity: *entity,
                position: Vec3::ZERO,
            });
            map.waypoint_index.insert(*entity, index as u32);
        }
        let mut outgoing: Vec<Vec<u32>> = vec![Vec::new(); links.len()];
        for (index, (_, waypoint)) in links.iter().enumerate() {
            for target in &waypoint.links {
                let Some(&to) = map.waypoint_index.get(target) else {
                    continue;
                };
                outgoing[index].push(to);
                if waypoint.two_way {
                    outgoing[to as usize].push(index as u32);
                }
            }
        }
        map.starts.clear();
        map.edges.clear();
        map.starts.push(0);
        for mut targets in outgoing {
            targets.sort_unstable();
            targets.dedup();
            map.edges.extend(targets);
            map.starts.push(map.edges.len() as u32);
        }
        world.for_each_metadata_entity(|id| map.tagged.push(id));
    }
    // Positions follow transforms every tick; the grid is rebuilt only if a zone moved.
    let mut moved = generation != map.generation || map.grid.is_empty();
    for zone in &mut map.zones {
        let Ok(transform) = world.global_transform(zone.entity) else {
            continue;
        };
        let matrix = transform.matrix();
        let center = transform.translation();
        let x_axis = matrix.x_axis.truncate();
        let z_axis = matrix.z_axis.truncate();
        let axis = Vec2::new(x_axis.x, x_axis.z)
            .try_normalize()
            .unwrap_or(Vec2::X);
        // Scale carries through: a zone under a scaled parent grows with it.
        let shape = match zone.local {
            ZoneShape::Circle { radius } => ZoneShape::Circle {
                radius: radius * x_axis.length().max(z_axis.length()),
            },
            ZoneShape::Rect { width, depth } => ZoneShape::Rect {
                width: width * x_axis.length(),
                depth: depth * z_axis.length(),
            },
        };
        let next = ZoneEntry {
            center: Vec2::new(center.x, center.z),
            base: center.y,
            axis,
            shape,
            ..*zone
        };
        if next != *zone {
            *zone = next;
            moved = true;
        }
    }
    let mut moved_waypoints = false;
    for waypoint in &mut map.waypoints {
        if let Ok(transform) = world.global_transform(waypoint.entity) {
            let position = transform.translation();
            moved_waypoints |= position != waypoint.position;
            waypoint.position = position;
        }
    }
    if moved {
        map.rebuild_grid();
    }
    if moved || moved_waypoints {
        map.edge_costs.clear();
    }
    world.restore_scene_map(map);
}

/// Map queries. Positions are as of the last fixed tick (or scene construction).
impl World {
    /// Zones containing `point` (base to top, shape on the XZ plane).
    pub fn zones_at(&self, point: Vec3) -> impl Iterator<Item = EntityId> + '_ {
        self.scene_map()
            .zones_at(point)
            .filter(|zone| self.global_transform(zone.entity).is_ok())
            .map(|zone| zone.entity)
    }

    /// Travel cost multiplier at `point` for a layer (`None`: every zone counts).
    /// Overlapping zones multiply; a blocked zone gives `f32::INFINITY`.
    pub fn cost_at(&self, point: Vec3, layer: Option<&str>) -> f32 {
        let map = self.scene_map();
        match layer.map(|name| map.layer_bit(name)) {
            // A layer no zone names: only zones for every layer apply.
            Some(None) => map.cost_at(point, Some(0)),
            Some(Some(bit)) => map.cost_at(point, Some(bit)),
            None => map.cost_at(point, None),
        }
    }

    /// Distance from `a` to `b` weighted by the zone costs along it; infinite if it
    /// crosses a blocked zone. Compare routes by this, not by distance.
    pub fn segment_cost(&self, a: Vec3, b: Vec3, layer: Option<&str>) -> f32 {
        let map = self.scene_map();
        let bit = match layer.map(|name| map.layer_bit(name)) {
            Some(None) => Some(0),
            Some(bit) => bit,
            None => None,
        };
        map.segment_cost(a, b, bit)
    }

    /// Objects whose metadata has `key` equal to `value`.
    pub fn items_where<'a>(
        &'a self,
        key: &'a str,
        value: &'a MetaValue,
    ) -> impl Iterator<Item = EntityId> + 'a {
        self.scene_map().tagged.iter().copied().filter(move |&id| {
            self.component::<Metadata>(id)
                .ok()
                .flatten()
                .and_then(|metadata| metadata.get(key))
                == Some(value)
        })
    }

    /// The waypoint closest to `point`, if any.
    pub fn nearest_waypoint(&self, point: Vec3) -> Option<EntityId> {
        self.scene_map()
            .waypoints
            .iter()
            .filter(|waypoint| self.global_transform(waypoint.entity).is_ok())
            .min_by(|a, b| {
                a.position
                    .distance_squared(point)
                    .total_cmp(&b.position.distance_squared(point))
            })
            .map(|waypoint| waypoint.entity)
    }

    /// Cheapest route through the waypoint network from `from` to `to`, writing the
    /// waypoints (both ends included) into `path`. Edges are weighted by
    /// [`segment_cost`](Self::segment_cost), so costly zones are avoided where a detour
    /// is cheaper and blocked ones never crossed. Returns the route's cost.
    pub fn waypoint_path(
        &mut self,
        from: EntityId,
        to: EntityId,
        layer: Option<&str>,
        path: &mut Vec<EntityId>,
    ) -> Result<f32, MapError> {
        path.clear();
        let mut map = self.take_scene_map();
        let result = (|| {
            let bit = match layer.map(|name| map.layer_bit(name)) {
                Some(None) => Some(0),
                Some(bit) => bit,
                None => None,
            };
            let start = *map
                .waypoint_index
                .get(&from)
                .ok_or(MapError::NotAWaypoint(from))?;
            let goal = *map
                .waypoint_index
                .get(&to)
                .ok_or(MapError::NotAWaypoint(to))?;
            let mut scratch = std::mem::take(&mut map.scratch);
            let layer_cache = match map.edge_costs.iter().position(|(layer, _)| *layer == bit) {
                Some(index) => index,
                None => {
                    let edges = map.edges.len();
                    map.edge_costs.push((bit, vec![f32::NAN; edges]));
                    map.edge_costs.len() - 1
                }
            };
            let count = map.waypoints.len();
            scratch.open.clear();
            scratch.cost.clear();
            scratch.cost.resize(count, f32::INFINITY);
            scratch.from.clear();
            scratch.from.resize(count, u32::MAX);
            let goal_position = map.waypoints[goal as usize].position;
            let estimate = |node: u32, map: &SceneMap| {
                map.waypoints[node as usize]
                    .position
                    .distance(goal_position)
                    * map.min_cost
            };
            scratch.cost[start as usize] = 0.0;
            scratch.open.push(Open {
                estimate: estimate(start, &map),
                node: start,
            });
            let mut found = false;
            while let Some(Open {
                estimate: guess,
                node,
            }) = scratch.open.pop()
            {
                if node == goal {
                    found = true;
                    break;
                }
                let here = scratch.cost[node as usize];
                // A stale queue entry for a node already reached more cheaply.
                if guess - estimate(node, &map) > here + 1e-3 {
                    continue;
                }
                let position = map.waypoints[node as usize].position;
                let (first, last) = (map.starts[node as usize], map.starts[node as usize + 1]);
                for edge in first..last {
                    let next = map.edges[edge as usize];
                    let cached = map.edge_costs[layer_cache].1[edge as usize];
                    let step = if cached.is_nan() {
                        let cost =
                            map.segment_cost(position, map.waypoints[next as usize].position, bit);
                        map.edge_costs[layer_cache].1[edge as usize] = cost;
                        cost
                    } else {
                        cached
                    };
                    let total = here + step;
                    if total < scratch.cost[next as usize] {
                        scratch.cost[next as usize] = total;
                        scratch.from[next as usize] = node;
                        scratch.open.push(Open {
                            estimate: total + estimate(next, &map),
                            node: next,
                        });
                    }
                }
            }
            let result = if found {
                let mut node = goal;
                path.push(map.waypoints[node as usize].entity);
                while node != start {
                    node = scratch.from[node as usize];
                    path.push(map.waypoints[node as usize].entity);
                }
                path.reverse();
                Ok(scratch.cost[goal as usize])
            } else {
                Err(MapError::NoRoute)
            };
            map.scratch = scratch;
            result
        })();
        self.restore_scene_map(map);
        result
    }
}
