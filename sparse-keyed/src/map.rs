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

/// A map from [`Key`]s to arbitrary values. The keys are created and managed by a [`KeyRegistry`].
///
/// `SecondaryMap` is designed to provide efficient operations for (in order of importance):
/// 1. **Iteration**: Iterating over all entries of the map is O(n), where n is the number of entries in the map. This is achieved by storing the entries in a dense vector. The map provides a read-only slice of its entries via [`SecondaryMap::deref`]. However, no guarantee is made on their order, as it may change after any insertion or deletion of entries.
/// 2. **Random access**: Accessing the value associated with a key in the map is O(1). This is achieved by storing a sparse vector of indices pointing to the dense vector. The tradeoff is the memory usage of the sparse vector (up to O(m), where m is the number of keys in the registry) and an extra level of indirection when accessing entries. Random access is therefore slower than a vector but still O(1).
/// 3. **Insertion and deletion**: Insertion and deletion of entries in the map are O(1) in the average case, but insertion can be O(n), O(m) or O(m+n) if the sparse vector, the dense vector or both need to be resized. Deletion is always O(1). Deleting entries from the map allows their indices to be reused for new entries, keeping `m` low and ensuring fast operations of both the `KeyRegistry` and the `SecondaryMap`s over time. Deleted entries are not automatically deleted from the map. However, deleted entries can be removed by calling [`SecondaryMap::remove_deleted`] with the registry.
///
/// # Examples
/// ```
/// # use sparse_keyed::{KeyRegistry, SecondaryMap};
/// let mut registry = KeyRegistry::new();
/// let mut map = SecondaryMap::new();
/// let key_0 = registry.create();
/// let key_1 = registry.create();
///
/// map.insert(key_0, "value 0");
/// map.insert(key_1, "value 1");
/// assert_eq!(map[key_0], "value 0");
/// assert_eq!(map[key_1], "value 1");
///
/// map.remove(key_0);
/// assert!(!map.contains(key_0));
/// assert!(map.contains(key_1));
/// ```
#[derive(Debug, Clone)]
pub struct SecondaryMap<T> {
    sparse: Vec<SparseEntry>,
    dense: Vec<(Key, T)>,
}

impl<T> SecondaryMap<T> {
    /// Creates a new empty `SecondaryMap`.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::SecondaryMap;
    /// let map: SecondaryMap<i32> = SecondaryMap::new();
    /// assert!(map.is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        SecondaryMap {
            sparse: Vec::new(),
            dense: Vec::new(),
        }
    }

    /// Inserts a value associated with a key in the map. If the key already has a value, it is replaced and the old value is returned. If the key does not have a value, it is inserted and `None` is returned.If the `SecondaryMap` is still storing a value for a deleted key, any insertion with a key created after the deletion could potentially overwrite that value, even if the key is different. In that case, the old value is replaced and `None` is returned.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::KeyRegistry;
    /// # use sparse_keyed::SecondaryMap;
    /// let mut registry = KeyRegistry::new();
    /// let mut map = SecondaryMap::new();
    /// let key = registry.create();
    ///
    /// let old_value = map.insert(key, "value");
    /// assert!(old_value.is_none());
    /// assert_eq!(map[key], "value");
    ///
    /// let old_value = map.insert(key, "new value");
    /// assert_eq!(old_value, Some("value"));
    /// assert_eq!(map[key], "new value");
    /// ```
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

    /// Removes the value associated with a key from the map. Returns the removed value if the key had a value, or `None` if the key did not have a value.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::KeyRegistry;
    /// # use sparse_keyed::SecondaryMap;
    /// let mut registry = KeyRegistry::new();
    /// let mut map = SecondaryMap::new();
    /// let key = registry.create();
    /// map.insert(key, "value");
    /// let removed_value = map.remove(key);
    /// assert_eq!(removed_value, Some("value"));
    /// assert!(!map.contains(key));
    /// ```
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

    /// Removes all values associated with keys that are no longer present in the registry. This is useful to keep the map clean and avoid keeping values for deleted keys, which could potentially be overwritten by new keys created after the deletion.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::KeyRegistry;
    /// # use sparse_keyed::SecondaryMap;
    /// let mut registry = KeyRegistry::new();
    /// let mut map = SecondaryMap::new();
    /// let key = registry.create();
    /// map.insert(key, "value");
    ///
    /// registry.delete(key);
    /// assert!(map.contains(key));
    /// map.remove_deleted(&registry);
    /// assert!(!map.contains(key));
    /// ```
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

    /// Returns `true` if the map contains a value for the given key, or `false` if the key does not have a value in the map. Note that this does not check if the key is present in the registry, so it may return `true` for deleted keys. To check if a key is present in the registry, use [`KeyRegistry::contains`].
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::KeyRegistry;
    /// # use sparse_keyed::SecondaryMap;
    /// let mut registry = KeyRegistry::new();
    /// let mut map = SecondaryMap::new();
    /// let key = registry.create();
    /// assert!(!map.contains(key));
    /// 
    /// map.insert(key, "value");
    /// assert!(map.contains(key));
    /// 
    /// registry.delete(key);
    /// assert!(map.contains(key));
    /// 
    /// map.remove_deleted(&registry);
    /// assert!(!map.contains(key));
    /// ```
    pub fn contains(&self, key: Key) -> bool {
        match self.sparse.get(key.index() as usize) {
            Some(entry) if entry.version == Some(key.version()) => true,
            _ => false,
        }
    }

    /// Returns a reference to the value associated with a key in the map, or `None` if the key does not have a value in the map. Note that this does not check if the key is present in the registry, so it may return `Some` for deleted keys. To check if a key is present in the registry, use [`KeyRegistry::contains`] before calling this method.
    /// 
    /// # Examples
    /// ```
    /// # use sparse_keyed::KeyRegistry;
    /// # use sparse_keyed::SecondaryMap;
    /// let mut registry = KeyRegistry::new();
    /// let mut map = SecondaryMap::new();
    /// let key = registry.create();
    /// assert!(map.get(key).is_none());
    /// 
    /// map.insert(key, "value");
    /// assert_eq!(map.get(key), Some(&"value"));
    /// 
    /// registry.delete(key);
    /// assert_eq!(map.get(key), Some(&"value"));
    /// 
    /// map.remove_deleted(&registry);
    /// assert!(map.get(key).is_none());
    /// ```
    pub fn get(&self, key: Key) -> Option<&T> {
        match self.sparse.get(key.index() as usize) {
            Some(sparse_entry) if sparse_entry.version == Some(key.version()) => {
                Some(&self.dense[sparse_entry.dense_index as usize].1)
            }
            _ => None,
        }
    }

    /// Returns a mutable reference to the value associated with a key in the map, or `None` if the key does not have a value in the map. Note that this does not check if the key is present in the registry, so it may return `Some` for deleted keys. To check if a key is present in the registry, use [`KeyRegistry::contains`] before calling this method.
    /// 
    /// # Examples
    /// ```
    /// # use sparse_keyed::KeyRegistry;
    /// # use sparse_keyed::SecondaryMap;
    /// let mut registry = KeyRegistry::new();
    /// let mut map = SecondaryMap::new();
    /// let key = registry.create();
    /// assert!(map.get_mut(key).is_none());
    /// 
    /// map.insert(key, "value");
    /// assert!(map.get_mut(key).is_some());
    /// 
    /// map.get_mut(key).map(|v| *v = "new value");
    /// assert_eq!(map.get(key), Some(&"new value"));
    /// ```
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
