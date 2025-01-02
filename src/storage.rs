use std::{
    fmt::Debug,
    hash::Hash,
    iter::FusedIterator,
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
            self.next.pos = self.dense.len();
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
        match self.sparse.get_mut(id.element) {
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

    pub fn dense_indices_u32(&self, ids: impl IntoIterator<Item = Id<T>>) -> Vec<u32> {
        ids.into_iter()
            .map(|id| self.sparse[id.element].pos as u32)
            .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct TestData(String);

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
}
