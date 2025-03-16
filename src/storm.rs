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
pub use material::MaterialManager;
pub use mesh::MeshManager;
pub use scene::{NodeId, Scene};
use storage::SparseSet;

pub struct Storm {
    assets: SparseSet<Asset>,
    textures: texture::TextureManager,
}

struct Asset {
    document: gltf::Document,
    buffers: Vec<gltf::buffer::Data>,
    images: Vec<gltf::image::Data>,
}
