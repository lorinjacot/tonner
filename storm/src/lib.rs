mod buffer;
mod material;
mod math;
mod mesh;
mod resource;
mod scene;
mod storage;
mod texture;

use std::{fmt::Display, path::Path};

use resource::Resources;
use scene::SceneManager;
use texture::{EnvironmentMap, TextureManager};

pub use resource::{Asset, Res};
pub use scene::{Node, Scene};
pub use storage::Id;

pub struct Storm {
    resources: Resources,
    textures: TextureManager,
    scenes: SceneManager,
    pub active_scene: Option<Id<Scene>>,
}

impl Storm {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let textures = TextureManager::new(&device);
        let scenes = SceneManager::new(&device);

        Self {
            resources: Resources::new(device, queue),
            textures,
            scenes,
            active_scene: None,
        }
    }

    pub fn import_gltf<P>(&mut self, path: P) -> Result<&mut Res<Asset>, gltf::Error>
    where
        P: AsRef<Path>,
    {
        resource::import_gltf(path, &mut self.resources)
    }

    pub fn scenes(&self) -> std::slice::Iter<'_, (Id<Scene>, Scene)> {
        self.scenes.iter()
    }

    pub fn create_environment_map(
        &mut self,
        name: Option<&str>,
        equirectangular_map: image::DynamicImage,
        srgb: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<EnvironmentMap> {
        let equirectangular_map = self.textures.create_dynamic_image(
            name,
            &equirectangular_map,
            srgb,
            wgpu::TextureUsages::TEXTURE_BINDING,
            device,
            queue,
        );
        self.textures
            .create_environment_map(name, equirectangular_map)
    }

    pub fn environment_map(&self, id: Id<EnvironmentMap>) -> Option<&EnvironmentMap> {
        self.textures.environment_map(id)
    }

    pub fn environment_maps(&self) -> std::slice::Iter<'_, (Id<EnvironmentMap>, EnvironmentMap)> {
        self.textures.environment_maps()
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
