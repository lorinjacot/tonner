use std::{
    fmt::Debug,
    marker::PhantomData,
    ops::{Deref, DerefMut, Index, IndexMut},
};

pub struct Id<T> {
    sparse: u16,
    version: u16,
    target: PhantomData<T>,
}

impl<T> Debug for Id<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("Id<{}>", std::any::type_name::<T>()))
            .field("sparse", &self.sparse)
            .field("version", &self.version)
            .finish()
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        Self {
            sparse: self.sparse,
            version: self.version,
            target: PhantomData,
        }
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.sparse == other.sparse && self.version == other.version
    }
}

impl<T> Eq for Id<T> {}

pub struct Resource<T> {
    id: Id<T>,
    value: T,
}

impl<T> Resource<T> {
    pub fn id(&self) -> Id<T> {
        self.id
    }
}

impl<T> Deref for Resource<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for Resource<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

#[derive(Clone)]
struct SparseElement {
    dense: u16,
    version: u16,
}

pub struct ResourseManager<T> {
    sparse: Vec<SparseElement>,
    dense: Vec<Resource<T>>,
    // the sparse indices can be recycled through
    // the implicit list described by next and available
    next: SparseElement,
    available: u16,
}

impl<T> ResourseManager<T> {
    pub fn new() -> Self {
        let next = SparseElement {
            dense: 0,
            version: 0,
        };

        Self {
            sparse: Vec::new(),
            dense: Vec::new(),
            next,
            available: 0,
        }
    }

    pub fn push(&mut self, value: T) -> Id<T> {
        let dense = self.dense.len() as u16;
        assert!(dense != u16::MAX, "resourse manager is full");
        if self.available == 0 {
            let sparse = self.sparse.len() as u16;
            let version = 0;
            let id = Id {
                sparse,
                version,
                target: PhantomData,
            };
            self.sparse.push(SparseElement { dense, version });
            self.dense.push(Resource { id, value });
            id
        } else {
            self.available -= 1;
            let sparse = self.next.dense; // despite its name, this is the next unused sparse index
            self.next.dense = dense; // will be the new sparse entry
            let id = Id {
                sparse,
                version: self.next.version,
                target: PhantomData,
            };
            std::mem::swap(&mut self.next, &mut self.sparse[sparse as usize]);
            self.dense.push(Resource { id, value });
            id
        }
    }

    pub fn pop(&mut self, id: Id<T>) -> Option<T> {
        if let Some(sparse) = self.sparse.get(id.sparse as usize) {
            if id.version != sparse.version {
                return None;
            }
            if let Some(resourse) = self.dense.get(sparse.dense as usize) {
                if id.sparse != resourse.id.sparse {
                    return None;
                }
            }
            
            let last = self.dense.len() - 1;
            let last_sparse = self.dense[last].id.sparse;
            self.dense.swap(last, sparse.dense as usize);
            self.sparse[last_sparse as usize].dense = sparse.dense;

            // add sparse element to implicit list
            self.sparse[id.sparse as usize] = self.next.clone();
            self.next.dense = id.sparse;
            self.next.version = id.version + 1;
            self.available += 1;

            Some(self.dense.pop().unwrap().value)
        } else {
            None
        }
    }

    pub fn contains(&self, id: Id<T>) -> bool {
        match self.sparse.get(id.sparse as usize) {
            Some(sparse) if id.version == sparse.version => {
                match self.dense.get(sparse.dense as usize) {
                    Some(resourse) => id.sparse == resourse.id.sparse,
                    None => false,
                }
            }
            _ => false,
        }
    }

