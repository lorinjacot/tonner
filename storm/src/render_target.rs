use std::{borrow::Borrow, iter::repeat_with};

use bytemuck::bytes_of;
use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::{Engine, texture::TextureBuilder};

/// A builder for [`RenderTarget`]. If possible, a builder should be reused, as calling
/// [`RenderTargetBuilder::new`] is recreating multiple [`wgpu::Texture`], [`wgpu::BindGroup`] and [`wgpu::RenderPipeline`].
#[must_use]
#[derive(Clone)]
pub struct RenderTargetBuilder {
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    opaque_attachment: wgpu::TextureView,
    accumulation_attachment: wgpu::TextureView,
    revealage_attachment: wgpu::TextureView,
    depth_attachment: wgpu::TextureView,
    brightness_bind_group: wgpu::BindGroup,
    bloom_textures: [(wgpu::TextureView, wgpu::BindGroup); 2],
    tone_mapping_bind_group: wgpu::BindGroup,
    tone_mapping_pipeline: wgpu::RenderPipeline,
}

impl RenderTargetBuilder {
    /// Create a new builder. If possible, a builder should be reused, as calling
    /// [`RenderTargetBuilder::new`] is recreating multiple [`wgpu::Texture`], [`wgpu::BindGroup`] and [`wgpu::RenderPipeline`].
    pub fn new(width: u32, height: u32, format: wgpu::TextureFormat, engine: &mut Engine) -> Self {
        let mut encoder = engine
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("RenderTarget::new() command encoder"),
            });

        let (
            [
                opaque_attachment,
                accumulation_attachment,
                revealage_attachment,
                depth_attachment,
            ],
            bloom_textures,
            [brightness_bind_group, tone_mapping_bind_group],
        ) = create_render_attachments(width, height, 10, engine, &mut encoder);

        let tone_mapping_pipeline =
            engine
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Tone mapping pipeline"),
                    layout: Some(&engine.tone_mapping_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &engine.tone_mapping_shader_module,
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
                        module: &engine.tone_mapping_shader_module,
                        entry_point: Some("fs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(format.into())],
                    }),
                    multiview: None,
                    cache: None,
                });

        Self {
            width,
            height,
            format,
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

    /// Create a render target from a texture view.
    pub fn build<'a, View: Borrow<wgpu::TextureView>>(
        self,
        target: View,
    ) -> Result<RenderTarget<View>, IncompatibleTarget> {
        let target_texture = target.borrow().texture();
        if target_texture.width() != self.width {
            return Err(IncompatibleTarget::Width {
                builder: self.width,
                target: target_texture.width(),
            });
        }
        if target_texture.height() != self.height {
            return Err(IncompatibleTarget::Height {
                builder: self.height,
                target: target_texture.height(),
            });
        }
        if target_texture.format() != self.format {
            return Err(IncompatibleTarget::Format {
                builder: self.format,
                target: target_texture.format(),
            });
        }

        Ok(RenderTarget {
            width: self.width,
            height: self.width,
            render_texture_view: target,
            opaque_attachment: self.opaque_attachment,
            accumulation_attachment: self.accumulation_attachment,
            revealage_attachment: self.revealage_attachment,
            depth_attachment: self.depth_attachment,
            brightness_bind_group: self.brightness_bind_group,
            bloom_textures: self.bloom_textures,
            tone_mapping_bind_group: self.tone_mapping_bind_group,
            tone_mapping_pipeline: self.tone_mapping_pipeline,
        })
    }
}

/// Erro when [`RenderTargetBuilder::build`] fails.
#[derive(Debug, Error)]
pub enum IncompatibleTarget {
    #[error("target's width ({target}) must match builder's width ({builder})")]
    Width { builder: u32, target: u32 },
    #[error("target's height ({target}) must match builder's height ({builder})")]
    Height { builder: u32, target: u32 },
    #[error("target's format ({target:?}) must match builder's format ({builder:?})")]
    Format {
        builder: wgpu::TextureFormat,
        target: wgpu::TextureFormat,
    },
}

