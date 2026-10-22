use std::any::TypeId;
use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

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
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Transform {
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
    transform_stack: Vec<EntityKey>,
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
        let unauthorized_entity_query = components
            .query_filtered::<EcsEntity, (Without<EntityIdentity>, Without<IsResource>)>();
        Self {
            id: NEXT_WORLD_ID.fetch_add(1, Ordering::Relaxed),
            entities: SlotMap::with_capacity_and_key(capacity),
            component_entities: SecondaryMap::with_capacity(capacity),
            components,
            unauthorized_entity_query,
            transform_stack: Vec::with_capacity(capacity),
        }
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
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
        Ok(existed)
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
    use std::f32::consts::FRAC_PI_2;

    use super::*;

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
}
