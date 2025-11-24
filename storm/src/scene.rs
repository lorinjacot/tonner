use std::u32;

use bytemuck::{Pod, Zeroable, bytes_of};
use glam::{Mat4, Vec3, usize};
use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::{
    Engine,
    environment::Environment,
    scene::{
        animation::AnimationManager,
        camera::{CameraId, CameraManager},
        light::LightManager,
        mesh_instance::MeshInstanceManager,
        node::NodeManager,
        skin::SkinManager,
    },
};

pub mod animation;
pub mod camera;
mod light;
mod mesh_instance;
mod node;
pub mod skin;

/// A scene describes a world. A scene can be evolve over time and can be rendered
/// to a screen or a texture.
///
/// A scene is made up of [Node]s. Nodes are organized in a parent-child hierachy, known as the
/// node-hierarchy or the scene graph. A node is called a root node when it doesn't have a parent.
/// Each node defines a local space. The local transform is used to get from the parent node local
/// space (parent space for short) to the local space. The global transform is used to get from the
/// scene space (or global space) to the local space. Both transforms are equal for root nodes.
///
/// To add an object to the scene, attach it to a node. For example, each node can have a mesh. During
/// rendering, the attached mesh will be rendered at the local space origin.
pub struct Scene {
    pub name: String,
    device: wgpu::Device,
    queue: wgpu::Queue,
    node_manager: NodeManager,
    skin_manager: SkinManager,
    camera_manager: CameraManager,
    mesh_instance_manager: MeshInstanceManager,
    animation_manager: AnimationManager,
    light_manager: LightManager,
    environment: Environment,
    render_bind_group_layout: wgpu::BindGroupLayout,
    skybox_bind_group_layout: wgpu::BindGroupLayout,
    skybox_pipeline: wgpu::RenderPipeline,
    brightness_pipeline: wgpu::RenderPipeline,
    gaussian_blur_pipeline: wgpu::RenderPipeline,
    bloom_amount: usize,
}

impl Scene {
    pub fn render(
        &self,
        target: &RenderTarget,
        camera: CameraId,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<(), RenderError> {
        let viewport_aspect_ratio = target.aspect_ratio();

        let projection_matrix = self
            .camera_manager
            .projection_matrix(camera, viewport_aspect_ratio)
            .ok_or(RenderError::InvalidCamera(camera))?;
        let camera_node = self.camera_manager.node(camera).unwrap();
        let camera_matrix = self
            .node_manager
            .global_matrix(camera_node)
            .ok_or(RenderError::InvalidCamera(camera))?;
        let camera_position = camera_matrix.transform_point3(Vec3::ZERO);

        let view_matrix = Mat4::look_to_rh(
            camera_position,
            camera_matrix.transform_vector3(-Vec3::Z),
            camera_matrix.transform_vector3(Vec3::Y),
        );
        let view_projection = projection_matrix * view_matrix;
        let camera_uniform = CameraUniform {
            view_projection,
            view: view_matrix,
            projection_inverse: projection_matrix.inverse(),
            position: camera_position,
            _pad: 0,
        };
        let camera_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Camera buffer"),
                contents: bytes_of(&camera_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let render_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{} render bind group", self.name)),
            layout: &self.render_bind_group_layout,
            entries: &[
                // nodes
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.node_manager.buffer().as_entire_binding(),
                },
                // skins
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.skin_manager.buffer().as_entire_binding(),
                },
                // camera
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: camera_buffer.as_entire_binding(),
                },
                // lights
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.light_manager.point_light_buffer().as_entire_binding(),
                },
                // irradiance map
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(
                        &self.environment.irradiance_map_view(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(
                        &self.environment.irradiance_map_sampler(),
                    ),
                },
                // prefilter map
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(
                        &self.environment.prefilter_map_view(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(
                        &self.environment.prefilter_map_sampler(),
                    ),
                },
                // BRDF LUT
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&self.environment.brdf_lut_view()),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&self.environment.brdf_lut_sampler()),
                },
            ],
        });

        let mut primitive_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Opaque render pass"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: &target.opaque_attachment,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &target.accumulation_attachment,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &target.revealage_attachment,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                        store: wgpu::StoreOp::Store,
                    },
                }),
            ],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &target.depth_attachment,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        primitive_render_pass.set_bind_group(0, &render_bind_group, &[]);

        self.mesh_instance_manager
            .render_opaque_primitives(&mut primitive_render_pass);
        self.mesh_instance_manager
            .render_transparent_primitives(&mut primitive_render_pass);
        drop(primitive_render_pass);

        let mut brightness_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Brightness render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.bloom_textures[0].0,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        brightness_render_pass.set_pipeline(&self.brightness_pipeline);
        brightness_render_pass.set_bind_group(0, &target.brightness_bind_group, &[]);
        brightness_render_pass.draw(0..3, 0..1);
        drop(brightness_render_pass);

        let mut horizontal = false;
        for _ in 0..self.bloom_amount {
            let source = &target.bloom_textures[horizontal as usize].1;
            horizontal = !horizontal;
            let target = &target.bloom_textures[horizontal as usize].0;
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Gaussian blur render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render_pass.set_pipeline(&self.gaussian_blur_pipeline);
            render_pass.set_bind_group(0, source, &[]);
            render_pass.draw(0..3, 0..1);
        }

        let mut tone_mapping_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Tone mapping render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.render_texture_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        tone_mapping_render_pass.set_pipeline(&target.tone_mapping_pipeline);
        tone_mapping_render_pass.set_bind_group(0, &target.tone_mapping_bind_group, &[]);
        tone_mapping_render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

