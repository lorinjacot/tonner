use sparse_keyed::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyId(Key);
