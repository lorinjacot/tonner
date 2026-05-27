use std::{
    iter::{once, repeat_n},
    num::NonZeroU32,
    ops::{Deref, Index, IndexMut},
};

use crate::{Key, KeyRegistry};

#[derive(Debug, Clone)]
struct SparseEntry {
    dense_index: u32,
    version: Option<NonZeroU32>,
}

#[derive(Debug, Clone)]
pub struct SecondaryMap<T> {
    sparse: Vec<SparseEntry>,
    dense: Vec<(Key, T)>,
}

impl<T> SecondaryMap<T> {
    #[must_use]
    pub fn new() -> Self {
        SecondaryMap {
            sparse: Vec::new(),
            dense: Vec::new(),
        }
    }

    pub fn insert(&mut self, key: Key, value: T) -> Option<T> {
        let sparse_index = key.index() as usize;
        match self.sparse.get_mut(sparse_index) {
            Some(sparse_entry) => match self.dense.get_mut(sparse_entry.dense_index as usize) {
                Some(dense_entry) if dense_entry.0 == key => {
                    // there is already a value for the key
                    Some(std::mem::replace(&mut dense_entry.1, value))
                }
                Some(dense_entry) => {
                    // a deleted entity with the same `sparse` had the component
                    sparse_entry.version = Some(key.version());
                    *dense_entry = (key, value);
                    None
                }
                None => {
                    sparse_entry.dense_index = self.dense.len() as u32;
                    sparse_entry.version = Some(key.version());
                    self.dense.push((key, value));
                    None
                }
            },
            None => {
                self.sparse.extend(
                    repeat_n(
                        SparseEntry {
                            dense_index: u32::MAX,
                            version: None,
                        },
                        sparse_index - self.sparse.len(),
                    )
                    .chain(once(SparseEntry {
                        dense_index: self.dense.len() as u32,
                        version: Some(key.version()),
                    })),
                );
                self.dense.push((key, value));
                None
            }
        }
    }

    pub fn remove(&mut self, key: Key) -> Option<T> {
        let sparse_index = key.index() as usize;
        match self.sparse.get(sparse_index) {
            Some(sparse_entry) if sparse_entry.version == Some(key.version()) => {
                let dense_index = sparse_entry.dense_index as usize;
                self.sparse[self.dense.last().unwrap().0.index() as usize].dense_index =
                    sparse_entry.dense_index;
                self.sparse[sparse_index] = SparseEntry {
                    dense_index: u32::MAX,
                    version: None,
                };
                Some(self.dense.swap_remove(dense_index).1)
            }
            _ => None,
        }
    }

    pub fn remove_deleted(&mut self, registry: &KeyRegistry) {
        let mut i = 0;
        while i < self.dense.len() {
            let key = self.dense[i].0;
            if registry.contains(key) {
                i += 1;
            } else {
                self.remove(key);
            }
        }
    }

    pub fn contains(&self, key: Key) -> bool {
        match self.sparse.get(key.index() as usize) {
            Some(entry) if entry.version == Some(key.version()) => true,
            _ => false,
        }
    }

    pub fn get(&self, key: Key) -> Option<&T> {
        match self.sparse.get(key.index() as usize) {
            Some(sparse_entry) if sparse_entry.version == Some(key.version()) => {
                Some(&self.dense[sparse_entry.dense_index as usize].1)
            }
            _ => None,
        }
    }

    pub fn get_mut(&mut self, key: Key) -> Option<&mut T> {
        match self.sparse.get(key.index() as usize) {
            Some(sparse_entry) if sparse_entry.version == Some(key.version()) => {
                Some(&mut self.dense[sparse_entry.dense_index as usize].1)
            }
            _ => None,
        }
    }
}

impl<T> Default for SecondaryMap<T> {
    fn default() -> Self {
        SecondaryMap::new()
    }
}

impl<T> Index<Key> for SecondaryMap<T> {
    type Output = T;

    fn index(&self, index: Key) -> &Self::Output {
        self.get(index).expect("no entry found for key")
    }
}

impl<T> IndexMut<Key> for SecondaryMap<T> {
    fn index_mut(&mut self, index: Key) -> &mut Self::Output {
        self.get_mut(index).expect("no entry found for key")
    }
}

impl<T> Deref for SecondaryMap<T> {
    type Target = [(Key, T)];

    fn deref(&self) -> &Self::Target {
        &self.dense
    }
}

#[cfg(test)]
mod tests {
    use crate::KeyRegistry;

    use super::*;

