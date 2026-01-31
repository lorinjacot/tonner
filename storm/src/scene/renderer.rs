use std::iter::repeat_with;

use crate::{
    Context,
    camera::Camera,
    environment::Environment,
    scene::{LightManager, MeshManager},
    scene_graph::{NodeId, SceneGraph},
    skin::SkinManager,
    texture::TextureBuilder,
};
use bytemuck::{Pod, Zeroable, bytes_of};
use glam::{Mat4, Vec3};
use log::warn;
use thiserror::Error;
use wgpu::util::DeviceExt;

pub struct Renderer {
    render_bind_group_layout: wgpu::BindGroupLayout,
    brightness_pipeline: wgpu::RenderPipeline,
    gaussian_blur_pipeline: wgpu::RenderPipeline,
    bloom_amount: usize,
    opaque_attachment: wgpu::TextureView,
    accumulation_attachment: wgpu::TextureView,
    revealage_attachment: wgpu::TextureView,
    depth_attachment: wgpu::TextureView,
    brightness_bind_group: wgpu::BindGroup,
    bloom_textures: [(wgpu::TextureView, wgpu::BindGroup); 2],
    tone_mapping_bind_group: wgpu::BindGroup,
    tone_mapping_pipeline: wgpu::RenderPipeline,
}

impl Renderer {
    /// Create a new builder. If possible, a builder should be reused, as calling
    /// [`RenderTargetBuilder::new`] is recreating multiple [`wgpu::Texture`], [`wgpu::BindGroup`] and [`wgpu::RenderPipeline`].
    pub fn new(width: u32, height: u32, format: wgpu::TextureFormat, ctx: &Context) -> Self {
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("RenderTarget::new() command encoder"),
            });

        let render_bind_group_layout = ctx.renderer_ctx.render_bind_group_layout.clone();
        let brightness_pipeline = ctx.renderer_ctx.brightness_pipeline.clone();
        let gaussian_blur_pipeline = ctx.renderer_ctx.gaussian_blur_pipeline.clone();

        let (
            [
                opaque_attachment,
                accumulation_attachment,
                revealage_attachment,
                depth_attachment,
            ],
            bloom_textures,
            [brightness_bind_group, tone_mapping_bind_group],
        ) = create_render_attachments(width, height, 10, ctx, &mut encoder);

        let tone_mapping_pipeline =
            ctx.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Tone mapping pipeline"),
                    layout: Some(&ctx.renderer_ctx.tone_mapping_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &ctx.renderer_ctx.tone_mapping_shader_module,
                        entry_point: Some("vs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        unclipped_depth: false,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &ctx.renderer_ctx.tone_mapping_shader_module,
                        entry_point: Some("fs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(format.into())],
                    }),
                    multiview: None,
                    cache: None,
                });

        Self {
            render_bind_group_layout,
            brightness_pipeline,
            gaussian_blur_pipeline,
            bloom_amount: 10,
            opaque_attachment,
            accumulation_attachment,
            revealage_attachment,
            depth_attachment,
            brightness_bind_group,
            bloom_textures,
            tone_mapping_bind_group,
            tone_mapping_pipeline,
        }
    }

    pub fn render(
        &mut self,
        camera: &Camera,
        target: &wgpu::TextureView,
        scene_graph: &SceneGraph,
        skin_manager: &SkinManager,
        mesh_manager: &MeshManager,
        light_manager: &LightManager,
        environment: &Environment,
        ctx: &Context,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<(), RenderError> {
        let target_texture = target.texture();
        let opaque_texture = self.opaque_attachment.texture();
        if target_texture.width() != opaque_texture.width()
            || target_texture.height() != opaque_texture.height()
            || target_texture.format() != opaque_texture.format()
        {
            warn!("Renderer is incompatible with the target. Recreating the renderer");
            *self = Self::new(
                target_texture.width(),
                target_texture.height(),
                target_texture.format(),
                ctx,
            )
        }

        let aspect_ratio = target.texture().width() as f32 / target.texture().height() as f32;
        let projection_matrix = camera.projection_matrix(aspect_ratio);

        let camera_matrix = scene_graph
            .get(camera.node)
            .ok_or(RenderError::InvalidCameraNode(camera.node))?
            .global_transformation();
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
        let camera_buffer = ctx
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Camera buffer"),
                contents: bytes_of(&camera_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let render_bind_group = ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render bind group"),
            layout: &self.render_bind_group_layout,
            entries: &[
                // skins
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: skin_manager.buffer().as_entire_binding(),
                },
                // camera
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: camera_buffer.as_entire_binding(),
                },
                // lights
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: light_manager.point_light_buffer().as_entire_binding(),
                },
                // irradiance map
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(environment.irradiance_map_view()),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(environment.irradiance_map_sampler()),
                },
                // prefilter map
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(environment.prefilter_map_view()),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(environment.prefilter_map_sampler()),
                },
                // BRDF LUT
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(environment.brdf_lut_view()),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(environment.brdf_lut_sampler()),
                },
            ],
        });

        let mut primitive_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Opaque render pass"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: &self.opaque_attachment,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &self.accumulation_attachment,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &self.revealage_attachment,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                        store: wgpu::StoreOp::Store,
                    },
                }),
            ],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_attachment,
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

        mesh_manager.render_opaque_primitives(&mut primitive_render_pass);
        mesh_manager.render_transparent_primitives(&mut primitive_render_pass);
        drop(primitive_render_pass);

        let mut brightness_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Brightness render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.bloom_textures[0].0,
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
        brightness_render_pass.set_bind_group(0, &self.brightness_bind_group, &[]);
        brightness_render_pass.draw(0..3, 0..1);
        drop(brightness_render_pass);

        let mut horizontal = false;
        for _ in 0..self.bloom_amount {
            let source = &self.bloom_textures[horizontal as usize].1;
            horizontal = !horizontal;
            let target = &self.bloom_textures[horizontal as usize].0;
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

        tone_mapping_render_pass.set_pipeline(&self.tone_mapping_pipeline);
        tone_mapping_render_pass.set_bind_group(0, &self.tone_mapping_bind_group, &[]);
        tone_mapping_render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

/// Error when [`Renderer::render()`] fails.
#[derive(Debug, Error)]
pub enum RenderError {
    #[error("camera node ({0}) is not part of the scene graph")]
    InvalidCameraNode(NodeId),
}

#[derive(Debug, Clone)]
pub(crate) struct RendererContext {
    render_bind_group_layout: wgpu::BindGroupLayout,
    brightness_pipeline: wgpu::RenderPipeline,
    gaussian_blur_pipeline: wgpu::RenderPipeline,
    tone_mapping_shader_module: wgpu::ShaderModule,
    tone_mapping_bind_group_layout: wgpu::BindGroupLayout,
    tone_mapping_pipeline_layout: wgpu::PipelineLayout,
    brightness_bind_group_layout: wgpu::BindGroupLayout,
    gaussian_blur_bind_group_layout: wgpu::BindGroupLayout,
    bloom_sampler: wgpu::Sampler,
}

impl RendererContext {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
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

        let brightness_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("../brightness.wgsl"));

        let brightness_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Brightness bind group layout"),
                entries: &[
                    // opaque texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let brightness_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Brightness pipeline layout"),
                bind_group_layouts: &[&brightness_bind_group_layout],
                push_constant_ranges: &[],
            });

        let gaussian_blur_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("../gaussian_blur.wgsl"));

        let gaussian_blur_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Gaussian blur bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let gaussian_blur_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Gaussian blur pipeline layout"),
                bind_group_layouts: &[&gaussian_blur_bind_group_layout],
                push_constant_ranges: &[],
            });

        let gaussian_blur_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Gaussian blur pipeline"),
                layout: Some(&gaussian_blur_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &gaussian_blur_shader_module,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &gaussian_blur_shader_module,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
                }),
                multiview: None,
                cache: None,
            });

        let brightness_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Brightness pipeline"),
            layout: Some(&brightness_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &brightness_shader_module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &brightness_shader_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
            }),
            multiview: None,
            cache: None,
        });

        let tone_mapping_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("../tone_mapping.wgsl"));

        let tone_mapping_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Tone mapping bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let tone_mapping_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Tone mapping pipeline layout"),
                bind_group_layouts: &[&tone_mapping_bind_group_layout],
                push_constant_ranges: &[],
            });

        let bloom_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Bloom texture samlper"),
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        Self {
            render_bind_group_layout,
            brightness_pipeline,
            gaussian_blur_pipeline,
            tone_mapping_shader_module,
            tone_mapping_bind_group_layout,
            tone_mapping_pipeline_layout,
            brightness_bind_group_layout,
            gaussian_blur_bind_group_layout,
            bloom_sampler,
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

fn create_render_attachments(
    width: u32,
    height: u32,
    bloom_amount: usize,
    ctx: &Context,
    encoder: &mut wgpu::CommandEncoder,
) -> (
    [wgpu::TextureView; 4],
    [(wgpu::TextureView, wgpu::BindGroup); 2],
    [wgpu::BindGroup; 2],
) {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let opaque_attachment = TextureBuilder::default()
        .name("Opaque render attachment")
        .empty(size, wgpu::TextureFormat::Rgba16Float)
        .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
        .build(ctx, encoder)
        .create_view(&wgpu::TextureViewDescriptor {
            label: Some("Opaque render attachment"),
            ..Default::default()
        });

    let accumulation_attachment = TextureBuilder::default()
        .name("Accumulation render attachment")
        .empty(size, wgpu::TextureFormat::Rgba16Float)
        .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
        .build(ctx, encoder)
        .create_view(&wgpu::TextureViewDescriptor {
            label: Some("Accumulation render attachment"),
            ..Default::default()
        });

    let revealage_attachment = TextureBuilder::default()
        .name("Revealage render attachment")
        .empty(size, wgpu::TextureFormat::R8Unorm)
        .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
        .build(ctx, encoder)
        .create_view(&wgpu::TextureViewDescriptor {
            label: Some("Revealage render attachment"),
            ..Default::default()
        });

    let depth_attachment = TextureBuilder::default()
        .name("Depth render attachment")
        .empty(size, wgpu::TextureFormat::Depth24Plus)
        .usage(wgpu::TextureUsages::RENDER_ATTACHMENT)
        .build(ctx, encoder)
        .create_view(&wgpu::TextureViewDescriptor {
            label: Some("Depth render attachment"),
            ..Default::default()
        });

    let brightness_bind_group = ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Brightness bind group"),
        layout: &ctx.renderer_ctx.brightness_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&opaque_attachment),
        }],
    });

    let mut horizontal = true;
    let bloom_textures: [(wgpu::TextureView, wgpu::BindGroup); 2] = repeat_with(|| {
        let texture = TextureBuilder::default()
            .name("Bloom texture")
            .empty(size, wgpu::TextureFormat::Rgba16Float)
            .usage(wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT)
            .build(ctx, encoder)
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Bloom texture view"),
                ..Default::default()
            });
        let horizontal_buffer =
            ctx.device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Gaussian blur horizontal buffer"),
                    contents: bytes_of(&(horizontal as u32)),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
        horizontal = !horizontal;
        let bloom_bind_group = ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bloom bind group"),
            layout: &ctx.renderer_ctx.gaussian_blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&ctx.renderer_ctx.bloom_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: horizontal_buffer.as_entire_binding(),
                },
            ],
        });

        (texture, bloom_bind_group)
    })
    .take(2)
    .collect::<Vec<_>>()
    .try_into()
    .unwrap();

    let tone_mapping_bind_group =
        create_tone_mapping_bind_group(ctx, &opaque_attachment, &bloom_textures, bloom_amount);

    (
        [
            opaque_attachment,
            accumulation_attachment,
            revealage_attachment,
            depth_attachment,
        ],
        bloom_textures,
        [brightness_bind_group, tone_mapping_bind_group],
    )
}

fn create_tone_mapping_bind_group(
    ctx: &Context,
    opaque_texture: &wgpu::TextureView,
    bloom_textures: &[(wgpu::TextureView, wgpu::BindGroup); 2],
    bloom_amount: usize,
) -> wgpu::BindGroup {
    let final_bloom_texture = (bloom_amount % 2) as usize;

    ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Tone mapping bind group"),
        layout: &ctx.renderer_ctx.tone_mapping_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(opaque_texture),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&ctx.renderer_ctx.bloom_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(
                    &bloom_textures[final_bloom_texture].0,
                ),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&ctx.renderer_ctx.bloom_sampler),
            },
        ],
    })
}
