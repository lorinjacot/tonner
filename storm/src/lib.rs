pub use math::Transform;
pub use scene::{Scene, SceneBuilder};
pub use scene::{camera, skin};

pub use asset::{environment, geometry, material, mesh};

use environment::EnvironmentManager;
use geometry::GeometryManager;
use material::MaterialManager;
use mesh::MeshManager;
use texture::TextureBuilderData;

mod asset {
    pub mod environment;
    pub mod geometry;
    pub mod material;
    pub mod mesh;
}
// mod gltf;
pub mod math;
mod scene;
mod texture;

/// This is the entry point of the crate.
/// To get started, create a new [Engine] using [EngineBuilder].
/// Once created, an engine can be used to create a [Scene].
/// The engine is also responsible to manage the resources shared between [Scene]s.
pub struct Engine {
    device: wgpu::Device,
    queue: wgpu::Queue,
    geometry_manager: GeometryManager,
    material_manager: MaterialManager,
    mesh_manager: MeshManager,
    texture_builder_data: TextureBuilderData,
    environment_manager: EnvironmentManager,
}

impl Engine {
    /// Create an engine builder with default values.
    pub fn builder<'a>() -> EngineBuilder<'a> {
        EngineBuilder::default()
    }
}

/// A builder for [Engine].
#[must_use]
pub struct EngineBuilder<'a> {
    device: Option<(wgpu::Device, wgpu::Queue)>,
    compatible_surface: Option<&'a wgpu::Surface<'a>>,
    target_format: wgpu::TextureFormat,
}

impl<'a> Default for EngineBuilder<'a> {
    fn default() -> Self {
        Self {
            device: None,
            compatible_surface: None,
            target_format: wgpu::TextureFormat::Rgba8UnormSrgb,
        }
    }
}

impl<'a> EngineBuilder<'a> {
    /// Use an existing [wgpu::Device] and [wgpu::Queue].
    pub fn device(mut self, device: wgpu::Device, queue: wgpu::Queue) -> Self {
        self.device = Some((device, queue));
        self
    }

    /// Ensure the engine is compatible with this surface.
    pub fn compatible_surface(mut self, surface: &'a wgpu::Surface<'a>) -> Self {
        self.compatible_surface = Some(surface);
        self
    }

    /// Change the [wgpu::TextureFormat] of the rendering target.
    /// This setting controls the encoding of the rendered [Scene]s.
    pub fn target_format(mut self, target_format: wgpu::TextureFormat) -> Self {
        self.target_format = target_format;
        self
    }

    /// Build the [Engine].
    pub async fn build(self) -> Engine {
        let (device, queue) = match self.device {
            Some(device) => device,
            None => {
                let instance =
                    wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
                let adapter = instance
                    .request_adapter(&wgpu::RequestAdapterOptions::default())
                    .await
                    .expect("Failed to get wgpu adapter");
                adapter
                    .request_device(&wgpu::wgt::DeviceDescriptor::default())
                    .await
                    .expect("Failed to get wgpu device")
            }
        };

        let mut encoder = device.create_command_encoder(&wgpu::wgt::CommandEncoderDescriptor {
            label: Some("Engine builder command encoder"),
        });

        let geometry_manager = GeometryManager::new();
        let material_manager = MaterialManager::new(&device);
        let mesh_manager = MeshManager::new(&device);
        let texture_builder_data = TextureBuilderData::new(&device);
        let environment_manager = EnvironmentManager::new(&device, &mut encoder);

        let engine = Engine {
            device,
            queue,
            geometry_manager,
            material_manager,
            mesh_manager,
            texture_builder_data,
            environment_manager,
        };

        engine.queue.submit([encoder.finish()]);

        engine
    }
}
