use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    iter::{FusedIterator, repeat_n},
    num::{NonZeroU16, NonZeroU32},
    ops::{Index, IndexMut},
};

#[cfg(feature = "python")]
use pyo3::prelude::*;

/// An entity manager is responsible of generating unique [EntityId]. It can also
/// recycle deleted ones.
#[derive(Debug, Default)]
pub struct EntityManager {
    dense: Vec<EntityId>,
    sparse: Vec<SparseEntry>,
    available: usize,
    next: u16,
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
    pub fn iter<'a>(&'a self) -> EntityIter<'a> {
        EntityIter {
            inner: self.dense.iter(),
        }
    }
}

pub struct EntityIter<'a> {
    inner: std::slice::Iter<'a, EntityId>,
}

impl<'a> Iterator for EntityIter<'a> {
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

impl<'a> ExactSizeIterator for EntityIter<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a> FusedIterator for EntityIter<'a> {}

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
    fn sparse(&self) -> u16 {
        let sparse = (self.0.get() & SPARSE_BITES) >> 16;
        sparse as u16
    }

    #[inline]
    fn version(&self) -> NonZeroU16 {
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

pub trait ComponentsView<T> {
    type Iter<'a>: Iterator<Item = (EntityId, &'a T)>
    where
        Self: 'a,
        T: 'a;

    type IterMut<'a>: Iterator<Item = (EntityId, &'a mut T)>
    where
        Self: 'a,
        T: 'a;

    /// Returns true if the entity has the component.
    fn has(&self, entity: EntityId) -> bool;

    /// Returns a reference to the component belonging to the entity.
    fn get(&self, entity: EntityId) -> Option<&T>;

    /// Returns a mutable reference to the component belonging to the entity.
    fn get_mut(&mut self, entity: EntityId) -> Option<&mut T>;

    /// An iterator visiting all components `T` in arbitrary order. The iterator element type is `(EntityId, &'a T)`.
    fn iter<'a>(&'a self) -> Self::Iter<'a>;

    /// An iterator visiting all components `T` in arbitrary order, with mutable references to the values.
    /// The iterator element type is `(EntityId, &'a T)`.
    fn iter_mut<'a>(&'a mut self) -> Self::IterMut<'a>;
}

pub trait ComponentStorage<T>: ComponentsView<T> {
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
}

pub struct HashMapIter<'a, T> {
    inner: std::collections::hash_map::Iter<'a, EntityId, T>,
}

impl<'a, T> Iterator for HashMapIter<'a, T> {
    type Item = (EntityId, &'a T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|(&entity, component)| (entity, component))
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
        self.inner
            .fold(init, |b, (&entity, component)| f(b, (entity, component)))
    }
}

impl<'a, T> ExactSizeIterator for HashMapIter<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, T> FusedIterator for HashMapIter<'a, T> {}

pub struct HashMapIterMut<'a, T> {
    inner: std::collections::hash_map::IterMut<'a, EntityId, T>,
}

impl<'a, T> Iterator for HashMapIterMut<'a, T> {
    type Item = (EntityId, &'a mut T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|(&entity, component)| (entity, component))
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
        self.inner
            .fold(init, |b, (&entity, component)| f(b, (entity, component)))
    }
}

impl<'a, T> ExactSizeIterator for HashMapIterMut<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, T> FusedIterator for HashMapIterMut<'a, T> {}

impl<T> ComponentsView<T> for HashMap<EntityId, T> {
    type Iter<'a>
        = HashMapIter<'a, T>
    where
        Self: 'a,
        T: 'a;

    type IterMut<'a>
        = HashMapIterMut<'a, T>
    where
        Self: 'a,
        T: 'a;

    fn has(&self, entity: EntityId) -> bool {
        self.contains_key(&entity)
    }

    fn get(&self, entity: EntityId) -> Option<&T> {
        self.get(&entity)
    }

    fn get_mut(&mut self, entity: EntityId) -> Option<&mut T> {
        self.get_mut(&entity)
    }

