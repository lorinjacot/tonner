use std::{
    iter::{FusedIterator, repeat_n},
    ops::{Index, IndexMut},
};

use crate::entity_component::{
    EntityId,
    component::{ComponentStorage, ComponentsView, ComponentsViewMut},
};

#[derive(Debug, Clone)]
struct SparseEntry {
    dense: u16,
    version: u16,
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
    /// Constructs a new, empty `SparseArray<T>`.
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

    /// Returns the number of components in the sparse array.
    ///
    /// This is not the same as the capacity of the sparse array, which is the maximum number of components it can hold without reallocating.
    pub fn len(&self) -> usize {
        self.dense.len()
    }

    /// Returns `true` if the sparse array contains no components.
    ///
    /// This is not the same as the capacity of the sparse array, which is the maximum number of components it can hold without reallocating.
    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }

    /// Returns an iterator over the entities and components of the sparse array.
    ///
    /// The order of the entities and components is not specified and may change when components are added or removed from the sparse array.
    /// If only the components are needed, use [`SparseArray::values`] or [`SparseArray::values_mut`] instead.
    pub fn values(&self) -> Values<'_, T> {
        Values {
            inner: self.dense.iter(),
        }
    }

    /// Returns an iterator over the mutable components of the sparse array.
    ///
    /// The order of the components is not specified and may change when components are added or removed from the sparse array.
    /// If only the components are needed, use [`SparseArray::values`] or [`SparseArray::values_mut`] instead.
    pub fn values_mut(&mut self) -> ValuesMut<'_, T> {
        ValuesMut {
            inner: self.dense.iter_mut(),
        }
    }

    /// Removes all components from the sparse array, returning an iterator over the entities and components that were removed.
    ///
    /// The order of the entities and components is not specified.
    /// The sparse array will be empty after this method returns.
    ///
    /// If the iterator is dropped before being fully consumed, it drops the remaining removed elements.
    pub fn drain(&mut self) -> Drain<'_, T> {
        Drain {
            inner: self.dense.drain(..),
        }
    }
}

impl<T> FromIterator<(EntityId, T)> for SparseArray<T> {
    fn from_iter<I: IntoIterator<Item = (EntityId, T)>>(iter: I) -> Self {
        let mut sparse_array = SparseArray::new();
        for (entity, component) in iter {
            sparse_array.add(entity, component);
        }
        sparse_array
    }
}

pub struct Iter<'a, T> {
    inner: std::slice::Iter<'a, DenseEntry<T>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
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

impl<'a, T> ExactSizeIterator for Iter<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, T> FusedIterator for Iter<'a, T> {}

pub struct IterMut<'a, T> {
    inner: std::slice::IterMut<'a, DenseEntry<T>>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
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

impl<'a, T> ExactSizeIterator for IterMut<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, T> FusedIterator for IterMut<'a, T> {}

/// An iterator over the components of a `SparseArray<T>`.
///
/// The order of the components is not specified and may change when components are added or removed from the `SparseArray<T>`.
/// If the entity is needed, use [`Iter`] or [`IterMut`] instead.
pub struct Values<'a, T> {
    inner: std::slice::Iter<'a, DenseEntry<T>>,
}

impl<'a, T> Iterator for Values<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|dense_entry| &dense_entry.component)
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
            .fold(init, |b, dense_entry| f(b, &dense_entry.component))
    }
}

impl<'a, T> ExactSizeIterator for Values<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, T> FusedIterator for Values<'a, T> {}

/// An iterator over the mutable components of a `SparseArray<T>`.
///
/// The order of the components is not specified and may change when components are added or removed from the `SparseArray<T>`.
/// If the entity is needed, use [`Iter`] or [`IterMut`] instead.
pub struct ValuesMut<'a, T> {
    inner: std::slice::IterMut<'a, DenseEntry<T>>,
}

impl<'a, T> Iterator for ValuesMut<'a, T> {
    type Item = &'a mut T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|dense_entry| &mut dense_entry.component)
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
            .fold(init, |b, dense_entry| f(b, &mut dense_entry.component))
    }
}

impl<'a, T> ExactSizeIterator for ValuesMut<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, T> FusedIterator for ValuesMut<'a, T> {}

/// An iterator that drains the components of a `SparseArray<T>`, yielding the entities and components as mutable references.
///
/// This iterator is created by the [`SparseArray::drain`] method. See its documentation for more.
pub struct Drain<'a, T> {
    inner: std::vec::Drain<'a, DenseEntry<T>>,
}

impl<'a, T> Iterator for Drain<'a, T> {
    type Item = (EntityId, T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|dense_entry| (dense_entry.entity, dense_entry.component))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, T> ExactSizeIterator for Drain<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, T> FusedIterator for Drain<'a, T> {}

impl<T> ComponentsView<T> for SparseArray<T> {
    type Iter<'a>
        = Iter<'a, T>
    where
        Self: 'a,
        T: 'a;

    fn has(&self, entity: EntityId) -> bool {
        match self.sparse.get(entity.sparse() as usize) {
            Some(entry) if entry.version == entity.version().get() => true,
            _ => false,
        }
    }

    fn get(&self, entity: EntityId) -> Option<&T> {
        match self.sparse.get(entity.sparse() as usize) {
            Some(sparse_entry) if sparse_entry.version == entity.version().get() => {
                Some(&self.dense[sparse_entry.dense as usize].component)
            }
            _ => None,
        }
    }

    fn iter<'a>(&'a self) -> Self::Iter<'a> {
        Iter {
            inner: self.dense.iter(),
        }
    }
}

impl<T> ComponentsViewMut<T> for SparseArray<T> {
    type IterMut<'a>
        = IterMut<'a, T>
    where
        Self: 'a,
        T: 'a;

    fn get_mut(&mut self, entity: EntityId) -> Option<&mut T> {
        match self.sparse.get(entity.sparse() as usize) {
            Some(sparse_entry) if sparse_entry.version == entity.version().get() => {
                Some(&mut self.dense[sparse_entry.dense as usize].component)
            }
            _ => None,
        }
    }

    fn iter_mut<'a>(&'a mut self) -> Self::IterMut<'a> {
        IterMut {
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
                        sparse_entry.version = entity.version().get();
                        dense_entry.entity = entity;
                        dense_entry.component = component;
                        None
                    }
                }
                _ => {
                    // no dense entry for the `sparse` index
                    sparse_entry.dense = self.dense.len() as u16;
                    sparse_entry.version = entity.version().get();
                    self.dense.push(DenseEntry { entity, component });
                    None
                }
            },
            None => {
                self.sparse.extend(
                    repeat_n(
                        SparseEntry {
                            dense: u16::MAX,
                            version: 0,
                        },
                        sparse_idx - self.sparse.len(),
                    )
                    .chain(Some(SparseEntry {
                        dense: self.dense.len() as u16,
                        version: entity.version().get(),
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
            Some(sparse_entry) if sparse_entry.version == entity.version().get() => {
                let dense_idx = sparse_entry.dense as usize;
                self.sparse[self.dense.last().unwrap().entity.sparse() as usize].dense =
                    sparse_entry.dense;
                self.sparse[sparse_idx].version = 0;
                Some(self.dense.swap_remove(dense_idx).component)
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
    use crate::entity_component::entity::EntityManager;

    use super::*;

    #[derive(Debug)]
    struct TestData(&'static str);

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
