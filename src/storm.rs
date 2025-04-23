mod buffer;
mod material;
mod mesh;
mod scene;
mod storage;
mod texture;
mod math;

use std::{fmt::Display, path::Path};

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
    pub fn new(render_format: wgpu::TextureFormat, device: &wgpu::Device) -> Self {
        let assets = SparseSet::new();
        let mut textures = TextureManager::new();
        let materials = MaterialManager::new(&mut textures, device);
        let mut buffers = BufferManager::new();
        let scenes = SceneManager::new(device);
        let meshes = MeshManager::new(
            scenes.camera_bind_group_layout(),
            &materials,
            &mut buffers,
            render_format,
            device,
        );

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
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
    ) -> Result<Id<Asset>, gltf::Error> {
        let (document, buffers, images) = gltf::import(path)?;

        let scenes = Vec::with_capacity(document.scenes().len());
        let id = self.assets.push(Asset { document, scenes });
        self.buffers.register_asset(id, buffers);
        self.textures.register_asset(id, images);

        let asset = &mut self.assets[id];
        for scene in asset.document.scenes() {
            asset.scenes.push(self.scenes.load_scene(
                id,
                scene,
                &mut self.buffers,
                &mut self.textures,
                &mut self.materials,
                &mut self.meshes,
                device,
                queue,
            ));
        }

        self.active_scene = asset
            .document
            .default_scene()
            .map(|scene| asset.scenes[scene.index()]);

        Ok(id)
    }

    pub fn update(&mut self, viewport_aspect_ratio: f32, queue: &wgpu::Queue) {
        if let Some(scene) = self.active_scene {
            self.scenes[scene].update(viewport_aspect_ratio, queue);
        }
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass) {
        if let Some(scene) = self.active_scene {
            self.scenes[scene].render(render_pass);
        }
    }

    pub fn active_scene(&self) -> Option<&Scene> {
        self.scenes.get(self.active_scene?)
    }

    pub fn active_scene_mut(&mut self) -> Option<&mut Scene> {
        self.scenes.get_mut(self.active_scene?)
    }
}

pub struct Asset {
    document: gltf::Document,
    scenes: Vec<Id<Scene>>,
}

#[derive(Clone)]
pub struct Name(pub String);

impl Name {
    fn from_name_or_else<F, T>(default: F, name: Option<&str>) -> Self
    where
        F: FnOnce() -> T,
        T: ToString,
    {
        Self(name.map_or_else(|| default().to_string(), |name| name.to_string()))
    }
}

impl Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
