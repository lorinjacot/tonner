use std::{
    fmt::{Debug, Display},
    hash::Hash,
    iter::repeat_n,
    marker::PhantomData,
    ops::{Index, IndexMut},
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

impl<T> Display for Id<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.sparse, self.version)
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

impl<T> Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.sparse.hash(state);
        self.version.hash(state);
    }
}

#[derive(Clone)]
struct SparseEntry {
    dense: u16,
    version: u16,
}

pub trait DenseEntry {
    type Key;

    fn id(&self) -> Id<Self::Key>;
}

pub struct SparseMap<T: DenseEntry> {
    sparse: Vec<SparseEntry>,
    dense: Vec<T>,
}

impl<T: DenseEntry> SparseMap<T> {
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

    pub fn insert(&mut self, entry: T) -> &mut T {
        match self.sparse.get_mut(entry.id().sparse as usize) {
            Some(sparse) if sparse.dense < u16::MAX => {
                sparse.version = entry.id().version;
                let dense_entry = &mut self.dense[sparse.dense as usize];
                *dense_entry = entry;
                dense_entry
            }
            Some(sparse) => {
                sparse.dense = self.dense.len() as u16;
                sparse.version = entry.id().version;
                self.dense.push(entry);
                self.dense.last_mut().unwrap()
            }
            None => {
                let iter = repeat_n(
                    SparseEntry {
                        dense: u16::MAX,
                        version: 0,
                    },
                    entry.id().sparse as usize - self.sparse.len(),
                )
                .chain(Some(SparseEntry {
                    dense: self.dense.len() as u16,
                    version: entry.id().version,
                }));
                self.sparse.extend(iter);
                self.dense.push(entry);
                self.dense.last_mut().unwrap()
            }
        }
    }

    pub fn remove(&mut self, id: Id<T::Key>) -> Option<T> {
        match self.sparse.get_mut(id.sparse as usize) {
            Some(sparse) if id.version == sparse.version && sparse.dense < u16::MAX => {
                let dense = std::mem::replace(&mut sparse.dense, u16::MAX);
                let value = self.dense.swap_remove(dense as usize);
                if let Some(entry) = self.dense.get(dense as usize) {
                    self.sparse[entry.id().sparse as usize].dense = dense;
                }
                Some(value)
            }
            _ => None,
        }
    }

    pub fn dense_index(&self, id: Id<T::Key>) -> Option<u16> {
        match self.sparse.get(id.sparse as usize) {
            Some(sparse) if id.version == sparse.version && sparse.dense < u16::MAX => {
                Some(sparse.dense)
            }
            _ => None,
        }
    }

    pub fn contains(&self, id: Id<T::Key>) -> bool {
        self.dense_index(id).is_some()
    }

    pub fn get(&self, id: Id<T::Key>) -> Option<&T> {
        self.dense_index(id)
            .map(|index| &self.dense[index as usize])
    }

    pub fn get_mut(&mut self, id: Id<T::Key>) -> Option<&mut T> {
        self.dense_index(id)
            .map(|index| &mut self.dense[index as usize])
    }

    pub fn entry(&mut self, id: Id<T::Key>) -> Entry<'_, T> {
        match self.dense_index(id) {
            Some(index) => Entry::Occupied(OccupiedEntry {
                value: &mut self.dense[index as usize],
            }),
            None => Entry::Vacant(VacantEntry { map: self }),
        }
    }

    pub fn len(&self) -> usize {
        self.dense.len()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.dense.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.dense.iter_mut()
    }
}

impl<T: DenseEntry> Index<Id<T::Key>> for SparseMap<T> {
    type Output = T;

    fn index(&self, index: Id<T::Key>) -> &Self::Output {
        self.get(index).expect("no entry found for id")
    }
}

impl<T: DenseEntry> IndexMut<Id<T::Key>> for SparseMap<T> {
    fn index_mut(&mut self, index: Id<T::Key>) -> &mut Self::Output {
        self.get_mut(index).expect("no entry found for id")
    }
}

impl<T: DenseEntry> IntoIterator for SparseMap<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.dense.into_iter()
    }
}

impl<'a, T: DenseEntry> IntoIterator for &'a SparseMap<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: DenseEntry> IntoIterator for &'a mut SparseMap<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: DenseEntry> FromIterator<T> for SparseMap<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let mut map = SparseMap::with_capacity(iter.size_hint().0);
        for entry in iter {
            map.insert(entry);
        }
        map
    }
}

pub enum Entry<'a, T: DenseEntry> {
    Occupied(OccupiedEntry<'a, T>),
    Vacant(VacantEntry<'a, T>),
}

impl<'a, T: DenseEntry> Entry<'a, T> {
    pub fn or_insert_with<F: FnOnce() -> T>(self, default: F) -> &'a mut T {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default()),
        }
    }
}

