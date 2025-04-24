use crate::storage::{Id, Storage};

use super::material::Material;

type MaterialId = Id<Material>;

pub struct MeshManager {
    meshes: Storage<Mesh>,
}

impl MeshManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let meshes = Storage::new();

        Self { meshes }
    }

    pub fn builder<'a>(&'a mut self, name: Option<&'a str>) -> MeshBuilder<'a> {
        MeshBuilder {
            manager: self,
            label: name,
            primitives: Vec::new(),
        }
    }
}

pub type MeshId = Id<Mesh>;
pub struct Mesh {
    primitives: Vec<Primitive>,
}

pub struct MeshBuilder<'a> {
    manager: &'a mut MeshManager,
    label: Option<&'a str>,
    primitives: Vec<Primitive>,
}

impl<'a> MeshBuilder<'a> {
    pub fn add_primitive<F>(
        &mut self,
        primitive: &gltf::Primitive,
        get_buffer_data: F,
        material: MaterialId,
    ) where
        F: Clone + Fn(gltf::Buffer) -> Option<&[u8]>,
    {
        todo!()
    }

    pub fn build(self) -> MeshId {
        self.manager.meshes.add(Mesh {
            primitives: self.primitives,
        })
    }
}

struct Primitive {
    vertex_count: u32,
    attributes: wgpu::Buffer,
    material: MaterialId,
    indices: Option<(wgpu::Buffer, wgpu::IndexFormat)>,
}
