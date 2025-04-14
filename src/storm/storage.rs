use std::{
    fmt::Debug,
    iter::{repeat_n, FusedIterator},
    marker::PhantomData,
    mem::replace,
    ops::{Index, IndexMut},
    u16,
};

pub struct Id<T> {
    sparse: u16,
    version: u16,
    target: PhantomData<T>,
}

impl<T> Debug for Id<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("Id<{}>", std::any::type_name::<T>()))
            .field("sparse", &self.sparse)
            .field("version", &self.version)
            .finish()
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        Self {
            sparse: self.sparse,
            version: self.version,
            target: PhantomData,
        }
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.sparse == other.sparse && self.version == other.version
    }
}

impl<T> Eq for Id<T> {}

#[derive(Clone)]
struct SparseEntry {
    dense: u16,
    version: u16,
}

pub struct SparseMap<K, V> {
    sparse: Vec<SparseEntry>,
    dense: Vec<(Id<K>, V)>,
}

impl<K, V> SparseMap<K, V> {
    pub fn new() -> Self {
        Self {
            sparse: Vec::new(),
            dense: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            sparse: Vec::with_capacity(capacity),
            dense: Vec::with_capacity(capacity),
        }
    }

    pub fn insert(&mut self, id: Id<K>, value: V) -> Option<V> {
        match self.sparse.get_mut(id.sparse as usize) {
            Some(sparse) if sparse.dense < u16::MAX => {
                sparse.version = id.version;
                let old = replace(&mut self.dense[sparse.dense as usize].1, value);
                Some(old)
            }
            Some(sparse) => {
                sparse.dense = self.dense.len() as u16;
                sparse.version = id.version;
                self.dense.push((id, value));
                None
            }
            None => {
                let entries = repeat_n(
                    SparseEntry {
                        dense: u16::MAX,
                        version: 0,
                    },
                    id.sparse as usize - self.sparse.len(),
                )
                .chain(Some(SparseEntry {
                    dense: self.dense.len() as u16,
                    version: id.version,
                }));
                self.sparse.extend(entries);
                self.dense.push((id, value));
                None
            }
        }
    }

    pub fn remove(&mut self, id: Id<K>) -> Option<V> {
        match self.sparse.get_mut(id.sparse as usize) {
            Some(sparse) if id.version == sparse.version && sparse.dense < u16::MAX => {
                let dense = replace(&mut sparse.dense, u16::MAX);
                let value = self.dense.swap_remove(dense as usize).1;
                if let Some(entry) = self.dense.get(dense as usize) {
                    self.sparse[entry.0.sparse as usize].dense = dense;
                }
                Some(value)
            }
            _ => None,
        }
    }

    pub fn contains(&self, id: Id<K>) -> bool {
        match self.sparse.get(id.sparse as usize) {
            Some(sparse) => id.version == sparse.version && sparse.dense < u16::MAX,
            None => false,
        }
    }

    pub fn get(&self, id: Id<K>) -> Option<&V> {
        match self.sparse.get(id.sparse as usize) {
            Some(sparse) if id.version == sparse.version && sparse.dense < u16::MAX => {
                Some(&self.dense[sparse.dense as usize].1)
            }
            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: Id<K>) -> Option<&mut V> {
        match self.sparse.get(id.sparse as usize) {
            Some(sparse) if id.version == sparse.version && sparse.dense < u16::MAX => {
                Some(&mut self.dense[sparse.dense as usize].1)
            }
            _ => None,
        }
    }

    pub fn entry(&mut self, id: Id<K>) -> Entry<'_, K, V> {
        if self.contains(id) {
            Entry::Occupied(OccupiedEntry {
                id,
                value: &mut self[id],
            })
        } else {
            Entry::Vacant(VacantEntry { id, map: self })
        }
    }

    pub fn len(&self) -> usize {
        self.dense.len()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, (Id<K>, V)> {
        self.dense.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, (Id<K>, V)> {
        self.dense.iter_mut()
    }

    pub fn values(
        &self,
    ) -> impl Iterator<Item = &V> + ExactSizeIterator + FusedIterator + DoubleEndedIterator {
        self.dense.iter().map(|entry| &entry.1)
    }
}

impl<K, V> Index<Id<K>> for SparseMap<K, V> {
    type Output = V;

    fn index(&self, index: Id<K>) -> &Self::Output {
        self.get(index).expect("no entry found for id")
    }
}

impl<K, V> IndexMut<Id<K>> for SparseMap<K, V> {
    fn index_mut(&mut self, index: Id<K>) -> &mut Self::Output {
        self.get_mut(index).expect("no entry found for id")
    }
}

impl<K, V> IntoIterator for SparseMap<K, V> {
    type Item = (Id<K>, V);
    type IntoIter = std::vec::IntoIter<(Id<K>, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.dense.into_iter()
    }
}

impl<'a, K, V> IntoIterator for &'a SparseMap<K, V> {
    type Item = &'a (Id<K>, V);
    type IntoIter = std::slice::Iter<'a, (Id<K>, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V> IntoIterator for &'a mut SparseMap<K, V> {
    type Item = &'a mut (Id<K>, V);
    type IntoIter = std::slice::IterMut<'a, (Id<K>, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

pub enum Entry<'a, K, V: 'a> {
    Occupied(OccupiedEntry<'a, K, V>),
    Vacant(VacantEntry<'a, K, V>),
}

impl<'a, K, V> Entry<'a, K, V> {
    pub fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default()),
        }
    }
}

impl<'a, K, V: Default> Entry<'a, K, V> {
    pub fn or_default(self) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(V::default()),
        }
    }
}

pub struct OccupiedEntry<'a, K, V: 'a> {
    id: Id<K>,
    value: &'a mut V,
}

impl<'a, K, V: 'a> OccupiedEntry<'a, K, V> {
    pub fn into_mut(self) -> &'a mut V {
        self.value
    }
}

