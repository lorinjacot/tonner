use std::{
    collections::HashMap,
    iter::{FusedIterator, repeat_n},
    ops::{Index, IndexMut},
};

/// A world is a collection of entities. Each entity is composed of a unique [EntityId]
/// and data components. Components can by dynamically added, accessed, modified and removed.
/// This pattern is known as an Entity component system (ECS).
#[derive(Debug, Default)]
pub struct World {
    dense: Vec<EntityId>,
    sparse: Vec<SparseEntry>,
    available: usize,
    next: u16,
}

impl World {
    pub fn new() -> World {
        World {
            dense: Vec::new(),
            sparse: Vec::new(),
            available: 0,
            next: 0,
        }
    }

    /// Generates a unique entity id.
    pub fn new_entity(&mut self) -> EntityId {
        if self.available == 0 {
            let entity = EntityId {
                sparse: self.sparse.len() as u16,
                version: 0,
            };
            self.sparse.push(SparseEntry {
                version: 0,
                dense: self.dense.len() as u16,
            });
            self.dense.push(entity);
            entity
        } else {
            let entry = &mut self.sparse[self.next as usize];
            entry.version += 1;
            let entity = EntityId {
                sparse: self.next,
                version: entry.version,
            };
            self.next = entry.dense;
            self.available -= 1;
            entry.dense = self.dense.len() as u16;
            self.dense.push(entity);
            entity
        }
    }

