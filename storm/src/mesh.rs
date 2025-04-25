use crate::{DenseEntry, Id, storage::SetEntry};

pub struct Mesh {
    id: Id<Mesh>,
    name: String,
    pub(super) primitives: Vec<Primitive>,
}

pub struct MeshDescriptor {
    pub(super) name: Option<String>,
    pub(super) primitives: Vec<Primitive>,
}

impl DenseEntry for Mesh {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

impl SetEntry for Mesh {
    type Descriptor = MeshDescriptor;

    fn new(id: Id<Self::Key>, desc: Self::Descriptor) -> Self {
        Self {
            id,
            name: desc.name.unwrap_or_else(|| id.to_string()),
            primitives: desc.primitives,
        }
    }
}

#[derive(Clone)]
pub struct Primitive {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) index_buffer: Option<IndexBuffer>,
    pub(super) vertex_buffers: Vec<wgpu::Buffer>,
    pub(super) vertex_count: u32,
}

#[derive(Debug, Clone)]
pub(super) struct IndexBuffer {
    pub(super) buffer: wgpu::Buffer,
    pub(super) offset: u64,
    pub(super) format: wgpu::IndexFormat,
}
