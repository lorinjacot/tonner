use crate::environment::EnvironmentContext;
use crate::mesh::MeshContext;
use crate::mesh::material::MaterialContext;
use crate::renderer::RendererContext;
use crate::texture::TextureContex;

pub mod environment;
pub mod geometry;
pub mod math;
pub mod mesh;
pub mod renderer;
pub mod scene_graph;
pub mod texture;

/// Contains everything long-lived and shared by the engine.
/// This is the first thing you need when using storm.
///
/// [Context] is cheap to clone, and any clone refers to the same data.
/// In general, two objects created with different contexts cannot be used together.
///
/// Contains:
/// - bind group layouts
/// - pipeline layouts
/// - shader modules
/// - pipelines
/// - default buffers, textures, samplers
/// - ...
#[derive(Debug, Clone)]
pub struct Context {
    device: wgpu::Device,
    queue: wgpu::Queue,

    texture_ctx: TextureContex,
    material_ctx: MaterialContext,
    mesh_ctx: MeshContext,
    environment_ctx: EnvironmentContext,
    renderer_ctx: RendererContext,
}

impl Context {
    /// Create the context using the provided [wgpu::Device] and [wgpu::Queue].
    pub fn from_device(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let mut encoder = device.create_command_encoder(&wgpu::wgt::CommandEncoderDescriptor {
            label: Some("storm::Context::from_device command encoder"),
        });

        let texture_ctx = TextureContex::new(&device);
        let material_ctx = MaterialContext::new(&device);
        let environment_ctx = EnvironmentContext::new(&device, &mut encoder);
        let renderer_ctx = RendererContext::new(&device, &environment_ctx.skybox_bind_group_layout);
        let mesh_ctx = MeshContext::new(&renderer_ctx.render_bind_group_layout, &device);

        queue.submit([encoder.finish()]);

        Self {
            device,
            queue,
            texture_ctx,
            material_ctx,
            mesh_ctx,
            environment_ctx,
            renderer_ctx,
        }
    }

    /// The GPU used by the engine.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The GPU command queue used by the engine.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}
