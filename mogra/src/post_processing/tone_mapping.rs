//! Tone mapping post-processing effect.
//! 
//! Tone mapping is the process of converting high dynamic range (HDR) images to low dynamic range (LDR) images.
//! This is necessary because most displays can only show a limited range of brightness values, while HDR images
//! can contain a much wider range of brightness values. Tone mapping algorithms are used to compress the dynamic
//! range of HDR images so that they can be displayed on LDR displays without losing important details or
//! introducing artifacts.
//! 
//! The supported tone mapping algorithm are:
//! - Reinhard tone mapping

use crate::Context;

/// Maps HDR textures to LDR ones. Currently only the *Reinhard tone mapping* is available.
/// 
/// Colors of HDR textures are represented by float allowed to take any value, including one higher than `1.0`.
/// Colors of LDR textures must be in the `[0.0, 1.0]` range.
#[derive(Debug, Clone)]
pub struct ToneMapping {
    pipeline: wgpu::RenderPipeline,
    input_texture: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

impl ToneMapping {
    /// Creates a new tone mapping instance using the *Reinhard tone mapping* algorithm.
    /// 
    /// The `width` and `height` parameters specify the dimensions of the input and output textures.
    /// The `output_format` parameter specifies the format of the output texture.
    pub fn new(width: u32, height: u32, output_format: wgpu::TextureFormat, ctx: &Context) -> Self {
        let pipeline: wgpu::RenderPipeline =
            ctx.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Tone mapping pipeline"),
                    layout: Some(&ctx.tone_mapping.pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &ctx.tone_mapping.shader_module,
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
                        module: &ctx.tone_mapping.shader_module,
                        entry_point: Some("fs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(output_format.into())],
                    }),
                    multiview_mask: None,
                    cache: None,
                });

        let input_texture = create_input_texture(width, height, ctx);
        let bind_group = create_bind_group(&input_texture, ctx);

        ToneMapping {
            pipeline,
            input_texture,
            bind_group,
        }
    }

    /// Changes the size of the input and output textures. The new width and height must be non-zero.
    pub fn resize(&mut self, width: u32, height: u32, ctx: &Context) {
        self.input_texture = create_input_texture(width, height, ctx);
        self.bind_group = create_bind_group(&self.input_texture, ctx);
    }

    /// Returns the input texture of the tone mapping. When the tone mapping is applied, the input texture is mapped to the output texture.
    pub fn input_texture_view(&self) -> &wgpu::TextureView {
        &self.input_texture
    }

    /// Applies the tone mapping to the input texture and writes the result to the output texture.
    /// 
    /// The output texture must have the same dimensions and format given to `ToneMapping::new()`.
    /// 
    /// # Panics
    /// This function will panic if the output texture has different dimensions or format than the input texture.
    pub fn render(&self, output: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Tone mapping render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
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

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

/// Resources shared between all tone mapping instances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToneMappingContext {
    shader_module: wgpu::ShaderModule,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    sampler: wgpu::Sampler,
}

impl ToneMappingContext {
    /// Creates the shared resources for tone mapping.
    pub fn new(device: &wgpu::Device) -> Self {
        let shader_module = device.create_shader_module(wgpu::include_wgsl!("tone_mapping.wgsl"));

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mogra tonne mapping sampler"),
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Tone mapping pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        ToneMappingContext {
            shader_module,
            bind_group_layout,
            pipeline_layout,
            sampler,
        }
    }
}

fn create_input_texture(width: u32, height: u32, ctx: &Context) -> wgpu::TextureView {
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mogra tonne mapping input texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        view_formats: &[],
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
    });

    texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("mogra tonne mapping input texture view"),
        ..Default::default()
    })
}

fn create_bind_group(input_texture: &wgpu::TextureView, ctx: &Context) -> wgpu::BindGroup {
    ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("mogra tonne mapping bind group"),
        layout: &ctx.tone_mapping.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&input_texture),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&ctx.tone_mapping.sampler),
            },
        ],
    })
}
