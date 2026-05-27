use std::{
    fmt::Display,
    num::{NonZeroU32, NonZeroU64},
    ops::Deref,
};

const INDEX_BITES: u64 = 0xFFFF_FFFF_0000_0000;
const INDEX_OFFSET: u64 = 32;
const VERSION_BITES: u64 = 0x0000_0000_FFFF_FFFF;
const FIRST_VERSION: NonZeroU32 = NonZeroU32::new(1).unwrap();

/// A key is an opaque identifier that can be used to index into a [`SecondaryMap`][crate::SecondaryMap]. It is created and managed by a [`KeyRegistry`]. It is also garanteed that `Key` has the same memory layout as `Option<Key>`.
///
/// A key is composed of an index and a version. The index is used to access the sparse vector of the registry and the `SecondaryMap`. The version is used to check if the key is still valid, i.e. if it has not been deleted and the index reused for another key. The combination of the index and the version is garanteed to be unique for each key created by the registry, even if the index is reused after deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Key(NonZeroU64);

impl Key {
    #[inline]
    #[must_use]
    pub(crate) fn new(index: u32, version: NonZeroU32) -> Key {
        let index = (index as u64) << INDEX_OFFSET;
        let version = version.get() as u64;
        // SAFETY: cannot be zero because `version` is non-zero
        Key(unsafe { NonZeroU64::new_unchecked(index | version) })
    }

    #[inline]
    pub(crate) fn index(&self) -> u32 {
        let index = (self.0.get() & INDEX_BITES) >> INDEX_OFFSET;
        index as u32
    }

    #[inline]
    pub(crate) fn version(&self) -> NonZeroU32 {
        let version = (self.0.get() & VERSION_BITES) as u32;
        // SAFETY: cannot be zero because the VERSION BITES cannot be all zero
        unsafe { NonZeroU32::new_unchecked(version) }
    }
}

impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.index(), self.version())
    }
}

#[derive(Debug, Clone)]
struct SparseEntry {
    dense_index: u32,
    version: NonZeroU32,
}

/// A key registry is responsible of creating and managing [`Key`]s. It can also delete keys, which makes them invalid and allows their index to be reused for new keys. The registry also provides a method to check if a key is still valid.
///
/// `KeyRegistry` is designed to provide efficient operations for (in order of importance):
/// 1. **Iteration**: Iterating over all entries of the registry is O(n), where n is the number keys in the registry. This is achieved by storing all the keys in a dense vector. The registry provides a read-only slice of its keys via [`KeyRegistry::deref`]. However, no guarantee is made on their order, as it may change after any creation or deletion of keys.
/// 2. **Random access**: Checking for the presence of a key in the registry is O(1). This is achieved by storing a sparse vector of indices pointing to the dense vector. The tradeoff is the memory usage of the sparse vector (up to O(m), where m is the largest number of simultaneous keys). Random access is therefore slower than a vector but still O(1).
/// 3. **Insertion and deletion**: Insertion and deletion of keys in the registry are O(1) in the average case, but insertion can be O(n), O(m) or O(m+n) if the sparse vector, the dense vector or both need to be resized. Deletion is always O(1). Deletion unused keys from the registry allows their index to be reused for new keys, keeping `m` low and ensuring fast operations of both the `KeyRegistry` and the [`SecondaryMap`][crate::SecondaryMap]s over time. Deleted keys are not automatically deleted from the `SecondaryMap`s. However, deleted keys can be removed by calling [`SecondaryMap::remove_deleted`][crate::SecondaryMap::remove_deleted] with the registry.
///
/// # Examples
/// ```
/// # use sparse_keyed::KeyRegistry;
/// let mut registry = KeyRegistry::new();
///
/// let key_a = registry.create();
/// let key_b = registry.create();
/// assert_ne!(key_a, key_b);
///
/// assert!(registry.contains(key_a));
/// assert!(registry.contains(key_b));
///
/// registry.delete(key_a);
/// assert!(!registry.contains(key_a));
/// assert!(registry.contains(key_b));
///
/// ```
#[derive(Debug, Clone)]
pub struct KeyRegistry {
    dense: Vec<Key>,
    sparse: Vec<SparseEntry>,
    available: usize,
    next: u32,
}

impl KeyRegistry {
    /// Creates a new empty `KeyRegistry`.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::KeyRegistry;
    /// let registry = KeyRegistry::new();
    /// assert!(registry.is_empty());
    /// ```
    #[must_use]
    pub fn new() -> KeyRegistry {
        KeyRegistry {
            dense: Vec::new(),
            sparse: Vec::new(),
            available: 0,
            next: 0,
        }
    }

    /// Creates a new key. The key is guaranteed to be unique throughout the lifetime of the registry.
    ///
    /// # Panics
    ///
    /// Might panic if the number of keys created exceeds `u32::MAX` or if the version of a key exceeds `u32::MAX`. However, this is unlikely to happen in practice, as it would require creating more than 4 billion keys or deleting and recreating the same key more than 4 billion times.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::KeyRegistry;
    /// let mut registry = KeyRegistry::new();
    /// let key_a = registry.create();
    /// let key_b = registry.create();
    /// assert_ne!(key_a, key_b);
    /// assert!(registry.contains(key_a));
    /// assert!(registry.contains(key_b));
    /// ```
    #[must_use]
    pub fn create(&mut self) -> Key {
        if self.available == 0 {
            let key = Key::new(self.sparse.len() as u32, FIRST_VERSION);
            self.sparse.push(SparseEntry {
                dense_index: self.dense.len() as u32,
                version: FIRST_VERSION,
            });
            self.dense.push(key);
            key
        } else {
            let entry = &mut self.sparse[self.next as usize];
            entry.version = entry.version.checked_add(1).unwrap();
            let key = Key::new(self.next, entry.version);
            self.next = entry.dense_index;
            self.available -= 1;
            entry.dense_index = self.dense.len() as u32;
            self.dense.push(key);
            key
        }
    }

