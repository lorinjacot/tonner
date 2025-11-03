use std::{
    collections::HashMap,
    ops::DerefMut,
    sync::{Arc, Mutex},
};

use uuid::Uuid;

/// A unique id for a [mesh][Mesh]. A mesh will always have the same id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshId(Uuid);

/// A mesh describe a 3D object. It wraps a [Geometry] with a [Material].
pub struct Mesh(Arc<MeshData>);

impl Mesh {
    /// Returns the mesh id. The id will never change.
    pub fn id(&self) -> MeshId {
        self.0.id
    }

    /// Optional user-provided name.
    ///
    /// This method will block the current thread until it is able to acquire the name.
    /// When the returned value goes out of scope, the name is released, allowing other
    /// threads to aquire it.
    ///
    /// # Panics
    /// This function might panic when called if the name is already acquired by the current thread.
    pub fn name(&self) -> impl DerefMut<Target = Option<String>> {
        self.0.name.lock().unwrap_or_else(|err| {
            let mut inner = err.into_inner();
            *inner = None;
            inner
        })
    }
}

/// Data contained in a [Mesh]. Private to this module.
struct MeshData {
    /// Unique id for the mesh. Will never change.
    id: MeshId,

    /// Optional user-provided name.
    name: Mutex<Option<String>>,
}

/// A container for all [meshes][Mesh]. This type is used to create, query and delete meshes.
pub struct MeshManager {
    meshes: HashMap<MeshId, MeshData>,
}
