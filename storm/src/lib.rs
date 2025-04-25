pub use asset::Asset;
use scene::Scene;
pub use storage::Id;
use storage::SparseSet;

mod asset;
mod scene;
mod storage;

pub struct Storm {
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_texture_format: wgpu::TextureFormat,
    assets: SparseSet<Asset>,
    scenes: SparseSet<Scene>,
    active_scene: Option<Id<Scene>>,
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
            active_scene: None,
            primitive_shader_module,
        }
    }

    pub fn update(&mut self, _aspect_ration: f32) {}

    pub fn render(&self, _render_pass: &mut wgpu::RenderPass) {}
}
