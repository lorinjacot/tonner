pub use asset::Asset;
pub use math::Transform;
pub use scene::Node;
pub use scene::Scene;
use storage::SparseSet;
pub use storage::{DenseEntry, Id};

mod asset;
mod math;
mod scene;
mod storage;

pub struct Storm {
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_texture_format: wgpu::TextureFormat,
    assets: SparseSet<Asset>,
    scenes: SparseSet<Scene>,
    scene: Option<Id<Scene>>,
    primitive_shader_module: wgpu::ShaderModule,
}

impl Storm {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        render_texture_format: wgpu::TextureFormat,
    ) -> Self {
        let assets = SparseSet::new();
        let scenes = SparseSet::new();
        let primitive_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("primitive.wgsl"));

        Self {
            device,
            queue,
            render_texture_format,
            assets,
            scenes,
            scene: None,
            primitive_shader_module,
        }
    }

    pub fn scene(&self) -> Option<&Scene> {
        self.scene.map(|scene| &self.scenes[scene])
    }

    pub fn scene_mut(&mut self) -> Option<&mut Scene> {
        self.scene.map(|scene| &mut self.scenes[scene])
    }

    pub fn set_scene(&mut self, id: Option<Id<Scene>>) {
        self.scene = id;
    }

    pub fn scenes(&self) -> std::slice::Iter<'_, Scene> {
        self.scenes.iter()
    }

    pub fn update(&mut self, _aspect_ration: f32) {}

    pub fn render(&self, _render_pass: &mut wgpu::RenderPass) {}
}
