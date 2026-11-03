use std::any::TypeId;
use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bevy_ecs::change_detection::Mut;
use bevy_ecs::component::{Component, Mutable};
use bevy_ecs::entity::Entity as EcsEntity;
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::query::{QueryState, Without};
use bevy_ecs::resource::{IsResource, Resource};
use bevy_ecs::schedule::Schedule;
use bevy_ecs::world::{DeferredWorld, World as EcsWorld};
use glam::{Affine3A, Mat4, Quat, Vec3};
use slotmap::{new_key_type, SecondaryMap, SlotMap};

use crate::behavior::{BehaviorGraph, BehaviorTransformWrite};
use crate::{
    BehaviorError, BehaviorEvaluationStats, BehaviorId, BehaviorRunState, BehaviorSpec,
    BehaviorValue, PortId,
};

new_key_type! {
    struct EntityKey;
}

static NEXT_WORLD_ID: AtomicU64 = AtomicU64::new(1);
const NORMALIZED_EPSILON: f32 = 1.0e-4;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EntityId {
    world: u64,
    key: EntityKey,
}

/// The canonical Emergent identity attached to an entity's component storage.
///
/// Systems may read this component to cross from a dense ECS query to an
/// Emergent graph boundary. Its value cannot be constructed or changed outside
/// the engine.
#[derive(Clone, Copy, Component, Debug, Eq, Hash, PartialEq)]
#[component(on_remove = identity_removed)]
pub struct EntityIdentity(EntityId);

impl EntityIdentity {
    pub const fn id(self) -> EntityId {
        self.0
    }
}

#[derive(Debug, Default, Resource)]
struct ComponentIntegrity {
    identity_removal_allowed: bool,
    unauthorized_identity_removal: bool,
}