    fn iter<'a>(&'a self) -> Self::Iter<'a> {
        HashMapIter { inner: self.iter() }
    }

    fn iter_mut<'a>(&'a mut self) -> Self::IterMut<'a> {
        HashMapIterMut {
            inner: self.iter_mut(),
        }
    }
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
}

#[derive(Debug, Clone)]
struct SparseEntry {
    dense: u16,
    version: NonZeroU16,
}

#[derive(Debug)]
struct DenseEntry<T> {
    entity: EntityId,
    component: T,
}

#[derive(Debug, Default)]
pub struct SparseArray<T> {
    sparse: Vec<SparseEntry>,
    dense: Vec<DenseEntry<T>>,
}

impl<T> SparseArray<T> {
    /// Constructs a new, empty SparseArray<T>.
    ///
    /// The sparse array will not allocate until components are added.
    pub fn new() -> SparseArray<T> {
        SparseArray {
            sparse: Vec::new(),
            dense: Vec::new(),
        }
    }

    /// Constructs a new, empty SparseArray<T> with at least the specified capacity.
    ///
    /// The sparse array will be able to hold at least capacity elements without reallocating.
    /// This method is allowed to allocate for more elements than capacity. If capacity is zero, the sparse array will not allocate.
    ///
    /// ## Panics
    /// Panics if the new capacity exceeds isize::MAX bytes.
    pub fn with_capacity(capacity: usize) -> SparseArray<T> {
        SparseArray {
            sparse: Vec::with_capacity(capacity),
            dense: Vec::with_capacity(capacity),
        }
    }
}

pub struct SparseArrayIter<'a, T> {
    inner: std::slice::Iter<'a, DenseEntry<T>>,
}

impl<'a, T> Iterator for SparseArrayIter<'a, T> {
    type Item = (EntityId, &'a T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|dense_entry| (dense_entry.entity, &dense_entry.component))
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
        self.inner.fold(init, |b, dense_entry| {
            f(b, (dense_entry.entity, &dense_entry.component))
        })
    }
}

impl<'a, T> ExactSizeIterator for SparseArrayIter<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, T> FusedIterator for SparseArrayIter<'a, T> {}

pub struct SparseArrayIterMut<'a, T> {
    inner: std::slice::IterMut<'a, DenseEntry<T>>,
}

impl<'a, T> Iterator for SparseArrayIterMut<'a, T> {
    type Item = (EntityId, &'a mut T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|dense_entry| (dense_entry.entity, &mut dense_entry.component))
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
        self.inner.fold(init, |b, dense_entry| {
            f(b, (dense_entry.entity, &mut dense_entry.component))
        })
    }
}

impl<'a, T> ExactSizeIterator for SparseArrayIterMut<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, T> FusedIterator for SparseArrayIterMut<'a, T> {}

impl<T> ComponentsView<T> for SparseArray<T> {
    type Iter<'a>
        = SparseArrayIter<'a, T>
    where
        Self: 'a,
        T: 'a;

    type IterMut<'a>
        = SparseArrayIterMut<'a, T>
    where
        Self: 'a,
        T: 'a;

    fn has(&self, entity: EntityId) -> bool {
        match self.sparse.get(entity.sparse() as usize) {
            Some(entry) if entry.version == entity.version() => true,
            _ => false,
        }
    }

