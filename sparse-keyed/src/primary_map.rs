//! A map from [`Key`]s to arbitrary values. The keys are created and managed by the map itself.

use std::{iter::FusedIterator, num::NonZeroU32};

use crate::{DenseEntry, Key};

const FIRST_VERSION: NonZeroU32 = NonZeroU32::new(1).unwrap();

#[derive(Debug, Clone)]
struct SparseEntry {
    dense_index: u32,
    version: NonZeroU32,
}

/// A map from [`Key`]s to arbitrary values. The keys are created and managed by the map itself. A `PrimaryMap` is therefore a special [`KeyRegistry`][crate::KeyRegistry] that stores values associated with the keys.
///
/// `PrimaryMap` is designed to provide efficient operations for (in order of importance):
/// 1. **Iteration**: Iterating over all entries of the `PrimaryMap` is O(n), where n is the number of entries in the map. This is achieved by storing the entries in a dense vector. The map provides a read-only slice of its entries. However, no guarantee is made on the order of the entries in the slice, as it may change after insertions and deletions.
/// 2. **Random access**: Checking for the presence of a key in the `PrimaryMap` or accessing the value associated with a key is O(1). This is achieved by storing a sparse vector of indices pointing to the dense vector. The tradeoff is the memory usage of the sparse vector (up to O(m), where m is the largest number of simultaneously present keys). Random access is therefore slower than a vector but still O(1).
/// 3. **Insertion and deletion**: Insertion and deletion of entries in the `PrimaryMap` are O(1) in the average case, but insertion can be O(n), O(m) or O(m+n) if the sparse vector, the dense vector or both need to be resized. Deletion is always O(1). Deleting keys from the `PrimaryMap` allows their indices to be reused for new keys, keeping `m` low and ensuring fast operations of both the `PrimaryMap` and any associated [`SecondaryMap`][crate::SecondaryMap]s over time. Deleted keys are not automatically deleted from any associated `SecondaryMap`s. However, they can be removed by calling [`SecondaryMap::remove_deleted_from_primary_map`][crate::SecondaryMap::remove_deleted_from_primary_map] with the map.
///
/// # Examples
/// ```
/// # use sparse_keyed::PrimaryMap;
/// let mut map = PrimaryMap::new();
///
/// let a = map.add("a");
/// assert_eq!(map.get(a), Some(&"a"));
///
/// let b = map.add("b");
/// assert_eq!(map.get(a), Some(&"a"));
/// assert_eq!(map.get(b), Some(&"b"));
///
/// map.remove(a);
/// assert_eq!(map.get(a), None);
/// assert_eq!(map.get(b), Some(&"b"));
/// ```
#[derive(Debug, Clone)]
pub struct PrimaryMap<T> {
    dense: Vec<DenseEntry<T>>,
    sparse: Vec<SparseEntry>,
    available: usize,
    next: u32,
}