pub struct VacantEntry<'a, K, V: 'a> {
    id: Id<K>,
    map: &'a mut SparseMap<K, V>,
}

impl<'a, K, V: 'a> VacantEntry<'a, K, V> {
    pub fn insert(self, value: V) -> &'a mut V {
        self.map.insert(self.id, value);
        &mut self.map[self.id]
    }
}

pub struct SparseSet<T> {
    map: SparseMap<T, T>,
    deleted: Vec<Id<T>>,
}

impl<T> SparseSet<T> {
    pub fn new() -> Self {
        Self {
            map: SparseMap::new(),
            deleted: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            map: SparseMap::with_capacity(capacity),
            deleted: Vec::new(),
        }
    }

    pub fn push(&mut self, value: T) -> Id<T> {
        let dense = self.map.dense.len() as u16;
        if let Some(deleted) = self.deleted.pop() {
            let id = Id {
                sparse: deleted.sparse,
                version: deleted.version + 1,
                target: PhantomData,
            };
            self.map.sparse[deleted.sparse as usize] = SparseEntry {
                dense,
                version: id.version,
            };
            self.map.dense.push((id, value));
            id
        } else {
            assert!(dense < u16::MAX, "sparse set is full");
            let id = Id {
                sparse: self.map.sparse.len() as u16,
                version: 0,
                target: PhantomData,
            };
            self.map.sparse.push(SparseEntry {
                dense,
                version: id.version,
            });
            self.map.dense.push((id, value));
            id
        }
    }

    pub fn remove(&mut self, id: Id<T>) -> Option<T> {
        match self.map.remove(id) {
            Some(value) => {
                self.deleted.push(id);
                Some(value)
            }
            None => None,
        }
    }

