use std::{
    ops::Range,
    sync::{Arc, RwLock},
};

use crate::{DenseEntry, Id, storage::SetEntry};

#[derive(Clone)]
pub struct Mesh {
    id: Id<Mesh>,
    name: Arc<RwLock<String>>,
    primitives: Arc<Vec<Primitive>>,
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
            name: Arc::new(RwLock::new(desc.name.unwrap_or_else(|| id.to_string()))),
            primitives: Arc::new(desc.primitives),
        }
    }
}

#[derive(Clone)]
pub struct Primitive {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) index_buffer: Option<IndexBuffer>,
    pub(super) vertex_buffers: Vec<wgpu::Buffer>,
}

#[derive(Debug, Clone)]
pub(super) struct IndexBuffer {
    pub(super) buffer: wgpu::Buffer,
    pub(super) bounds: Range<u64>,
    pub(super) format: wgpu::IndexFormat,
}