impl<T> PrimaryMap<T> {
    /// Creates a new, empty `PrimaryMap`.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::PrimaryMap;
    /// let map: PrimaryMap<&str> = PrimaryMap::new();
    /// assert!(map.is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        PrimaryMap {
            dense: Vec::new(),
            sparse: Vec::new(),
            available: 0,
            next: 0,
        }
    }

    /// Adds a new entry to the map and returns the key associated with it. The key is guaranteed to be unique and valid until it is removed from the map. The key can be used with a [`SecondaryMap`][crate::SecondaryMap] to store additional data associated with the entry.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::PrimaryMap;
    /// let mut map = PrimaryMap::new();
    ///
    /// let a = map.add("a");
    /// assert_eq!(map.get(a), Some(&"a"));
    /// ```
    pub fn add(&mut self, value: T) -> Key {
        if self.available == 0 {
            let key = Key::new(self.sparse.len() as u32, FIRST_VERSION);
            self.sparse.push(SparseEntry {
                dense_index: self.dense.len() as u32,
                version: FIRST_VERSION,
            });
            self.dense.push(DenseEntry { value, key });
            key
        } else {
            let entry = &mut self.sparse[self.next as usize];
            entry.version = entry.version.checked_add(1).unwrap();
            let key = Key::new(self.next, entry.version);
            self.next = entry.dense_index;
            self.available -= 1;
            entry.dense_index = self.dense.len() as u32;
            self.dense.push(DenseEntry { value, key });
            key
        }
    }

    /// Removes the entry associated with the given key from the map and returns its value. Keys removed from the map are not automatically removed from any associated [`SecondaryMap`][crate::SecondaryMap]s. However, they can be removed by calling [`SecondaryMap::remove_deleted_from_primary_map`][crate::SecondaryMap::remove_deleted_from_primary_map] with the map.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::PrimaryMap;
    /// let mut map = PrimaryMap::new();
    ///
    /// let a = map.add("a");
    /// let b = map.add("b");
    ///
    /// map.remove(a);
    /// assert_eq!(map.get(a), None);
    /// assert_eq!(map.get(b), Some(&"b"));
    /// ```
    ///
    /// ```
    /// # use sparse_keyed::{PrimaryMap, SecondaryMap};
    /// let mut primary_map = PrimaryMap::new();
    /// let mut secondary_map = SecondaryMap::new();
    ///
    /// let key = primary_map.add("value");
    /// secondary_map.insert(key, "associated value");
    ///
    /// primary_map.remove(key);
    /// assert!(secondary_map.get(key).is_some());
    ///
    /// secondary_map.remove_deleted_from_primary_map(&mut primary_map);
    /// assert!(secondary_map.get(key).is_none());
    /// ```
    pub fn remove(&mut self, key: Key) -> Option<T> {
        let sparse_index = key.index() as usize;
        match self.sparse.get_mut(sparse_index) {
            Some(sparse_entry) if sparse_entry.version == key.version() => {
                let dense_index = sparse_entry.dense_index as usize;
                match self.dense.get(dense_index) {
                    Some(dense_entry) if dense_entry.key.index() == key.index() => {
                        self.sparse[self.dense.last().unwrap().key.index() as usize].dense_index =
                            sparse_entry.dense_index;
                        let removed = self.dense.swap_remove(dense_index);
                        self.sparse[sparse_index].dense_index = self.next;
                        self.next = key.index();
                        self.available += 1;
                        Some(removed.value)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Returns a slice of all entries in the map. The order of the entries in the slice is not guaranteed and may change after insertions and deletions.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::PrimaryMap;
    /// let mut map = PrimaryMap::new();
    ///
    /// let a = map.add("a");
    /// let b = map.add("b");
    ///
    /// let dense_entries = map.as_slice();
    /// let index_a = map.index(a).unwrap() as usize;
    /// assert_eq!(dense_entries[index_a].key, a);
    /// assert_eq!(dense_entries[index_a].value, "a");
    /// ```
    pub fn as_slice(&self) -> &[DenseEntry<T>] {
        &self.dense
    }

    /// Returns the index of the entry associated with the given key in the dense vector, or `None` if the key is not present in the map. This method can be used to manually index into the dense slice returned by [`as_slice`][Self::as_slice] for advanced use cases.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::PrimaryMap;
    /// let mut map = PrimaryMap::new();
    ///
    /// let a = map.add("a");
    /// let b = map.add("b");
    ///
    /// let dense_entries = map.as_slice();
    /// let index_a = map.index(a).unwrap() as usize;
    /// assert_eq!(dense_entries[index_a].key, a);
    /// assert_eq!(dense_entries[index_a].value, "a");
    /// ```
    pub fn index(&self, key: Key) -> Option<u32> {
        let sparse_index = key.index() as usize;
        match self.sparse.get(sparse_index) {
            Some(sparse_entry) if sparse_entry.version == key.version() => {
                match self.dense.get(sparse_entry.dense_index as usize) {
                    Some(dense_entry) if dense_entry.key.index() == key.index() => {
                        Some(sparse_entry.dense_index)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Returns the number of entries in the map.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::PrimaryMap;
    /// let mut map = PrimaryMap::new();
    /// assert_eq!(0, map.len());
    ///
    /// map.add("a");
    /// assert_eq!(1, map.len());
    /// ```
    pub fn len(&self) -> usize {
        self.dense.len()
    }

    /// Returns `true` iff the map contains no entries.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::PrimaryMap;
    /// let mut map = PrimaryMap::new();
    /// assert!(map.is_empty());
    ///
    /// map.add("a");
    /// assert!(!map.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }

    /// Returns `true` iff the map contains an entry associated with the given key.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::PrimaryMap;
    /// let mut map = PrimaryMap::new();
    ///
    /// let a = map.add("a");
    /// assert!(map.contains(a));
    ///
    /// map.remove(a);
    /// assert!(!map.contains(a));
    /// ```
    pub fn contains(&self, key: Key) -> bool {
        self.index(key).is_some()
    }

    /// Returns a reference to the value associated with the given key, or `None` if the key is not present in the map.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::PrimaryMap;
    /// let mut map = PrimaryMap::new();
    ///
    /// let a = map.add("a");
    pub fn get(&self, key: Key) -> Option<&T> {
        let sparse_index = key.index() as usize;
        match self.sparse.get(sparse_index) {
            Some(sparse_entry) if sparse_entry.version == key.version() => {
                match self.dense.get(sparse_entry.dense_index as usize) {
                    Some(dense_entry) if dense_entry.key.index() == key.index() => {
                        Some(&dense_entry.value)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Returns a mutable reference to the value associated with the given key, or `None` if the key is not present in the map.
    /// 
    /// # Examples
    /// ```
    /// # use sparse_keyed::PrimaryMap;
    /// let mut map = PrimaryMap::new();
    /// let a = map.add("a");
    /// 
    /// if let Some(value) = map.get_mut(a) {
    ///   *value = "modified a";
    /// }
    /// 
    /// assert_eq!(map.get(a), Some(&"modified a"));
    /// ```
    pub fn get_mut(&mut self, key: Key) -> Option<&mut T> {
        let sparse_index = key.index() as usize;
        match self.sparse.get(sparse_index) {
            Some(sparse_entry) if sparse_entry.version == key.version() => {
                match self.dense.get_mut(sparse_entry.dense_index as usize) {
                    Some(dense_entry) if dense_entry.key.index() == key.index() => {
                        Some(&mut dense_entry.value)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Returns an iterator visiting all entries in arbitrary order. The order of the entries is not guaranteed and may change after insertions and deletions.
    /// 
    /// # Examples
    /// ```
    /// # use sparse_keyed::PrimaryMap;
    /// let mut map = PrimaryMap::new();
    /// let a = map.add("a");
    /// 
    /// let mut iter = map.iter();
    /// assert_eq!(iter.next(), Some((a, &"a")));
    /// assert_eq!(iter.next(), None);
    /// ```
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            inner: self.dense.iter(),
        }
    }

    /// Returns an iterator visiting all entries in arbitrary order with mutable references to their values. The order of the entries is not guaranteed and may change after insertions and deletions.
    /// 
    /// # Examples
    /// ```
    /// # use sparse_keyed::PrimaryMap;
    /// let mut map = PrimaryMap::new();
    /// let a = map.add("a");
    /// let b = map.add("b");
    /// 
    /// for (key, value) in map.iter_mut() {
    ///   if key == a {
    ///     *value = "modified a";
    ///   }
    /// }
    /// 
    /// assert_eq!(map.get(a), Some(&"modified a"));
    /// assert_eq!(map.get(b), Some(&"b"));
    /// ```
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            inner: self.dense.iter_mut(),
        }
    }

    /// Returns an iterator visiting all keys in arbitrary order. The order of the keys is not guaranteed and may change after insertions and deletions.
    /// 
    /// # Examples
    /// ```
    /// # use sparse_keyed::PrimaryMap;
    /// let mut map = PrimaryMap::new();
    /// let a = map.add("a");
    /// 
    /// let mut iter = map.keys();
    /// assert_eq!(iter.next(), Some(a));
    /// assert_eq!(iter.next(), None);
    /// ```
    pub fn keys(&self) -> Keys<'_, T> {
        Keys {
            inner: self.dense.iter(),
        }
    }

    /// Returns an iterator visiting all values in arbitrary order. The order of the values is not guaranteed and may change after insertions and deletions.
    /// 
    /// # Examples
    /// ```
    /// # use sparse_keyed::PrimaryMap;
    /// let mut map = PrimaryMap::new();
    /// let a = map.add("a");
    /// 
    /// let mut iter = map.values();
    /// assert_eq!(iter.next(), Some(&"a"));
    /// assert_eq!(iter.next(), None);
    /// ```
    pub fn values(&self) -> Values<'_, T> {
        Values {
            inner: self.dense.iter(),
        }
    }

    /// Returns an iterator visiting all values in arbitrary order with mutable references to their values. The order of the values is not guaranteed and may change after insertions and deletions.
    /// 
    /// # Examples
    /// ```
    /// # use sparse_keyed::PrimaryMap;
    /// let mut map = PrimaryMap::new();
    /// let a = map.add("a");
    /// 
    /// for value in map.values_mut() {
    ///   *value = "modified a";
    /// }
    /// 
    /// assert_eq!(map.get(a), Some(&"modified a"));
    /// ```
    pub fn values_mut(&mut self) -> ValuesMut<'_, T> {
        ValuesMut {
            inner: self.dense.iter_mut(),
        }
    }
}

impl<T> IntoIterator for PrimaryMap<T> {
    type Item = (Key, T);
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            inner: self.dense.into_iter(),
        }
    }
}

impl<'a, T> IntoIterator for &'a PrimaryMap<T> {
    type Item = (Key, &'a T);
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut PrimaryMap<T> {
    type Item = (Key, &'a mut T);
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// An iterator visiting all entries in arbitrary order. Created by calling `into_iter()` on a `PrimaryMap`. The order of the entries is not guaranteed and may change after insertions and deletions.
///
/// # Examples
/// ```
/// # use sparse_keyed::PrimaryMap;
/// let mut map = PrimaryMap::new();
///
/// let a = map.add("a");
///
/// let mut iter = map.into_iter();
/// assert_eq!(iter.next(), Some((a, "a")));
/// assert_eq!(iter.next(), None);
/// ```
pub struct IntoIter<T> {
    inner: std::vec::IntoIter<DenseEntry<T>>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = (Key, T);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|entry| (entry.key, entry.value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    fn count(self) -> usize {
        self.inner.count()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth(n).map(|entry| (entry.key, entry.value))
    }

    fn last(self) -> Option<Self::Item> {
        self.inner.last().map(|entry| (entry.key, entry.value))
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner
            .fold(init, |acc, entry| f(acc, (entry.key, entry.value)))
    }
}

impl<T> ExactSizeIterator for IntoIter<T> {}

impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|entry| (entry.key, entry.value))
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth_back(n).map(|entry| (entry.key, entry.value))
    }
}

impl<T> FusedIterator for IntoIter<T> {}

/// An iterator visiting all entries in arbitrary order. Created by [`PrimaryMap::iter()`]. The order of the entries is not guaranteed and may change after insertions and deletions.
/// 
/// # Examples
/// ```
/// # use sparse_keyed::PrimaryMap;
/// let mut map = PrimaryMap::new();
/// let a = map.add("a");
/// 
/// let mut iter = map.iter();
/// assert_eq!(iter.next(), Some((a, &"a")));
/// assert_eq!(iter.next(), None);
/// ```
pub struct Iter<'a, T> {
    inner: std::slice::Iter<'a, DenseEntry<T>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (Key, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|entry| (entry.key, &entry.value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    fn count(self) -> usize {
        self.inner.count()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth(n).map(|entry| (entry.key, &entry.value))
    }

    fn last(self) -> Option<Self::Item> {
        self.inner.last().map(|entry| (entry.key, &entry.value))
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner
            .fold(init, |acc, entry| f(acc, (entry.key, &entry.value)))
    }
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> {}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner
            .next_back()
            .map(|entry| (entry.key, &entry.value))
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.inner
            .nth_back(n)
            .map(|entry| (entry.key, &entry.value))
    }
}

impl<'a, T> FusedIterator for Iter<'a, T> {}

/// An iterator visiting all entries in arbitrary order with mutable references to their values. Created by [`PrimaryMap::iter_mut()`]. The order of the entries is not guaranteed and may change after insertions and deletions.
/// 
/// # Examples
/// ```
/// # use sparse_keyed::PrimaryMap;
/// let mut map = PrimaryMap::new();
/// let a = map.add("a");
/// let b = map.add("b");
/// 
/// for (key, value) in map.iter_mut() {
///     if key == a {
///         *value = "modified a";
///     }
/// }
/// assert_eq!(map.get(a), Some(&"modified a"));
/// assert_eq!(map.get(b), Some(&"b"));
/// ```
pub struct IterMut<'a, T> {
    inner: std::slice::IterMut<'a, DenseEntry<T>>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (Key, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|entry| (entry.key, &mut entry.value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    fn count(self) -> usize {
        self.inner.count()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth(n).map(|entry| (entry.key, &mut entry.value))
    }

    fn last(self) -> Option<Self::Item> {
        self.inner.last().map(|entry| (entry.key, &mut entry.value))
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner
            .fold(init, |acc, entry| f(acc, (entry.key, &mut entry.value)))
    }
}

impl<'a, T> ExactSizeIterator for IterMut<'a, T> {}

impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner
            .next_back()
            .map(|entry| (entry.key, &mut entry.value))
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.inner
            .nth_back(n)
            .map(|entry| (entry.key, &mut entry.value))
    }
}

impl<'a, T> FusedIterator for IterMut<'a, T> {}

/// An iterator visiting all keys in arbitrary order. Created by [`PrimaryMap::keys()`]. The order of the keys is not guaranteed and may change after insertions and deletions.
/// 
/// # Examples
/// ```
/// # use sparse_keyed::PrimaryMap;
/// let mut map = PrimaryMap::new();
/// let a = map.add("a");
/// 
/// let mut iter = map.keys();
/// assert_eq!(iter.next(), Some(a));
/// assert_eq!(iter.next(), None);
/// ```
pub struct Keys<'a, T> {
    inner: std::slice::Iter<'a, DenseEntry<T>>,
}

impl<'a, T> Iterator for Keys<'a, T> {
    type Item = Key;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|entry| entry.key)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    fn count(self) -> usize {
        self.inner.count()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth(n).map(|entry| entry.key)
    }

    fn last(self) -> Option<Self::Item> {
        self.inner.last().map(|entry| entry.key)
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |acc, entry| f(acc, entry.key))
    }
}

impl<'a, T> ExactSizeIterator for Keys<'a, T> {}

impl<'a, T> DoubleEndedIterator for Keys<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|entry| entry.key)
    }
}

