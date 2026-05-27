use std::{
    fmt::Display,
    num::{NonZeroU32, NonZeroU64},
    ops::Deref,
};

const INDEX_BITES: u64 = 0xFFFF_FFFF_0000_0000;
const INDEX_OFFSET: u64 = 32;
const VERSION_BITES: u64 = 0x0000_0000_FFFF_FFFF;
const FIRST_VERSION: NonZeroU32 = NonZeroU32::new(1).unwrap();

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Key(NonZeroU64);

impl Key {
    #[inline]
    fn new(index: u32, version: NonZeroU32) -> Key {
        let index = (index as u64) << INDEX_OFFSET;
        let version = version.get() as u64;
        // SAFETY: cannot be zero because `version` is non-zero
        Key(unsafe { NonZeroU64::new_unchecked(index | version) })
    }

    #[inline]
    fn index(&self) -> u32 {
        let index = (self.0.get() & INDEX_BITES) >> INDEX_OFFSET;
        index as u32
    }

    #[inline]
    fn version(&self) -> NonZeroU32 {
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

#[derive(Debug, Clone)]
pub struct KeyRegistry {
    dense: Vec<Key>,
    sparse: Vec<SparseEntry>,
    available: usize,
    next: u32,
}

impl KeyRegistry {
    #[must_use]
    pub fn new() -> KeyRegistry {
        KeyRegistry {
            dense: Vec::new(),
            sparse: Vec::new(),
            available: 0,
            next: 0,
        }
    }

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

    pub fn contains(&mut self, key: Key) -> bool {
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
