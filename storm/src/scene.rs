use crate::storage::{DenseEntry, Id};

pub struct Scene {
    id: Id<Self>
}

impl DenseEntry for Scene {
    type Key = Self;
    type Value = ();

    fn new(id: Id<Self::Key>, _value: Self::Value) -> Self {
        Scene { id }
    }

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}