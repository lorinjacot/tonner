mod buffer;
mod camera;
mod material;
mod mesh;
mod mesh_old;
mod scene;
mod storage;
mod texture;
mod texture_old;

use std::path::Path;

use buffer::BufferManager;
pub use camera::{Controls, OrbitControls, PerspectiveCamera};
use material::MaterialManager;
use mesh::MeshManager;
pub use scene::{NodeId, Scene};
use storage::{Id, SparseSet};
use texture::TextureManager;

pub struct Storm {
    assets: SparseSet<Asset>,
    textures: TextureManager,
    materials: MaterialManager,
    buffers: BufferManager,
    meshes: MeshManager,
}

impl Storm {
    pub fn new(device: &wgpu::Device) -> Self {
        let assets = SparseSet::new();
        let textures = TextureManager::new();
        let materials = MaterialManager::new();
        let buffers = BufferManager::new();
        let meshes = MeshManager::new(device, &materials);

        Self {
            assets,
            textures,
            materials,
            buffers,
            meshes,
        }
    }

    pub fn open_asset(
        &mut self,
        path: impl AsRef<Path>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
    ) -> Result<Id<Asset>, gltf::Error> {
        let (document, buffers, images) = gltf::import(path)?;

        let id = self.assets.push(Asset { document });
        self.buffers.register_asset(id, buffers);
        self.textures.register_asset(id, images);

        for mesh in self.assets[id].document.meshes() {
            self.meshes.load_mesh(
                id,
                mesh,
                &mut self.buffers,
                &mut self.textures,
                &mut self.materials,
                device,
                queue,
            );
        }

        Ok(id)
    }
}

pub struct Asset {
    document: gltf::Document,
}
