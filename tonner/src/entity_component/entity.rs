#[cfg(feature = "python")]
use std::sync::Mutex;
use std::{
    fmt::{Debug, Display},
    iter::FusedIterator,
    num::{NonZeroU16, NonZeroU32},
    sync::Arc,
};

#[cfg(feature = "python")]
use pyo3::prelude::*;
use uuid::uuid;

#[cfg(feature = "python")]
use crate::world::PyWorld;
use crate::world::{StaticField, World};

/// Unique id for a [`manager`] entity. This is used to associate
/// entities with their components.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "python", pyclass(frozen, str, from_py_object))]
#[repr(transparent)]
pub struct EntityId(NonZeroU32);

const SPARSE_BITES: u32 = 0b1111_1111_1111_1111_0000_0000_0000_0000;
const VERSION_BITES: u32 = 0b0000_0000_0000_0000_1111_1111_1111_1111;

impl EntityId {
    #[inline]
    fn new(sparse: u16, version: NonZeroU16) -> EntityId {
        let sparse = (sparse as u32) << 16;
        let version = (version.get() as u32) & VERSION_BITES;
        // SAFETY: cannot be zero because `version` is non zero
        EntityId(unsafe { NonZeroU32::new_unchecked(sparse | version) })
    }

    #[inline]
    pub(super) fn sparse(&self) -> u16 {
        let sparse = (self.0.get() & SPARSE_BITES) >> 16;
        sparse as u16
    }

    #[inline]
    pub(super) fn version(&self) -> NonZeroU16 {
        let version = (self.0.get() & VERSION_BITES) as u16;
        // SAFETY: cannot be zero because the VERSION BITES cannot all be zero
        unsafe { NonZeroU16::new_unchecked(version) }
    }
}

impl Debug for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.sparse(), self.version())
    }
}

impl Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.sparse(), self.version())
    }
}

/// An entity manager is responsible of generating unique [EntityId]. It can also
/// recycle deleted ones.
#[derive(Debug, Default)]
pub struct EntityManager {
    dense: Vec<EntityId>,
    sparse: Vec<SparseEntry>,
    available: usize,
    next: u16,
}

#[derive(Debug, Clone)]
struct SparseEntry {
    dense: u16,
    version: NonZeroU16,
}

const FIRST_VERSION: NonZeroU16 = NonZeroU16::new(1).unwrap();

impl EntityManager {
    pub fn new() -> EntityManager {
        EntityManager {
            dense: Vec::new(),
            sparse: Vec::new(),
            available: 0,
            next: 0,
        }
    }

    /// Generates a unique entity id.
    #[must_use]
    pub fn new_entity(&mut self) -> EntityId {
        if self.available == 0 {
            let entity = EntityId::new(self.sparse.len() as u16, FIRST_VERSION);
            self.sparse.push(SparseEntry {
                dense: self.dense.len() as u16,
                version: FIRST_VERSION,
            });
            self.dense.push(entity);
            entity
        } else {
            let entry = &mut self.sparse[self.next as usize];
            entry.version = entry.version.checked_add(1).unwrap();
            let entity = EntityId::new(self.next, entry.version);
            self.next = entry.dense;
            self.available -= 1;
            entry.dense = self.dense.len() as u16;
            self.dense.push(entity);
            entity
        }
    }