    pub fn get(&self, id: Id<T>) -> Option<&Resource<T>> {
        match self.sparse.get(id.sparse as usize) {
            Some(sparse) if id.version == sparse.version => {
                match self.dense.get(sparse.dense as usize) {
                    Some(resourse) if id.sparse == resourse.id.sparse => Some(resourse),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: Id<T>) -> Option<&mut Resource<T>> {
        match self.sparse.get(id.sparse as usize) {
            Some(sparse) if id.version == sparse.version => {
                match self.dense.get_mut(sparse.dense as usize) {
                    Some(resourse) if id.sparse == resourse.id.sparse => Some(resourse),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn values(&self) -> impl Iterator<Item = &Resource<T>> {
        self.dense.iter()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Resource<T>> {
        self.dense.iter_mut()
    }
}

impl<T> Index<Id<T>> for ResourseManager<T> {
    type Output = Resource<T>;

    fn index(&self, index: Id<T>) -> &Self::Output {
        self.get(index).expect("no entry found for id")
    }
}

impl<T> IndexMut<Id<T>> for ResourseManager<T> {
    fn index_mut(&mut self, index: Id<T>) -> &mut Self::Output {
        self.get_mut(index).expect("no entry found for id")
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct TestData(String);

    #[test]
    fn test_push() {
        let mut manager = ResourseManager::new();

        let id_1 = manager.push(TestData(String::from("data 1")));
        let id_2 = manager.push(TestData(String::from("data 2")));

        assert_ne!(id_1, id_2);
        assert_eq!("data 1", manager[id_1].0);
        assert_eq!("data 2", manager[id_2].0);
    }

    #[test]
    fn test_contains() {
        let mut manager = ResourseManager::new();

        let mut id_1 = manager.push(TestData(String::from("data 1")));
        let mut id_2 = manager.push(TestData(String::from("data 2")));

        assert!(manager.contains(id_1));
        assert!(manager.contains(id_2));

        id_1.version += 1;
        assert!(!manager.contains(id_1));

        id_2.sparse += 1;
        assert!(!manager.contains(id_2));
    }

    #[test]
    fn test_remove() {
        let mut manager = ResourseManager::new();

        let id_1 = manager.push(TestData(String::from("data 1")));
        let id_2 = manager.push(TestData(String::from("data 2")));

        manager.pop(id_1);
        assert!(manager.get(id_1).is_none());
        assert_eq!("data 2", manager[id_2].0);

        assert_eq!(manager.available, 1);
        assert_eq!(manager.next.dense, id_1.sparse);
        assert_eq!(manager.next.version, id_1.version + 1);

        manager.pop(id_2);
        assert!(manager.get(id_2).is_none());

        assert_eq!(manager.available, 2);
        assert_eq!(manager.next.dense, id_2.sparse);
        assert_eq!(manager.next.version, id_2.version + 1);
    }

    #[test]
    fn test_recycling() {
        let mut manager = ResourseManager::new();

        let id_1_v1 = manager.push(TestData(String::from("data 1 v1")));
        let id_2_v1 = manager.push(TestData(String::from("data 2 v1")));

        manager.pop(id_2_v1);
        manager.pop(id_1_v1);

        let id_1_v2 = manager.push(TestData(String::from("data 1 v2")));
        assert_ne!(id_1_v1, id_1_v2);

        assert_eq!(manager.available, 1);
        assert_eq!(manager.next.dense, id_2_v1.sparse);
        assert_eq!(manager.next.version, id_2_v1.version + 1);

        let id_2_v2 = manager.push(TestData(String::from("data 2 v2")));
        assert_ne!(id_2_v1, id_2_v2);

        assert_eq!(id_1_v2.sparse, 0);
        assert_eq!(id_1_v2.version, 1);

        assert_eq!(id_2_v2.sparse, 1);
        assert_eq!(id_2_v2.version, 1);

        let id_3_v1 = manager.push(TestData(String::from("data 3 v1")));
        assert_eq!(id_3_v1.sparse, 2);
        assert_eq!(id_3_v1.version, 0);

        manager.pop(id_2_v2);
        let id_2_v3 = manager.push(TestData(String::from("data 2 v3")));
        assert_ne!(id_2_v1, id_2_v3);
        assert_ne!(id_2_v2, id_2_v3);

        assert_eq!(id_2_v3.sparse, 1);
        assert_eq!(id_2_v3.version, 2);
    }
}