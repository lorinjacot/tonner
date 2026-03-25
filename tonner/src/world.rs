use std::{collections::HashMap, iter::repeat_n};

/// A world is a collection of entities. Each entity is composed of a unique [EntityId]
/// and data components. Components can by dynamically added, accessed, modified and removed.
/// This pattern is known as an Entity component system (ECS).
#[derive(Debug, Default)]
pub struct World {
    deleted_entities: Vec<EntityId>,
    next_sparse: u16,
}

impl World {
    /// Generates a unique entity id.
    pub fn new_entity(&mut self) -> EntityId {
        self.deleted_entities.pop().map_or_else(
            || {
                let sparse = self.next_sparse;
                self.next_sparse += 1;
                EntityId { version: 0, sparse }
            },
            |EntityId { version, sparse }| EntityId {
                version: version + 1,
                sparse,
            },
        )
    }

    pub fn delete_entity(&mut self, entity: EntityId) {
        todo!()
    }
}

/// Unique id for a [`World`] entity. This is used to associate
/// entities with their components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId {
    version: u16,
    /// Always strictly smaller than `u16::MAX`.
    sparse: u16,
}

pub trait ComponentStorage<T> {
    /// Adds the component to the entity.
    ///
    /// If the entity did not have this component, `None` is returned.
    ///
    /// If the entity did have this component, the component is updated and the old value is returned.
    fn add(&mut self, entity: EntityId, component: T) -> Option<T>;

    /// Removes the component from the entity, returning the component value if the entity previously had it.
    fn remove(&mut self, entity: EntityId) -> Option<T>;

    /// Removes all component `T` from all entities. Keeps the allocated memory for reuse.
    fn clear(&mut self);

    /// Returns true if the entity has the component.
    fn contains(&self, entity: EntityId) -> bool;

    /// Returns a reference to the component belonging to the entity.
    fn get(&self, entity: EntityId) -> Option<&T>;

    /// Returns a mutable reference to the component belonging to the entity.
    fn get_mut(&mut self, entity: EntityId) -> Option<&mut T>;
}

impl<T> ComponentStorage<T> for HashMap<EntityId, T> {
    fn add(&mut self, entity: EntityId, component: T) -> Option<T> {
        self.insert(entity, component)
    }

    fn remove(&mut self, entity: EntityId) -> Option<T> {
        self.remove(&entity)
    }

    fn clear(&mut self) {
        self.clear();
    }

    fn contains(&self, entity: EntityId) -> bool {
        self.contains_key(&entity)
    }

    fn get(&self, entity: EntityId) -> Option<&T> {
        self.get(&entity)
    }

    fn get_mut(&mut self, entity: EntityId) -> Option<&mut T> {
        self.get_mut(&entity)
    }
}

#[derive(Debug, Clone)]
struct SparseEntry {
    version: u16,
    dense: u16,
}

#[derive(Debug, Default)]
pub struct SparseSet<T> {
    sparse: Vec<SparseEntry>,
    dense: Vec<(EntityId, T)>,
}

impl<T> SparseSet<T> {
    pub fn new() -> SparseSet<T> {
        SparseSet {
            sparse: Vec::new(),
            dense: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> SparseSet<T> {
        SparseSet {
            sparse: Vec::with_capacity(capacity),
            dense: Vec::with_capacity(capacity),
        }
    }
}

impl<T> ComponentStorage<T> for SparseSet<T> {
    fn add(&mut self, entity: EntityId, component: T) -> Option<T> {
        let sparse_idx = entity.sparse as usize;
        match self.sparse.get_mut(sparse_idx) {
            Some(entry) => match self.dense.get_mut(entry.dense as usize) {
                Some((old_entity, value)) if entity.version == old_entity.version => {
                    old_entity.version = entity.version;
                    Some(std::mem::replace(value, component))
                }
                Some((old_entity, value)) => {
                    entry.version = entity.version;
                    old_entity.version = entity.version;
                    *value = component;
                    None
                }
                None => {
                    entry.dense = self.dense.len() as u16;
                    entry.version = entity.version;
                    self.dense.push((entity, component));
                    None
                }
            },
            None => {
                self.sparse.extend(
                    repeat_n(
                        SparseEntry {
                            version: u16::MAX,
                            dense: u16::MAX,
                        },
                        sparse_idx - self.sparse.len(),
                    )
                    .chain(Some(SparseEntry {
                        version: entity.version,
                        dense: self.dense.len() as u16,
                    })),
                );
                self.dense.push((entity, component));
                None
            }
        }
    }

    fn remove(&mut self, entity: EntityId) -> Option<T> {
        let sparse_idx = entity.sparse as usize;
        match self.sparse.get(sparse_idx) {
            Some(entry) if entity.version == entity.version => {
                let dense_idx = entry.dense as usize;
                self.sparse[self.dense.last().unwrap().0.sparse as usize].dense = entry.dense;
                self.sparse[sparse_idx].dense = u16::MAX;
                Some(self.dense.swap_remove(dense_idx).1)
            }
            _ => None,
        }
    }

    fn clear(&mut self) {
        self.dense.clear();
        self.sparse.clear();
    }

    fn contains(&self, entity: EntityId) -> bool {
        match self.sparse.get(entity.sparse as usize) {
            Some(entry) if entry.version == entity.version => true,
            _ => false,
        }
    }

    fn get(&self, entity: EntityId) -> Option<&T> {
        match self.sparse.get(entity.sparse as usize) {
            Some(entry) if entry.version == entity.version => {
                match self.dense.get(entry.dense as usize) {
                    Some((_, component)) => Some(component),
                    None => None,
                }
            }
            _ => None,
        }
    }

    fn get_mut(&mut self, entity: EntityId) -> Option<&mut T> {
        match self.sparse.get(entity.sparse as usize) {
            Some(entry) if entry.version == entity.version => {
                match self.dense.get_mut(entry.dense as usize) {
                    Some((_, component)) => Some(component),
                    None => None,
                }
            }
            _ => None,
        }
    }
}