/// A builder for [Scene].
#[must_use]
#[derive(Default)]
pub struct SceneBuilder {
    name: String,
}

impl SceneBuilder {
    pub fn name(self, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..self
        }
    }

    pub fn build(self, engine: &mut Engine) -> Scene {
        let device = engine.device.clone();
        let queue = engine.queue.clone();

        let node_manager = NodeManager::new(&device);
        let skin_manager = SkinManager::new(&device);
        let camera_manager = CameraManager::new();
        let mesh_instance_manager = MeshInstanceManager::new(&device);
        let animation_manager = AnimationManager::new();
        let light_manager = LightManager::new(&device);

        let environment = engine.environment_manager.default();

        let render_bind_group_layout = engine.render_bind_group_layout.clone();
        let skybox_bind_group_layout = engine.skybox_bind_group_layout.clone();

        let skybox_pipeline = engine.skybox_pipeline.clone();
        let brightness_pipeline = engine.brightness_pipeline.clone();
        let gaussian_blur_pipeline = engine.gaussian_blur_pipeline.clone();

        Scene {
            name: self.name,
            device,
            queue,
            node_manager,
            skin_manager,
            camera_manager,
            mesh_instance_manager,
            animation_manager,
            light_manager,
            environment,
            render_bind_group_layout,
            skybox_bind_group_layout,
            skybox_pipeline,
            brightness_pipeline,
            gaussian_blur_pipeline,
            bloom_amount: 10,
        }
    }
}

pub struct RenderTarget {
    render_texture_view: wgpu::TextureView,
    opaque_attachment: wgpu::TextureView,
    accumulation_attachment: wgpu::TextureView,
    revealage_attachment: wgpu::TextureView,
    depth_attachment: wgpu::TextureView,
    compose_bind_group: wgpu::BindGroup,
    brightness_bind_group: wgpu::BindGroup,
    bloom_textures: [(wgpu::TextureView, wgpu::BindGroup); 2],
    tone_mapping_bind_group: wgpu::BindGroup,
    tone_mapping_pipeline: wgpu::RenderPipeline,
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

impl RenderTarget {
    fn aspect_ratio(&self) -> f32 {
        self.render_texture_view.texture().width() as f32
            / self.render_texture_view.texture().height() as f32
    }
}

#[derive(Debug, Error)]
pub enum SimulateError {}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("invalid camera: {0}")]
    InvalidCamera(CameraId),
}
