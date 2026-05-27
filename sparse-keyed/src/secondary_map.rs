//! A map from [`Key`]s to arbitrary values. The keys are created and managed by a [`KeyRegistry`].

use std::{
    iter::{FusedIterator, once, repeat_n},
    num::NonZeroU32,
    ops::{Deref, Index, IndexMut},
    slice::SliceIndex,
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

    /// Removes all entries from the map.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::{KeyRegistry, SecondaryMap};
    /// let mut registry = KeyRegistry::new();
    /// let mut map = SecondaryMap::new();
    /// let key_0 = registry.create();
    /// let key_1 = registry.create();
    /// map.insert(key_0, "value 0");
    /// map.insert(key_1, "value 1");
    ///
    /// map.clear();
    /// assert!(map.is_empty());
    /// assert!(!map.contains(key_0));
    /// assert!(!map.contains(key_1));
    /// ```
    pub fn clear(&mut self) {
        self.dense.clear();
        self.sparse.fill(SparseEntry {
            dense_index: u32::MAX,
            version: None,
        });
    }

    /// Returns the index of the value associated with a key in the dense vector, or `None` if the key does not have a value in the map. The returned index is only valid until the next map modification. The index of one `SecondaryMap` is in general different from the index of another `SecondaryMap` for the same key. Note that this does not check if the key is present in the registry, so it may return `Some` for deleted keys. To check if a key is present in the registry, use [`KeyRegistry::contains`] before calling this method. This method can be used to manually index into the dense vector of the map for advanced use cases.
    ///
    /// ```
    /// # use sparse_keyed::KeyRegistry;
    /// # use sparse_keyed::SecondaryMap;
    /// let mut registry = KeyRegistry::new();
    /// let mut map = SecondaryMap::new();
    /// let key = registry.create();
    /// assert!(map.index(key).is_none());
    ///
    /// map.insert(key, "value");
    /// assert!(map.index(key).is_some());
    ///
    /// let index = map.index(key).unwrap();
    /// assert_eq!(map[index].0, key);
    /// assert_eq!(map[index].1, "value");
    ///
    /// let key_2 = registry.create();
    /// map.insert(key_2, "value 2");
    /// // assert_eq!(map[index].0, key); // might panic
    /// ```
    pub fn index(&self, key: Key) -> Option<usize> {
        match self.sparse.get(key.index() as usize) {
            Some(entry) if entry.version == Some(key.version()) => Some(entry.dense_index as usize),
            _ => None,
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
        self.index(key).is_some()
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
        self.index(key).map(|index| &self.dense[index].1)
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
        self.index(key).map(|index| &mut self.dense[index].1)
    }

    /// Returns an iterator visiting all entries in arbitrary order with mutable references to their values. Note that this does not check if the keys associated with the values are present in the registry, so it may return entries for deleted keys. To check if a key is present in the registry, use [`KeyRegistry::contains`] before calling this method.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::KeyRegistry;
    /// # use sparse_keyed::SecondaryMap;
    /// let mut registry = KeyRegistry::new();
    /// let mut map = SecondaryMap::new();
    /// let key_0 = registry.create();
    /// let key_1 = registry.create();
    /// map.insert(key_0, "value 0");
    /// map.insert(key_1, "value 1");
    ///
    /// for (key, value) in map.iter_mut() {
    ///     *value = "new value";
    /// }
    ///
    /// assert_eq!(map[key_0], "new value");
    /// assert_eq!(map[key_1], "new value");
    /// ```
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            inner: self.dense.iter_mut(),
        }
    }

    /// Returns an iterator visiting all keys in arbitrary order. Note that this does not check if the keys are present in the registry, so it may return keys for deleted keys. To check if a key is present in the registry, use [`KeyRegistry::contains`] before calling this method.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::KeyRegistry;
    /// # use sparse_keyed::SecondaryMap;
    /// let mut registry = KeyRegistry::new();
    /// let mut map = SecondaryMap::new();
    /// let key_0 = registry.create();
    /// let key_1 = registry.create();
    /// map.insert(key_0, "value 0");
    /// map.insert(key_1, "value 1");
    ///
    /// let mut keys = map.keys();
    /// assert_eq!(keys.next(), Some(&key_0));
    /// assert_eq!(keys.next(), Some(&key_1));
    /// assert_eq!(keys.next(), None);
    /// ```
    pub fn keys(&self) -> Keys<'_, T> {
        Keys {
            inner: self.dense.iter(),
        }
    }

    /// Returns an iterator visiting all values in arbitrary order. Note that this does not check if the keys associated with the values are present in the registry, so it may return values for deleted keys. To check if a key is present in the registry, use [`KeyRegistry::contains`] before calling this method.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::KeyRegistry;
    /// # use sparse_keyed::SecondaryMap;
    /// let mut registry = KeyRegistry::new();
    /// let mut map = SecondaryMap::new();
    /// let key_0 = registry.create();
    /// let key_1 = registry.create();
    /// map.insert(key_0, "value 0");
    /// map.insert(key_1, "value 1");
    ///
    /// let mut values = map.values();
    /// assert_eq!(values.next(), Some(&"value 0"));
    /// assert_eq!(values.next(), Some(&"value 1"));
    /// assert_eq!(values.next(), None);
    /// ```
    pub fn values(&self) -> Values<'_, T> {
        Values {
            inner: self.dense.iter(),
        }
    }

    /// Returns a mutable iterator visiting all values in arbitrary order. Note that this does not check if the keys associated with the values are present in the registry, so it may return values for deleted keys. To check if a key is present in the registry, use [`KeyRegistry::contains`] before calling this method.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::KeyRegistry;
    /// # use sparse_keyed::SecondaryMap;
    /// let mut registry = KeyRegistry::new();
    /// let mut map = SecondaryMap::new();
    /// let key_0 = registry.create();
    /// let key_1 = registry.create();
    /// map.insert(key_0, "value 0");
    /// map.insert(key_1, "value 1");
    ///
    /// for value in map.values_mut() {
    ///     *value = "new value";
    /// }
    ///
    /// assert_eq!(map[key_0], "new value");
    /// assert_eq!(map[key_1], "new value");
    /// ```
    pub fn values_mut(&mut self) -> ValuesMut<'_, T> {
        ValuesMut {
            inner: self.dense.iter_mut(),
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

impl<T, I: SliceIndex<[(Key, T)]>> Index<I> for SecondaryMap<T> {
    type Output = I::Output;

    fn index(&self, index: I) -> &Self::Output {
        &self.dense[index]
    }
}

impl<T> FromIterator<(Key, T)> for SecondaryMap<T> {
    fn from_iter<I: IntoIterator<Item = (Key, T)>>(iter: I) -> Self {
        let mut dense: Vec<_> = iter.into_iter().collect();
        let sparse_capacity = dense
            .iter()
            .map(|(key, _)| key.index())
            .max()
            .map_or(0, |count| count as usize + 1);
        let mut sparse: Vec<SparseEntry> = Vec::with_capacity(sparse_capacity);
        let mut i = 0;
        while i < dense.len() {
            let key = dense[i].0;
            let sparse_index = key.index() as usize;
            match sparse.get_mut(sparse_index) {
                Some(entry) => {
                    if entry.version.is_none() {
                        entry.dense_index = i as u32;
                        entry.version = Some(key.version());
                        i += 1;
                    } else {
                        entry.version = Some(key.version());
                        dense.swap(i, entry.dense_index as usize);
                        dense.swap_remove(i);
                    }
                }
                None => {
                    sparse.extend(
                        repeat_n(
                            SparseEntry {
                                dense_index: u32::MAX,
                                version: None,
                            },
                            sparse_index - sparse.len(),
                        )
                        .chain(once(SparseEntry {
                            dense_index: i as u32,
                            version: Some(key.version()),
                        })),
                    );
                    i += 1;
                }
            }
        }

        SecondaryMap { sparse, dense }
    }
}

/// An iterator visiting all entries in arbitrary order with mutable references to their values. Created by [`SecondaryMap::iter_mut()`]. Note that this does not check if the keys associated with the values are present in the registry, so it may return entries for deleted keys. To check if a key is present in the registry, use [`KeyRegistry::contains`] before calling this iterator.
///
/// # Examples
/// ```
/// # use sparse_keyed::{KeyRegistry, SecondaryMap};
/// let mut registry = KeyRegistry::new();
/// let mut map = SecondaryMap::new();
/// let key_0 = registry.create();
/// let key_1 = registry.create();
/// map.insert(key_0, "value 0");
/// map.insert(key_1, "value 1");
///
/// for (key, value) in map.iter_mut() {
///    *value = "new value";
/// }
/// ```
pub struct IterMut<'a, T> {
    inner: std::slice::IterMut<'a, (Key, T)>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (&'a Key, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|entry| (&entry.0, &mut entry.1))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.inner.count()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth(n).map(|entry| (&entry.0, &mut entry.1))
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.inner.last().map(|entry| (&entry.0, &mut entry.1))
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner
            .fold(init, |b, entry| f(b, (&entry.0, &mut entry.1)))
    }
}

