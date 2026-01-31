use std::{time::Duration, u32};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use thiserror::Error;

use crate::{
    Context,
    environment::{Environment, EnvironmentBuilder},
    mesh::MeshManager,
    scene::{light::LightManager, skin::SkinManager},
    scene_graph::SceneGraph,
};

pub mod camera;
pub mod light;
pub mod renderer;
pub mod scene_graph;
pub mod skin;

/// A scene describes a world. A scene can be evolve over time and can be rendered
/// to a screen or a texture.
///
/// A scene is made up of nodes. Nodes are organized in a parent-child hierachy, known as the
/// node-hierarchy or the scene graph. A node is called a root node when it doesn't have a parent.
/// Each node defines a local space. The local transform is used to get from the parent node local
/// space (parent space for short) to the local space. The global transform is used to get from the
/// scene space (or global space) to the local space. Both transforms are equal for root nodes.
///
/// To add an object to the scene, attach it to a node. For example, each node can have a mesh. During
/// rendering, the attached mesh will be rendered at the local space origin.
#[derive(Debug)]
pub struct Scene {
    pub name: String,
    pub scene_graph: SceneGraph,
    ctx: Context,
    skin_manager: SkinManager,
    pub(crate) mesh_manager: MeshManager,
    light_manager: LightManager,
    environment: Environment,
}

impl Scene {
    pub fn context(&self) -> &Context {
        &self.ctx
    }

    pub fn skin_manager(&self) -> &SkinManager {
        &self.skin_manager
    }

    pub fn mesh_manager(&self) -> &MeshManager {
        &self.mesh_manager
    }

    pub fn light_manager(&self) -> &LightManager {
        &self.light_manager
    }

    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    pub fn simulate(
        &mut self,
        _duration: Duration,
        _encoder: &mut wgpu::CommandEncoder,
    ) -> Result<(), SimulateError> {
        self.light_manager
            .update_point_light_buffer(&self.scene_graph, &self.ctx.device, &self.ctx.queue)
            .unwrap();

        self.skin_manager
            .update_buffer(&self.scene_graph, &self.ctx.device, &self.ctx.queue)
            .unwrap();

        self.mesh_manager
            .update_buffer(
                &self.scene_graph,
                &self.skin_manager,
                &self.ctx.device,
                &self.ctx.queue,
            )
            .unwrap();

        Ok(())
    }
}

/// A builder for [Scene].
#[must_use]
#[derive(Default)]
pub struct SceneBuilder {
    name: String,
    environment: Option<Environment>,
}

impl SceneBuilder {
    pub fn name(self, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..self
        }
    }

    pub fn environment(mut self, environment: impl Into<Environment>) -> Self {
        self.environment = Some(environment.into());
        self
    }

    pub fn build(self, ctx: &Context) -> Scene {
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::wgt::CommandEncoderDescriptor {
                label: Some("Engine builder encoder"),
            });

        let scene_graph = SceneGraph::new(ctx);
        let skin_manager = SkinManager::new(&ctx.device);
        let mesh_manager = MeshManager::new(&ctx.device);
        let light_manager = LightManager::new(&ctx.device);

        let environment = self
            .environment
            .unwrap_or_else(|| EnvironmentBuilder::default().build(ctx, &mut encoder));

        ctx.queue.submit([encoder.finish()]);

        Scene {
            name: self.name,
            ctx: ctx.clone(),
            scene_graph,
            skin_manager,
            mesh_manager,
            light_manager,
            environment,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct SceneContext {
    pub(super) render_bind_group_layout: wgpu::BindGroupLayout,
}

impl SceneContext {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render bind group layout"),
                entries: &[
                    // skins
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // camera
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // lights
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // irradiance map
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::Cube,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // prefilter map
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::Cube,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // BRDF LUT
                    wgpu::BindGroupLayoutEntry {
                        binding: 8,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 9,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        Self {
            render_bind_group_layout,
        }
    }
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct CameraUniform {
    view_projection: Mat4,
    view: Mat4,
    projection_inverse: Mat4,
    position: Vec3,
    _pad: u32,
}

#[derive(Debug, Error)]
pub enum SimulateError {}