impl<'a, T> FusedIterator for Keys<'a, T> {}

/// An iterator visiting all values in arbitrary order. Created by [`PrimaryMap::values()`]. The order of the values is not guaranteed and may change after insertions and deletions.
/// 
/// # Examples
/// ```
/// # use sparse_keyed::PrimaryMap;
/// let mut map = PrimaryMap::new();
/// let a = map.add("a");
/// 
/// let mut iter = map.values();
/// assert_eq!(iter.next(), Some(&"a"));
/// assert_eq!(iter.next(), None);
/// ```
pub struct Values<'a, T> {
    inner: std::slice::Iter<'a, DenseEntry<T>>,
}

impl<'a, T> Iterator for Values<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|entry| &entry.value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    fn count(self) -> usize {
        self.inner.count()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth(n).map(|entry| &entry.value)
    }

    fn last(self) -> Option<Self::Item> {
        self.inner.last().map(|entry| &entry.value)
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner
            .fold(init, |acc, entry| f(acc, &entry.value))
    }
}

impl<'a, T> ExactSizeIterator for Values<'a, T> {}

impl<'a, T> DoubleEndedIterator for Values<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|entry| &entry.value)
    }
}

impl<'a, T> FusedIterator for Values<'a, T> {}

/// An iterator visiting all values in arbitrary order with mutable references to their values. Created by [`PrimaryMap::values_mut()`]. The order of the values is not guaranteed and may change after insertions and deletions.
/// 
/// # Examples
/// ```
/// # use sparse_keyed::PrimaryMap;
/// let mut map = PrimaryMap::new();
/// let a = map.add("a");
/// 
/// for value in map.values_mut() {
///  *value = "modified a";
/// }
/// 
/// assert_eq!(map.get(a), Some(&"modified a"));
/// ```
pub struct ValuesMut<'a, T> {
    inner: std::slice::IterMut<'a, DenseEntry<T>>,
}