impl<'a, T: DenseEntry> Entry<'a, T>
where
    T: Default,
{
    pub fn or_default(self) -> &'a mut T {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(T::default()),
        }
    }
}

pub struct OccupiedEntry<'a, T: DenseEntry> {
    value: &'a mut T,
}

impl<'a, T: DenseEntry> OccupiedEntry<'a, T> {
    pub fn into_mut(self) -> &'a mut T {
        self.value
    }
}

pub struct VacantEntry<'a, T: DenseEntry> {
    map: &'a mut SparseMap<T>,
}

impl<'a, T: DenseEntry> VacantEntry<'a, T> {
    pub fn insert(self, value: T) -> &'a mut T {
        self.map.insert(value)
    }
}

pub struct SparseSet<T: DenseEntry> {
    map: SparseMap<T>,
    deleted: Vec<Id<T::Key>>,
}

impl<T: DenseEntry> SparseSet<T> {
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

    pub fn next_id(&self) -> Id<T::Key> {
        match self.deleted.last() {
            Some(deleted) => Id {
                sparse: deleted.sparse,
                version: deleted.version + 1,
                target: PhantomData,
            },
            None => Id {
                sparse: self.map.len() as u16,
                version: 0,
                target: PhantomData,
            },
        }
    }

    pub fn insert(&mut self, entry: T) -> &mut T {
        self.map.insert(entry)
    }

    pub fn delete(&mut self, id: Id<T::Key>) -> Option<T> {
        match self.map.remove(id) {
            Some(value) => {
                self.deleted.push(id);
                Some(value)
            }
            None => None,
        }
    }

    pub fn contains(&self, id: Id<T::Key>) -> bool {
        self.map.contains(id)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn get(&self, id: Id<T::Key>) -> Option<&T> {
        self.map.get(id)
    }

    pub fn get_mut(&mut self, id: Id<T::Key>) -> Option<&mut T> {
        self.map.get_mut(id)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.map.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.map.iter_mut()
    }

    pub fn dense_index(&self, id: Id<T::Key>) -> Option<u16> {
        self.map.dense_index(id)
    }
}

impl<T: DenseEntry> Index<Id<T::Key>> for SparseSet<T> {
    type Output = T;

    fn index(&self, index: Id<T::Key>) -> &Self::Output {
        self.get(index).expect("no entry found for id")
    }
}

impl<T: DenseEntry> IndexMut<Id<T::Key>> for SparseSet<T> {
    fn index_mut(&mut self, index: Id<T::Key>) -> &mut Self::Output {
        self.get_mut(index).expect("no entry found for id")
    }
}

impl<T: DenseEntry> IntoIterator for SparseSet<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.map.into_iter()
    }
}

