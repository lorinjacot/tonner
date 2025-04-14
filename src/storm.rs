mod buffer;
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
use storage::SparseSet;
use texture::TextureManager;

pub use scene::{Node, Scene};
pub use storage::Id;

pub struct Storm {
    assets: SparseSet<Asset>,
    textures: TextureManager,
    materials: MaterialManager,
    buffers: BufferManager,
    meshes: MeshManager,
    pub scenes: SceneManager,
    pub active_scene: Option<Id<Scene>>,
}

impl Storm {
    pub fn new(
        render_format: wgpu::TextureFormat,
        device: &wgpu::Device,
    ) -> Self {
        let assets = SparseSet::new();
        let mut textures = TextureManager::new();
        let materials = MaterialManager::new(&mut textures, device);
        let mut buffers = BufferManager::new();
        let meshes = MeshManager::new(&materials, &mut buffers, render_format, device);
        let scenes = SceneManager::new();

        Self {
            assets,
            textures,
            materials,
            buffers,
            meshes,
            scenes,
            active_scene: None,
        }
    }

    pub fn load_asset(
        &mut self,
        path: impl AsRef<Path>,
        viewport_aspect_ratio: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
    ) -> Result<Id<Asset>, gltf::Error> {
        let (document, buffers, images) = gltf::import(path)?;

        let id = self.assets.push(Asset { document });
        self.buffers.register_asset(id, buffers);
        self.textures.register_asset(id, images);

        let document = &self.assets[id].document;
        for scene in document.scenes() {
            self.scenes.load_scene(
                id,
                scene,
                viewport_aspect_ratio,
                &mut self.buffers,
                &mut self.textures,
                &mut self.materials,
                &mut self.meshes,
                device,
                queue,
            );
        }

        self.active_scene = document.default_scene().map(|scene| {
            self.scenes.load_scene(
                id,
                scene,
                viewport_aspect_ratio,
                &mut self.buffers,
                &mut self.textures,
                &mut self.materials,
                &mut self.meshes,
                device,
                queue,
            )
        });

        Ok(id)
    }

    pub fn update(&mut self, _device: &wgpu::Device, queue: &wgpu::Queue) {
        if let Some(scene) = self.active_scene {
            if let Some(scene) = self.scenes.get_mut(scene) {
                scene.update(queue);
            }
        }
    }

    pub fn render(&self, device: &wgpu::Device, render_pass: &mut wgpu::RenderPass) {
        if let Some(scene) = self.active_scene {
            self.scenes[scene].render(device, render_pass);
        }
    }

    pub fn active_scene(&self) -> Option<&Scene> {
        self.scenes.get(self.active_scene?)
    }
}

pub struct Asset {
    document: gltf::Document,
}