impl<'a, T> Iterator for ValuesMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|entry| &mut entry.value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    fn count(self) -> usize {
        self.inner.count()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth(n).map(|entry| &mut entry.value)
    }

    fn last(self) -> Option<Self::Item> {
        self.inner.last().map(|entry| &mut entry.value)
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner
            .fold(init, |acc, entry| f(acc, &mut entry.value))
    }
}

impl<'a, T> ExactSizeIterator for ValuesMut<'a, T> {}

impl<'a, T> DoubleEndedIterator for ValuesMut<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|entry| &mut entry.value)
    }
}

impl<'a, T> FusedIterator for ValuesMut<'a, T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primary_map() {
        let mut map = PrimaryMap::new();
        assert_eq!(0, map.available);

        let key_0_v1 = map.add("0 v1");
        assert_eq!(1, map.len());
        assert!(map.dense.contains(&DenseEntry {
            value: "0 v1",
            key: key_0_v1
        }));
        assert_eq!(map.get(key_0_v1), Some(&"0 v1"));
        assert_eq!(0, key_0_v1.index());
        assert_eq!(1, key_0_v1.version().get());
        assert!(map.contains(key_0_v1));

        let key_1_v1 = map.add("1 v1");
        assert_eq!(2, map.len());
        assert!(map.dense.contains(&DenseEntry {
            value: "1 v1",
            key: key_1_v1
        }));
        assert_eq!(map.get(key_1_v1), Some(&"1 v1"));
        assert_eq!(1, key_1_v1.index());
        assert_eq!(1, key_1_v1.version().get());
        assert!(map.contains(key_0_v1));
        assert!(map.contains(key_1_v1));

