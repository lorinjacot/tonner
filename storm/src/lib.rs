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
}

impl Storm {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        render_texture_format: wgpu::TextureFormat,
    ) -> Self {
        let assets = SparseSet::new();
        let scenes = SparseSet::new();

        Self {
            device,
            queue,
            render_texture_format,
            assets,
            scenes,
            active_scene: None,
        }
    }

    pub fn load_gltf(&mut self, _path: impl AsRef<std::path::Path>) -> Result<Asset, gltf::Error> {
        todo!()
    }

    pub fn update(&mut self, _aspect_ration: f32) {}

    pub fn render(&self, _render_pass: &mut wgpu::RenderPass) {}
}
