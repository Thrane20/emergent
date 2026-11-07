//! An airport's ground network, built from its tagged items whenever the map changes.
//!
//! Taxiway lines need not be drawn connected: lines that cross are joined at the crossing, and a
//! line's loose end is joined to any other line within [`JOIN_DISTANCE`] (times the airport's
//! scale). Stands and holding
//! positions not linked into the network join the closest point on the nearest line. Explicit
//! waypoint links still count (and one-way links stay one-way). Edge costs come from the map's
//! zone costs, so costly zones are avoided and blocked ones never crossed.
//!
//! Positions are captured at build time; the tower rebuilds on every map generation change.
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::{EntityId, Vec3, Waypoint, World};

/// A line end this close to another line joins it.
pub const JOIN_DISTANCE: f32 = 15.0;
/// Splits this close to a segment end reuse the end node.
const SNAP: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NodeKind {
    /// A taxiway point or a junction the graph added.
    Taxi,
    Stand,
    Holding,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TaxiGraph {
    pub(crate) nodes: Vec<(Vec3, NodeKind)>,
    /// Outgoing edges per node: (to, cost).
    edges: Vec<Vec<(u32, f32)>>,
    /// The node each tagged item became.
    pub(crate) anchors: HashMap<EntityId, u32>,
    /// Reused by [`route`](Self::route).
    cost: Vec<f32>,
    from: Vec<u32>,
    open: BinaryHeap<Open>,
}

#[derive(Clone, Copy)]
struct Segment {
    a: u32,
    b: u32,
    two_way: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Open(f32, u32);
impl Eq for Open {}
impl Ord for Open {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.total_cmp(&self.0)
    }
}
impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn flat(v: Vec3) -> (f32, f32) {
    (v.x, v.z)
}

/// Closest point on segment `a`–`b` to `p` on the ground plane, as the parameter along it.
fn project(a: Vec3, b: Vec3, p: Vec3) -> f32 {
    let ((ax, az), (bx, bz), (px, pz)) = (flat(a), flat(b), flat(p));
    let (dx, dz) = (bx - ax, bz - az);
    let length = dx * dx + dz * dz;
    if length < 1e-6 {
        return 0.0;
    }
    (((px - ax) * dx + (pz - az) * dz) / length).clamp(0.0, 1.0)
}

/// Where two segments cross on the ground plane, as parameters along each (interior only).
fn crossing(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> Option<(f32, f32)> {
    let ((ax, az), (bx, bz), (cx, cz), (dx, dz)) = (flat(a), flat(b), flat(c), flat(d));
    let (rx, rz, sx, sz) = (bx - ax, bz - az, dx - cx, dz - cz);
    let denominator = rx * sz - rz * sx;
    if denominator.abs() < 1e-6 {
        return None;
    }
    let t = ((cx - ax) * sz - (cz - az) * sx) / denominator;
    let u = ((cx - ax) * rz - (cz - az) * rx) / denominator;
    (t > 0.0 && t < 1.0 && u > 0.0 && u < 1.0).then_some((t, u))
}

impl TaxiGraph {
    /// The network of one airport: its taxiways, stands and holding positions.
    pub(crate) fn build(
        world: &World,
        taxiways: &[EntityId],
        stands: &[EntityId],
        holding: &[EntityId],
        scale: f32,
    ) -> Self {
        let mut graph = Self::default();
        let position = |id: EntityId| world.global_transform(id).ok().map(|t| t.translation());
        for (items, kind) in [
            (taxiways, NodeKind::Taxi),
            (stands, NodeKind::Stand),
            (holding, NodeKind::Holding),
        ] {
            for &id in items {
                if let Some(at) = position(id) {
                    graph.anchors.insert(id, graph.nodes.len() as u32);
                    graph.nodes.push((at, kind));
                }
            }
        }

        // Drawn links between network items.
        let mut segments: Vec<Segment> = Vec::new();
        let mut degree = vec![0u32; graph.nodes.len()];
        for (&id, &a) in &graph.anchors {
            let Some(waypoint) = world.component::<Waypoint>(id).ok().flatten() else {
                continue;
            };
            for link in &waypoint.links {
                let Some(&b) = graph.anchors.get(link) else {
                    continue;
                };
                if a == b {
                    continue;
                }
                // Two-way links drawn from both ends are one segment.
                if let Some(existing) = segments
                    .iter_mut()
                    .find(|s| (s.a == b && s.b == a) || (s.a == a && s.b == b))
                {
                    if existing.a == b {
                        existing.two_way = true;
                    }
                    continue;
                }
                segments.push(Segment {
                    a,
                    b,
                    two_way: waypoint.two_way,
                });
                degree[a as usize] += 1;
                degree[b as usize] += 1;
            }
        }
        // Deterministic order regardless of hash iteration.
        segments.sort_by_key(|s| (s.a, s.b));

        // Splits along each segment: (parameter, node).
        let mut splits: Vec<Vec<(f32, u32)>> = vec![Vec::new(); segments.len()];
        let mut joins: Vec<(u32, u32)> = Vec::new();
        let at = |graph: &Self, node: u32| graph.nodes[node as usize].0;
        let split = |graph: &mut Self, splits: &mut Vec<Vec<(f32, u32)>>, index: usize, t: f32| {
            let s = segments[index];
            let (a, b) = (at(graph, s.a), at(graph, s.b));
            let length = Vec3::new(b.x - a.x, 0.0, b.z - a.z).length();
            if t * length < SNAP {
                return s.a;
            }
            if (1.0 - t) * length < SNAP {
                return s.b;
            }
            if let Some(&(_, node)) = splits[index]
                .iter()
                .find(|(other, _)| ((other - t) * length).abs() < SNAP)
            {
                return node;
            }
            let node = graph.nodes.len() as u32;
            graph.nodes.push((a.lerp(b, t), NodeKind::Taxi));
            splits[index].push((t, node));
            node
        };

        // Lines that cross are joined where they cross.
        for i in 0..segments.len() {
            for j in i + 1..segments.len() {
                let (s, o) = (segments[i], segments[j]);
                if s.a == o.a || s.a == o.b || s.b == o.a || s.b == o.b {
                    continue;
                }
                let points = (
                    at(&graph, s.a),
                    at(&graph, s.b),
                    at(&graph, o.a),
                    at(&graph, o.b),
                );
                if let Some((t, u)) = crossing(points.0, points.1, points.2, points.3) {
                    let node = split(&mut graph, &mut splits, i, t);
                    let other = split(&mut graph, &mut splits, j, u);
                    if node != other {
                        joins.push((node, other));
                    }
                }
            }
        }

        // Loose ends: taxiway line ends join a nearby line; unlinked stands and holding
        // positions join the nearest line however far it is.
        let anchored = graph.anchors.len() as u32;
        let mut ends: Vec<u32> = (0..anchored)
            .filter(|&node| match graph.nodes[node as usize].1 {
                NodeKind::Taxi => degree[node as usize] <= 1,
                _ => degree[node as usize] == 0,
            })
            .collect();
        ends.sort_unstable();
        for node in ends {
            let (p, kind) = graph.nodes[node as usize];
            let nearest = segments
                .iter()
                .enumerate()
                .filter(|(_, s)| s.a != node && s.b != node)
                .map(|(index, s)| {
                    let (a, b) = (at(&graph, s.a), at(&graph, s.b));
                    let t = project(a, b, p);
                    let q = a.lerp(b, t);
                    (index, t, Vec3::new(q.x - p.x, 0.0, q.z - p.z).length())
                })
                .min_by(|x, y| x.2.total_cmp(&y.2));
            let Some((index, t, distance)) = nearest else {
                continue;
            };
            if kind == NodeKind::Taxi && distance > JOIN_DISTANCE * scale {
                continue;
            }
            let target = split(&mut graph, &mut splits, index, t);
            if target != node {
                joins.push((node, target));
            }
        }

        // Edges: each segment chained through its splits, plus the joins (both ways).
        graph.edges = vec![Vec::new(); graph.nodes.len()];
        let connect = |graph: &mut Self, a: u32, b: u32, two_way: bool| {
            let cost = world.segment_cost(at(graph, a), at(graph, b), None);
            if !cost.is_finite() {
                return;
            }
            graph.edges[a as usize].push((b, cost));
            if two_way {
                graph.edges[b as usize].push((a, cost));
            }
        };
        for (index, segment) in segments.iter().enumerate() {
            let chain = &mut splits[index];
            chain.sort_by(|x, y| x.0.total_cmp(&y.0));
            let mut previous = segment.a;
            for &(_, node) in chain.iter() {
                connect(&mut graph, previous, node, segment.two_way);
                previous = node;
            }
            connect(&mut graph, previous, segment.b, segment.two_way);
        }
        for (a, b) in joins {
            connect(&mut graph, a, b, true);
        }
        graph
    }

    /// The node closest to `to` among those `keep` accepts.
    pub(crate) fn nearest(
        &self,
        to: Vec3,
        keep: impl Fn(u32, Vec3, NodeKind) -> bool,
    ) -> Option<u32> {
        (0..self.nodes.len() as u32)
            .filter(|&n| {
                let (p, kind) = self.nodes[n as usize];
                keep(n, p, kind)
            })
            .min_by(|&a, &b| {
                self.nodes[a as usize]
                    .0
                    .distance_squared(to)
                    .total_cmp(&self.nodes[b as usize].0.distance_squared(to))
            })
    }

    /// Positions along the cheapest route from `from` to `to` (both included) into `out`.
    /// Returns false, leaving `out` empty, when there is no route.
    pub(crate) fn route(&mut self, from: u32, to: u32, out: &mut Vec<Vec3>) -> bool {
        out.clear();
        let count = self.nodes.len();
        self.cost.clear();
        self.cost.resize(count, f32::INFINITY);
        self.from.clear();
        self.from.resize(count, u32::MAX);
        self.open.clear();
        self.cost[from as usize] = 0.0;
        self.open.push(Open(0.0, from));
        while let Some(Open(cost, node)) = self.open.pop() {
            if node == to {
                break;
            }
            if cost > self.cost[node as usize] {
                continue;
            }
            for &(next, step) in &self.edges[node as usize] {
                let total = cost + step;
                if total < self.cost[next as usize] {
                    self.cost[next as usize] = total;
                    self.from[next as usize] = node;
                    self.open.push(Open(total, next));
                }
            }
        }
        if !self.cost[to as usize].is_finite() {
            return false;
        }
        let mut node = to;
        out.push(self.nodes[node as usize].0);
        while node != from {
            node = self.from[node as usize];
            out.push(self.nodes[node as usize].0);
        }
        out.reverse();
        true
    }
}