        assert_eq!(0, map.available);
        map.remove(key_1_v1);
        assert_eq!(1, map.len());
        assert!(map.dense.contains(&DenseEntry {
            value: "0 v1",
            key: key_0_v1
        }));
        assert!(!map.dense.contains(&DenseEntry {
            value: "1 v1",
            key: key_1_v1
        }));
        assert!(!map.dense.contains(&DenseEntry {
            value: "1 v1",
            key: key_1_v1
        }));
        assert_eq!(1, map.available);
        assert_eq!(1, map.next);
        assert!(map.contains(key_0_v1));
        assert!(!map.contains(key_1_v1));

        let key_1_v2 = map.add("1 v2");
        assert_eq!(2, map.len());
        assert!(map.dense.contains(&DenseEntry {
            value: "1 v2",
            key: key_1_v2
        }));
        assert!(!map.dense.contains(&DenseEntry {
            value: "1 v1",
            key: key_1_v1
        }));
        assert!(map.dense.contains(&DenseEntry {
            value: "1 v2",
            key: key_1_v2
        }));
        assert_eq!(map.get(key_1_v2), Some(&"1 v2"));
        assert_eq!(1, key_1_v2.index());
        assert_eq!(2, key_1_v2.version().get());
        assert_eq!(0, map.available);
        assert!(map.contains(key_0_v1));
        assert!(!map.contains(key_1_v1));
        assert!(map.contains(key_1_v2));