impl<'a, T> ExactSizeIterator for IterMut<'a, T> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|entry| (&entry.0, &mut entry.1))
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth_back(n).map(|entry| (&entry.0, &mut entry.1))
    }
}

impl<'a, T> FusedIterator for IterMut<'a, T> {}

/// An iterator visiting all keys in arbitrary order. Created by [`SecondaryMap::keys()`]. Note that this does not check if the keys are present in the registry, so it may return keys for deleted keys. To check if a key is present in the registry, use [`KeyRegistry::contains`] before calling this iterator.
///
/// # Examples
/// ```
/// # use sparse_keyed::{KeyRegistry, SecondaryMap};
/// let mut registry = KeyRegistry::new();
/// let mut map = SecondaryMap::new();
/// let key_0 = registry.create();
/// let key_1 = registry.create();
/// map.insert(key_0, "value 0");
/// map.insert(key_1, "value 1");
///  
/// let mut keys = map.keys();
/// assert_eq!(keys.next(), Some(&key_0));
/// assert_eq!(keys.next(), Some(&key_1));
/// assert_eq!(keys.next(), None);
/// ```
pub struct Keys<'a, T> {
    inner: std::slice::Iter<'a, (Key, T)>,
}

impl<'a, T> Iterator for Keys<'a, T> {
    type Item = &'a Key;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|entry| &entry.0)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.inner.count()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth(n).map(|entry| &entry.0)
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.inner.last().map(|entry| &entry.0)
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |b, entry| f(b, &entry.0))
    }
}

