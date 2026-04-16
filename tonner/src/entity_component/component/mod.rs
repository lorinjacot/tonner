use std::sync::{Arc, Mutex};

use crate::{
    entity_component::{EntityId, entity::EntityHandle},
    world::{DynamicField, WorldHandle},
};

pub mod hash_map;
pub mod sparse_array;

pub trait ComponentsView<T> {
    type Iter<'a>: Iterator<Item = (EntityId, &'a T)>
    where
        Self: 'a,
        T: 'a;

    /// Returns true if the entity has the component.
    fn has(&self, entity: EntityId) -> bool;

    /// Returns a reference to the component belonging to the entity.
    fn get(&self, entity: EntityId) -> Option<&T>;

    /// An iterator visiting all components `T` in arbitrary order. The iterator element type is `(EntityId, &'a T)`.
    fn iter<'a>(&'a self) -> Self::Iter<'a>;
}

pub trait ComponentsViewMut<T>: ComponentsView<T> {
    type IterMut<'a>: Iterator<Item = (EntityId, &'a mut T)>
    where
        Self: 'a,
        T: 'a;

    /// Returns a mutable reference to the component belonging to the entity.
    fn get_mut(&mut self, entity: EntityId) -> Option<&mut T>;

    /// An iterator visiting all components `T` in arbitrary order, with mutable references to the values.
    /// The iterator element type is `(EntityId, &'a T)`.
    fn iter_mut<'a>(&'a mut self) -> Self::IterMut<'a>;
}

pub trait ComponentStorage<T>: ComponentsViewMut<T> {
    /// Adds the component to the entity.
    ///
    /// If the entity did not have this component, `None` is returned.
    ///
    /// If the entity did have this component, the component is updated and the old value is returned.
    fn add(&mut self, entity: EntityId, component: T) -> Option<T>;

    /// Removes the component from the entity, returning the component value if the entity previously had it.
    fn remove(&mut self, entity: EntityId) -> Option<T>;

    /// Removes all component `T` from all entities. Keeps the allocated memory for reuse.
    fn clear(&mut self);
}

pub trait ComponentBuilder {
    type Component: ComponentHandle;

    fn build(self, world: &WorldHandle) -> Self::Component;
}

pub trait ComponentHandle {
    // fn add(entity: &EntityHandle)

    /// Returns `true` if and only if the entity has this component.
    fn has(entity: &EntityHandle) -> bool;

    /// Returns an handle to this entity's component. The component is added to
    /// the entity if the entity did not have it.
    fn new(entity: EntityHandle) -> Self;

    fn entity(&self) -> &EntityHandle;

    fn world(&self) -> &WorldHandle {
        self.entity().world()
    }
}