    #[test]
    fn test_secondary_map() {
        let mut registrey = KeyRegistry::new();
        let mut map = SecondaryMap::new();

        let key_0_v1 = registrey.create();
        let key_1_v1 = registrey.create();

        let data = map.insert(key_0_v1, "0 v1");
        assert!(data.is_none());
        assert_eq!(1, map.dense.len());
        assert!(map.contains(key_0_v1));
        assert!(!map.contains(key_1_v1));
        assert_eq!("0 v1", map[key_0_v1]);
        assert!(map.get(key_1_v1).is_none());

        let data = map.insert(key_1_v1, "1 v1");
        assert!(data.is_none());
        assert_eq!(2, map.dense.len());
        assert!(map.contains(key_0_v1));
        assert!(map.contains(key_1_v1));
        assert_eq!("0 v1", map[key_0_v1]);
        assert_eq!("1 v1", map[key_1_v1]);

        registrey.delete(key_0_v1);
        let key_0_v2 = registrey.create();

        let data = map.remove(key_0_v1).unwrap();
        assert_eq!("0 v1", data);
        assert_eq!(1, map.dense.len());
        assert!(!map.contains(key_0_v1));
        assert!(!map.contains(key_0_v2));
        assert!(map.contains(key_1_v1));
        assert!(map.get(key_0_v1).is_none());
        assert_eq!("1 v1", map[key_1_v1]);

        let data = map.insert(key_0_v2, "0 v2");
        assert!(data.is_none());
        assert_eq!(2, map.dense.len());
        assert!(!map.contains(key_0_v1));
        assert!(map.contains(key_0_v2));
        assert!(map.contains(key_1_v1));
        assert!(map.contains(key_1_v1));
        assert!(map.get(key_0_v1).is_none());
        assert_eq!("0 v2", map[key_0_v2]);
        assert_eq!("1 v1", map[key_1_v1]);

        registrey.delete(key_1_v1);
        let key_1_v2 = registrey.create();

        let data = map.insert(key_1_v1, "1 v1 prime").unwrap();
        assert_eq!("1 v1", data);
        assert_eq!(2, map.dense.len());
        assert!(!map.contains(key_0_v1));
        assert!(map.contains(key_0_v2));
        assert!(map.contains(key_1_v1));
        assert!(!map.contains(key_1_v2));
        assert!(map.get(key_0_v1).is_none());
        assert_eq!("0 v2", map[key_0_v2]);
        assert_eq!("1 v1 prime", map[key_1_v1]);
        assert!(map.get(key_1_v2).is_none());

        let data = map.insert(key_1_v2, "1 v2");
        assert!(data.is_none());
        assert_eq!(2, map.dense.len());
        assert!(!map.contains(key_0_v1));
        assert!(map.contains(key_0_v2));
        assert!(!map.contains(key_1_v1));
        assert!(map.contains(key_1_v2));
        assert!(map.get(key_0_v1).is_none());
        assert_eq!("0 v2", map[key_0_v2]);
        assert!(map.get(key_1_v1).is_none());
        assert_eq!("1 v2", map[key_1_v2]);

        let key_2_v1 = registrey.create();
        let key_3_v1 = registrey.create();

        let data = map.insert(key_3_v1, "3 v1");
        assert!(data.is_none());
        assert_eq!(3, map.dense.len());
        assert!(!map.contains(key_0_v1));
        assert!(map.contains(key_0_v2));
        assert!(!map.contains(key_1_v1));
        assert!(map.contains(key_1_v2));
        assert!(!map.contains(key_2_v1));
        assert!(map.contains(key_3_v1));
        assert!(map.get(key_0_v1).is_none());
        assert_eq!("0 v2", map[key_0_v2]);
        assert!(map.get(key_1_v1).is_none());
        assert_eq!("1 v2", map[key_1_v2]);
        assert!(map.get(key_2_v1).is_none());
        assert_eq!("3 v1", map[key_3_v1]);

        let data = map.insert(key_2_v1, "2 v1");
        assert!(data.is_none());
        assert_eq!(4, map.dense.len());
        assert!(!map.contains(key_0_v1));
        assert!(map.contains(key_0_v2));
        assert!(!map.contains(key_1_v1));
        assert!(map.contains(key_1_v2));
        assert!(map.contains(key_2_v1));
        assert!(map.contains(key_3_v1));
        assert!(map.get(key_0_v1).is_none());
        assert_eq!("0 v2", map[key_0_v2]);
        assert!(map.get(key_1_v1).is_none());
        assert_eq!("1 v2", map[key_1_v2]);
        assert_eq!("2 v1", map[key_2_v1]);
        assert_eq!("3 v1", map[key_3_v1]);
    }
}