/// Somewhere to render a [`Scene`][crate::Scene].
pub struct RenderTarget<View: Borrow<wgpu::TextureView>> {
    width: u32,
    height: u32,
    pub(crate) render_texture_view: View,
    pub(crate) opaque_attachment: wgpu::TextureView,
    pub(crate) accumulation_attachment: wgpu::TextureView,
    pub(crate) revealage_attachment: wgpu::TextureView,
    pub(crate) depth_attachment: wgpu::TextureView,
    pub(crate) brightness_bind_group: wgpu::BindGroup,
    pub(crate) bloom_textures: [(wgpu::TextureView, wgpu::BindGroup); 2],
    pub(crate) tone_mapping_bind_group: wgpu::BindGroup,
    pub(crate) tone_mapping_pipeline: wgpu::RenderPipeline,
}

impl<View: Borrow<wgpu::TextureView>> RenderTarget<View> {
    pub fn width(&self) -> u32 {
        self.width
    }
    
    pub fn height(&self) -> u32 {
        self.height
    }

    pub(crate) fn aspect_ratio(&self) -> f32 {
        let target_texture = self.render_texture_view.borrow().texture();
        target_texture.width() as f32 / target_texture.height() as f32
    }
}

fn create_render_attachments(
    width: u32,
    height: u32,
    bloom_amount: usize,
    engine: &mut Engine,
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
        .build(engine, encoder)
        .create_view(&wgpu::TextureViewDescriptor {
            label: Some("Opaque render attachment"),
            ..Default::default()
        });

    let accumulation_attachment = TextureBuilder::default()
        .name("Accumulation render attachment")
        .empty(size, wgpu::TextureFormat::Rgba16Float)
        .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
        .build(engine, encoder)
        .create_view(&wgpu::TextureViewDescriptor {
            label: Some("Accumulation render attachment"),
            ..Default::default()
        });

    let revealage_attachment = TextureBuilder::default()
        .name("Revealage render attachment")
        .empty(size, wgpu::TextureFormat::R8Unorm)
        .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
        .build(engine, encoder)
        .create_view(&wgpu::TextureViewDescriptor {
            label: Some("Revealage render attachment"),
            ..Default::default()
        });

    let depth_attachment = TextureBuilder::default()
        .name("Depth render attachment")
        .empty(size, wgpu::TextureFormat::Depth24Plus)
        .usage(wgpu::TextureUsages::RENDER_ATTACHMENT)
        .build(engine, encoder)
        .create_view(&wgpu::TextureViewDescriptor {
            label: Some("Depth render attachment"),
            ..Default::default()
        });

    let brightness_bind_group = engine.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Brightness bind group"),
        layout: &engine.brightness_bind_group_layout,
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
            .build(engine, encoder)
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Bloom texture view"),
                ..Default::default()
            });
        let horizontal_buffer =
            engine
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Gaussian blur horizontal buffer"),
                    contents: bytes_of(&(horizontal as u32)),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
        horizontal = !horizontal;
        let bloom_bind_group = engine.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bloom bind group"),
            layout: &engine.gaussian_blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&engine.bloom_sampler),
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
        create_tone_mapping_bind_group(engine, &opaque_attachment, &bloom_textures, bloom_amount);

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
    engine: &mut Engine,
    opaque_texture: &wgpu::TextureView,
    bloom_textures: &[(wgpu::TextureView, wgpu::BindGroup); 2],
    bloom_amount: usize,
) -> wgpu::BindGroup {
    let final_bloom_texture = (bloom_amount % 2) as usize;

    engine.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Tone mapping bind group"),
        layout: &engine.tone_mapping_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(opaque_texture),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&engine.bloom_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(
                    &bloom_textures[final_bloom_texture].0,
                ),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&engine.bloom_sampler),
            },
        ],
    })
}
