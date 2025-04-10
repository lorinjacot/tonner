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
use material::MaterialManager;
use mesh::MeshManager;
use scene::SceneManager;
use storage::{Id, SparseSet};
use texture::TextureManager;

pub struct Storm {
    assets: SparseSet<Asset>,
    textures: TextureManager,
    materials: MaterialManager,
    buffers: BufferManager,
    meshes: MeshManager,
    scenes: SceneManager,
}

impl Storm {
    pub fn new(device: &wgpu::Device) -> Self {
        let assets = SparseSet::new();
        let mut textures = TextureManager::new();
        let materials = MaterialManager::new(&mut textures, device);
        let mut buffers = BufferManager::new();
        let meshes = MeshManager::new(&materials, &mut buffers, device);
        let scenes = SceneManager::new();

        Self {
            assets,
            textures,
            materials,
            buffers,
            meshes,
            scenes,
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

        // for mesh in self.assets[id].document.meshes() {
        //     self.meshes.load_mesh(
        //         id,
        //         mesh,
        //         &mut self.buffers,
        //         &mut self.textures,
        //         &mut self.materials,
        //         device,
        //         queue,
        //     );
        // }

        let scene = self.assets[id].document.scenes().next().unwrap();

        let _id = self.scenes.create_scene(
            id,
            scene,
            &mut self.buffers,
            &mut self.textures,
            &mut self.materials,
            &mut self.meshes,
            device,
            queue,
        );

        Ok(id)
    }
}

pub struct Asset {
    document: gltf::Document,
}
