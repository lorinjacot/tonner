use std::{
    collections::HashMap,
    ops::DerefMut,
    sync::{Arc, Mutex},
};

use uuid::Uuid;

use crate::{
    asset::geometry::{Geometry, GeometryIndices},
    material::AlphaMode,
};

/// A unique id for a [mesh][Mesh]. A mesh will always have the same id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshId(Uuid);

/// A mesh describe a 3D object. It wraps a [Geometry] with a [Material].
#[derive(Clone)]
pub struct Mesh(Arc<MeshData>);

impl Mesh {
    /// Returns the mesh id. The id will never change.
    pub fn id(&self) -> MeshId {
        self.0.id
    }

    /// User-provided name.
    ///
    /// This method will block the current thread until it is able to acquire the name.
    /// When the returned value goes out of scope, the name is released, allowing other
    /// threads to aquire it.
    ///
    /// # Panics
    /// This function might panic when called if the name is already acquired by the current thread.
    pub fn name(&self) -> impl DerefMut<Target = String> {
        self.0.name.lock().unwrap_or_else(|err| {
            let mut inner = err.into_inner();
            *inner = String::new();
            inner
        })
    }

    /// Returns the number of morph target. A morph target is used to deform the mesh based on some
    /// scalar coefficients, called `weights`.
    pub fn morph_target_count(&self) -> usize {
        todo!()
    }

    /// The primitives that are part of this mesh. A primitive is a [`Geometry`] and [`Material`] pair and
    /// describe the shape and material (part) of the mesh.
    pub fn primitives(&self) -> &[MeshPrimitive] {
        todo!()
    }
}

/// Data contained in a [Mesh]. Private to this module.
struct MeshData {
    /// Unique id for the mesh. Will never change.
    id: MeshId,

    /// User-provided name.
    name: Mutex<String>,
}

/// A primitive is a [`Geometry`], [`Material`] pair. A [`Mesh`] is described as a list of primitives.
pub struct MeshPrimitive {
    geometry: Geometry,
}

impl MeshPrimitive {
    /// Returns the render pipelines. The first should be used when the model matrix has a positive determinant,
    /// and the second one is for negative determinant.
    ///
    /// TODO: add expected buffer & bind groups & render attachments.
    pub fn render_pipeline(&self) -> (&wgpu::RenderPipeline, &wgpu::RenderPipeline) {
        todo!()
    }

    /// Returns the geometry vertex buffer.
    pub fn geomery_vertex_buffer(&self) -> &wgpu::Buffer {
        self.geometry.vertex_buffer()
    }

    /// Returns the material bind group. [`Self::render_pipeline`] expects this bind group
    /// at index 2.
    pub fn material_bind_group(&self) -> &wgpu::BindGroup {
        todo!()
    }

    /// Describe how to interpret the `alpha` channel of the rendered primitive.
    pub fn alpha_mode(&self) -> AlphaMode {
        todo!()
    }

    /// Return indices data if the primitive has some. Indices are a way to use the same
    /// geometry vertix in multiple triangles.
    pub fn indices(&self) -> &Option<GeometryIndices> {
        self.geometry.indices()
    }

    /// The number of vertices that describe the primitive geometry. If th geometry is indexed,
    /// this number is usually smaller than the index count.
    pub fn vertex_count(&self) -> usize {
        self.geometry.vertex_count()
    }
}

/// A container for all [meshes][Mesh]. This type is used to create, query and delete meshes.
pub(crate) struct MeshManager {
    meshes: HashMap<MeshId, Mesh>,
}
