use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use glam::{Affine3A, Mat4, Quat, Vec3};
use slotmap::{new_key_type, SlotMap};

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
}

impl fmt::Display for WorldError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongWorld(id) => write!(formatter, "entity {id:?} belongs to another world"),
            Self::MissingEntity(id) => write!(formatter, "entity {id:?} does not exist"),
            Self::HierarchyCycle => formatter.write_str("parenting would create a hierarchy cycle"),
            Self::InvalidTransform => formatter
                .write_str("transform values must be finite and rotations must be normalized"),
        }
    }
}

impl Error for WorldError {}

pub struct World {
    id: u64,
    entities: SlotMap<EntityKey, Entity>,
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
        Self {
            id: NEXT_WORLD_ID.fetch_add(1, Ordering::Relaxed),
            entities: SlotMap::with_capacity_and_key(capacity),
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
        self.id_for(key)
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
        if let Some(parent) = self.entities[root_key].parent {
            self.entities[parent]
                .children
                .retain(|child| *child != root_key);
        }

        self.prepare_transform_stack();
        self.transform_stack.push(root_key);
        let mut removed = 0;
        while let Some(key) = self.transform_stack.pop() {
            if let Some(entity) = self.entities.remove(key) {
                self.transform_stack.extend(entity.children);
                removed += 1;
            }
        }
        Ok(removed)
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
}
