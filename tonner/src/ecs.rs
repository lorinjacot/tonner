use sparse_keyed::{Key, KeyRegistry, SecondaryMap};

pub type EntityId = Key;
pub type EntityRegistry = KeyRegistry;
pub type ComponentStorage<T> = SecondaryMap<T>;