impl<'a, T> ExactSizeIterator for Keys<'a, T> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, T> DoubleEndedIterator for Keys<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|entry| &entry.0)
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth_back(n).map(|entry| &entry.0)
    }
}

impl<'a, T> FusedIterator for Keys<'a, T> {}

/// An iterator visiting all values in arbitrary order. Created by [`SecondaryMap::values()`]. Note that this does not check if the keys associated with the values are present in the registry, so it may return values for deleted keys. To check if a key is present in the registry, use [`KeyRegistry::contains`] before calling this iterator.
///
/// # Examples
/// ```
/// # use sparse_keyed::{KeyRegistry, SecondaryMap};
/// let mut registry = KeyRegistry::new();
/// let mut map = SecondaryMap::new();
/// let key_0 = registry.create();
/// let key_1 = registry.create();
/// map.insert(key_0, "value 0");
/// map.insert(key_1, "value 1");
///
/// let mut values = map.values();
/// assert_eq!(values.next(), Some(&"value 0"));
/// assert_eq!(values.next(), Some(&"value 1"));
/// assert_eq!(values.next(), None);
/// ```
pub struct Values<'a, T> {
    inner: std::slice::Iter<'a, (Key, T)>,
}

impl<'a, T> Iterator for Values<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|entry| &entry.1)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.inner.count()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth(n).map(|entry| &entry.1)
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.inner.last().map(|entry| &entry.1)
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |b, entry| f(b, &entry.1))
    }
}

impl<'a, T> ExactSizeIterator for Values<'a, T> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, T> DoubleEndedIterator for Values<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|entry| &entry.1)
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth_back(n).map(|entry| &entry.1)
    }
}

impl<'a, T> FusedIterator for Values<'a, T> {}

/// An iterator visiting all values in arbitrary order with mutable references to their values. Created by [`SecondaryMap::values_mut()`]. Note that this does not check if the keys associated with the values are present in the registry, so it may return values for deleted keys. To check if a key is present in the registry, use [`KeyRegistry::contains`] before calling this iterator.
///
/// # Examples
/// ```
/// # use sparse_keyed::{KeyRegistry, SecondaryMap};
/// let mut registry = KeyRegistry::new();
/// let mut map = SecondaryMap::new();
/// let key_0 = registry.create();
/// let key_1 = registry.create();
/// map.insert(key_0, "value 0");
/// map.insert(key_1, "value 1");
///
/// for value in map.values_mut() {
///   *value = "new value";
/// }
/// ```
pub struct ValuesMut<'a, T> {
    inner: std::slice::IterMut<'a, (Key, T)>,
}

impl<'a, T> Iterator for ValuesMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|entry| &mut entry.1)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.inner.count()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth(n).map(|entry| &mut entry.1)
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.inner.last().map(|entry| &mut entry.1)
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |b, entry| f(b, &mut entry.1))
    }
}

impl<'a, T> ExactSizeIterator for ValuesMut<'a, T> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, T> DoubleEndedIterator for ValuesMut<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|entry| &mut entry.1)
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth_back(n).map(|entry| &mut entry.1)
    }
}

impl<'a, T> FusedIterator for ValuesMut<'a, T> {}

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

    #[test]
    fn test_from_iterator() {
        let mut registry = KeyRegistry::new();
        let key_0 = registry.create();
        let key_1 = registry.create();

        let map: SecondaryMap<_> = vec![(key_0, "value 0"), (key_1, "value 1")]
            .into_iter()
            .collect();
        assert_eq!(map[key_0], "value 0");
        assert_eq!(map[key_1], "value 1");

        registry.delete(key_0);
        let key_0_v2 = registry.create();

        let map: SecondaryMap<_> = vec![
            (key_0, "value 0"),
            (key_0_v2, "value 0 v2"),
            (key_1, "value 1"),
        ]
        .into_iter()
        .collect();
        assert!(!map.contains(key_0));
        assert_eq!(map[key_0_v2], "value 0 v2");
        assert_eq!(map[key_1], "value 1");

        let map: SecondaryMap<_> = vec![
            (key_1, "value 1"),
            (key_0_v2, "value 0 v2"),
            (key_0, "value 0"),
        ]
        .into_iter()
        .collect();
        assert_eq!(map[key_1], "value 1");
        assert!(!map.contains(key_0_v2));
        assert_eq!(map[key_0], "value 0");
    }
}
