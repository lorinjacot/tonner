use crate::storage::{DenseEntry, Id};

pub struct Asset {
    id: Id<Self>,
}

impl DenseEntry for Asset {
    type Key = Self;
    type Value = ();

    fn new(id: Id<Self::Key>, value: Self::Value) -> Self {
        Self { id }
    }

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}
