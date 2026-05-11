use std::iter::repeat_with;

#[cfg(feature = "egui")]
use crate::renderer::billboard_label::BillboardLabel;
use crate::{
    Context,
    entity_component::{ComponentsView, EntityId},
    environment::Environment,
    geometry::skin::{SkinError, SkinId, SkinManager},
    mesh::{MeshInstance, MeshInstanceId, PrimitiveRenderer},
    renderer::{
        camera::Camera,
        light::{LightError, LightManager},
    },
    scene_graph::SceneGraph,
    texture::TextureBuilder,
};
use bytemuck::{Pod, Zeroable, bytes_of};
use glam::{Mat4, Vec3};
use log::warn;
use thiserror::Error;
use wgpu::util::DeviceExt;

#[cfg(feature = "egui")]
pub mod billboard_label;
pub mod camera;
pub mod light;

pub struct Renderer {
    format: wgpu::TextureFormat,
    render_bind_group_layout: wgpu::BindGroupLayout,
    primitive_renderer: PrimitiveRenderer,
    skybox_pipeline: wgpu::RenderPipeline,
    compose_pipeline: wgpu::RenderPipeline,
    brightness_pipeline: wgpu::RenderPipeline,
    gaussian_blur_pipeline: wgpu::RenderPipeline,
    bloom_amount: usize,
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
        let primitive_renderer = PrimitiveRenderer::new(ctx);
        let skybox_pipeline = ctx.renderer_ctx.skybox_pipeline.clone();
        let compose_pipeline = ctx.renderer_ctx.compose_pipeline.clone();
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
            [
                compose_bind_group,
                brightness_bind_group,
                tone_mapping_bind_group,
            ],
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
                    multiview_mask: None,
                    cache: None,
                });

        Self {
            format,
            render_bind_group_layout,
            primitive_renderer,
            skybox_pipeline,
            compose_pipeline,
            brightness_pipeline,
            gaussian_blur_pipeline,
            bloom_amount: 10,
            opaque_attachment,
            accumulation_attachment,
            revealage_attachment,
            depth_attachment,
            compose_bind_group,
            brightness_bind_group,
            bloom_textures,
            tone_mapping_bind_group,
            tone_mapping_pipeline,
        }
    }

    pub fn render<'a>(
        &mut self,
        camera: &Camera,
        target: &wgpu::TextureView,
        scene_graph: &SceneGraph,
        skin_manager: &mut SkinManager,
        mesh_instances: impl IntoIterator<Item = &'a MeshInstance>,
        light_manager: &mut LightManager,
        environment: &Environment,
        ctx: &Context,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<(), RenderError> {
        light_manager.update_point_light_buffer(scene_graph, ctx.device(), ctx.queue())?;
        let prepared_skins = skin_manager.prepare(scene_graph, ctx)?;

        let target_texture = target.texture();
        let opaque_texture = self.opaque_attachment.texture();
        if target_texture.format() != self.format
            || target_texture.height() != opaque_texture.height()
            || target_texture.width() != opaque_texture.width()
        {
            warn!("Renderer is incompatible with the target. Recreating the renderer.");
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
            .get(camera.entity)
            .ok_or(RenderError::InvalidCameraNode(camera.entity))?
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
                    resource: prepared_skins.buffer().as_entire_binding(),
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

        let mut opaque_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
                None,
                None,
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
            multiview_mask: None,
        });
        opaque_render_pass.set_bind_group(0, &render_bind_group, &[]);

        let mut prepared_primitives =
            self.primitive_renderer
                .prepare(mesh_instances, scene_graph, prepared_skins, ctx)?;

        prepared_primitives.render_opaque_primitives(&mut opaque_render_pass);

        opaque_render_pass.set_pipeline(&self.skybox_pipeline);
        opaque_render_pass.set_bind_group(1, environment.skybox_bind_group(), &[]);
        opaque_render_pass.draw(0..3, 0..1);

        drop(opaque_render_pass);

        let mut transparent_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Transparent render pass"),
            color_attachments: &[
                None,
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
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        transparent_render_pass.set_bind_group(0, &render_bind_group, &[]);

        prepared_primitives.render_transparent_primitives(&mut transparent_render_pass);

        drop(transparent_render_pass);

        let mut compose_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Compose render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.opaque_attachment,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        compose_render_pass.set_pipeline(&self.compose_pipeline);
        compose_render_pass.set_bind_group(0, &self.compose_bind_group, &[]);
        compose_render_pass.draw(0..3, 0..1);

        drop(compose_render_pass);

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
            multiview_mask: None,
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
                multiview_mask: None,
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
            multiview_mask: None,
        });

        tone_mapping_render_pass.set_pipeline(&self.tone_mapping_pipeline);
        tone_mapping_render_pass.set_bind_group(0, &self.tone_mapping_bind_group, &[]);
        tone_mapping_render_pass.draw(0..3, 0..1);

        Ok(())
    }

    #[cfg(feature = "egui")]
    pub fn render_billboard_labels<'a>(
        &mut self,
        camera: &Camera,
        target: &wgpu::TextureView,
        scene_graph: &SceneGraph,
        labels: impl IntoIterator<Item = (EntityId, &'a BillboardLabel)>,
        egui_ctx: &egui::Context,
    ) -> Result<(), RenderError> {
        let target_texture = target.texture();
        let aspect_ratio = target_texture.width() as f32 / target_texture.height() as f32;
        let projection_matrix = camera.projection_matrix(aspect_ratio);

        let camera_matrix = scene_graph
            .get(camera.entity)
            .ok_or(RenderError::InvalidCameraNode(camera.entity))?
            .global_transformation();
        let camera_position = camera_matrix.transform_point3(Vec3::ZERO);

        let view_matrix = Mat4::look_to_rh(
            camera_position,
            camera_matrix.transform_vector3(-Vec3::Z),
            camera_matrix.transform_vector3(Vec3::Y),
        );
        let view_projection = projection_matrix * view_matrix;

        use glam::Vec4;
        let mut labels = labels
            .into_iter()
            .filter_map(|(entity, label)| {
                let label_node = scene_graph.get(entity)?;
                let world_position = label_node.global_transformation() * Vec4::W;
                let clip = view_projection * world_position;
                if clip.w <= 0.0 {
                    return None;
                }

                let view_position = view_matrix * world_position;
                let screen_position = egui::pos2(
                    (clip.x / clip.w + 1.0) * 0.5 * target_texture.width() as f32,
                    (1.0 - clip.y / clip.w) * 0.5 * target_texture.height() as f32,
                );

                Some((view_position.z, label.text.as_str(), screen_position))
            })
            .collect::<Vec<_>>();

        // Farthest first, closest last.
        labels.sort_by(|a, b| a.0.total_cmp(&b.0));

        let painter = egui_ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("billboard_labels"),
        ));

        let font_id = egui::FontId::proportional(14.0);
        let style = egui_ctx.global_style();
        let visuals = &style.visuals;
        let margin = &style.spacing.window_margin;

        for (_, text, screen_position) in labels {
            let mut job = egui::text::LayoutJob::default();
            job.append(
                text,
                0.0,
                egui::TextFormat {
                    font_id: font_id.clone(),
                    color: visuals.text_color(),
                    ..Default::default()
                },
            );
            job.wrap.max_width = 180.0;

            let galley = egui_ctx.fonts_mut(|fonts| fonts.layout_job(job));

            let rect = egui::Rect::from_min_size(
                screen_position - egui::vec2(margin.left as f32, margin.top as f32),
                galley.size()
                    + egui::vec2(
                        (margin.left + margin.right) as f32,
                        (margin.top + margin.bottom) as f32,
                    ),
            );

            painter.rect_filled(rect, visuals.window_corner_radius, visuals.window_fill);
            painter.rect_stroke(
                rect,
                visuals.window_corner_radius,
                visuals.window_stroke,
                egui::StrokeKind::Inside,
            );
            painter.galley(
                rect.min + egui::vec2(margin.left as f32, margin.top as f32),
                galley,
                visuals.text_color(),
            );
        }

        Ok(())
    }
}

