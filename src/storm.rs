mod buffer;
mod camera;
mod material;
mod mesh;
mod mesh_old;
mod scene;
mod storage;
mod texture;
mod texture_old;

use buffer::BufferManager;
pub use camera::{Controls, OrbitControls, PerspectiveCamera};
use material::MaterialManager;
pub use mesh_old::MeshManager;
pub use scene::{NodeId, Scene};
use texture::TextureManager;

pub struct Storm {
    assets: storage::SparseSet<Asset>,
    textures: TextureManager,
    materials: MaterialManager,
    buffers: BufferManager,
    meshes: mesh::MeshManager,
}

impl Storm {
    pub fn new(device: &wgpu::Device) -> Self {
        let mut textures = TextureManager::new();
        let materials = MaterialManager::new(&mut textures, device);
        let buffers = BufferManager::new();
        let meshes = mesh::MeshManager::new(device);

        Self {
            assets: storage::SparseSet::new(),
            textures,
            materials,
            buffers,
            meshes,
        }
    }
}

struct Asset {
    document: gltf::Document,
    buffers: Vec<gltf::buffer::Data>,
    images: Vec<gltf::image::Data>,
}
