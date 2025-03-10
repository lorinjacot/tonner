use std::{
    fmt::Debug,
    hash::Hash,
    iter::{repeat_n, FusedIterator},
    marker::PhantomData,
    ops::{Index, IndexMut},
};

pub struct Id<T> {
    element: usize,
    version: u16,
    element_type: PhantomData<T>,
}

impl<T> Debug for Id<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("Id<{}>", std::any::type_name::<T>()))
            .field("element", &self.element)
            .field("version", &self.version)
            .finish()
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        Self {
            element: self.element,
            version: self.version,
            element_type: PhantomData,
        }
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.element == other.element && self.version == other.version
    }
}

impl<T> Eq for Id<T> {}

impl<T> Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.element.hash(state);
        self.version.hash(state);
    }
}

#[derive(Debug, Clone)]
struct SparseElement {
    pos: usize,
    version: u16,
}

struct DenseElement<T> {
    value: T,
    element: usize,
}

pub struct Storage<T> {
    sparse: Vec<SparseElement>,
    dense: Vec<DenseElement<T>>,
    next: SparseElement,
    available: usize,
}

impl<T> Storage<T> {
    pub fn new() -> Self {
        Self {
            sparse: Vec::new(),
            dense: Vec::new(),
            next: SparseElement { pos: 0, version: 0 },
            available: 0,
        }
    }

    pub fn add(&mut self, value: T) -> Id<T> {
        let pos = self.dense.len();
        if self.available > 0 {
            let element = self.next.pos;
            let version = self.next.version;
            self.next.pos = pos;
            std::mem::swap(&mut self.next, &mut self.sparse[element]);
            self.available -= 1;

            self.dense.push(DenseElement { value, element });

            Id {
                element,
                version,
                element_type: PhantomData,
            }
        } else {
            let element = self.sparse.len();
            let version = 0;
            self.sparse.push(SparseElement { pos, version });
            self.dense.push(DenseElement { value, element });
            Id {
                element,
                version,
                element_type: PhantomData,
            }
        }
    }