impl<'a, T: DenseEntry> IntoIterator for &'a SparseSet<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: DenseEntry> IntoIterator for &'a mut SparseSet<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestData(Id<TestData>, String);

    impl DenseEntry for TestData {
        type Key = Self;

        fn id(&self) -> Id<Self::Key> {
            self.0
        }
    }

    #[derive(Debug)]
    struct TestData2(Id<TestData>, String);

    impl DenseEntry for TestData2 {
        type Key = TestData;

        fn id(&self) -> Id<Self::Key> {
            self.0
        }
    }

    #[test]
    fn test_next_id() {
        let mut set: SparseSet<TestData> = SparseSet::new();

        let id_1 = set.next_id();
        set.insert(TestData(id_1, "data 1".to_string()));

        let id_2 = set.next_id();
        set.insert(TestData(id_2, "data 2".to_string()));

        assert_ne!(id_1, id_2);
        assert_eq!("data 1", set[id_1].1);
        assert_eq!("data 2", set[id_2].1);
    }

    #[test]
    fn test_contains() {
        let mut set: SparseSet<TestData> = SparseSet::new();

        let mut id_1 = set.next_id();
        set.insert(TestData(id_1, "data 1".to_string()));

        let mut id_2 = set.next_id();
        set.insert(TestData(id_2, "data 2".to_string()));

        assert!(set.contains(id_1));
        assert!(set.contains(id_2));

        id_1.version += 1;
        assert!(!set.contains(id_1));

        id_2.sparse += 1;
        assert!(!set.contains(id_2));
    }

    #[test]
    fn test_remove() {
        let mut set: SparseSet<TestData> = SparseSet::new();

        let id_1 = set.next_id();
        set.insert(TestData(id_1, "data 1".to_string()));

        let id_2 = set.next_id();
        set.insert(TestData(id_2, "data 2".to_string()));

        set.delete(id_1);
        assert!(set.get(id_1).is_none());
        assert_eq!("data 2", set[id_2].1);

        assert!(set.deleted.contains(&id_1));

        set.delete(id_2);
        assert!(set.get(id_2).is_none());

        assert!(set.deleted.contains(&id_2));
    }

    #[test]
    fn test_recycling() {
        let mut set: SparseSet<TestData> = SparseSet::new();

        let id_1_v1 = set.next_id();
        set.insert(TestData(id_1_v1, "data 1".to_string()));

        let id_2_v1 = set.next_id();
        set.insert(TestData(id_2_v1, "data 2".to_string()));

        set.delete(id_2_v1);
        set.delete(id_1_v1);
        assert!(set.deleted.contains(&id_1_v1));
        assert!(set.deleted.contains(&id_2_v1));

        let id_1_v2 = set.next_id();
        set.insert(TestData(id_1_v2, "data 1 v2".to_string()));
        assert_ne!(id_1_v1, id_1_v2);

        assert!(!set.deleted.contains(&id_1_v1));
        assert!(!set.deleted.contains(&id_1_v2));
        assert!(set.deleted.contains(&id_2_v1));

        let id_2_v2 = set.next_id();
        set.insert(TestData(id_2_v2, "data 2 v2".to_string()));
        assert_ne!(id_2_v1, id_2_v2);

        let id_3_v1 = set.next_id();
        set.insert(TestData(id_3_v1, "data 3 v1".to_string()));
        assert!(set[id_3_v1].1 == "data 3 v1");

        set.delete(id_2_v2);
        let id_2_v3 = set.next_id();
        set.insert(TestData(id_2_v3, "data 2 v3".to_string()));
        assert!(set[id_2_v3].1 == "data 2 v3");
        assert!(set[id_3_v1].1 == "data 3 v1");
        assert!(set.get(id_2_v2).is_none());
        assert_ne!(id_2_v1, id_2_v3);
        assert_ne!(id_2_v2, id_2_v3);
    }

    #[test]
    fn test_map() {
        let mut set: SparseSet<TestData> = SparseSet::new();
        let mut map: SparseMap<TestData2> = SparseMap::new();

        let id_1_v1 = set.next_id();
        set.insert(TestData(id_1_v1, String::from("data 1 v1")));
        let id_2_v1 = set.next_id();
        set.insert(TestData(id_2_v1, String::from("data 2 v1")));
        let id_3_v1 = set.next_id();
        set.insert(TestData(id_3_v1, String::from("data 3 v1")));

        map.insert(TestData2(id_2_v1, String::from("data2 2 v1")));
        assert!(map.get(id_1_v1).is_none());
        assert_eq!(map[id_2_v1].1, "data2 2 v1");
        assert!(map.get(id_3_v1).is_none());

        map.insert(TestData2(id_1_v1, String::from("data2 1 v1")));
        assert_eq!(map[id_1_v1].1, "data2 1 v1");
        assert_eq!(map[id_2_v1].1, "data2 2 v1");
        assert!(map.get(id_3_v1).is_none());

        map.remove(id_2_v1);
        assert_eq!(map[id_1_v1].1, "data2 1 v1");
        assert!(map.get(id_2_v1).is_none());
        assert!(map.get(id_3_v1).is_none());

        set.delete(id_2_v1);
        set.delete(id_3_v1);

        let id_2_v2 = set.next_id();
        set.insert(TestData(id_2_v2, String::from("data 2 v2")));
        let id_3_v2 = set.next_id();
        set.insert(TestData(id_3_v2, String::from("data 3 v2")));

        assert!(map.get(id_2_v2).is_none());
        assert!(map.get(id_3_v2).is_none());

        map.insert(TestData2(id_3_v2, String::from("data2 3 v2")));
        assert_eq!(map[id_1_v1].1, "data2 1 v1");
        assert!(map.get(id_2_v2).is_none());
        assert_eq!(map[id_3_v2].1, "data2 3 v2");

        set.delete(id_1_v1);
        let id_1_v2 = set.next_id();
        set.insert(TestData(id_1_v2, String::from("data 1 v2")));
        assert!(map.get(id_1_v2).is_none());

        map.insert(TestData2(id_1_v2, String::from("data2 1 v2")));
        assert!(map.get(id_1_v1).is_none());
        assert_eq!(map[id_1_v2].1, "data2 1 v2");
        assert!(map.get(id_2_v2).is_none());
        assert_eq!(map[id_3_v2].1, "data2 3 v2");

        map.insert(TestData2(id_2_v2, String::from("data2 2 v2")));
        assert_eq!(map[id_1_v2].1, "data2 1 v2");
        assert_eq!(map[id_2_v2].1, "data2 2 v2");
        assert_eq!(map[id_3_v2].1, "data2 3 v2");
    }
}
