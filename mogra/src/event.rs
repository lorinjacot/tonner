pub use named_key::NamedKey;
pub use physical_key::PhysicalKey;

mod named_key;
mod physical_key;

pub struct KeyEvent {
    pub physical_key: PhysicalKey,
}
