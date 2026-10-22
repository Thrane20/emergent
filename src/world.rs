use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use slotmap::{new_key_type, SlotMap};

new_key_type! {
    struct EntityKey;
}

static NEXT_WORLD_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EntityId {
    world: u64,
    key: EntityKey,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub translation: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
        }
    }
}

#[derive(Debug)]
pub struct Entity {
    name: String,
    transform: Transform,
    parent: Option<EntityKey>,
    children: Vec<EntityKey>,
}

impl Entity {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn transform(&self) -> &Transform {
        &self.transform
    }

    pub fn transform_mut(&mut self) -> &mut Transform {
        &mut self.transform
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldError {
    WrongWorld(EntityId),
    MissingEntity(EntityId),
    HierarchyCycle,
}

impl fmt::Display for WorldError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongWorld(id) => write!(formatter, "entity {id:?} belongs to another world"),
            Self::MissingEntity(id) => write!(formatter, "entity {id:?} does not exist"),
            Self::HierarchyCycle => formatter.write_str("parenting would create a hierarchy cycle"),
        }
    }
}

impl Error for WorldError {}

pub struct World {
    id: u64,
    entities: SlotMap<EntityKey, Entity>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        Self {
            id: NEXT_WORLD_ID.fetch_add(1, Ordering::Relaxed),
            entities: SlotMap::with_key(),
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
            transform: Transform::default(),
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

    pub fn get_mut(&mut self, id: EntityId) -> Result<&mut Entity, WorldError> {
        let key = self.key_for(id)?;
        Ok(&mut self.entities[key])
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

        if let Some(old_parent) = self.entities[entity_key].parent {
            self.entities[old_parent]
                .children
                .retain(|child| *child != entity_key);
        }

        self.entities[entity_key].parent = new_parent_key;
        if let Some(parent) = new_parent_key {
            self.entities[parent].children.push(entity_key);
        }
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

        let mut pending = vec![root_key];
        let mut removed = 0;
        while let Some(key) = pending.pop() {
            if let Some(entity) = self.entities.remove(key) {
                pending.extend(entity.children);
                removed += 1;
            }
        }
        Ok(removed)
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
    use super::*;

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
    }

    #[test]
    fn rejects_cycles_and_cross_world_handles() {
        let mut world = World::new();
        let root = world.spawn("root");
        let child = world.spawn_child(root, "child").unwrap();

        assert_eq!(
            world.reparent(root, Some(child)),
            Err(WorldError::HierarchyCycle)
        );

        let other = World::new();
        assert!(matches!(other.get(root), Err(WorldError::WrongWorld(_))));
    }
}
