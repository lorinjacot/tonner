mod asset;
mod camera;
mod material;
mod mesh;
mod scene;
mod storage;
mod texture;
mod texture_old;

pub use asset::AssetManager;
pub use camera::{Controls, OrbitControls, PerspectiveCamera};
use material::MaterialManager;
pub use mesh::MeshManager;
pub use scene::{NodeId, Scene};
use texture::TextureManager;

pub struct Storm {
    assets: storage::SparseSet<Asset>,
    textures: TextureManager,
    materials: MaterialManager,
}

impl Storm {
    pub fn new(device: &wgpu::Device) -> Self {
        let mut textures = TextureManager::new();
        let materials = MaterialManager::new(&mut textures, device);

        Self {
            assets: storage::SparseSet::new(),
            textures,
            materials,
        }
    }
}

struct Asset {
    document: gltf::Document,
    buffers: Vec<gltf::buffer::Data>,
    images: Vec<gltf::image::Data>,
}