    /// Deletes the given entity. Returns `true` if the world previously had the entity.
    /// Returns `false` on subsequent deletions or if the world never had the entity.
    ///
    /// This operation does not automatically delete all its componenets.
    /// However, this enable future entities to reuse the same storage offset, deacreasing the overall
    /// memomy usage of the different component storages.
    pub fn delete_entity(&mut self, entity: EntityId) -> bool {
        let sparse_idx = entity.sparse as usize;
        match self.sparse.get_mut(sparse_idx) {
            Some(sparse_entry) if sparse_entry.version == entity.version => {
                let dense_idx = sparse_entry.dense as usize;
                match self.dense.get(dense_idx) {
                    Some(dense_entry) if dense_entry.sparse == entity.sparse => {
                        self.sparse[self.dense.last().unwrap().sparse as usize].dense =
                            sparse_entry.dense;
                        self.dense.swap_remove(dense_idx);
                        self.sparse[sparse_idx].dense = self.next;
                        self.next = entity.sparse;
                        self.available += 1;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// Returns `true` if the world contains the entity.
    /// Returns `false` it the entity has been deleted or was never created.
    pub fn contains(&self, entity: EntityId) -> bool {
        match self.sparse.get(entity.sparse as usize) {
            Some(sparse_entry) if sparse_entry.version == entity.version => {
                match self.dense.get(sparse_entry.dense as usize) {
                    Some(dense_entry) if dense_entry.sparse == entity.sparse => true,
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

/// Unique id for a [`World`] entity. This is used to associate
/// entities with their components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId {
    version: u16,
    sparse: u16,
}

pub trait ComponentStorage<T> {
    type Iter<'a>: Iterator<Item = (EntityId, &'a T)>
    where
        Self: 'a,
        T: 'a;

    type IterMut<'a>: Iterator<Item = (EntityId, &'a mut T)>
    where
        Self: 'a,
        T: 'a;

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

    /// An iterator visiting all components `T` in arbitrary order. The iterator element type is `(EntityId, &'a T)`.
    fn iter<'a>(&'a self) -> Self::Iter<'a>;

    /// An iterator visiting all components `T` in arbitrary order, with mutable references to the values.
    /// The iterator element type is `(EntityId, &'a T)`.
    fn iter_mut<'a>(&'a mut self) -> Self::IterMut<'a>;
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

impl<T> ComponentStorage<T> for HashMap<EntityId, T> {
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

    fn iter<'a>(&'a self) -> Self::Iter<'a> {
        HashMapIter { inner: self.iter() }
    }

    fn iter_mut<'a>(&'a mut self) -> Self::IterMut<'a> {
        HashMapIterMut {
            inner: self.iter_mut(),
        }
    }
}

#[derive(Debug, Clone)]
struct SparseEntry {
    version: u16,
    dense: u16,
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

impl<T> ComponentStorage<T> for SparseArray<T> {
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

    fn add(&mut self, entity: EntityId, component: T) -> Option<T> {
        let sparse_idx = entity.sparse as usize;
        match self.sparse.get_mut(sparse_idx) {
            Some(sparse_entry) => match self.dense.get_mut(sparse_entry.dense as usize) {
                Some(dense_entry) if dense_entry.entity.version == entity.version => {
                    // the entity already had the component
                    Some(std::mem::replace(&mut dense_entry.component, component))
                }
                Some(dense_entry) => {
                    // a deleted entity with the same `sparse` had the component
                    sparse_entry.version = entity.version;
                    dense_entry.entity.version = entity.version;
                    dense_entry.component = component;
                    None
                }
                None => {
                    // no dense entry for the `sparse` index
                    sparse_entry.dense = self.dense.len() as u16;
                    sparse_entry.version = entity.version;
                    self.dense.push(DenseEntry { entity, component });
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
                self.dense.push(DenseEntry { entity, component });
                None
            }
        }
    }

    fn remove(&mut self, entity: EntityId) -> Option<T> {
        let sparse_idx = entity.sparse as usize;
        match self.sparse.get(sparse_idx) {
            Some(sparse_entry) if sparse_entry.version == entity.version => {
                let dense_idx = sparse_entry.dense as usize;
                match self.dense.get(dense_idx) {
                    Some(dense_entry) if dense_entry.entity.sparse == entity.sparse => {
                        self.sparse[self.dense.last().unwrap().entity.sparse as usize].dense =
                            sparse_entry.dense;
                        self.sparse[sparse_idx].version = u16::MAX;
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

    fn contains(&self, entity: EntityId) -> bool {
        match self.sparse.get(entity.sparse as usize) {
            Some(entry) if entry.version == entity.version => true,
            _ => false,
        }
    }

    fn get(&self, entity: EntityId) -> Option<&T> {
        match self.sparse.get(entity.sparse as usize) {
            Some(sparse_entry) if sparse_entry.version == entity.version => {
                match self.dense.get(sparse_entry.dense as usize) {
                    Some(dense_entry) if dense_entry.entity.sparse == entity.sparse => {
                        Some(&dense_entry.component)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn get_mut(&mut self, entity: EntityId) -> Option<&mut T> {
        match self.sparse.get(entity.sparse as usize) {
            Some(sparse_entry) if sparse_entry.version == entity.version => {
                match self.dense.get_mut(sparse_entry.dense as usize) {
                    Some(dense_entry) if dense_entry.entity.sparse == entity.sparse => {
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

    #[derive(Debug)]
    struct TestData2(&'static str);

    #[test]
    fn test_world() {
        let mut world = World::new();
        assert_eq!(0, world.available);

        let entity_0_v0 = world.new_entity();
        assert_eq!(1, world.dense.len());
        assert!(world.dense.contains(&entity_0_v0));
        assert_eq!(0, entity_0_v0.sparse);
        assert_eq!(0, entity_0_v0.version);
        assert!(world.contains(entity_0_v0));

        let entity_1_v0 = world.new_entity();
        assert_eq!(2, world.dense.len());
        assert!(world.dense.contains(&entity_1_v0));
        assert_eq!(1, entity_1_v0.sparse);
        assert_eq!(0, entity_1_v0.version);
        assert!(world.contains(entity_0_v0));
        assert!(world.contains(entity_1_v0));

        assert_eq!(0, world.available);
        world.delete_entity(entity_1_v0);
        assert_eq!(1, world.dense.len());
        assert!(world.dense.contains(&entity_0_v0));
        assert!(!world.dense.contains(&entity_1_v0));
        assert_eq!(1, world.available);
        assert_eq!(1, world.next);
        assert!(world.contains(entity_0_v0));
        assert!(!world.contains(entity_1_v0));

        let entity_1_v1 = world.new_entity();
        assert_eq!(2, world.dense.len());
        assert!(world.dense.contains(&entity_0_v0));
        assert!(!world.dense.contains(&entity_1_v0));
        assert!(world.dense.contains(&entity_1_v1));
        assert_eq!(1, entity_1_v1.sparse);
        assert_eq!(1, entity_1_v1.version);
        assert_eq!(0, world.available);
        assert!(world.contains(entity_0_v0));
        assert!(!world.contains(entity_1_v0));
        assert!(world.contains(entity_1_v1));

        let entity_2_v0 = world.new_entity();
        assert_eq!(2, entity_2_v0.sparse);
        assert_eq!(0, entity_2_v0.version);
        assert_eq!(3, world.dense.len());
        assert!(world.contains(entity_0_v0));
        assert!(!world.contains(entity_1_v0));
        assert!(world.contains(entity_1_v1));
        assert!(world.contains(entity_2_v0));

        world.delete_entity(entity_0_v0);
        assert_eq!(2, world.dense.len());
        assert_eq!(1, world.available);
        assert_eq!(0, world.next);
        assert!(!world.contains(entity_0_v0));
        assert!(world.contains(entity_1_v1));
        assert!(world.contains(entity_2_v0));

        world.delete_entity(entity_2_v0);
        assert_eq!(1, world.dense.len());
        assert_eq!(2, world.available);
        assert_eq!(2, world.next);
        assert!(world.contains(entity_1_v1));
        assert!(!world.contains(entity_2_v0));

        let entity_2_v1 = world.new_entity();
        assert_eq!(2, entity_2_v1.sparse);
        assert_eq!(1, entity_2_v1.version);
        assert_eq!(1, world.available);
        assert_eq!(0, world.next);
        assert!(world.contains(entity_1_v1));
        assert!(!world.contains(entity_2_v0));
        assert!(world.contains(entity_2_v1));
    }

    // #[test]
    // fn test_map() {
    //     let mut set: SparseSet<TestData> = SparseSet::new();
    //     let mut map: SparseMap<TestData2> = SparseMap::new();

    //     let id_1_v1 = set.next_id();
    //     set.insert(TestData(id_1_v1, String::from("data 1 v1")));
    //     let id_2_v1 = set.next_id();
    //     set.insert(TestData(id_2_v1, String::from("data 2 v1")));
    //     let id_3_v1 = set.next_id();
    //     set.insert(TestData(id_3_v1, String::from("data 3 v1")));

    //     map.insert(TestData2(id_2_v1, String::from("data2 2 v1")));
    //     assert!(map.get(id_1_v1).is_none());
    //     assert_eq!(map[id_2_v1].1, "data2 2 v1");
    //     assert!(map.get(id_3_v1).is_none());

    //     map.insert(TestData2(id_1_v1, String::from("data2 1 v1")));
    //     assert_eq!(map[id_1_v1].1, "data2 1 v1");
    //     assert_eq!(map[id_2_v1].1, "data2 2 v1");
    //     assert!(map.get(id_3_v1).is_none());

    //     map.remove(id_2_v1);
    //     assert_eq!(map[id_1_v1].1, "data2 1 v1");
    //     assert!(map.get(id_2_v1).is_none());
    //     assert!(map.get(id_3_v1).is_none());

    //     set.remove(id_2_v1);
    //     set.remove(id_3_v1);

    //     let id_2_v2 = set.next_id();
    //     set.insert(TestData(id_2_v2, String::from("data 2 v2")));
    //     let id_3_v2 = set.next_id();
    //     set.insert(TestData(id_3_v2, String::from("data 3 v2")));

    //     assert!(map.get(id_2_v2).is_none());
    //     assert!(map.get(id_3_v2).is_none());

    //     map.insert(TestData2(id_3_v2, String::from("data2 3 v2")));
    //     assert_eq!(map[id_1_v1].1, "data2 1 v1");
    //     assert!(map.get(id_2_v2).is_none());
    //     assert_eq!(map[id_3_v2].1, "data2 3 v2");

    //     set.remove(id_1_v1);
    //     let id_1_v2 = set.next_id();
    //     set.insert(TestData(id_1_v2, String::from("data 1 v2")));
    //     assert!(map.get(id_1_v2).is_none());

    //     map.insert(TestData2(id_1_v2, String::from("data2 1 v2")));
    //     assert!(map.get(id_1_v1).is_none());
    //     assert_eq!(map[id_1_v2].1, "data2 1 v2");
    //     assert!(map.get(id_2_v2).is_none());
    //     assert_eq!(map[id_3_v2].1, "data2 3 v2");

    //     map.insert(TestData2(id_2_v2, String::from("data2 2 v2")));
    //     assert_eq!(map[id_1_v2].1, "data2 1 v2");
    //     assert_eq!(map[id_2_v2].1, "data2 2 v2");
    //     assert_eq!(map[id_3_v2].1, "data2 3 v2");
    // }
}
