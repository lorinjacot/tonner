pub use asset::{environment, geometry, material, mesh};
pub use scene::{Scene, SceneBuilder};
pub use scene::{animation, camera, light, mesh_instance, node, scene_graph, skin};

use crate::asset::environment::EnvironmentContext;
use crate::asset::material::MaterialContext;
use crate::asset::mesh::MeshContext;
use crate::render_target::RenderTargetContext;
use crate::scene::SceneContext;
use crate::texture::TextureContex;

mod asset {
    pub mod environment;
    pub mod geometry;
    pub mod material;
    pub mod mesh;
}
pub mod gltf;
pub mod math;
pub mod render_target;
mod scene;
mod texture;

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
    scene_ctx: SceneContext,
    render_target_ctx: RenderTargetContext,
}

impl Context {
    /// Create the context using the provided [wgpu::Device] and [wgpu::Queue].
    pub fn from_device(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let mut encoder = device.create_command_encoder(&wgpu::wgt::CommandEncoderDescriptor {
            label: Some("storm::Context::from_device command encoder"),
        });

        let render_target_ctx = RenderTargetContext::new(&device);
        let scene_ctx = SceneContext::new(
            &render_target_ctx.brightness_bind_group_layout,
            &render_target_ctx.gaussian_blur_bind_group_layout,
            &device,
        );
        let texture_ctx = TextureContex::new(&device);
        let material_ctx = MaterialContext::new(&device);
        let mesh_ctx = MeshContext::new(&scene_ctx.render_bind_group_layout, &device);
        let environment_ctx = EnvironmentContext::new(&device, &mut encoder);

        queue.submit([encoder.finish()]);

        Self {
            device,
            queue,
            texture_ctx,
            material_ctx,
            mesh_ctx,
            environment_ctx,
            scene_ctx,
            render_target_ctx,
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