    pub fn remove(&mut self, id: Id<T>) -> Option<T> {
        match self.sparse.get(id.element) {
            Some(sparse_element) if id.version == sparse_element.version => {
                match self.dense.get(sparse_element.pos) {
                    Some(dense_element) if id.element == dense_element.element => {
                        let last_pos = self.dense.len() - 1;
                        let last_element = self.dense[last_pos].element;
                        self.dense.swap(last_pos, sparse_element.pos);
                        self.sparse[last_element].pos = sparse_element.pos;

                        // add element to implicit list of deleted elements
                        let next = &mut self.sparse[id.element];
                        next.pos = id.element;
                        next.version += 1;
                        std::mem::swap(&mut self.next, next);
                        self.available += 1;

                        Some(self.dense.pop().unwrap().value)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn get(&self, id: Id<T>) -> Option<&T> {
        match self.sparse.get(id.element) {
            Some(sparse_element) if id.version == sparse_element.version => {
                match self.dense.get(sparse_element.pos) {
                    Some(dense_element) if dense_element.element == id.element => {
                        Some(&dense_element.value)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: Id<T>) -> Option<&mut T> {
        match self.sparse.get(id.element) {
            Some(sparse_element) if id.version == sparse_element.version => {
                match self.dense.get_mut(sparse_element.pos) {
                    Some(dense_element) if dense_element.element == id.element => {
                        Some(&mut dense_element.value)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn contains(&self, id: Id<T>) -> bool {
        match self.sparse.get(id.element) {
            Some(sparse_element) if id.version == sparse_element.version => {
                match self.dense.get(sparse_element.pos) {
                    Some(dense_element) if id.element == dense_element.element => true,
                    _ => false,
                }
            }
            _ => false,
        }
    }

    pub fn values(&self) -> Values<'_, T> {
        Values(self.dense.iter())
    }

    pub fn values_mut(&mut self) -> ValuesMut<'_, T> {
        ValuesMut(self.dense.iter_mut())
    }
}

impl<T> Index<Id<T>> for Storage<T> {
    type Output = T;

    fn index(&self, index: Id<T>) -> &Self::Output {
        self.get(index).expect("no value found for id")
    }
}

impl<T> IndexMut<Id<T>> for Storage<T> {
    fn index_mut(&mut self, index: Id<T>) -> &mut Self::Output {
        self.get_mut(index).expect("no value found for id")
    }
}

pub struct SecondaryStorage<P, S> {
    sparse: Vec<SparseElement>,
    dense: Vec<DenseElement<S>>,
    primary: PhantomData<P>,
}

impl<P, S> SecondaryStorage<P, S> {
    pub fn new() -> Self {
        let sparse = Vec::new();
        let dense = Vec::new();
        let primary = PhantomData;
        Self {
            sparse,
            dense,
            primary,
        }
    }

    pub fn add(&mut self, value: S, primary: Id<P>) {
        let pos = self.dense.len();
        let element = primary.element;
        self.dense.push(DenseElement { value, element });
        match self.sparse.get_mut(primary.element) {
            Some(id) => {
                id.pos = pos;
                id.version = primary.version;
            }
            None => {
                let iter = repeat_n(
                    SparseElement {
                        pos,
                        version: primary.version,
                    },
                    element - self.sparse.len() + 1,
                );
                self.sparse.extend(iter);
            }
        }
    }

    fn remove(&mut self, id: Id<P>) -> Option<S> {
        match self.sparse.get(id.element) {
            Some(sparce_element) if id.version == sparce_element.version => {
                match self.dense.get(sparce_element.pos) {
                    Some(dense_element) if id.element == dense_element.element => {
                        let last_pos = self.dense.len() - 1;
                        let last_element = self.dense[last_pos].element;
                        self.dense.swap(last_pos, sparce_element.pos);
                        self.sparse[last_element].pos = sparce_element.pos;
                        self.sparse[id.element].version += 1;
                        Some(self.dense.pop().unwrap().value)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn contains(&self, id: Id<P>) -> bool {
        match self.sparse.get(id.element) {
            Some(sparse_element) if id.version == sparse_element.version => {
                match self.dense.get(sparse_element.pos) {
                    Some(dense_element) if dense_element.element == id.element => true,
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn get(&self, id: Id<P>) -> Option<&S> {
        match self.sparse.get(id.element) {
            Some(sparse_element) if id.version == sparse_element.version => {
                match self.dense.get(sparse_element.pos) {
                    Some(dense_element) if dense_element.element == id.element => {
                        Some(&dense_element.value)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: Id<P>) -> Option<&mut S> {
        match self.sparse.get(id.element) {
            Some(sparse_element) if id.version == sparse_element.version => {
                match self.dense.get_mut(sparse_element.pos) {
                    Some(dense_element) if dense_element.element == id.element => {
                        Some(&mut dense_element.value)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn values(&self) -> Values<'_, S> {
        Values(self.dense.iter())
    }

    pub fn values_mut(&mut self) -> ValuesMut<'_, S> {
        ValuesMut(self.dense.iter_mut())
    }
}

impl<P, S> Index<Id<P>> for SecondaryStorage<P, S> {
    type Output = S;

    fn index(&self, index: Id<P>) -> &Self::Output {
        self.get(index).expect("no value found for id")
    }
}

impl<P, S> IndexMut<Id<P>> for SecondaryStorage<P, S> {
    fn index_mut(&mut self, index: Id<P>) -> &mut Self::Output {
        self.get_mut(index).expect("no value found for id")
    }
}

pub struct Values<'a, T>(std::slice::Iter<'a, DenseElement<T>>);

impl<'a, T> Iterator for Values<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        self.0.next().map(|dense_element| &dense_element.value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    fn count(self) -> usize {
        self.0.len()
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.0
            .fold(init, |acc, dense_element| f(acc, &dense_element.value))
    }
}

impl<'a, T> ExactSizeIterator for Values<'a, T> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'a, T> FusedIterator for Values<'a, T> {}

pub struct ValuesMut<'a, T>(std::slice::IterMut<'a, DenseElement<T>>);

impl<'a, T> Iterator for ValuesMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<&'a mut T> {
        self.0.next().map(|dense_element| &mut dense_element.value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    fn count(self) -> usize {
        self.0.len()
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.0
            .fold(init, |acc, dense_element| f(acc, &mut dense_element.value))
    }
}

impl<'a, T> ExactSizeIterator for ValuesMut<'a, T> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'a, T> FusedIterator for ValuesMut<'a, T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct TestData(String);

    #[derive(Debug, PartialEq)]
    struct TestDataSecondary(String);

    #[test]
    fn test_add() {
        let mut storage = Storage::new();

        let id_1 = storage.add(TestData(String::from("data 1")));
        let id_2 = storage.add(TestData(String::from("data 2")));

        assert_ne!(id_1, id_2);
        assert_eq!("data 1", storage[id_1].0);
        assert_eq!("data 2", storage[id_2].0);
    }

    #[test]
    fn test_contains() {
        let mut storage = Storage::new();

        let mut id_1 = storage.add(TestData(String::from("data 1")));
        let mut id_2 = storage.add(TestData(String::from("data 2")));

        assert!(storage.contains(id_1));
        assert!(storage.contains(id_2));

        id_1.version += 1;
        assert!(!storage.contains(id_1));

        id_2.element += 1;
        assert!(!storage.contains(id_2));
    }

    #[test]
    fn test_remove() {
        let mut storage = Storage::new();

        let id_1 = storage.add(TestData(String::from("data 1")));
        let id_2 = storage.add(TestData(String::from("data 2")));

        storage.remove(id_1);
        assert_eq!(None, storage.get(id_1));
        assert_eq!("data 2", storage[id_2].0);

        assert_eq!(storage.available, 1);
        assert_eq!(storage.next.pos, id_1.element);
        assert_eq!(storage.next.version, id_1.version + 1);

        storage.remove(id_2);
        assert_eq!(None, storage.get(id_2));

        assert_eq!(storage.available, 2);
        assert_eq!(storage.next.pos, id_2.element);
        assert_eq!(storage.next.version, id_2.version + 1);
    }

    #[test]
    fn test_recycling() {
        let mut storage = Storage::new();

        let id_1_v1 = storage.add(TestData(String::from("data 1 v1")));
        let id_2_v1 = storage.add(TestData(String::from("data 2 v1")));

        storage.remove(id_2_v1);
        storage.remove(id_1_v1);

        let id_1_v2 = storage.add(TestData(String::from("data 1 v2")));
        assert_ne!(id_1_v1, id_1_v2);

        assert_eq!(storage.available, 1);
        assert_eq!(storage.next.pos, id_2_v1.element);
        assert_eq!(storage.next.version, id_2_v1.version + 1);

        let id_2_v2 = storage.add(TestData(String::from("data 2 v2")));
        assert_ne!(id_2_v1, id_2_v2);

        assert_eq!(id_1_v2.element, 0);
        assert_eq!(id_1_v2.version, 1);

        assert_eq!(id_2_v2.element, 1);
        assert_eq!(id_2_v2.version, 1);

        let id_3_v1 = storage.add(TestData(String::from("data 3 v1")));
        assert_eq!(id_3_v1.element, 2);
        assert_eq!(id_3_v1.version, 0);

        storage.remove(id_2_v2);
        let id_2_v3 = storage.add(TestData(String::from("data 2 v3")));
        assert_ne!(id_2_v1, id_2_v3);
        assert_ne!(id_2_v2, id_2_v3);

        assert_eq!(id_2_v3.element, 1);
        assert_eq!(id_2_v3.version, 2);
    }

    #[test]
    fn test_add_secondary() {
        let mut storage = Storage::new();
        let mut sec_storage = SecondaryStorage::new();

        let id_1 = storage.add(TestData(String::from("data 1")));
        let id_2 = storage.add(TestData(String::from("data 2")));

        sec_storage.add(TestDataSecondary(String::from("data 1, prim 2")), id_2);
        sec_storage.add(TestDataSecondary(String::from("data 2, prim 1")), id_1);

        assert_eq!("data 2, prim 1", sec_storage[id_1].0);
        assert_eq!("data 1, prim 2", sec_storage[id_2].0);
    }

    #[test]
    fn test_contains_secondary() {
        let mut storage = Storage::new();
        let mut sec_storage = SecondaryStorage::new();

        let mut id_1 = storage.add(TestData(String::from("data 1")));
        let mut id_2 = storage.add(TestData(String::from("data 2")));

        sec_storage.add(TestDataSecondary(String::from("data 1, prim 2")), id_2);
        sec_storage.add(TestDataSecondary(String::from("data 2, prim 1")), id_1);

        assert!(sec_storage.contains(id_1));
        assert!(sec_storage.contains(id_2));

        id_1.version += 1;
        assert!(!sec_storage.contains(id_1));

        id_2.element += 1;
        assert!(!sec_storage.contains(id_2));
    }

    #[test]
    fn test_remove_secondary() {
        let mut storage = Storage::new();
        let mut sec_storage = SecondaryStorage::new();

        let id_1 = storage.add(TestData(String::from("data 1")));
        let id_2 = storage.add(TestData(String::from("data 2")));

        sec_storage.add(TestDataSecondary(String::from("data 1, prim 2")), id_2);
        sec_storage.add(TestDataSecondary(String::from("data 2, prim 1")), id_1);

        sec_storage.remove(id_2);
        assert_eq!(None, sec_storage.get(id_2));
        assert_eq!("data 2, prim 1", sec_storage[id_1].0);

        sec_storage.remove(id_1);
        assert_eq!(None, sec_storage.get(id_1));
    }
}