    fn get(&self, entity: EntityId) -> Option<&T> {
        match self.sparse.get(entity.sparse() as usize) {
            Some(sparse_entry) if sparse_entry.version == entity.version() => {
                match self.dense.get(sparse_entry.dense as usize) {
                    Some(dense_entry) if dense_entry.entity.sparse() == entity.sparse() => {
                        Some(&dense_entry.component)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn get_mut(&mut self, entity: EntityId) -> Option<&mut T> {
        match self.sparse.get(entity.sparse() as usize) {
            Some(sparse_entry) if sparse_entry.version == entity.version() => {
                match self.dense.get_mut(sparse_entry.dense as usize) {
                    Some(dense_entry) if dense_entry.entity.sparse() == entity.sparse() => {
                        Some(&mut dense_entry.component)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn iter<'a>(&'a self) -> Self::Iter<'a> {
        SparseArrayIter {
            inner: self.dense.iter(),
        }
    }

    fn iter_mut<'a>(&'a mut self) -> Self::IterMut<'a> {
        SparseArrayIterMut {
            inner: self.dense.iter_mut(),
        }
    }
}

impl<T> ComponentStorage<T> for SparseArray<T> {
    fn add(&mut self, entity: EntityId, component: T) -> Option<T> {
        let sparse_idx = entity.sparse() as usize;
        match self.sparse.get_mut(sparse_idx) {
            Some(sparse_entry) => match self.dense.get_mut(sparse_entry.dense as usize) {
                Some(dense_entry) if dense_entry.entity.sparse() == entity.sparse() => {
                    if dense_entry.entity.version() == entity.version() {
                        // the entity already had the component
                        Some(std::mem::replace(&mut dense_entry.component, component))
                    } else {
                        // a deleted entity with the same `sparse` had the component
                        sparse_entry.version = entity.version();
                        dense_entry.entity = entity;
                        dense_entry.component = component;
                        None
                    }
                }
                _ => {
                    // no dense entry for the `sparse` index
                    sparse_entry.dense = self.dense.len() as u16;
                    sparse_entry.version = entity.version();
                    self.dense.push(DenseEntry { entity, component });
                    None
                }
            },
            None => {
                self.sparse.extend(
                    repeat_n(
                        SparseEntry {
                            dense: u16::MAX,
                            version: NonZeroU16::new(u16::MAX).unwrap(),
                        },
                        sparse_idx - self.sparse.len(),
                    )
                    .chain(Some(SparseEntry {
                        dense: self.dense.len() as u16,
                        version: entity.version(),
                    })),
                );
                self.dense.push(DenseEntry { entity, component });
                None
            }
        }
    }

    fn remove(&mut self, entity: EntityId) -> Option<T> {
        let sparse_idx = entity.sparse() as usize;
        match self.sparse.get(sparse_idx) {
            Some(sparse_entry) if sparse_entry.version == entity.version() => {
                let dense_idx = sparse_entry.dense as usize;
                match self.dense.get(dense_idx) {
                    Some(dense_entry) if dense_entry.entity.sparse() == entity.sparse ()=> {
                        self.sparse[self.dense.last().unwrap().entity.sparse() as usize].dense =
                            sparse_entry.dense;
                        self.sparse[sparse_idx].version = NonZeroU16::new(u16::MAX).unwrap();
                        Some(self.dense.swap_remove(dense_idx).component)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn clear(&mut self) {
        self.dense.clear();
    }
}

impl<T> Index<EntityId> for SparseArray<T> {
    type Output = T;

    fn index(&self, index: EntityId) -> &Self::Output {
        self.get(index).expect("no component for the entity")
    }
}

impl<T> IndexMut<EntityId> for SparseArray<T> {
    fn index_mut(&mut self, index: EntityId) -> &mut Self::Output {
        self.get_mut(index).expect("no component for the entity")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestData(&'static str);

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

    #[test]
    fn test_sparse_array() {
        let mut manager = EntityManager::new();
        let mut storage = SparseArray::new();

        let entity_0_v1 = manager.new_entity();
        let entity_1_v1 = manager.new_entity();

        let data = storage.add(entity_0_v1, TestData("0 v1"));
        assert!(data.is_none());
        assert_eq!(1, storage.dense.len());
        assert!(storage.has(entity_0_v1));
        assert!(!storage.has(entity_1_v1));
        assert_eq!("0 v1", storage[entity_0_v1].0);
        assert!(storage.get(entity_1_v1).is_none());

        let data = storage.add(entity_1_v1, TestData("1 v1"));
        assert!(data.is_none());
        assert_eq!(2, storage.dense.len());
        assert!(storage.has(entity_0_v1));
        assert!(storage.has(entity_1_v1));
        assert_eq!("0 v1", storage[entity_0_v1].0);
        assert_eq!("1 v1", storage[entity_1_v1].0);

        manager.delete_entity(entity_0_v1);
        let entity_0_v2 = manager.new_entity();

        let data = storage.remove(entity_0_v1).unwrap();
        assert_eq!("0 v1", data.0);
        assert_eq!(1, storage.dense.len());
        assert!(!storage.has(entity_0_v1));
        assert!(!storage.has(entity_0_v2));
        assert!(storage.has(entity_1_v1));
        assert!(storage.get(entity_0_v1).is_none());
        assert_eq!("1 v1", storage[entity_1_v1].0);

        let data = storage.add(entity_0_v2, TestData("0 v2"));
        assert!(data.is_none());
        assert_eq!(2, storage.dense.len());
        assert!(!storage.has(entity_0_v1));
        assert!(storage.has(entity_0_v2));
        assert!(storage.has(entity_1_v1));
        assert!(storage.has(entity_1_v1));
        assert!(storage.get(entity_0_v1).is_none());
        assert_eq!("0 v2", storage[entity_0_v2].0);
        assert_eq!("1 v1", storage[entity_1_v1].0);

        manager.delete_entity(entity_1_v1);
        let entity_1_v2 = manager.new_entity();

        let data = storage.add(entity_1_v1, TestData("1 v1 prime")).unwrap();
        assert_eq!("1 v1", data.0);
        assert_eq!(2, storage.dense.len());
        assert!(!storage.has(entity_0_v1));
        assert!(storage.has(entity_0_v2));
        assert!(storage.has(entity_1_v1));
        assert!(!storage.has(entity_1_v2));
        assert!(storage.get(entity_0_v1).is_none());
        assert_eq!("0 v2", storage[entity_0_v2].0);
        assert_eq!("1 v1 prime", storage[entity_1_v1].0);
        assert!(storage.get(entity_1_v2).is_none());

        let data = storage.add(entity_1_v2, TestData("1 v2"));
        assert!(data.is_none());
        assert_eq!(2, storage.dense.len());
        assert!(!storage.has(entity_0_v1));
        assert!(storage.has(entity_0_v2));
        assert!(!storage.has(entity_1_v1));
        assert!(storage.has(entity_1_v2));
        assert!(storage.get(entity_0_v1).is_none());
        assert_eq!("0 v2", storage[entity_0_v2].0);
        assert!(storage.get(entity_1_v1).is_none());
        assert_eq!("1 v2", storage[entity_1_v2].0);

        let entity_2_v1 = manager.new_entity();
        let entity_3_v1 = manager.new_entity();

        let data = storage.add(entity_3_v1, TestData("3 v1"));
        assert!(data.is_none());
        assert_eq!(3, storage.dense.len());
        assert!(!storage.has(entity_0_v1));
        assert!(storage.has(entity_0_v2));
        assert!(!storage.has(entity_1_v1));
        assert!(storage.has(entity_1_v2));
        assert!(!storage.has(entity_2_v1));
        assert!(storage.has(entity_3_v1));
        assert!(storage.get(entity_0_v1).is_none());
        assert_eq!("0 v2", storage[entity_0_v2].0);
        assert!(storage.get(entity_1_v1).is_none());
        assert_eq!("1 v2", storage[entity_1_v2].0);
        assert!(storage.get(entity_2_v1).is_none());
        assert_eq!("3 v1", storage[entity_3_v1].0);

        let data = storage.add(entity_2_v1, TestData("2 v1"));
        assert!(data.is_none());
        assert_eq!(4, storage.dense.len());
        assert!(!storage.has(entity_0_v1));
        assert!(storage.has(entity_0_v2));
        assert!(!storage.has(entity_1_v1));
        assert!(storage.has(entity_1_v2));
        assert!(storage.has(entity_2_v1));
        assert!(storage.has(entity_3_v1));
        assert!(storage.get(entity_0_v1).is_none());
        assert_eq!("0 v2", storage[entity_0_v2].0);
        assert!(storage.get(entity_1_v1).is_none());
        assert_eq!("1 v2", storage[entity_1_v2].0);
        assert_eq!("2 v1", storage[entity_2_v1].0);
        assert_eq!("3 v1", storage[entity_3_v1].0);
    }
}
