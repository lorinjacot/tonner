#[cfg(feature = "python")]
use pyo3::prelude::*;
#[cfg(feature = "python")]
use pyo3::types::PyType;
#[cfg(feature = "python")]
use uuid::Uuid;
use uuid::uuid;

use crate::environment::EnvironmentContext;
use crate::mesh::MeshContext;
use crate::mesh::material::MaterialContext;
use crate::renderer::RendererContext;
use crate::texture::TextureContex;
use crate::world::StaticField;

pub mod entity_component;
pub mod environment;
pub mod geometry;
pub mod math;
pub mod mesh;
pub mod renderer;
pub mod scene_graph;
pub mod texture;
pub mod world;

/// Contains everything long-lived and shared by the engine.
/// This is the first thing you need when using storm.
///
/// [Context] can be cloned, and any clone refers to the same data.
/// In general, two objects created with different contexts cannot be used together.
///
/// Contains:
/// - bind group layouts
/// - pipeline layouts
/// - shader modules
/// - pipelines
/// - default buffers, textures, samplers
/// - ...
#[cfg_attr(feature = "python", pyclass(frozen, skip_from_py_object, extends=world::WorldField))]
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
    /// Creates a new context using one of the following GPU api:
    /// - VULKAN
    /// - METAL
    /// - DX12
    /// - BROWSER_WEBGPU
    ///
    /// ## Panics
    ///
    /// This method will panic if it fails to get a GPU adapter or handle.
    pub async fn new() -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: wgpu::InstanceFlags::from_env_or_default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::from_env_or_default(),
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptionsBase {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .expect("Failed to get GPU adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::defaults(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .expect("Failed to get GPU handle");

        Self::from_device(device, queue)
    }

    /// Create the context using the provided [wgpu::Device] and [wgpu::Queue].
    pub fn from_device(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let mut encoder = device.create_command_encoder(&wgpu::wgt::CommandEncoderDescriptor {
            label: Some("tonner::Context::from_device command encoder"),
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

impl StaticField for Context {
    const ID: world::FieldId = uuid!("386ed978-e1c9-4870-83ac-cdb4e6fbf8bc");

    #[cfg(feature = "python")]
    fn as_py<'py>(&self, py: Python<'py>) -> Option<Bound<'py, world::WorldField>> {
        Some(
            Bound::new(py, (self.clone(), world::WorldField))
                .unwrap()
                .into_super(),
        )
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl Context {
    #[new]
    fn py_new() -> (Context, world::WorldField) {
        (pollster::block_on(Context::new()), world::WorldField)
    }

    #[classmethod]
    fn id(_cls: &Bound<'_, PyType>) -> PyResult<Uuid> {
        Ok(Self::ID)
    }
}

#[cfg(feature = "python")]
#[pyo3::pymodule(name = "tonner")]
mod tonner_py {
    use super::*;

    #[pymodule_export]
    use Context;

    #[pymodule_export]
    use world::{PyWorld, WorldField};

    #[pymodule_export]
    use entity_component::entity::Entity;
}