fn identity_removed(mut world: DeferredWorld, _context: HookContext) {
    if let Some(mut integrity) = world.get_resource_mut::<ComponentIntegrity>() {
        if !integrity.identity_removal_allowed {
            integrity.unauthorized_identity_removal = true;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityLifecycleCommandKind {
    Spawn,
    SpawnChild,
    RemoveSubtree,
}

#[derive(Debug)]
enum EntityLifecycleCommand {
    Spawn { name: String },
    SpawnChild { parent: EntityId, name: String },
    RemoveSubtree { root: EntityId },
}

/// Structural changes requested by scheduled component systems.
///
/// The engine commits these commands in insertion order after the `Fixed`
/// schedule. The buffer is retained and reused between ticks.
#[derive(Debug, Default, Resource)]
pub struct EntityLifecycleQueue {
    commands: Vec<EntityLifecycleCommand>,
}

impl EntityLifecycleQueue {
    pub fn spawn(&mut self, name: impl Into<String>) {
        self.commands
            .push(EntityLifecycleCommand::Spawn { name: name.into() });
    }

    pub fn spawn_child(&mut self, parent: EntityId, name: impl Into<String>) {
        self.commands.push(EntityLifecycleCommand::SpawnChild {
            parent,
            name: name.into(),
        });
    }

    pub fn remove_subtree(&mut self, root: EntityId) {
        self.commands
            .push(EntityLifecycleCommand::RemoveSubtree { root });
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntityLifecycleResult {
    Spawned(EntityId),
    Removed {
        root: EntityId,
        count: usize,
    },
    Failed {
        command: EntityLifecycleCommandKind,
        error: WorldError,
    },
}

#[derive(Debug, Default, Resource)]
pub struct EntityLifecycleResults {
    results: Vec<EntityLifecycleResult>,
}

impl EntityLifecycleResults {
    pub fn as_slice(&self) -> &[EntityLifecycleResult] {
        &self.results
    }

    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }
}

/// An entity's authored transform relative to its parent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// Position in metres, relative to the parent coordinate frame.
    pub translation: Vec3,
    pub rotation: Quat,
    /// Dimensionless multipliers of local geometry and child coordinates; one preserves size.
    pub scale: Vec3,
}

impl Transform {
    /// Converts an affine matrix only when a finite, invertible TRS reproduces it.
    /// Shear is rejected rather than silently discarded.
    pub fn try_from_matrix(matrix: Mat4) -> Result<Self, WorldError> {
        if !matrix.is_finite() || !matrix.inverse().is_finite() {
            return Err(WorldError::UnrepresentableTransform);
        }
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
        let result = Self::new(translation, rotation, scale);
        if !result.is_valid() || scale.abs().min_element() == 0.0 {
            return Err(WorldError::UnrepresentableTransform);
        }
        let reconstructed = Mat4::from_scale_rotation_translation(scale, rotation, translation);
        if matrix
            .to_cols_array_2d()
            .iter()
            .zip(reconstructed.to_cols_array_2d())
            .any(|(expected, actual)| {
                let tolerance = expected
                    .iter()
                    .fold(0.0_f32, |max, value| max.max(value.abs()))
                    * 0.0001;
                expected
                    .iter()
                    .zip(actual)
                    .any(|(a, b)| (a - b).abs() > tolerance)
            })
        {
            return Err(WorldError::UnrepresentableTransform);
        }
        Ok(result)
    }
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    pub const fn new(translation: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            translation,
            rotation,
            scale,
        }
    }

    pub const fn from_translation(translation: Vec3) -> Self {
        Self::new(translation, Quat::IDENTITY, Vec3::ONE)
    }

    pub const fn from_rotation(rotation: Quat) -> Self {
        Self::new(Vec3::ZERO, rotation, Vec3::ONE)
    }

    pub const fn from_scale(scale: Vec3) -> Self {
        Self::new(Vec3::ZERO, Quat::IDENTITY, scale)
    }

    fn is_valid(self) -> bool {
        self.translation.is_finite()
            && self.rotation.is_finite()
            && (self.rotation.length_squared() - 1.0).abs() <= NORMALIZED_EPSILON
            && self.scale.is_finite()
    }

    fn to_affine(self) -> Affine3A {
        Affine3A::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// A cached transform from entity-local space into world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobalTransform {
    affine: Affine3A,
}

impl GlobalTransform {
    pub const IDENTITY: Self = Self {
        affine: Affine3A::IDENTITY,
    };

    pub fn translation(self) -> Vec3 {
        self.affine.translation.into()
    }

    /// Normalized world-space local -Z; absent for a degenerate transform.
    pub fn look_direction(self) -> Option<Vec3> {
        self.affine.transform_vector3(Vec3::NEG_Z).try_normalize()
    }

    pub fn transform_point(self, point: Vec3) -> Vec3 {
        self.affine.transform_point3(point)
    }

    pub fn matrix(self) -> Mat4 {
        Mat4::from(self.affine)
    }
}

impl Default for GlobalTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PropagationStats {
    /// Entities checked while finding the roots of dirty subtrees.
    pub scanned: usize,
    /// Dirty entities whose global transform was recomputed.
    pub updated: usize,
}

#[derive(Debug)]
pub struct Entity {
    name: String,
    local_transform: Transform,
    global_transform: GlobalTransform,
    transform_dirty: bool,
    parent: Option<EntityKey>,
    children: Vec<EntityKey>,
}

impl Entity {
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldError {
    WrongWorld(EntityId),
    MissingEntity(EntityId),
    HierarchyCycle,
    InvalidTransform,
    UnrepresentableTransform,
    ComponentMappingMissing(EntityId),
    ComponentMappingCorrupt(EntityId),
    ComponentStoreCardinality,
    UnsupportedComponentLifecycle,
    MissingEngineResource,
    ProtectedComponent,
}

impl fmt::Display for WorldError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongWorld(id) => write!(formatter, "entity {id:?} belongs to another world"),
            Self::MissingEntity(id) => write!(formatter, "entity {id:?} does not exist"),
            Self::HierarchyCycle => formatter.write_str("parenting would create a hierarchy cycle"),
            Self::UnrepresentableTransform => formatter.write_str("transform cannot be represented as position/rotation/scale without shear or a singular matrix; use Local mode or change the parent scale"),
            Self::InvalidTransform => formatter
                .write_str("transform values must be finite and rotations must be normalized"),
            Self::ComponentMappingMissing(id) => {
                write!(formatter, "entity {id:?} has no component-storage mapping")
            }
            Self::ComponentMappingCorrupt(id) => {
                write!(
                    formatter,
                    "entity {id:?} has a corrupt component-storage mapping"
                )
            }
            Self::ComponentStoreCardinality => formatter.write_str(
                "component storage entity count does not match canonical entity storage",
            ),
            Self::UnsupportedComponentLifecycle => formatter.write_str(
                "a scheduled system changed engine-owned component entity lifecycle directly",
            ),
            Self::MissingEngineResource => {
                formatter.write_str("a scheduled system removed an engine-owned ECS resource")
            }
            Self::ProtectedComponent => {
                formatter.write_str("the canonical entity identity component is engine-owned")
            }
        }
    }
}

impl Error for WorldError {}

pub struct World {
    id: u64,
    entities: SlotMap<EntityKey, Entity>,
    component_entities: SecondaryMap<EntityKey, EcsEntity>,
    components: EcsWorld,
    unauthorized_entity_query:
        QueryState<EcsEntity, (Without<EntityIdentity>, Without<IsResource>)>,
    behaviors: BehaviorGraph,
    particles: Option<crate::ParticleStore>,
    particle_query: QueryState<(&'static EntityIdentity, &'static crate::ParticleEffect)>,
    transform_stack: Vec<EntityKey>,
    #[cfg(feature = "physics")]
    physics: Option<crate::physics::runtime::PhysicsState>,
    #[cfg(feature = "physics")]
    physics_query: QueryState<(&'static EntityIdentity, &'static crate::PhysicsBody)>,
    controller_query: QueryState<(&'static EntityIdentity, &'static crate::CharacterController)>,
    follow_query: QueryState<(&'static EntityIdentity, &'static crate::Follow)>,
    spin_query: QueryState<(&'static EntityIdentity, &'static crate::Spin)>,
    camera_query: QueryState<(&'static EntityIdentity, &'static crate::FollowCamera)>,
    mouse_look_query: QueryState<(&'static EntityIdentity, &'static crate::MouseLook)>,
    look_query: QueryState<(&'static EntityIdentity, &'static crate::LookAt)>,
    brain_query: QueryState<(&'static EntityIdentity, &'static mut crate::BrainAgent)>,
    audio_emitter_query: QueryState<(&'static EntityIdentity, &'static mut crate::AudioEmitter)>,
    audio_listener_query: QueryState<(&'static EntityIdentity, &'static crate::AudioListener)>,
    map_zone_query: QueryState<(&'static EntityIdentity, &'static crate::map::Zone)>,
    map_waypoint_query: QueryState<(&'static EntityIdentity, &'static crate::map::Waypoint)>,
    metadata_query:
        QueryState<&'static EntityIdentity, bevy_ecs::query::With<crate::map::Metadata>>,
    scene_map: Option<crate::map::SceneMap>,
    group_query: QueryState<(&'static EntityIdentity, &'static crate::Group)>,
    obstacle_query: QueryState<(&'static EntityIdentity, &'static crate::PhysicsBody)>,
    locomotion_query:
        QueryState<&'static EntityIdentity, bevy_ecs::query::With<crate::LocomotionAgent>>,
    locomotion_ids: Option<Vec<EntityId>>,
    brains: Option<crate::brain::BrainScratch>,
    motion: Option<crate::motion::MotionScratch>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let mut components = EcsWorld::new();
        components.insert_resource(EntityLifecycleQueue::default());
        components.insert_resource(EntityLifecycleResults::default());
        components.insert_resource(ComponentIntegrity::default());
        components.insert_resource(crate::map::MapGeneration::default());
        let unauthorized_entity_query = components
            .query_filtered::<EcsEntity, (Without<EntityIdentity>, Without<IsResource>)>();
        #[cfg(feature = "physics")]
        let physics_query = components.query::<(&EntityIdentity, &crate::PhysicsBody)>();
        let particle_query = components.query::<(&EntityIdentity, &crate::ParticleEffect)>();
        let controller_query = components.query::<(&EntityIdentity, &crate::CharacterController)>();
        let follow_query = components.query::<(&EntityIdentity, &crate::Follow)>();
        let spin_query = components.query::<(&EntityIdentity, &crate::Spin)>();
        let camera_query = components.query::<(&EntityIdentity, &crate::FollowCamera)>();
        let mouse_look_query = components.query::<(&EntityIdentity, &crate::MouseLook)>();
        let look_query = components.query::<(&EntityIdentity, &crate::LookAt)>();
        let brain_query = components.query::<(&EntityIdentity, &mut crate::BrainAgent)>();
        let group_query = components.query::<(&EntityIdentity, &crate::Group)>();
        let audio_emitter_query = components.query::<(&EntityIdentity, &mut crate::AudioEmitter)>();
        let audio_listener_query = components.query::<(&EntityIdentity, &crate::AudioListener)>();
        let map_zone_query = components.query::<(&EntityIdentity, &crate::map::Zone)>();
        let map_waypoint_query = components.query::<(&EntityIdentity, &crate::map::Waypoint)>();
        let metadata_query = components
            .query_filtered::<&EntityIdentity, bevy_ecs::query::With<crate::map::Metadata>>();
        let obstacle_query = components.query::<(&EntityIdentity, &crate::PhysicsBody)>();
        let locomotion_query = components
            .query_filtered::<&EntityIdentity, bevy_ecs::query::With<crate::LocomotionAgent>>();
        let id = NEXT_WORLD_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            id,
            entities: SlotMap::with_capacity_and_key(capacity),
            component_entities: SecondaryMap::with_capacity(capacity),
            components,
            unauthorized_entity_query,
            behaviors: BehaviorGraph::new(id),
            particles: Some(crate::ParticleStore::default()),
            particle_query,
            transform_stack: Vec::with_capacity(capacity),
            #[cfg(feature = "physics")]
            physics: None,
            #[cfg(feature = "physics")]
            physics_query,
            controller_query,
            follow_query,
            spin_query,
            camera_query,
            look_query,
            mouse_look_query,
            brain_query,
            audio_emitter_query,
            audio_listener_query,
            map_zone_query,
            map_waypoint_query,
            metadata_query,
            scene_map: Some(Default::default()),
            group_query,
            obstacle_query,
            locomotion_query,
            locomotion_ids: Some(Vec::new()),
            brains: Some(Default::default()),
            motion: Some(Default::default()),
        }
    }

    /// Motion components for this tick in a buffer reused across ticks.
    pub(crate) fn take_motion_scratch(&mut self) -> crate::motion::MotionScratch {
        let mut scratch = self.motion.take().unwrap_or_default();
        scratch.mouse_looks.clear();
        scratch.mouse_looks.extend(
            self.mouse_look_query
                .iter(&self.components)
                .map(|(id, look)| (id.id(), *look)),
        );
        scratch.controllers.clear();
        scratch.controllers.extend(
            self.controller_query
                .iter(&self.components)
                .map(|(id, controller)| (id.id(), *controller)),
        );
        scratch.follows.clear();
        scratch.follows.extend(
            self.follow_query
                .iter(&self.components)
                .map(|(id, follow)| (id.id(), *follow)),
        );
        scratch.spins.clear();
        scratch.spins.extend(
            self.spin_query
                .iter(&self.components)
                .map(|(id, spin)| (id.id(), *spin)),
        );
        scratch.cameras.clear();
        scratch.cameras.extend(
            self.camera_query
                .iter(&self.components)
                .map(|(id, camera)| (id.id(), *camera)),
        );
        scratch.looks.clear();
        scratch.looks.extend(
            self.look_query
                .iter(&self.components)
                .map(|(id, look)| (id.id(), *look)),
        );
        scratch
    }

    /// Committed positions of group members, brain agents and the objects brains name,
    /// in a buffer reused across ticks. Velocities are filled in by the brain step.
    pub(crate) fn take_brain_scratch(&mut self) -> crate::brain::BrainScratch {
        use crate::brain::Entry;
        let mut scratch = self.brains.take().unwrap_or_default();
        scratch.entries.clear();
        scratch.index.clear();
        let push = |scratch: &mut crate::brain::BrainScratch, id: EntityId, group: Option<u32>| {
            if !scratch.index.contains_key(&id) {
                scratch.index.insert(id, scratch.entries.len());
                scratch.entries.push(Entry {
                    id,
                    position: Vec3::ZERO,
                    velocity: Vec3::ZERO,
                    measured_velocity: None,
                    group,
                });
            }
        };
        for (id, group) in self.group_query.iter(&self.components) {
            push(&mut scratch, id.id(), Some(group.0));
        }
        for (id, agent) in self.brain_query.iter(&self.components) {
            push(&mut scratch, id.id(), None);
            for layer in &agent.layers {
                for &object in &layer.brain.objects {
                    push(&mut scratch, object, None);
                }
            }
        }
        // Box colliders block sight and give cover, collected only when a brain asks.
        scratch.obstacles.clear();
        let mut needs_obstacles = false;
        for (_, agent) in self.brain_query.iter(&self.components) {
            needs_obstacles |= agent.layers.iter().any(|layer| layer.brain.needs_obstacles);
        }
        if needs_obstacles {
            let boxes: Vec<_> = self
                .obstacle_query
                .iter(&self.components)
                .filter_map(|(id, body)| body.collider.as_box().map(|b| (id.id(), b)))
                .collect();
            // shortcut: allocates this short list per tick when sight is used; keep a buffer if profiles show it.
            for (id, collider) in boxes {
                if let Ok(model) = self.global_transform(id).map(|t| t.matrix()) {
                    scratch
                        .obstacles
                        .extend(crate::brain::Obstacle::new(id, model, collider));
                }
            }
        }
        // Positions after the scan: objects deleted since load drop out of the snapshot.
        scratch
            .entries
            .retain(|entry| self.key_for(entry.id).is_ok());
        scratch.index.clear();
        for i in 0..scratch.entries.len() {
            let id = scratch.entries[i].id;
            scratch.entries[i].position = self
                .global_transform(id)
                .map_or(Vec3::ZERO, |t| t.translation());
            if self
                .component::<crate::LocomotionAgent>(id)
                .ok()
                .flatten()
                .is_some_and(|agent| agent.model.is_physical())
            {
                let velocity = Vec3::ZERO;
                #[cfg(feature = "physics")]
                let velocity = self
                    .physics_body_state(id)
                    .ok()
                    .flatten()
                    .map_or(velocity, |state| state.linear_velocity);
                scratch.entries[i].measured_velocity = Some(velocity);
            }
            scratch.index.insert(id, i);
        }
        scratch
    }

    /// Objects with locomotion this tick, in a buffer reused across ticks.
    pub(crate) fn take_locomotion_ids(&mut self) -> Vec<EntityId> {
        let mut ids = self.locomotion_ids.take().unwrap_or_default();
        ids.clear();
        ids.extend(
            self.locomotion_query
                .iter(&self.components)
                .map(|id| id.id()),
        );
        ids
    }

    pub(crate) fn restore_locomotion_ids(&mut self, ids: Vec<EntityId>) {
        self.locomotion_ids = Some(ids);
    }

    /// The brain buffers without rebuilding the snapshot (for applying looks).
    pub(crate) fn take_brain_scratch_only(&mut self) -> Option<crate::brain::BrainScratch> {
        self.brains.take()
    }

    pub(crate) fn restore_brain_scratch(&mut self, scratch: crate::brain::BrainScratch) {
        self.brains = Some(scratch);
    }

    pub(crate) fn for_each_brain_agent(
        &mut self,
        mut f: impl FnMut(EntityId, &mut crate::BrainAgent),
    ) {
        for (id, mut agent) in self.brain_query.iter_mut(&mut self.components) {
            f(id.id(), &mut agent);
        }
    }

    pub(crate) fn for_each_audio_emitter(
        &mut self,
        mut f: impl FnMut(EntityId, &mut crate::AudioEmitter),
    ) {
        for (id, mut emitter) in self.audio_emitter_query.iter_mut(&mut self.components) {
            f(id.id(), &mut emitter);
        }
    }

    pub(crate) fn for_each_audio_listener(
        &mut self,
        mut f: impl FnMut(EntityId, &crate::AudioListener),
    ) {
        for (id, listener) in self.audio_listener_query.iter(&self.components) {
            f(id.id(), listener);
        }
    }

    pub(crate) fn take_scene_map(&mut self) -> crate::map::SceneMap {
        self.scene_map.take().unwrap_or_default()
    }

    pub(crate) fn restore_scene_map(&mut self, map: crate::map::SceneMap) {
        self.scene_map = Some(map);
    }

    pub(crate) fn scene_map(&self) -> &crate::map::SceneMap {
        static EMPTY: std::sync::OnceLock<crate::map::SceneMap> = std::sync::OnceLock::new();
        // Only absent while a `&mut self` map operation runs, which `&self` callers can't observe.
        self.scene_map
            .as_ref()
            .unwrap_or_else(|| EMPTY.get_or_init(Default::default))
    }

    pub(crate) fn for_each_map_zone(&mut self, mut f: impl FnMut(EntityId, &crate::map::Zone)) {
        for (id, zone) in self.map_zone_query.iter(&self.components) {
            f(id.id(), zone);
        }
    }

    pub(crate) fn for_each_map_waypoint(
        &mut self,
        mut f: impl FnMut(EntityId, &crate::map::Waypoint),
    ) {
        for (id, waypoint) in self.map_waypoint_query.iter(&self.components) {
            f(id.id(), waypoint);
        }
    }

    pub(crate) fn for_each_metadata_entity(&mut self, mut f: impl FnMut(EntityId)) {
        for id in self.metadata_query.iter(&self.components) {
            f(id.id());
        }
    }

    pub(crate) fn restore_motion_scratch(&mut self, scratch: crate::motion::MotionScratch) {
        self.motion = Some(scratch);
    }

    /// Drives a dynamic body for its next physics step: sets its horizontal velocity
    /// (`velocity.y` is ignored) and keeps it upright. `jump` sets the vertical speed only
    /// while the body is not already moving vertically.
    #[cfg(feature = "physics")]
    pub fn drive_body(
        &mut self,
        id: EntityId,
        velocity: Vec3,
        jump: Option<f32>,
    ) -> Result<(), crate::PhysicsError> {
        self.drive_body_with_force(id, velocity, jump, 0.0)
    }

    /// Like [`World::drive_body`], but a positive `force` (newtons) accelerates the body toward
    /// `velocity` at `force / mass` instead of setting it, and a zero `velocity` leaves the body
    /// to coast under contact friction.
    #[cfg(feature = "physics")]
    pub fn drive_body_with_force(
        &mut self,
        id: EntityId,
        velocity: Vec3,
        jump: Option<f32>,
        force: f32,
    ) -> Result<(), crate::PhysicsError> {
        self.key_for(id)?;
        if self
            .component::<crate::LocomotionAgent>(id)?
            .is_some_and(|agent| agent.model.is_physical())
        {
            return Err(crate::PhysicsError::LocomotionConflict);
        }
        if !matches!(
            self.component::<crate::PhysicsBody>(id)?,
            Some(body) if body.body == crate::BodyKind::Dynamic
        ) {
            return Err(crate::PhysicsError::NotDynamicBody);
        }
        self.physics
            .get_or_insert_with(crate::physics::runtime::PhysicsState::default)
            .queue_drive(id, velocity, jump, force)
    }

    /// Supporting contact committed by the most recent physics step. Missing bodies are not grounded.
    #[cfg(feature = "physics")]
    pub fn is_grounded(&self, id: EntityId) -> Result<bool, WorldError> {
        self.key_for(id)?;
        Ok(self
            .physics
            .as_ref()
            .is_some_and(|state| state.grounded(id)))
    }

    /// Linear velocity of a simulated dynamic body, or `None` before its first physics step.
    #[cfg(feature = "physics")]
    pub fn linear_velocity(&self, id: EntityId) -> Result<Option<Vec3>, WorldError> {
        self.key_for(id)?;
        Ok(self
            .physics
            .as_ref()
            .and_then(|state| state.linear_velocity(id)))
    }

    /// Angular velocity from the last committed physics step, world-space radians/s.
    /// Returns `None` before the body's first physics step. Like linear velocity,
    /// this is a backend snapshot; component edits reconcile at the next fixed step.
    #[cfg(feature = "physics")]
    pub fn angular_velocity(&self, id: EntityId) -> Result<Option<Vec3>, WorldError> {
        self.key_for(id)?;
        Ok(self
            .physics
            .as_ref()
            .and_then(|state| state.angular_velocity(id)))
    }

    /// Committed dynamic-body state, or None before initialization or while body
    /// settings/pose differ from the committed body. A reconstruction gets a new
    /// revision; callers must reset control history instead of reusing old feedback.
    #[cfg(feature = "physics")]
    pub fn physics_body_state(
        &self,
        id: EntityId,
    ) -> Result<Option<crate::PhysicsBodyState>, WorldError> {
        self.key_for(id)?;
        let Some(settings) = self.component::<crate::PhysicsBody>(id)? else {
            return Ok(None);
        };
        let local = *self.local_transform(id)?;
        Ok(self
            .physics
            .as_ref()
            .and_then(|state| state.body_state(id, *settings, local)))
    }

    /// Steps physics only when explicitly called by the fixed simulation loop.
    #[cfg(feature = "physics")]
    pub(crate) fn step_physics(&mut self, dt: Duration) -> Result<(), crate::PhysicsError> {
        let mut state = self.physics.take().unwrap_or_default();
        state.inputs.clear();

        state.inputs.extend(
            self.physics_query
                .iter(&self.components)
                .map(|(id, body)| (id.id(), *body)),
        );
        let result = state.step(self, dt);
        self.physics = Some(state);
        result
    }

    /// Current particle simulation data, extracted read-only by a renderer.
    pub fn particles(&self) -> &crate::ParticleStore {
        self.particles
            .as_ref()
            .expect("particle store is restored before world traversal")
    }

    pub(crate) fn step_particles(&mut self, dt: f32) -> Result<(), crate::ParticleError> {
        let mut store = self
            .particles
            .take()
            .expect("particle stepping is not reentrant");
        let result = (|| {
            let mut bindings = std::mem::take(&mut store.bindings);
            bindings.retain(|entity, handle| {
                let keep = self
                    .component::<crate::ParticleEffect>(*entity)
                    .ok()
                    .flatten()
                    .is_some();
                if !keep {
                    store.remove(*handle);
                }
                keep
            });
            store.bindings = bindings;
            for (identity, effect) in self.particle_query.iter(&self.components) {
                let entity = identity.id();
                let model = self.entities[entity.key].global_transform.matrix();
                if let Some(&handle) = store.bindings.get(&entity) {
                    if store
                        .bound_effect(handle)
                        .is_some_and(|old| Arc::ptr_eq(&old.definition, &effect.definition))
                    {
                        store.set_transform(handle, model)?;
                        continue;
                    }
                    store.remove(handle);
                }
                let handle = store.insert(effect.clone(), model)?;
                store.bindings.insert(entity, handle);
            }
            if store.bindings.is_empty() {
                return Ok(());
            }
            let steps = (dt / 0.1).ceil().clamp(1., 120.) as usize;
            let step = dt / steps as f32;
            for _ in 0..steps {
                store.step(step)?;
            }
            Ok(())
        })();
        self.particles = Some(store);
        result
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn behavior_count(&self) -> usize {
        self.behaviors.len()
    }

    pub fn spawn(&mut self, name: impl Into<String>) -> EntityId {
        let key = self.entities.insert(Entity {
            name: name.into(),
            local_transform: Transform::IDENTITY,
            global_transform: GlobalTransform::IDENTITY,
            transform_dirty: true,
            parent: None,
            children: Vec::new(),
        });
        let id = self.id_for(key);
        let component_entity = self.components.spawn(EntityIdentity(id)).id();
        self.component_entities.insert(key, component_entity);
        id
    }

    pub fn spawn_child(
        &mut self,
        parent: EntityId,
        name: impl Into<String>,
    ) -> Result<EntityId, WorldError> {
        let parent_key = self.key_for(parent)?;
        let child = self.spawn(name);
        let child_key = child.key;
        self.entities[child_key].parent = Some(parent_key);
        self.entities[parent_key].children.push(child_key);
        Ok(child)
    }

    pub fn get(&self, id: EntityId) -> Result<&Entity, WorldError> {
        let key = self.key_for(id)?;
        Ok(&self.entities[key])
    }

    /// Adds or replaces an ordinary game component on an entity.
    pub fn insert_component<T: Component>(
        &mut self,
        id: EntityId,
        component: T,
    ) -> Result<(), WorldError> {
        if TypeId::of::<T>() == TypeId::of::<EntityIdentity>() {
            return Err(WorldError::ProtectedComponent);
        }
        let component_entity = self.component_entity_for(id)?;
        self.components
            .entity_mut(component_entity)
            .insert(component);
        Ok(())
    }

    pub fn component<T: Component>(&self, id: EntityId) -> Result<Option<&T>, WorldError> {
        let component_entity = self.component_entity_for(id)?;
        Ok(self.components.get::<T>(component_entity))
    }

    pub fn component_mut<T: Component<Mutability = Mutable>>(
        &mut self,
        id: EntityId,
    ) -> Result<Option<Mut<'_, T>>, WorldError> {
        let component_entity = self.component_entity_for(id)?;
        Ok(self.components.get_mut::<T>(component_entity))
    }

    pub fn remove_component<T: Component>(&mut self, id: EntityId) -> Result<bool, WorldError> {
        if TypeId::of::<T>() == TypeId::of::<EntityIdentity>() {
            return Err(WorldError::ProtectedComponent);
        }
        let component_entity = self.component_entity_for(id)?;
        let mut entity = self.components.entity_mut(component_entity);
        let existed = entity.contains::<T>();
        entity.remove::<T>();
        if TypeId::of::<T>() == TypeId::of::<crate::ParticleEffect>() {
            if let Some(store) = &mut self.particles {
                if let Some(handle) = store.bindings.remove(&id) {
                    store.remove(handle);
                }
            }
        }
        Ok(existed)
    }

    /// Adds a force to a dynamic body for its next fixed physics step.
    ///
    /// The force is a world-space vector in newtons. Forces submitted multiple
    /// times for the same entity before that step are summed. Submit the force
    /// again on later ticks to keep applying it.
    pub fn add_force(&mut self, id: EntityId, force: Vec3) -> Result<(), crate::PhysicsError> {
        #[cfg(feature = "physics")]
        {
            self.key_for(id)?;
            if !matches!(
                self.component::<crate::PhysicsBody>(id)?,
                Some(body) if body.body == crate::BodyKind::Dynamic
            ) {
                return Err(crate::PhysicsError::NotDynamicBody);
            }
            self.physics
                .get_or_insert_with(crate::physics::runtime::PhysicsState::default)
                .queue_force(id, force)
        }
        #[cfg(not(feature = "physics"))]
        {
            let _ = (id, force);
            Err(crate::PhysicsError::FeatureDisabled)
        }
    }

    /// Adds a world-space torque in N·m to a dynamic body for its next fixed step.
    /// Multiple submissions sum; resubmit every tick for sustained torque. This
    /// changes angular acceleration, not the body's pose or angular velocity directly.
    pub fn add_torque(&mut self, id: EntityId, torque: Vec3) -> Result<(), crate::PhysicsError> {
        #[cfg(feature = "physics")]
        {
            self.key_for(id)?;
            if !matches!(
                self.component::<crate::PhysicsBody>(id)?,
                Some(body) if body.body == crate::BodyKind::Dynamic
            ) {
                return Err(crate::PhysicsError::NotDynamicBody);
            }
            self.physics
                .get_or_insert_with(crate::physics::runtime::PhysicsState::default)
                .queue_torque(id, torque)
        }
        #[cfg(not(feature = "physics"))]
        {
            let _ = (id, torque);
            Err(crate::PhysicsError::FeatureDisabled)
        }
    }

    pub fn insert_resource<R: Resource>(&mut self, resource: R) {
        self.components.insert_resource(resource);
    }

    pub fn resource<R: Resource>(&self) -> Option<&R> {
        self.components.get_resource::<R>()
    }

    pub fn resource_mut<R: Resource<Mutability = Mutable>>(&mut self) -> Option<Mut<'_, R>> {
        self.components.get_resource_mut::<R>()
    }

    pub fn add_behavior(
        &mut self,
        owner: EntityId,
        spec: BehaviorSpec,
    ) -> Result<BehaviorId, BehaviorError> {
        if self.key_for(owner).is_err() {
            return Err(BehaviorError::InvalidOwner(owner));
        }
        let initial_translation = spec.initial_translation();
        let behavior = self.behaviors.add(owner, spec)?;
        if let Some(translation) = initial_translation {
            let current = *self
                .local_transform(owner)
                .expect("validated behaviour owner");
            let transform = Transform::new(translation, current.rotation, current.scale);
            if self.set_local_transform(owner, transform).is_err() {
                self.behaviors
                    .remove(behavior)
                    .expect("new behaviour must roll back");
                return Err(BehaviorError::InvalidConfiguration);
            }
        }
        Ok(behavior)
    }

    pub fn remove_behavior(&mut self, behavior: BehaviorId) -> Result<(), BehaviorError> {
        self.behaviors.remove(behavior)
    }

    pub fn connect_behaviors(
        &mut self,
        source: BehaviorId,
        output: PortId,
        target: BehaviorId,
        input: PortId,
    ) -> Result<(), BehaviorError> {
        self.behaviors.connect(source, output, target, input)
    }

    pub fn disconnect_behavior_input(
        &mut self,
        target: BehaviorId,
        input: PortId,
    ) -> Result<(), BehaviorError> {
        self.behaviors.disconnect(target, input)
    }

    pub fn set_behavior_enabled(
        &mut self,
        behavior: BehaviorId,
        enabled: bool,
    ) -> Result<(), BehaviorError> {
        self.behaviors.set_enabled(behavior, enabled)
    }

    pub fn behavior_output(&self, behavior: BehaviorId) -> Result<BehaviorValue, BehaviorError> {
        self.behaviors.output(behavior)
    }

    pub fn behavior_state(&self, behavior: BehaviorId) -> Result<BehaviorRunState, BehaviorError> {
        self.behaviors.state(behavior)
    }

    pub fn behavior_owner(&self, behavior: BehaviorId) -> Result<EntityId, BehaviorError> {
        self.behaviors.owner(behavior)
    }

    pub fn evaluate_behaviors(
        &mut self,
        fixed_delta: Duration,
    ) -> Result<BehaviorEvaluationStats, WorldError> {
        let stats = self.behaviors.evaluate(fixed_delta);
        let writes = self.behaviors.take_transform_writes();
        let result = self.apply_behavior_transform_writes(&writes);
        self.behaviors.restore_transform_writes(writes);
        result.map(|()| stats)
    }

    pub fn parent(&self, id: EntityId) -> Result<Option<EntityId>, WorldError> {
        let key = self.key_for(id)?;
        Ok(self.entities[key].parent.map(|parent| self.id_for(parent)))
    }

    pub fn children(&self, id: EntityId) -> Result<Vec<EntityId>, WorldError> {
        let key = self.key_for(id)?;
        Ok(self.entities[key]
            .children
            .iter()
            .copied()
            .map(|child| self.id_for(child))
            .collect())
    }

    pub fn local_transform(&self, id: EntityId) -> Result<&Transform, WorldError> {
        let key = self.key_for(id)?;
        Ok(&self.entities[key].local_transform)
    }

    /// Returns the last propagated global transform.
    ///
    /// Call [`World::propagate_transforms`] after hierarchy or local-transform
    /// mutations when an up-to-date value is required outside the engine loop.
    pub fn global_transform(&self, id: EntityId) -> Result<&GlobalTransform, WorldError> {
        let key = self.key_for(id)?;
        Ok(&self.entities[key].global_transform)
    }

    /// Resolves a desired global TRS to local TRS using propagated parent state.
    pub fn local_from_global(
        &self,
        id: EntityId,
        desired: Transform,
    ) -> Result<Transform, WorldError> {
        if !desired.is_valid() {
            return Err(WorldError::InvalidTransform);
        }
        let parent = self
            .parent(id)?
            .map(|parent| self.global_transform(parent).map(|t| t.matrix()))
            .transpose()?
            .unwrap_or(Mat4::IDENTITY);
        Transform::try_from_matrix(
            parent.inverse()
                * Mat4::from_scale_rotation_translation(
                    desired.scale,
                    desired.rotation,
                    desired.translation,
                ),
        )
    }

    /// Moves a world-space position while retaining the current local rotation/scale.
    pub fn local_from_global_position(
        &self,
        id: EntityId,
        position: Vec3,
    ) -> Result<Transform, WorldError> {
        if !position.is_finite() {
            return Err(WorldError::InvalidTransform);
        }
        let parent = self
            .parent(id)?
            .map(|parent| self.global_transform(parent).map(|t| t.matrix()))
            .transpose()?
            .unwrap_or(Mat4::IDENTITY);
        let local = parent.inverse().transform_point3(position);
        if !local.is_finite() {
            return Err(WorldError::UnrepresentableTransform);
        }
        let mut result = *self.local_transform(id)?;
        result.translation = local;
        Ok(result)
    }

    pub fn is_transform_dirty(&self, id: EntityId) -> Result<bool, WorldError> {
        let key = self.key_for(id)?;
        Ok(self.entities[key].transform_dirty)
    }

    pub fn set_local_transform(
        &mut self,
        id: EntityId,
        transform: Transform,
    ) -> Result<(), WorldError> {
        let key = self.key_for(id)?;
        if !transform.is_valid() {
            return Err(WorldError::InvalidTransform);
        }
        if self.entities[key].local_transform == transform {
            return Ok(());
        }

        self.entities[key].local_transform = transform;
        self.mark_subtree_dirty(key);
        Ok(())
    }

    pub fn reparent(
        &mut self,
        entity: EntityId,
        new_parent: Option<EntityId>,
    ) -> Result<(), WorldError> {
        let entity_key = self.key_for(entity)?;
        let new_parent_key = new_parent.map(|id| self.key_for(id)).transpose()?;

        let mut cursor = new_parent_key;
        while let Some(key) = cursor {
            if key == entity_key {
                return Err(WorldError::HierarchyCycle);
            }
            cursor = self.entities[key].parent;
        }

        let old_parent = self.entities[entity_key].parent;
        if old_parent == new_parent_key {
            return Ok(());
        }

        if let Some(old_parent) = old_parent {
            self.entities[old_parent]
                .children
                .retain(|child| *child != entity_key);
        }

        self.entities[entity_key].parent = new_parent_key;
        if let Some(parent) = new_parent_key {
            self.entities[parent].children.push(entity_key);
        }
        self.mark_subtree_dirty(entity_key);
        Ok(())
    }

    /// Removes an entity and its complete owned subtree without recursion.
    pub fn remove_subtree(&mut self, root: EntityId) -> Result<usize, WorldError> {
        let root_key = self.key_for(root)?;

        self.prepare_transform_stack();
        self.transform_stack.push(root_key);
        let mut cursor = 0;
        while cursor < self.transform_stack.len() {
            let key = self.transform_stack[cursor];
            for child_index in 0..self.entities[key].children.len() {
                let child = self.entities[key].children[child_index];
                self.transform_stack.push(child);
            }
            cursor += 1;
        }

        for index in 0..self.transform_stack.len() {
            let key = self.transform_stack[index];
            self.validate_component_entity(self.id_for(key), key)?;
        }

        if let Some(parent) = self.entities[root_key].parent {
            self.entities[parent]
                .children
                .retain(|child| *child != root_key);
        }

        for index in 0..self.transform_stack.len() {
            let key = self.transform_stack[index];
            let id = self.id_for(key);
            if let Some(store) = &mut self.particles {
                if let Some(handle) = store.bindings.remove(&id) {
                    store.remove(handle);
                }
            }
            self.behaviors.remove_owner(id);
        }
        self.behaviors.compact_order();

        self.components
            .resource_mut::<ComponentIntegrity>()
            .identity_removal_allowed = true;
        for index in 0..self.transform_stack.len() {
            let key = self.transform_stack[index];
            let component_entity = self
                .component_entities
                .remove(key)
                .expect("validated mapping");
            let removed = self.components.despawn(component_entity);
            debug_assert!(removed, "validated component entity disappeared");
        }
        self.components
            .resource_mut::<ComponentIntegrity>()
            .identity_removal_allowed = false;

        let removed = self.transform_stack.len();
        for index in 0..removed {
            let key = self.transform_stack[index];
            let entity = self.entities.remove(key);
            debug_assert!(entity.is_some(), "collected entity disappeared");
        }
        Ok(removed)
    }

    /// Performs a complete diagnostic check of the SlotMap-to-ECS mapping.
    ///
    /// This scans every entity and is intended for tests, diagnostics, and
    /// explicit integrity checks rather than normal dense frame work.
    pub fn validate_component_mappings(&self) -> Result<(), WorldError> {
        self.validate_component_store_entities()?;
        for (key, _) in &self.entities {
            self.validate_component_entity(self.id_for(key), key)?;
        }
        Ok(())
    }

    pub(crate) fn run_component_schedule(
        &mut self,
        schedule: &mut Schedule,
    ) -> Result<(), WorldError> {
        schedule.run(&mut self.components);
        self.validate_component_mapping_counts()?;
        if self
            .unauthorized_entity_query
            .iter(&self.components)
            .next()
            .is_some()
        {
            return Err(WorldError::ComponentStoreCardinality);
        }
        if cfg!(debug_assertions) {
            self.validate_component_mappings()?;
        }
        Ok(())
    }

    pub(crate) fn commit_lifecycle_commands(&mut self) {
        let mut commands = {
            let mut queue = self.components.resource_mut::<EntityLifecycleQueue>();
            std::mem::take(&mut queue.commands)
        };
        let mut results = {
            let mut published = self.components.resource_mut::<EntityLifecycleResults>();
            let mut results = std::mem::take(&mut published.results);
            results.clear();
            results
        };

        for command in commands.drain(..) {
            match command {
                EntityLifecycleCommand::Spawn { name } => {
                    results.push(EntityLifecycleResult::Spawned(self.spawn(name)));
                }
                EntityLifecycleCommand::SpawnChild { parent, name } => {
                    match self.spawn_child(parent, name) {
                        Ok(id) => results.push(EntityLifecycleResult::Spawned(id)),
                        Err(error) => results.push(EntityLifecycleResult::Failed {
                            command: EntityLifecycleCommandKind::SpawnChild,
                            error,
                        }),
                    }
                }
                EntityLifecycleCommand::RemoveSubtree { root } => match self.remove_subtree(root) {
                    Ok(count) => results.push(EntityLifecycleResult::Removed { root, count }),
                    Err(error) => results.push(EntityLifecycleResult::Failed {
                        command: EntityLifecycleCommandKind::RemoveSubtree,
                        error,
                    }),
                },
            }
        }

        self.components
            .resource_mut::<EntityLifecycleQueue>()
            .commands = commands;
        self.components
            .resource_mut::<EntityLifecycleResults>()
            .results = results;
    }

    /// Updates cached global transforms in root-to-leaf order.
    pub fn propagate_transforms(&mut self) -> PropagationStats {
        self.prepare_transform_stack();

        for (key, entity) in &self.entities {
            let parent_is_dirty = entity
                .parent
                .is_some_and(|parent| self.entities[parent].transform_dirty);
            if entity.transform_dirty && !parent_is_dirty {
                self.transform_stack.push(key);
            }
        }

        let mut updated = 0;
        while let Some(key) = self.transform_stack.pop() {
            let (parent, local) = {
                let entity = &self.entities[key];
                (entity.parent, entity.local_transform)
            };
            let parent_affine = parent
                .map(|parent| self.entities[parent].global_transform.affine)
                .unwrap_or(Affine3A::IDENTITY);

            {
                let entity = &mut self.entities[key];
                entity.global_transform = GlobalTransform {
                    affine: parent_affine * local.to_affine(),
                };
                entity.transform_dirty = false;
            }

            self.transform_stack
                .extend(self.entities[key].children.iter().copied());
            updated += 1;
        }

        PropagationStats {
            scanned: self.entities.len(),
            updated,
        }
    }

    fn mark_subtree_dirty(&mut self, root: EntityKey) {
        self.prepare_transform_stack();
        self.transform_stack.push(root);
        while let Some(key) = self.transform_stack.pop() {
            self.entities[key].transform_dirty = true;
            self.transform_stack
                .extend(self.entities[key].children.iter().copied());
        }
    }

    fn prepare_transform_stack(&mut self) {
        self.transform_stack.clear();
        let additional = self
            .entities
            .len()
            .saturating_sub(self.transform_stack.capacity());
        self.transform_stack.reserve(additional);
    }

    fn apply_behavior_transform_writes(
        &mut self,
        writes: &[BehaviorTransformWrite],
    ) -> Result<(), WorldError> {
        for write in writes {
            let current = *self.local_transform(write.owner)?;
            self.set_local_transform(
                write.owner,
                Transform::new(write.translation, current.rotation, current.scale),
            )?;
        }
        Ok(())
    }

    fn key_for(&self, id: EntityId) -> Result<EntityKey, WorldError> {
        if id.world != self.id {
            return Err(WorldError::WrongWorld(id));
        }
        if !self.entities.contains_key(id.key) {
            return Err(WorldError::MissingEntity(id));
        }
        Ok(id.key)
    }

    fn component_entity_for(&self, id: EntityId) -> Result<EcsEntity, WorldError> {
        let key = self.key_for(id)?;
        self.validate_component_entity(id, key)
    }

    fn validate_component_entity(
        &self,
        id: EntityId,
        key: EntityKey,
    ) -> Result<EcsEntity, WorldError> {
        let component_entity = self
            .component_entities
            .get(key)
            .copied()
            .ok_or(WorldError::ComponentMappingMissing(id))?;
        let identity = self
            .components
            .get::<EntityIdentity>(component_entity)
            .ok_or(WorldError::ComponentMappingCorrupt(id))?;
        if identity.id() != id {
            return Err(WorldError::ComponentMappingCorrupt(id));
        }
        Ok(component_entity)
    }

    fn validate_component_mapping_counts(&self) -> Result<(), WorldError> {
        let integrity = self
            .components
            .get_resource::<ComponentIntegrity>()
            .ok_or(WorldError::MissingEngineResource)?;
        if !self.components.contains_resource::<EntityLifecycleQueue>()
            || !self
                .components
                .contains_resource::<EntityLifecycleResults>()
        {
            return Err(WorldError::MissingEngineResource);
        }
        if integrity.unauthorized_identity_removal {
            return Err(WorldError::UnsupportedComponentLifecycle);
        }
        if self.entities.len() != self.component_entities.len() {
            return Err(WorldError::ComponentStoreCardinality);
        }

        Ok(())
    }

    fn validate_component_store_entities(&self) -> Result<(), WorldError> {
        self.validate_component_mapping_counts()?;
        let mut component_entity_count = 0;
        for entity in self.components.iter_entities() {
            if entity.contains::<IsResource>() {
                continue;
            }
            component_entity_count += 1;
            if !entity.contains::<EntityIdentity>() {
                return Err(WorldError::ComponentStoreCardinality);
            }
        }
        if component_entity_count == self.entities.len() {
            Ok(())
        } else {
            Err(WorldError::ComponentStoreCardinality)
        }
    }

    fn id_for(&self, key: EntityKey) -> EntityId {
        EntityId {
            world: self.id,
            key,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn look_direction_follows_rotation_and_scaled_parent_hierarchy() {
        use super::*;
        assert_eq!(
            GlobalTransform::IDENTITY.look_direction(),
            Some(Vec3::NEG_Z)
        );
        let mut world = World::new();
        let parent = world.spawn("parent");
        let child = world.spawn_child(parent, "child").unwrap();
        world
            .set_local_transform(
                parent,
                Transform::new(
                    Vec3::new(10., 20., 30.),
                    Quat::from_rotation_z(0.7),
                    Vec3::new(2., 3., 4.),
                ),
            )
            .unwrap();
        world
            .set_local_transform(
                child,
                Transform::from_rotation(Quat::from_euler(crate::EulerRot::XYZ, 0.2, 0.4, 0.6)),
            )
            .unwrap();
        world.propagate_transforms();
        let global = world.global_transform(child).unwrap();
        let direction = global.look_direction().unwrap();
        assert!((direction.length() - 1.).abs() < 1e-6);
        assert!(direction.abs_diff_eq(
            global.matrix().transform_vector3(Vec3::NEG_Z).normalize(),
            1e-6
        ));
        world
            .set_local_transform(child, Transform::from_scale(Vec3::ZERO))
            .unwrap();
        world.propagate_transforms();
        assert_eq!(
            world.global_transform(child).unwrap().look_direction(),
            None
        );
    }
    #[test]
    fn global_edit_resolves_rotated_scaled_parent_without_changing_world_identity() {
        let mut world = World::new();
        let parent = world.spawn("parent");
        let child = world.spawn_child(parent, "child").unwrap();
        world
            .set_local_transform(
                parent,
                Transform::new(
                    Vec3::new(10., 0., 0.),
                    Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                    Vec3::splat(2.),
                ),
            )
            .unwrap();
        world
            .set_local_transform(child, Transform::from_translation(Vec3::X))
            .unwrap();
        world.propagate_transforms();
        assert!(world
            .global_transform(child)
            .unwrap()
            .translation()
            .abs_diff_eq(Vec3::new(10., 2., 0.), 1e-5));
        let desired = Transform::new(
            Vec3::new(10., 4., 0.),
            Quat::from_rotation_z(std::f32::consts::PI),
            Vec3::splat(4.),
        );
        let local = world.local_from_global(child, desired).unwrap();
        assert!(local.translation.abs_diff_eq(Vec3::new(2., 0., 0.), 1e-5));
        assert!(local.scale.abs_diff_eq(Vec3::splat(2.), 1e-5));
        world.set_local_transform(child, local).unwrap();
        world.propagate_transforms();
        let expected = Mat4::from_scale_rotation_translation(
            desired.scale,
            desired.rotation,
            desired.translation,
        );
        assert!(world
            .global_transform(child)
            .unwrap()
            .matrix()
            .abs_diff_eq(expected, 1e-4));
        world.remove_subtree(parent).unwrap();
        assert!(matches!(
            world.local_from_global(child, desired),
            Err(WorldError::MissingEntity(_))
        ));
    }
    #[test]
    fn global_position_preserves_sheared_hierarchy_and_global_trs_rejects_lossy_conversion() {
        let mut world = World::new();
        let parent = world.spawn("parent");
        let child = world.spawn_child(parent, "child").unwrap();
        world
            .set_local_transform(parent, Transform::from_scale(Vec3::new(2., 1., 1.)))
            .unwrap();
        let original = Transform::new(
            Vec3::X,
            Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),
            Vec3::ONE,
        );
        world.set_local_transform(child, original).unwrap();
        world.propagate_transforms();
        let before = world.global_transform(child).unwrap().matrix();
        assert_eq!(
            Transform::try_from_matrix(before),
            Err(WorldError::UnrepresentableTransform)
        );
        assert!(world
            .local_from_global(child, Transform::from_rotation(original.rotation))
            .is_err());
        let local = world
            .local_from_global_position(child, Vec3::new(10., 3., 0.))
            .unwrap();
        assert_eq!(local.rotation, original.rotation);
        assert_eq!(local.scale, original.scale);
        world.set_local_transform(child, local).unwrap();
        world.propagate_transforms();
        let after = world.global_transform(child).unwrap().matrix();
        assert!(after
            .w_axis
            .truncate()
            .abs_diff_eq(Vec3::new(10., 3., 0.), 1e-5));
        assert_eq!(before.x_axis, after.x_axis);
        assert_eq!(before.y_axis, after.y_axis);
        assert!(Transform::try_from_matrix(Mat4::ZERO).is_err());
    }
    #[test]
    fn decomposable_small_global_scale_is_not_confused_with_shear() {
        let matrix = Mat4::from_scale(Vec3::splat(1e-6));
        let transform = Transform::try_from_matrix(matrix).unwrap();
        assert!(transform.scale.abs_diff_eq(Vec3::splat(1e-6), 1e-10));
        let sheared = Mat4::from_scale(Vec3::new(2e-6, 1e-6, 1e-6))
            * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_4);
        assert_eq!(
            Transform::try_from_matrix(sheared),
            Err(WorldError::UnrepresentableTransform)
        );
    }
    use std::f32::consts::FRAC_PI_2;

    use super::*;
    use crate::behavior_ports;

    fn assert_vec3_close(actual: Vec3, expected: Vec3) {
        assert!(
            actual.abs_diff_eq(expected, 1.0e-5),
            "expected {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn composes_translation_rotation_and_scale() {
        let mut world = World::new();
        let root = world.spawn("root");
        let child = world.spawn_child(root, "child").unwrap();

        world
            .set_local_transform(
                root,
                Transform::new(
                    Vec3::new(10.0, 0.0, 0.0),
                    Quat::from_rotation_y(FRAC_PI_2),
                    Vec3::splat(2.0),
                ),
            )
            .unwrap();
        world
            .set_local_transform(child, Transform::from_translation(Vec3::NEG_Z))
            .unwrap();

        assert_eq!(world.propagate_transforms().updated, 2);
        assert_vec3_close(
            world.global_transform(child).unwrap().translation(),
            Vec3::new(8.0, 0.0, 0.0),
        );
    }

    #[test]
    fn dirties_only_the_changed_subtree() {
        let mut world = World::new();
        let root = world.spawn("root");
        let child = world.spawn_child(root, "child").unwrap();
        let unrelated = world.spawn("unrelated");
        assert_eq!(world.propagate_transforms().updated, 3);

        world
            .set_local_transform(root, Transform::from_translation(Vec3::X))
            .unwrap();

        assert!(world.is_transform_dirty(root).unwrap());
        assert!(world.is_transform_dirty(child).unwrap());
        assert!(!world.is_transform_dirty(unrelated).unwrap());
        assert_eq!(world.propagate_transforms().updated, 2);
        let child_global = *world.global_transform(child).unwrap();
        let unrelated_global = *world.global_transform(unrelated).unwrap();
        assert_eq!(world.propagate_transforms().updated, 0);
        assert_eq!(*world.global_transform(child).unwrap(), child_global);
        assert_eq!(
            *world.global_transform(unrelated).unwrap(),
            unrelated_global
        );
    }

    #[test]
    fn reparenting_keeps_local_transform_and_updates_global_transform() {
        let mut world = World::new();
        let first_root = world.spawn("first");
        let second_root = world.spawn("second");
        let child = world.spawn_child(first_root, "child").unwrap();
        world
            .set_local_transform(first_root, Transform::from_translation(Vec3::X))
            .unwrap();
        world
            .set_local_transform(
                second_root,
                Transform::from_translation(Vec3::new(10.0, 0.0, 0.0)),
            )
            .unwrap();
        world
            .set_local_transform(child, Transform::from_translation(Vec3::Y))
            .unwrap();
        world.propagate_transforms();
        assert_vec3_close(
            world.global_transform(child).unwrap().translation(),
            Vec3::new(1.0, 1.0, 0.0),
        );

        world.reparent(child, Some(second_root)).unwrap();
        assert_eq!(world.propagate_transforms().updated, 1);
        assert_eq!(
            *world.local_transform(child).unwrap(),
            Transform::from_translation(Vec3::Y)
        );
        assert_vec3_close(
            world.global_transform(child).unwrap().translation(),
            Vec3::new(10.0, 1.0, 0.0),
        );

        world.reparent(child, None).unwrap();
        world.propagate_transforms();
        assert_vec3_close(
            world.global_transform(child).unwrap().translation(),
            Vec3::Y,
        );
    }

    #[test]
    fn rejects_invalid_transforms_without_mutating_state() {
        let mut world = World::new();
        let entity = world.spawn("entity");
        world.propagate_transforms();
        let invalid = Transform::from_translation(Vec3::new(f32::NAN, 0.0, 0.0));

        assert_eq!(
            world.set_local_transform(entity, invalid),
            Err(WorldError::InvalidTransform)
        );
        assert_eq!(*world.local_transform(entity).unwrap(), Transform::IDENTITY);
        assert!(!world.is_transform_dirty(entity).unwrap());

        let zero_scale = Transform::from_scale(Vec3::ZERO);
        world.set_local_transform(entity, zero_scale).unwrap();
        world.propagate_transforms();
        assert!(world.global_transform(entity).unwrap().matrix().is_finite());
    }

    #[test]
    fn builds_and_removes_a_hierarchy() {
        let mut world = World::new();
        let root = world.spawn("root");
        let child = world.spawn_child(root, "child").unwrap();
        let grandchild = world.spawn_child(child, "grandchild").unwrap();

        assert_eq!(world.parent(child).unwrap(), Some(root));
        assert_eq!(world.children(root).unwrap(), vec![child]);
        assert_eq!(world.remove_subtree(child).unwrap(), 2);
        assert_eq!(world.len(), 1);
        assert!(world.children(root).unwrap().is_empty());
        assert!(matches!(
            world.get(grandchild),
            Err(WorldError::MissingEntity(_))
        ));
        world
            .set_local_transform(root, Transform::from_translation(Vec3::X))
            .unwrap();
        assert_eq!(world.propagate_transforms().updated, 1);
        assert_vec3_close(world.global_transform(root).unwrap().translation(), Vec3::X);
    }

    #[test]
    fn rejects_cycles_and_cross_world_handles_without_mutation() {
        let mut world = World::new();
        let root = world.spawn("root");
        let child = world.spawn_child(root, "child").unwrap();
        world
            .set_local_transform(root, Transform::from_translation(Vec3::X))
            .unwrap();
        world.propagate_transforms();
        let root_global = *world.global_transform(root).unwrap();
        let child_global = *world.global_transform(child).unwrap();

        assert_eq!(
            world.reparent(root, Some(child)),
            Err(WorldError::HierarchyCycle)
        );
        assert_eq!(world.parent(root).unwrap(), None);
        assert_eq!(world.parent(child).unwrap(), Some(root));
        assert_eq!(*world.global_transform(root).unwrap(), root_global);
        assert_eq!(*world.global_transform(child).unwrap(), child_global);

        let other = World::new();
        assert!(matches!(other.get(root), Err(WorldError::WrongWorld(_))));
    }

    #[test]
    fn handles_a_deep_hierarchy_without_recursion() {
        const DEPTH: usize = 20_000;

        let mut world = World::with_capacity(DEPTH);
        let root = world.spawn("root");
        let mut parent = root;
        for index in 1..DEPTH {
            parent = world.spawn_child(parent, index.to_string()).unwrap();
        }

        assert_eq!(world.propagate_transforms().updated, DEPTH);
        world
            .set_local_transform(root, Transform::from_translation(Vec3::X))
            .unwrap();
        assert_eq!(world.propagate_transforms().updated, DEPTH);
        assert_vec3_close(
            world.global_transform(parent).unwrap().translation(),
            Vec3::X,
        );
        assert_eq!(world.remove_subtree(root).unwrap(), DEPTH);
        assert!(world.is_empty());
    }

    #[derive(Component, Debug, Eq, PartialEq)]
    struct Health(u32);

    #[test]
    fn keeps_component_identity_coherent_through_subtree_removal_and_slot_reuse() {
        let mut world = World::new();
        let root = world.spawn("root");
        let child = world.spawn_child(root, "child").unwrap();
        world.insert_component(root, Health(100)).unwrap();
        world.insert_component(child, Health(50)).unwrap();

        assert_eq!(world.component::<Health>(root).unwrap(), Some(&Health(100)));
        assert_eq!(
            world
                .component::<EntityIdentity>(child)
                .unwrap()
                .copied()
                .map(EntityIdentity::id),
            Some(child)
        );
        assert_eq!(
            world.insert_component(root, EntityIdentity(root)),
            Err(WorldError::ProtectedComponent)
        );
        world.validate_component_mappings().unwrap();

        assert_eq!(world.remove_subtree(root).unwrap(), 2);
        assert!(matches!(
            world.component::<Health>(child),
            Err(WorldError::MissingEntity(_))
        ));

        let replacement = world.spawn("replacement");
        assert_ne!(replacement, root);
        world.insert_component(replacement, Health(75)).unwrap();
        assert_eq!(
            world.component::<Health>(replacement).unwrap(),
            Some(&Health(75))
        );
        assert!(matches!(
            world.component::<Health>(root),
            Err(WorldError::MissingEntity(_))
        ));
        world.validate_component_mappings().unwrap();
    }

    #[test]
    fn commits_lifecycle_commands_in_order_and_reports_failures() {
        let mut world = World::new();
        let doomed = world.spawn("doomed");
        let foreign_parent = World::new().spawn("foreign");
        {
            let mut queue = world.resource_mut::<EntityLifecycleQueue>().unwrap();
            queue.spawn("created");
            queue.spawn_child(foreign_parent, "invalid child");
            queue.remove_subtree(doomed);
        }

        world.commit_lifecycle_commands();

        let results = world.resource::<EntityLifecycleResults>().unwrap();
        assert!(matches!(
            results.as_slice(),
            [
                EntityLifecycleResult::Spawned(_),
                EntityLifecycleResult::Failed {
                    command: EntityLifecycleCommandKind::SpawnChild,
                    error: WorldError::WrongWorld(_),
                },
                EntityLifecycleResult::Removed { root, count: 1 },
            ] if *root == doomed
        ));
        assert_eq!(world.len(), 1);
        world.validate_component_mappings().unwrap();
        assert!(world.resource::<EntityLifecycleQueue>().unwrap().is_empty());
    }

    fn vector_output(world: &World, behavior: BehaviorId) -> Vec3 {
        let BehaviorValue::Vector(value) = world.behavior_output(behavior).unwrap() else {
            panic!("expected vector behaviour output");
        };
        value
    }

    fn scalar_output(world: &World, behavior: BehaviorId) -> f32 {
        let BehaviorValue::Scalar(value) = world.behavior_output(behavior).unwrap() else {
            panic!("expected scalar behaviour output");
        };
        value
    }

    #[test]
    fn evaluates_linked_motion_from_the_previous_snapshot() {
        let mut world = World::new();
        let leader_entity = world.spawn("leader");
        let first_entity = world.spawn("first follower");
        let second_entity = world.spawn("second follower");
        let leader = world
            .add_behavior(
                leader_entity,
                BehaviorSpec::linear_motion(Vec3::ZERO, Vec3::X).unwrap(),
            )
            .unwrap();
        let first = world
            .add_behavior(
                first_entity,
                BehaviorSpec::follow_position(Vec3::NEG_X, 10.0).unwrap(),
            )
            .unwrap();
        let second = world
            .add_behavior(
                second_entity,
                BehaviorSpec::follow_position(Vec3::new(-2.0, 0.0, 0.0), 10.0).unwrap(),
            )
            .unwrap();
        world
            .connect_behaviors(
                leader,
                behavior_ports::POSITION,
                first,
                behavior_ports::TARGET,
            )
            .unwrap();
        world
            .connect_behaviors(
                first,
                behavior_ports::POSITION,
                second,
                behavior_ports::TARGET,
            )
            .unwrap();

        let stats = world.evaluate_behaviors(Duration::from_secs(1)).unwrap();
        assert_eq!(stats.evaluated, 3);
        assert_eq!(stats.transform_writes, 3);
        assert_vec3_close(vector_output(&world, leader), Vec3::X);
        assert_vec3_close(vector_output(&world, first), Vec3::ZERO);
        assert_vec3_close(vector_output(&world, second), Vec3::NEG_X);

        world.evaluate_behaviors(Duration::from_secs(1)).unwrap();
        assert_vec3_close(vector_output(&world, leader), Vec3::new(2.0, 0.0, 0.0));
        assert_vec3_close(vector_output(&world, first), Vec3::X);
        assert_vec3_close(vector_output(&world, second), Vec3::ZERO);
        assert_vec3_close(
            world.local_transform(second_entity).unwrap().translation,
            Vec3::ZERO,
        );
    }

    #[test]
    fn feedback_advances_once_per_tick_and_disconnection_uses_default() {
        let mut world = World::new();
        let entity = world.spawn("feedback");
        let behavior = world
            .add_behavior(entity, BehaviorSpec::add_scalar(0.0, 1.0, 10.0).unwrap())
            .unwrap();
        world
            .connect_behaviors(
                behavior,
                behavior_ports::VALUE,
                behavior,
                behavior_ports::VALUE,
            )
            .unwrap();

        world.evaluate_behaviors(Duration::from_secs(1)).unwrap();
        assert_eq!(scalar_output(&world, behavior), 1.0);
        world.evaluate_behaviors(Duration::from_secs(1)).unwrap();
        assert_eq!(scalar_output(&world, behavior), 2.0);

        world
            .disconnect_behavior_input(behavior, behavior_ports::VALUE)
            .unwrap();
        let stats = world.evaluate_behaviors(Duration::from_secs(1)).unwrap();
        assert_eq!(scalar_output(&world, behavior), 11.0);
        assert_eq!(stats.defaulted, 1);
        assert_eq!(
            world.behavior_state(behavior).unwrap(),
            BehaviorRunState::Defaulted
        );
    }

    #[test]
    fn deleted_source_becomes_unavailable_and_stale_behavior_does_not_alias() {
        let mut world = World::new();
        let source_entity = world.spawn("source");
        let follower_entity = world.spawn("follower");
        let source = world
            .add_behavior(
                source_entity,
                BehaviorSpec::linear_motion(Vec3::ZERO, Vec3::X).unwrap(),
            )
            .unwrap();
        let follower = world
            .add_behavior(
                follower_entity,
                BehaviorSpec::follow_position(Vec3::NEG_X, 1.0).unwrap(),
            )
            .unwrap();
        world
            .connect_behaviors(
                source,
                behavior_ports::POSITION,
                follower,
                behavior_ports::TARGET,
            )
            .unwrap();

        assert_eq!(world.remove_subtree(source_entity).unwrap(), 1);
        let replacement_entity = world.spawn("replacement source");
        let replacement = world
            .add_behavior(
                replacement_entity,
                BehaviorSpec::linear_motion(Vec3::splat(100.0), Vec3::ZERO).unwrap(),
            )
            .unwrap();
        assert_ne!(source, replacement);
        assert!(matches!(
            world.behavior_output(source),
            Err(BehaviorError::MissingBehavior(_))
        ));

        let stats = world.evaluate_behaviors(Duration::from_secs(1)).unwrap();
        assert_eq!(stats.suspended, 1);
        assert_eq!(
            world.behavior_state(follower).unwrap(),
            BehaviorRunState::Suspended
        );
        assert_vec3_close(vector_output(&world, follower), Vec3::NEG_X);
    }

    #[test]
    fn validates_port_types_and_transform_writer_conflicts() {
        let mut world = World::new();
        let entity = world.spawn("entity");
        let scalar = world
            .add_behavior(entity, BehaviorSpec::add_scalar(0.0, 1.0, 0.0).unwrap())
            .unwrap();
        let writer = world
            .add_behavior(
                entity,
                BehaviorSpec::linear_motion(Vec3::ZERO, Vec3::X).unwrap(),
            )
            .unwrap();
        let follower_entity = world.spawn("follower");
        let follower = world
            .add_behavior(
                follower_entity,
                BehaviorSpec::follow_position(Vec3::ZERO, 1.0).unwrap(),
            )
            .unwrap();

        assert_eq!(
            world.connect_behaviors(
                scalar,
                behavior_ports::VALUE,
                follower,
                behavior_ports::TARGET,
            ),
            Err(BehaviorError::PortTypeMismatch {
                source: crate::BehaviorValueType::Scalar,
                target: crate::BehaviorValueType::Vector,
            })
        );
        assert_eq!(
            world.add_behavior(
                entity,
                BehaviorSpec::follow_position(Vec3::ZERO, 1.0).unwrap(),
            ),
            Err(BehaviorError::TransformWriterConflict(entity))
        );

        world.set_behavior_enabled(writer, false).unwrap();
        let replacement_writer = world
            .add_behavior(
                entity,
                BehaviorSpec::follow_position(Vec3::ZERO, 1.0).unwrap(),
            )
            .unwrap();
        assert_eq!(
            world.set_behavior_enabled(writer, true),
            Err(BehaviorError::TransformWriterConflict(entity))
        );
        assert_eq!(world.behavior_owner(replacement_writer).unwrap(), entity);
    }

    #[test]
    fn rewires_links_and_suspends_when_the_new_source_is_disabled() {
        let mut world = World::new();
        let first_entity = world.spawn("first source");
        let second_entity = world.spawn("second source");
        let follower_entity = world.spawn("follower");
        let first = world
            .add_behavior(
                first_entity,
                BehaviorSpec::linear_motion(Vec3::X, Vec3::ZERO).unwrap(),
            )
            .unwrap();
        let second = world
            .add_behavior(
                second_entity,
                BehaviorSpec::linear_motion(Vec3::NEG_X, Vec3::ZERO).unwrap(),
            )
            .unwrap();
        let follower = world
            .add_behavior(
                follower_entity,
                BehaviorSpec::follow_position(Vec3::ZERO, 10.0).unwrap(),
            )
            .unwrap();

        world
            .connect_behaviors(
                first,
                behavior_ports::POSITION,
                follower,
                behavior_ports::TARGET,
            )
            .unwrap();
        world.evaluate_behaviors(Duration::from_secs(1)).unwrap();
        assert_vec3_close(vector_output(&world, follower), Vec3::X);

        world
            .connect_behaviors(
                second,
                behavior_ports::POSITION,
                follower,
                behavior_ports::TARGET,
            )
            .unwrap();
        world.evaluate_behaviors(Duration::from_secs(1)).unwrap();
        assert_vec3_close(vector_output(&world, follower), Vec3::NEG_X);

        world.set_behavior_enabled(second, false).unwrap();
        let stats = world.evaluate_behaviors(Duration::from_secs(1)).unwrap();
        assert_eq!(stats.suspended, 1);
        assert_eq!(
            world.behavior_state(second).unwrap(),
            BehaviorRunState::Disabled
        );
        assert_eq!(
            world.behavior_state(follower).unwrap(),
            BehaviorRunState::Suspended
        );
        assert_vec3_close(vector_output(&world, follower), Vec3::NEG_X);
    }

    #[test]
    fn scene_teardown_removes_owned_behaviors() {
        let mut world = World::new();
        let scene = world.spawn("scene");
        let child = world.spawn_child(scene, "child").unwrap();
        let scene_behavior = world
            .add_behavior(
                scene,
                BehaviorSpec::linear_motion(Vec3::ZERO, Vec3::ZERO).unwrap(),
            )
            .unwrap();
        let child_behavior = world
            .add_behavior(child, BehaviorSpec::add_scalar(0.0, 1.0, 0.0).unwrap())
            .unwrap();

        assert_eq!(world.remove_subtree(scene).unwrap(), 2);
        assert_eq!(world.behavior_count(), 0);
        assert!(matches!(
            world.behavior_output(scene_behavior),
            Err(BehaviorError::MissingBehavior(_))
        ));
        assert!(matches!(
            world.behavior_output(child_behavior),
            Err(BehaviorError::MissingBehavior(_))
        ));
    }

    #[test]
    fn equivalent_behavior_runs_are_deterministic() {
        fn run() -> (Vec3, Vec3) {
            let mut world = World::new();
            let leader_entity = world.spawn("leader");
            let follower_entity = world.spawn("follower");
            let leader = world
                .add_behavior(
                    leader_entity,
                    BehaviorSpec::linear_motion(Vec3::ZERO, Vec3::new(0.25, 0.5, -0.75)).unwrap(),
                )
                .unwrap();
            let follower = world
                .add_behavior(
                    follower_entity,
                    BehaviorSpec::follow_position(Vec3::splat(-1.0), 2.0).unwrap(),
                )
                .unwrap();
            world
                .connect_behaviors(
                    leader,
                    behavior_ports::POSITION,
                    follower,
                    behavior_ports::TARGET,
                )
                .unwrap();
            for _ in 0..600 {
                world
                    .evaluate_behaviors(Duration::from_secs_f64(1.0 / 60.0))
                    .unwrap();
            }
            (
                vector_output(&world, leader),
                vector_output(&world, follower),
            )
        }

        assert_eq!(run(), run());
    }
}