    /// Deletes the given entity. Returns `true` if the manager previously had the entity.
    /// Returns `false` on subsequent deletions or if the manager never had the entity.
    ///
    /// This operation does not automatically delete all its componenets.
    /// However, this enable future entities to reuse the same storage offset, deacreasing the overall
    /// memomy usage of the different component storages.
    pub fn delete_entity(&mut self, entity: EntityId) -> bool {
        let sparse_idx = entity.sparse() as usize;
        match self.sparse.get_mut(sparse_idx) {
            Some(sparse_entry) if sparse_entry.version == entity.version() => {
                let dense_idx = sparse_entry.dense as usize;
                match self.dense.get(dense_idx) {
                    Some(dense_entry) if dense_entry.sparse() == entity.sparse() => {
                        self.sparse[self.dense.last().unwrap().sparse() as usize].dense =
                            sparse_entry.dense;
                        self.dense.swap_remove(dense_idx);
                        self.sparse[sparse_idx].dense = self.next;
                        self.next = entity.sparse();
                        self.available += 1;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// Returns `true` if the manager contains the entity.
    /// Returns `false` it the entity has been deleted or was never created.
    pub fn contains(&self, entity: EntityId) -> bool {
        match self.sparse.get(entity.sparse() as usize) {
            Some(sparse_entry) if sparse_entry.version == entity.version() => {
                match self.dense.get(sparse_entry.dense as usize) {
                    Some(dense_entry) if dense_entry.sparse() == entity.sparse() => true,
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// An iterator visiting all entities in arbitrary order.
    pub fn iter<'a>(&'a self) -> Iter<'a> {
        Iter {
            inner: self.dense.iter(),
        }
    }
}

impl StaticField for Mutex<EntityManager> {
    const ID: crate::world::FieldId = uuid!("5d30c780-9045-49fa-93db-bd5f31091de2");
}

/// An iterator visiting all entities in arbitrary order. Created by [`EntityManager::iter()`].
pub struct Iter<'a> {
    inner: std::slice::Iter<'a, EntityId>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = EntityId;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().copied()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline]
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.inner.count()
    }

    #[inline]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |b, &entity| f(b, entity))
    }
}

impl<'a> ExactSizeIterator for Iter<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a> FusedIterator for Iter<'a> {}

/// An entity handle. Even though this class can be used in rust,
/// it is only needed when using the python api.
#[cfg_attr(feature = "python", pyclass(frozen, str))]
#[derive(Debug)]
pub struct Entity {
    id: EntityId,
    world: Arc<World>,
}

#[cfg(feature = "python")]
#[pymethods]
impl Entity {
    #[new]
    pub fn new(world: &PyWorld) -> Entity {
        let world = world.0.clone();
        let id = match world.get::<Mutex<EntityManager>>() {
            Some(manager) => manager.lock().unwrap().new_entity(),
            None => {
                let mut manager = EntityManager::new();
                let id = manager.new_entity();
                let field = Mutex::new(manager);
                world.add(Arc::new(field));
                id
            }
        };
        Entity { id, world }
    }
}

impl Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Entity({})", self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_manager() {
        let mut manager = EntityManager::new();
        assert_eq!(0, manager.available);

        let entity_0_v1 = manager.new_entity();
        assert_eq!(1, manager.dense.len());
        assert!(manager.dense.contains(&entity_0_v1));
        assert_eq!(0, entity_0_v1.sparse());
        assert_eq!(1, entity_0_v1.version().get());
        assert!(manager.contains(entity_0_v1));

        let entity_1_v1 = manager.new_entity();
        assert_eq!(2, manager.dense.len());
        assert!(manager.dense.contains(&entity_1_v1));
        assert_eq!(1, entity_1_v1.sparse());
        assert_eq!(1, entity_1_v1.version().get());
        assert!(manager.contains(entity_0_v1));
        assert!(manager.contains(entity_1_v1));

        assert_eq!(0, manager.available);
        manager.delete_entity(entity_1_v1);
        assert_eq!(1, manager.dense.len());
        assert!(manager.dense.contains(&entity_0_v1));
        assert!(!manager.dense.contains(&entity_1_v1));
        assert_eq!(1, manager.available);
        assert_eq!(1, manager.next);
        assert!(manager.contains(entity_0_v1));
        assert!(!manager.contains(entity_1_v1));

        let entity_1_v2 = manager.new_entity();
        assert_eq!(2, manager.dense.len());
        assert!(manager.dense.contains(&entity_0_v1));
        assert!(!manager.dense.contains(&entity_1_v1));
        assert!(manager.dense.contains(&entity_1_v2));
        assert_eq!(1, entity_1_v2.sparse());
        assert_eq!(2, entity_1_v2.version().get());
        assert_eq!(0, manager.available);
        assert!(manager.contains(entity_0_v1));
        assert!(!manager.contains(entity_1_v1));
        assert!(manager.contains(entity_1_v2));

        let entity_2_v1 = manager.new_entity();
        assert_eq!(2, entity_2_v1.sparse());
        assert_eq!(1, entity_2_v1.version().get());
        assert_eq!(3, manager.dense.len());
        assert!(manager.contains(entity_0_v1));
        assert!(!manager.contains(entity_1_v1));
        assert!(manager.contains(entity_1_v2));
        assert!(manager.contains(entity_2_v1));

        manager.delete_entity(entity_0_v1);
        assert_eq!(2, manager.dense.len());
        assert_eq!(1, manager.available);
        assert_eq!(0, manager.next);
        assert!(!manager.contains(entity_0_v1));
        assert!(manager.contains(entity_1_v2));
        assert!(manager.contains(entity_2_v1));

        manager.delete_entity(entity_2_v1);
        assert_eq!(1, manager.dense.len());
        assert_eq!(2, manager.available);
        assert_eq!(2, manager.next);
        assert!(manager.contains(entity_1_v2));
        assert!(!manager.contains(entity_2_v1));

        let entity_2_v2 = manager.new_entity();
        assert_eq!(2, entity_2_v2.sparse());
        assert_eq!(2, entity_2_v2.version().get());
        assert_eq!(1, manager.available);
        assert_eq!(0, manager.next);
        assert!(manager.contains(entity_1_v2));
        assert!(!manager.contains(entity_2_v1));
        assert!(manager.contains(entity_2_v2));
    }
}
