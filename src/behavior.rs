use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::time::Duration;

use glam::Vec3;
use slotmap::{new_key_type, SlotMap};

use crate::EntityId;

new_key_type! {
    struct BehaviorKey;
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BehaviorId {
    world: u64,
    key: BehaviorKey,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PortId(&'static str);

impl PortId {
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    pub const fn name(self) -> &'static str {
        self.0
    }
}

pub mod ports {
    use super::PortId;

    pub const POSITION: PortId = PortId::new("position");
    pub const TARGET: PortId = PortId::new("target");
    pub const VALUE: PortId = PortId::new("value");
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BehaviorValueType {
    Scalar,
    Vector,
    Boolean,
    Entity,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BehaviorValue {
    Scalar(f32),
    Vector(Vec3),
    Boolean(bool),
    Entity(EntityId),
}

impl BehaviorValue {
    pub const fn value_type(self) -> BehaviorValueType {
        match self {
            Self::Scalar(_) => BehaviorValueType::Scalar,
            Self::Vector(_) => BehaviorValueType::Vector,
            Self::Boolean(_) => BehaviorValueType::Boolean,
            Self::Entity(_) => BehaviorValueType::Entity,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BehaviorRunState {
    Ready,
    Suspended,
    Defaulted,
    Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum BehaviorKind {
    LinearMotion { velocity: Vec3 },
    FollowPosition { max_speed: f32 },
    AddScalar { increment: f32, default_input: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BehaviorSpec {
    kind: BehaviorKind,
    initial_output: BehaviorValue,
}

impl BehaviorSpec {
    pub fn linear_motion(position: Vec3, velocity: Vec3) -> Result<Self, BehaviorError> {
        if !position.is_finite() || !velocity.is_finite() {
            return Err(BehaviorError::InvalidConfiguration);
        }
        Ok(Self {
            kind: BehaviorKind::LinearMotion { velocity },
            initial_output: BehaviorValue::Vector(position),
        })
    }

    pub fn follow_position(position: Vec3, max_speed: f32) -> Result<Self, BehaviorError> {
        if !position.is_finite() || !max_speed.is_finite() || max_speed < 0.0 {
            return Err(BehaviorError::InvalidConfiguration);
        }
        Ok(Self {
            kind: BehaviorKind::FollowPosition { max_speed },
            initial_output: BehaviorValue::Vector(position),
        })
    }

    pub fn add_scalar(
        initial_value: f32,
        increment: f32,
        default_input: f32,
    ) -> Result<Self, BehaviorError> {
        if !initial_value.is_finite() || !increment.is_finite() || !default_input.is_finite() {
            return Err(BehaviorError::InvalidConfiguration);
        }
        Ok(Self {
            kind: BehaviorKind::AddScalar {
                increment,
                default_input,
            },
            initial_output: BehaviorValue::Scalar(initial_value),
        })
    }

    fn input(self) -> Option<(PortId, BehaviorValueType)> {
        match self.kind {
            BehaviorKind::LinearMotion { .. } => None,
            BehaviorKind::FollowPosition { .. } => Some((ports::TARGET, BehaviorValueType::Vector)),
            BehaviorKind::AddScalar { .. } => Some((ports::VALUE, BehaviorValueType::Scalar)),
        }
    }

    fn output(self) -> (PortId, BehaviorValueType) {
        match self.kind {
            BehaviorKind::LinearMotion { .. } | BehaviorKind::FollowPosition { .. } => {
                (ports::POSITION, BehaviorValueType::Vector)
            }
            BehaviorKind::AddScalar { .. } => (ports::VALUE, BehaviorValueType::Scalar),
        }
    }

    fn writes_transform(self) -> bool {
        matches!(
            self.kind,
            BehaviorKind::LinearMotion { .. } | BehaviorKind::FollowPosition { .. }
        )
    }

    pub(crate) fn initial_translation(self) -> Option<Vec3> {
        if !self.writes_transform() {
            return None;
        }
        let BehaviorValue::Vector(translation) = self.initial_output else {
            unreachable!("transform writers publish vector positions");
        };
        Some(translation)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BehaviorError {
    InvalidConfiguration,
    InvalidOwner(EntityId),
    WrongWorld(BehaviorId),
    MissingBehavior(BehaviorId),
    UnknownInputPort {
        behavior: BehaviorId,
        port: PortId,
    },
    UnknownOutputPort {
        behavior: BehaviorId,
        port: PortId,
    },
    PortTypeMismatch {
        source: BehaviorValueType,
        target: BehaviorValueType,
    },
    TransformWriterConflict(EntityId),
}

impl fmt::Display for BehaviorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration => {
                formatter.write_str("behaviour configuration values must be finite and valid")
            }
            Self::InvalidOwner(owner) => write!(formatter, "behaviour owner {owner:?} is invalid"),
            Self::WrongWorld(id) => {
                write!(formatter, "behaviour {id:?} belongs to another world")
            }
            Self::MissingBehavior(id) => write!(formatter, "behaviour {id:?} does not exist"),
            Self::UnknownInputPort { behavior, port } => write!(
                formatter,
                "behaviour {behavior:?} has no input port named {}",
                port.name()
            ),
            Self::UnknownOutputPort { behavior, port } => write!(
                formatter,
                "behaviour {behavior:?} has no output port named {}",
                port.name()
            ),
            Self::PortTypeMismatch { source, target } => {
                write!(
                    formatter,
                    "cannot link {source:?} output to {target:?} input"
                )
            }
            Self::TransformWriterConflict(owner) => {
                write!(
                    formatter,
                    "entity {owner:?} already has an enabled transform writer"
                )
            }
        }
    }
}

impl Error for BehaviorError {}

#[derive(Clone, Copy, Debug)]
struct BehaviorLink {
    source: BehaviorId,
    output: PortId,
}

#[derive(Debug)]
struct BehaviorNode {
    owner: EntityId,
    spec: BehaviorSpec,
    input_link: Option<BehaviorLink>,
    current_output: BehaviorValue,
    next_output: BehaviorValue,
    enabled: bool,
    state: BehaviorRunState,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BehaviorEvaluationStats {
    pub evaluated: usize,
    pub suspended: usize,
    pub defaulted: usize,
    pub transform_writes: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BehaviorTransformWrite {
    pub owner: EntityId,
    pub translation: Vec3,
}

pub(crate) struct BehaviorGraph {
    world: u64,
    nodes: SlotMap<BehaviorKey, BehaviorNode>,
    order: Vec<BehaviorKey>,
    owners: HashMap<EntityId, Vec<BehaviorKey>>,
    transform_writes: Vec<BehaviorTransformWrite>,
}

impl BehaviorGraph {
    pub(crate) fn new(world: u64) -> Self {
        Self {
            world,
            nodes: SlotMap::with_key(),
            order: Vec::new(),
            owners: HashMap::new(),
            transform_writes: Vec::new(),
        }
    }

    pub(crate) fn add(
        &mut self,
        owner: EntityId,
        spec: BehaviorSpec,
    ) -> Result<BehaviorId, BehaviorError> {
        if spec.writes_transform() && self.owner_has_enabled_transform_writer(owner, None) {
            return Err(BehaviorError::TransformWriterConflict(owner));
        }
        let output = spec.initial_output;
        let key = self.nodes.insert(BehaviorNode {
            owner,
            spec,
            input_link: None,
            current_output: output,
            next_output: output,
            enabled: true,
            state: BehaviorRunState::Ready,
        });
        self.order.push(key);
        self.owners.entry(owner).or_default().push(key);
        Ok(self.id_for(key))
    }

    pub(crate) fn len(&self) -> usize {
        self.nodes.len()
    }

    pub(crate) fn remove(&mut self, id: BehaviorId) -> Result<(), BehaviorError> {
        let key = self.key_for(id)?;
        let node = self.nodes.remove(key).expect("validated behaviour key");
        self.order.retain(|candidate| *candidate != key);
        if let Some(owned) = self.owners.get_mut(&node.owner) {
            owned.retain(|candidate| *candidate != key);
            if owned.is_empty() {
                self.owners.remove(&node.owner);
            }
        }
        Ok(())
    }

    pub(crate) fn remove_owner(&mut self, owner: EntityId) -> usize {
        let Some(keys) = self.owners.remove(&owner) else {
            return 0;
        };
        let removed = keys.len();
        for key in keys {
            self.nodes.remove(key);
        }
        removed
    }

    pub(crate) fn compact_order(&mut self) {
        self.order.retain(|key| self.nodes.contains_key(*key));
    }

    pub(crate) fn connect(
        &mut self,
        source: BehaviorId,
        output: PortId,
        target: BehaviorId,
        input: PortId,
    ) -> Result<(), BehaviorError> {
        let source_key = self.key_for(source)?;
        let target_key = self.key_for(target)?;
        let (actual_output, source_type) = self.nodes[source_key].spec.output();
        if output != actual_output {
            return Err(BehaviorError::UnknownOutputPort {
                behavior: source,
                port: output,
            });
        }
        let Some((actual_input, target_type)) = self.nodes[target_key].spec.input() else {
            return Err(BehaviorError::UnknownInputPort {
                behavior: target,
                port: input,
            });
        };
        if input != actual_input {
            return Err(BehaviorError::UnknownInputPort {
                behavior: target,
                port: input,
            });
        }
        if source_type != target_type {
            return Err(BehaviorError::PortTypeMismatch {
                source: source_type,
                target: target_type,
            });
        }
        self.nodes[target_key].input_link = Some(BehaviorLink { source, output });
        Ok(())
    }

    pub(crate) fn disconnect(
        &mut self,
        target: BehaviorId,
        input: PortId,
    ) -> Result<(), BehaviorError> {
        let target_key = self.key_for(target)?;
        let Some((actual_input, _)) = self.nodes[target_key].spec.input() else {
            return Err(BehaviorError::UnknownInputPort {
                behavior: target,
                port: input,
            });
        };
        if input != actual_input {
            return Err(BehaviorError::UnknownInputPort {
                behavior: target,
                port: input,
            });
        }
        self.nodes[target_key].input_link = None;
        Ok(())
    }

    pub(crate) fn set_enabled(
        &mut self,
        id: BehaviorId,
        enabled: bool,
    ) -> Result<(), BehaviorError> {
        let key = self.key_for(id)?;
        if enabled
            && !self.nodes[key].enabled
            && self.nodes[key].spec.writes_transform()
            && self.owner_has_enabled_transform_writer(self.nodes[key].owner, Some(key))
        {
            return Err(BehaviorError::TransformWriterConflict(
                self.nodes[key].owner,
            ));
        }
        let node = &mut self.nodes[key];
        node.enabled = enabled;
        if !enabled {
            node.state = BehaviorRunState::Disabled;
        }
        Ok(())
    }

    pub(crate) fn output(&self, id: BehaviorId) -> Result<BehaviorValue, BehaviorError> {
        let key = self.key_for(id)?;
        Ok(self.nodes[key].current_output)
    }

    pub(crate) fn state(&self, id: BehaviorId) -> Result<BehaviorRunState, BehaviorError> {
        let key = self.key_for(id)?;
        Ok(self.nodes[key].state)
    }

    pub(crate) fn owner(&self, id: BehaviorId) -> Result<EntityId, BehaviorError> {
        let key = self.key_for(id)?;
        Ok(self.nodes[key].owner)
    }

    pub(crate) fn evaluate(&mut self, fixed_delta: Duration) -> BehaviorEvaluationStats {
        self.transform_writes.clear();
        let delta_seconds = fixed_delta.as_secs_f32();
        let mut stats = BehaviorEvaluationStats::default();

        for index in 0..self.order.len() {
            let key = self.order[index];
            let (enabled, owner, spec, current, link) = {
                let node = &self.nodes[key];
                (
                    node.enabled,
                    node.owner,
                    node.spec,
                    node.current_output,
                    node.input_link,
                )
            };
            if !enabled {
                continue;
            }

            let input = link.and_then(|link| self.resolve_link(link));
            let (next, state) = match (spec.kind, current, input) {
                (BehaviorKind::LinearMotion { velocity }, BehaviorValue::Vector(position), _) => (
                    BehaviorValue::Vector(position + velocity * delta_seconds),
                    BehaviorRunState::Ready,
                ),
                (
                    BehaviorKind::FollowPosition { max_speed },
                    BehaviorValue::Vector(position),
                    Some(BehaviorValue::Vector(target)),
                ) => {
                    let offset = target - position;
                    let distance = offset.length();
                    let step = (max_speed * delta_seconds).min(distance);
                    let next = if distance > 0.0 {
                        position + offset * (step / distance)
                    } else {
                        position
                    };
                    (BehaviorValue::Vector(next), BehaviorRunState::Ready)
                }
                (BehaviorKind::FollowPosition { .. }, BehaviorValue::Vector(position), _) => {
                    stats.suspended += 1;
                    (BehaviorValue::Vector(position), BehaviorRunState::Suspended)
                }
                (
                    BehaviorKind::AddScalar {
                        increment,
                        default_input,
                    },
                    _,
                    input,
                ) => {
                    let (value, state) = match input {
                        Some(BehaviorValue::Scalar(value)) => (value, BehaviorRunState::Ready),
                        _ => {
                            stats.defaulted += 1;
                            (default_input, BehaviorRunState::Defaulted)
                        }
                    };
                    (BehaviorValue::Scalar(value + increment), state)
                }
                _ => unreachable!("validated behaviour storage has matching output types"),
            };

            let node = &mut self.nodes[key];
            node.next_output = next;
            node.state = state;
            stats.evaluated += 1;
            if spec.writes_transform() {
                let BehaviorValue::Vector(translation) = next else {
                    unreachable!("transform writers publish vector positions");
                };
                self.transform_writes
                    .push(BehaviorTransformWrite { owner, translation });
            }
        }

        for index in 0..self.order.len() {
            let node = &mut self.nodes[self.order[index]];
            if node.enabled {
                node.current_output = node.next_output;
            }
        }
        stats.transform_writes = self.transform_writes.len();
        stats
    }

    pub(crate) fn take_transform_writes(&mut self) -> Vec<BehaviorTransformWrite> {
        std::mem::take(&mut self.transform_writes)
    }

    pub(crate) fn restore_transform_writes(&mut self, mut writes: Vec<BehaviorTransformWrite>) {
        writes.clear();
        self.transform_writes = writes;
    }

    fn resolve_link(&self, link: BehaviorLink) -> Option<BehaviorValue> {
        let source_key = self.key_for(link.source).ok()?;
        let source = &self.nodes[source_key];
        let (output, _) = source.spec.output();
        if source.enabled && output == link.output {
            Some(source.current_output)
        } else {
            None
        }
    }

    fn owner_has_enabled_transform_writer(
        &self,
        owner: EntityId,
        except: Option<BehaviorKey>,
    ) -> bool {
        self.owners.get(&owner).is_some_and(|keys| {
            keys.iter().copied().any(|key| {
                Some(key) != except
                    && self.nodes[key].enabled
                    && self.nodes[key].spec.writes_transform()
            })
        })
    }

    fn key_for(&self, id: BehaviorId) -> Result<BehaviorKey, BehaviorError> {
        if id.world != self.world {
            return Err(BehaviorError::WrongWorld(id));
        }
        if !self.nodes.contains_key(id.key) {
            return Err(BehaviorError::MissingBehavior(id));
        }
        Ok(id.key)
    }

    fn id_for(&self, key: BehaviorKey) -> BehaviorId {
        BehaviorId {
            world: self.world,
            key,
        }
    }
}
