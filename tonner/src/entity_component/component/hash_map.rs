use std::{collections::HashMap, iter::FusedIterator};

use crate::entity_component::{EntityId, component::{ComponentStorage, ComponentsView}};

pub struct Iter<'a, T> {
    inner: std::collections::hash_map::Iter<'a, EntityId, T>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (EntityId, &'a T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|(&entity, component)| (entity, component))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline]
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.inner.count()
    }

    #[inline]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner
            .fold(init, |b, (&entity, component)| f(b, (entity, component)))
    }
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, T> FusedIterator for Iter<'a, T> {}

pub struct IterMut<'a, T> {
    inner: std::collections::hash_map::IterMut<'a, EntityId, T>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (EntityId, &'a mut T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|(&entity, component)| (entity, component))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline]
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.inner.count()
    }

    #[inline]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner
            .fold(init, |b, (&entity, component)| f(b, (entity, component)))
    }
}

impl<'a, T> ExactSizeIterator for IterMut<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, T> FusedIterator for IterMut<'a, T> {}

impl<T> ComponentsView<T> for HashMap<EntityId, T> {
    type Iter<'a>
        = Iter<'a, T>
    where
        Self: 'a,
        T: 'a;

    type IterMut<'a>
        = IterMut<'a, T>
    where
        Self: 'a,
        T: 'a;

    fn has(&self, entity: EntityId) -> bool {
        self.contains_key(&entity)
    }

    fn get(&self, entity: EntityId) -> Option<&T> {
        self.get(&entity)
    }

    fn get_mut(&mut self, entity: EntityId) -> Option<&mut T> {
        self.get_mut(&entity)
    }

    fn iter<'a>(&'a self) -> Self::Iter<'a> {
        Iter { inner: self.iter() }
    }

    fn iter_mut<'a>(&'a mut self) -> Self::IterMut<'a> {
        IterMut {
            inner: self.iter_mut(),
        }
    }
}

impl<T> ComponentStorage<T> for HashMap<EntityId, T> {
    fn add(&mut self, entity: EntityId, component: T) -> Option<T> {
        self.insert(entity, component)
    }

    fn remove(&mut self, entity: EntityId) -> Option<T> {
        self.remove(&entity)
    }

    fn clear(&mut self) {
        self.clear();
    }
}