        let key_2_v1 = map.add("2 v1");
        assert_eq!(2, key_2_v1.index());
        assert_eq!(1, key_2_v1.version().get());
        assert_eq!(3, map.len());
        assert!(map.contains(key_0_v1));
        assert!(!map.contains(key_1_v1));
        assert!(map.contains(key_1_v2));
        assert!(map.contains(key_2_v1));

        map.remove(key_0_v1);
        assert_eq!(2, map.len());
        assert_eq!(1, map.available);
        assert_eq!(0, map.next);
        assert!(!map.contains(key_0_v1));
        assert!(map.contains(key_1_v2));
        assert!(map.contains(key_2_v1));

        map.remove(key_2_v1);
        assert_eq!(1, map.len());
        assert_eq!(2, map.available);
        assert_eq!(2, map.next);
        assert!(map.contains(key_1_v2));
        assert!(!map.contains(key_2_v1));

        let key_2_v2 = map.add("2 v2");
        assert_eq!(2, key_2_v2.index());
        assert_eq!(2, key_2_v2.version().get());
        assert_eq!(1, map.available);
        assert_eq!(0, map.next);
        assert!(map.contains(key_1_v2));
        assert!(!map.contains(key_2_v1));
        assert!(map.contains(key_2_v2));
    }
}