/// Error when [`Renderer::render()`] fails.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum RenderError {
    #[error("invalid light: {0}")]
    InvalidLight(#[from] LightError),

    #[error("mesh instance ({0}) node ({1}) is not part of the scene graph")]
    InvalidMeshInstanceNode(MeshInstanceId, EntityId),

    #[error("invalid skin: {0}")]
    InvalidSkin(#[from] SkinError),

    #[error("mesh instance ({0}) skin ({1}) is not part of the skin manager")]
    InvalidMeshInstanceSkin(MeshInstanceId, SkinId),

    #[error("camera node ({0}) is not part of the scene graph")]
    InvalidCameraNode(EntityId),
}

#[derive(Debug, Clone)]
pub(crate) struct RendererContext {
    pub(crate) render_bind_group_layout: wgpu::BindGroupLayout,
    skybox_pipeline: wgpu::RenderPipeline,
    compose_pipeline: wgpu::RenderPipeline,
    brightness_pipeline: wgpu::RenderPipeline,
    gaussian_blur_pipeline: wgpu::RenderPipeline,
    compose_bind_group_layout: wgpu::BindGroupLayout,
    tone_mapping_shader_module: wgpu::ShaderModule,
    tone_mapping_bind_group_layout: wgpu::BindGroupLayout,
    tone_mapping_pipeline_layout: wgpu::PipelineLayout,
    brightness_bind_group_layout: wgpu::BindGroupLayout,
    gaussian_blur_bind_group_layout: wgpu::BindGroupLayout,
    bloom_sampler: wgpu::Sampler,
}

impl RendererContext {
    pub(crate) fn new(
        device: &wgpu::Device,
        skybox_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
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

        let skybox_shader_module = device.create_shader_module(wgpu::include_wgsl!("skybox.wgsl"));
        let skybox_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Skybox pipeline layout"),
                bind_group_layouts: &[
                    Some(&render_bind_group_layout),
                    Some(skybox_bind_group_layout),
                ],
                immediate_size: 0,
            });
        let skybox_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skybox pipeline"),
            layout: Some(&skybox_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &skybox_shader_module,
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &skybox_shader_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba16Float.into()), None, None],
            }),
            multiview_mask: None,
            cache: None,
        });

        let brightness_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("brightness.wgsl"));

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
                bind_group_layouts: &[Some(&brightness_bind_group_layout)],
                immediate_size: 0,
            });

        let gaussian_blur_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("gaussian_blur.wgsl"));

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
                bind_group_layouts: &[Some(&gaussian_blur_bind_group_layout)],
                immediate_size: 0,
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
                multiview_mask: None,
                cache: None,
            });

        let composer_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("compose.wgsl"));

        let compose_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Compose bind group layout"),
                entries: &[
                    // accumulation texture
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
                    // revealage texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
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

        let compose_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Compose pipeline layout"),
                bind_group_layouts: &[Some(&compose_bind_group_layout)],
                immediate_size: 0,
            });

        const COMPOSE_BLEND: wgpu::BlendComponent = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let compose_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Compose pipeline"),
            layout: Some(&compose_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &composer_shader_module,
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
                module: &composer_shader_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: Some(wgpu::BlendState {
                        color: COMPOSE_BLEND,
                        alpha: COMPOSE_BLEND,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
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
            multiview_mask: None,
            cache: None,
        });

        let tone_mapping_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("tone_mapping.wgsl"));

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
                bind_group_layouts: &[Some(&tone_mapping_bind_group_layout)],
                immediate_size: 0,
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
            skybox_pipeline,
            compose_pipeline,
            brightness_pipeline,
            gaussian_blur_pipeline,
            compose_bind_group_layout,
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
    [wgpu::BindGroup; 3],
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

    let compose_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Compose bind group"),
        layout: &ctx.renderer_ctx.compose_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&accumulation_attachment),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&revealage_attachment),
            },
        ],
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
        [
            compose_bind_group,
            brightness_bind_group,
            tone_mapping_bind_group,
        ],
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