    /// Deletes a key. No future key created by the registry will be equal to the deleted key. Keys deleted from the registry are not automatically deleted from the [`SecondaryMap`][crate::SecondaryMap]s. However, deleted keys can be removed by calling [`SecondaryMap::remove_deleted`][crate::SecondaryMap::remove_deleted] with the registry.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::KeyRegistry;
    /// let mut registry = KeyRegistry::new();
    /// let key_a = registry.create();
    /// registry.delete(key_a);
    /// assert!(!registry.contains(key_a));
    /// ```
    pub fn delete(&mut self, key: Key) -> bool {
        let sparse_index = key.index() as usize;
        match self.sparse.get_mut(sparse_index) {
            Some(sparse_entry) if sparse_entry.version == key.version() => {
                let dense_index = sparse_entry.dense_index as usize;
                match self.dense.get(dense_index) {
                    Some(dense_entry) if dense_entry.index() == key.index() => {
                        self.sparse[self.dense.last().unwrap().index() as usize].dense_index =
                            sparse_entry.dense_index;
                        self.dense.swap_remove(dense_index);
                        self.sparse[sparse_index].dense_index = self.next;
                        self.next = key.index();
                        self.available += 1;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// Returns `true` if the registry contains the key. Returns `false` if the key has been deleted or was never created by the registry.
    ///
    /// # Examples
    /// ```
    /// # use sparse_keyed::KeyRegistry;
    /// let mut registry = KeyRegistry::new();
    /// let key_a = registry.create();
    /// assert!(registry.contains(key_a));
    /// registry.delete(key_a);
    /// assert!(!registry.contains(key_a));
    /// ```
    pub fn contains(&self, key: Key) -> bool {
        match self.sparse.get(key.index() as usize) {
            Some(sparse_entry) if sparse_entry.version == key.version() => {
                match self.dense.get(sparse_entry.dense_index as usize) {
                    Some(dense_entry) if dense_entry.index() == key.index() => true,
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

impl Default for KeyRegistry {
    fn default() -> Self {
        KeyRegistry::new()
    }
}

impl Deref for KeyRegistry {
    type Target = [Key];

    fn deref(&self) -> &Self::Target {
        &self.dense
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_layout() {
        use std::mem::{align_of, size_of};
        assert_eq!(size_of::<Key>(), size_of::<Option<Key>>());
        assert_eq!(align_of::<Key>(), align_of::<Option<Key>>());
    }

    #[test]
    fn test_registry() {
        let mut registry = KeyRegistry::new();
        assert_eq!(0, registry.available);

        let key_0_v1 = registry.create();
        assert_eq!(1, registry.len());
        assert!(registry.dense.contains(&key_0_v1));
        assert_eq!(0, key_0_v1.index());
        assert_eq!(1, key_0_v1.version().get());
        assert!(registry.contains(key_0_v1));

        let key_1_v1 = registry.create();
        assert_eq!(2, registry.len());
        assert!(registry.dense.contains(&key_1_v1));
        assert_eq!(1, key_1_v1.index());
        assert_eq!(1, key_1_v1.version().get());
        assert!(registry.contains(key_0_v1));
        assert!(registry.contains(key_1_v1));

        assert_eq!(0, registry.available);
        registry.delete(key_1_v1);
        assert_eq!(1, registry.len());
        assert!(registry.dense.contains(&key_0_v1));
        assert!(!registry.dense.contains(&key_1_v1));
        assert_eq!(1, registry.available);
        assert_eq!(1, registry.next);
        assert!(registry.contains(key_0_v1));
        assert!(!registry.contains(key_1_v1));

        let key_1_v2 = registry.create();
        assert_eq!(2, registry.len());
        assert!(registry.dense.contains(&key_0_v1));
        assert!(!registry.dense.contains(&key_1_v1));
        assert!(registry.dense.contains(&key_1_v2));
        assert_eq!(1, key_1_v2.index());
        assert_eq!(2, key_1_v2.version().get());
        assert_eq!(0, registry.available);
        assert!(registry.contains(key_0_v1));
        assert!(!registry.contains(key_1_v1));
        assert!(registry.contains(key_1_v2));

        let key_2_v1 = registry.create();
        assert_eq!(2, key_2_v1.index());
        assert_eq!(1, key_2_v1.version().get());
        assert_eq!(3, registry.len());
        assert!(registry.contains(key_0_v1));
        assert!(!registry.contains(key_1_v1));
        assert!(registry.contains(key_1_v2));
        assert!(registry.contains(key_2_v1));

        registry.delete(key_0_v1);
        assert_eq!(2, registry.len());
        assert_eq!(1, registry.available);
        assert_eq!(0, registry.next);
        assert!(!registry.contains(key_0_v1));
        assert!(registry.contains(key_1_v2));
        assert!(registry.contains(key_2_v1));

        registry.delete(key_2_v1);
        assert_eq!(1, registry.len());
        assert_eq!(2, registry.available);
        assert_eq!(2, registry.next);
        assert!(registry.contains(key_1_v2));
        assert!(!registry.contains(key_2_v1));

        let key_2_v2 = registry.create();
        assert_eq!(2, key_2_v2.index());
        assert_eq!(2, key_2_v2.version().get());
        assert_eq!(1, registry.available);
        assert_eq!(0, registry.next);
        assert!(registry.contains(key_1_v2));
        assert!(!registry.contains(key_2_v1));
        assert!(registry.contains(key_2_v2));
    }
}
