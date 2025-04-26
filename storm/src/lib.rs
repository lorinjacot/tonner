pub use asset::Asset;
pub use math::Transform;
use mesh::Mesh;
pub use scene::{Node, Camera};
pub use scene::Scene;
use storage::SparseSet;
pub use storage::{DenseEntry, Id};

mod asset;
mod math;
mod mesh;
mod scene;
mod storage;

pub struct Storm {
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_texture_format: wgpu::TextureFormat,
    assets: SparseSet<Asset>,
    meshes: SparseSet<Mesh>,
    scenes: SparseSet<Scene>,
    scene: Option<Id<Scene>>,
    primitive_shader_module: wgpu::ShaderModule,
    render_bind_group_layout: wgpu::BindGroupLayout,
}

impl Storm {
    pub fn new(render_texture_format: wgpu::TextureFormat, device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let assets = SparseSet::new();
        let meshes = SparseSet::new();
        let scenes = SparseSet::new();

        let primitive_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("primitive.wgsl"));
        let render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        Self {
            device,
            queue,
            render_texture_format,
            assets,
            meshes,
            scenes,
            scene: None,
            primitive_shader_module,
            render_bind_group_layout,
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
}