    pub fn contains(&self, id: Id<T>) -> bool {
        self.map.contains(id)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn get(&self, id: Id<T>) -> Option<&T> {
        self.map.get(id)
    }

    pub fn get_mut(&mut self, id: Id<T>) -> Option<&mut T> {
        self.map.get_mut(id)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, (Id<T>, T)> {
        self.map.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, (Id<T>, T)> {
        self.map.iter_mut()
    }

    pub fn values(
        &self,
    ) -> impl Iterator<Item = &T> + ExactSizeIterator + FusedIterator + DoubleEndedIterator {
        self.map.values()
    }
}

impl<V> IntoIterator for SparseSet<V> {
    type Item = (Id<V>, V);
    type IntoIter = std::vec::IntoIter<(Id<V>, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.map.into_iter()
    }
}

impl<'a, V> IntoIterator for &'a SparseSet<V> {
    type Item = &'a (Id<V>, V);
    type IntoIter = std::slice::Iter<'a, (Id<V>, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, V> IntoIterator for &'a mut SparseSet<V> {
    type Item = &'a mut (Id<V>, V);
    type IntoIter = std::slice::IterMut<'a, (Id<V>, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T> Index<Id<T>> for SparseSet<T> {
    type Output = T;

    fn index(&self, index: Id<T>) -> &Self::Output {
        self.get(index).expect("no entry found for id")
    }
}

impl<T> IndexMut<Id<T>> for SparseSet<T> {
    fn index_mut(&mut self, index: Id<T>) -> &mut Self::Output {
        self.get_mut(index).expect("no entry found for id")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct TestData(String);

    #[derive(Debug, PartialEq)]
    struct TestData2(String);

    #[test]
    fn test_push() {
        let mut set: SparseSet<TestData> = SparseSet::new();

        let id_1 = set.push(TestData(String::from("data 1")));
        let id_2 = set.push(TestData(String::from("data 2")));

        assert_ne!(id_1, id_2);
        assert_eq!("data 1", set[id_1].0);
        assert_eq!("data 2", set[id_2].0);
    }

    #[test]
    fn test_contains() {
        let mut set = SparseSet::new();

        let mut id_1 = set.push(TestData(String::from("data 1")));
        let mut id_2 = set.push(TestData(String::from("data 2")));

        assert!(set.contains(id_1));
        assert!(set.contains(id_2));

        id_1.version += 1;
        assert!(!set.contains(id_1));

        id_2.sparse += 1;
        assert!(!set.contains(id_2));
    }

    #[test]
    fn test_remove() {
        let mut set = SparseSet::new();

        let id_1 = set.push(TestData(String::from("data 1")));
        let id_2 = set.push(TestData(String::from("data 2")));

        set.remove(id_1);
        assert!(set.get(id_1).is_none());
        assert_eq!("data 2", set[id_2].0);

        assert!(set.deleted.contains(&id_1));

        set.remove(id_2);
        assert!(set.get(id_2).is_none());

        assert!(set.deleted.contains(&id_2));
    }

    #[test]
    fn test_recycling() {
        let mut set = SparseSet::new();

        let id_1_v1 = set.push(TestData(String::from("data 1 v1")));
        let id_2_v1 = set.push(TestData(String::from("data 2 v1")));

        set.remove(id_2_v1);
        set.remove(id_1_v1);
        assert!(set.deleted.contains(&id_1_v1));
        assert!(set.deleted.contains(&id_2_v1));

        let id_1_v2 = set.push(TestData(String::from("data 1 v2")));
        assert_ne!(id_1_v1, id_1_v2);

        assert!(!set.deleted.contains(&id_1_v1));
        assert!(!set.deleted.contains(&id_1_v2));
        assert!(set.deleted.contains(&id_2_v1));

        let id_2_v2 = set.push(TestData(String::from("data 2 v2")));
        assert_ne!(id_2_v1, id_2_v2);

        let id_3_v1 = set.push(TestData(String::from("data 3 v1")));
        assert!(set[id_3_v1].0 == "data 3 v1");

        set.remove(id_2_v2);
        let id_2_v3 = set.push(TestData(String::from("data 2 v3")));
        assert!(set[id_2_v3].0 == "data 2 v3");
        assert!(set[id_3_v1].0 == "data 3 v1");
        assert!(set.get(id_2_v2).is_none());
        assert_ne!(id_2_v1, id_2_v3);
        assert_ne!(id_2_v2, id_2_v3);
    }

    #[test]
    fn test_map() {
        let mut set = SparseSet::new();
        let mut map = SparseMap::new();

        let id_1_v1 = set.push(TestData(String::from("data 1 v1")));
        let id_2_v1 = set.push(TestData(String::from("data 2 v1")));
        let id_3_v1 = set.push(TestData(String::from("data 3 v1")));

        map.insert(id_2_v1, TestData2(String::from("data2 2 v1")));
        assert!(map.get(id_1_v1).is_none());
        assert_eq!(map[id_2_v1].0, "data2 2 v1");
        assert!(map.get(id_3_v1).is_none());

        map.insert(id_1_v1, TestData2(String::from("data2 1 v1")));
        assert_eq!(map[id_1_v1].0, "data2 1 v1");
        assert_eq!(map[id_2_v1].0, "data2 2 v1");
        assert!(map.get(id_3_v1).is_none());

        map.remove(id_2_v1);
        assert_eq!(map[id_1_v1].0, "data2 1 v1");
        assert!(map.get(id_2_v1).is_none());
        assert!(map.get(id_3_v1).is_none());

        set.remove(id_2_v1);
        set.remove(id_3_v1);

        let id_2_v2 = set.push(TestData(String::from("data 2 v2")));
        let id_3_v2 = set.push(TestData(String::from("data 3 v2")));

        assert!(map.get(id_2_v2).is_none());
        assert!(map.get(id_3_v2).is_none());

        map.insert(id_3_v2, TestData2(String::from("data2 3 v2")));
        assert_eq!(map[id_1_v1].0, "data2 1 v1");
        assert!(map.get(id_2_v2).is_none());
        assert_eq!(map[id_3_v2].0, "data2 3 v2");

        set.remove(id_1_v1);
        let id_1_v2 = set.push(TestData(String::from("data 1 v2")));
        assert!(map.get(id_1_v2).is_none());

        map.insert(id_1_v2, TestData2(String::from("data2 1 v2")));
        assert!(map.get(id_1_v1).is_none());
        assert_eq!(map[id_1_v2].0, "data2 1 v2");
        assert!(map.get(id_2_v2).is_none());
        assert_eq!(map[id_3_v2].0, "data2 3 v2");

        map.insert(id_2_v2, TestData2(String::from("data2 2 v2")));
        assert_eq!(map[id_1_v2].0, "data2 1 v2");
        assert_eq!(map[id_2_v2].0, "data2 2 v2");
        assert_eq!(map[id_3_v2].0, "data2 3 v2");
    }
}
