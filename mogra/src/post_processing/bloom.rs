//! Bloom post-processing effect.
//! 
//! "Bright light sources and brightly lit regions are often difficult to convey to the viewer
//! as the intensity range of a monitor is limited. One way to distinguish bright light sources
//! on a monitor is by making them glow; the light then bleeds around the light source. This
//! effectively gives the viewer the illusion these light sources or bright regions are intensely
//! bright." learnopengl.com bloom description.
//! 
//! The supported bloom algorithms are:
//! - Physically Based Bloom (based on the ACM Siggraph presentation in 2014 by Jorge Jimenez for
//! Call of Duty: Advanced Warfare)

use bytemuck::{Pod, Zeroable, bytes_of};
use glam::{Vec2, vec2};
use wgpu::util::DeviceExt;

use crate::Context;

/// Applies bloom effect. Currently only the *Physically Based Bloom* algorithm is available.
#[derive(Debug, Clone)]
pub struct Bloom {
    buffers: Vec<wgpu::Buffer>,
    downsample_pipeline: wgpu::RenderPipeline,
    downsample_io: Vec<(wgpu::BindGroup, wgpu::TextureView)>,
    upsample_pipeline: wgpu::RenderPipeline,
    upsample_io: Vec<(wgpu::BindGroup, wgpu::TextureView)>,
    input_texture: wgpu::TextureView,
    last_bind_group: wgpu::BindGroup,
}

impl Bloom {
    /// Creates a new bloom instance using the *Physically Based Bloom* algorithm.
    /// 
    /// The `width` and `height` parameters specify the dimensions of the input and output textures.
    /// The `mips` parameter specifies how many times the texture will be up/down-sampled.
    /// The `filter_radius` parameter specifies the radius of the filter used for upsampling.
    pub fn new(width: u32, height: u32, mips: u32, filter_radius: f32, ctx: &Context) -> Self {
        assert!(mips > 0);
        let buffers = create_buffers(width, height, mips, filter_radius, &ctx.device);
        let IO {
            input_texture,
            downsample,
            upsample,
            last_bind_group,
        } = IO::new(width, height, &buffers, ctx);

        Bloom {
            buffers,
            downsample_pipeline: ctx.bloom.downsample_pipeline.clone(),
            downsample_io: downsample,
            upsample_pipeline: ctx.bloom.upsample_pipeline.clone(),
            upsample_io: upsample,
            input_texture,
            last_bind_group,
        }
    }

    /// Changes the size of the input and output textures. The new width and height must be strictly positive.
    pub fn resize(&mut self, width: u32, height: u32, ctx: &Context) {
        let IO {
            input_texture,
            downsample,
            upsample,
            last_bind_group,
        } = IO::new(width, height, &self.buffers, ctx);
        self.input_texture = input_texture;
        self.downsample_io = downsample;
        self.upsample_io = upsample;
        self.last_bind_group = last_bind_group;
    }

    /// Returns the input texture of the bloom effect.
    pub fn input_texture_view(&self) -> &wgpu::TextureView {
        &self.input_texture
    }

    ///Applies the bloom effect to the input texture and writes the result to the output texture.
    /// 
    /// The output texture must have the same dimensions as given to `Bloom::new` or `Bloom::resize`.
    /// 
    /// # Panics
    /// This function will panic if the output texture has a different size than the input texture.
    pub fn render(&self, output: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        for (input, output) in &self.downsample_io {
            self.downsample(input, output, encoder);
        }

        for (input, output) in &self.upsample_io {
            self.upsample(input, output, encoder);
        }
        self.upsample(&self.last_bind_group, output, encoder);
    }

    fn downsample(
        &self,
        input: &wgpu::BindGroup,
        output: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mogra bloom downsample render pass"),
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

        render_pass.set_pipeline(&self.downsample_pipeline);
        render_pass.set_bind_group(0, input, &[]);
        render_pass.draw(0..3, 0..1);
    }

    fn upsample(
        &self,
        input: &wgpu::BindGroup,
        output: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mogra bloom upsample render pass"),
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

        render_pass.set_pipeline(&self.upsample_pipeline);
        render_pass.set_bind_group(0, input, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

/// Resources shared between all bloom instances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BloomContext {
    bind_group_layout: wgpu::BindGroupLayout,
    downsample_pipeline: wgpu::RenderPipeline,
    upsample_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

impl BloomContext {
    /// Creates the shared resources for bloom.
    pub fn new(device: &wgpu::Device) -> Self {
        let shader_module = device.create_shader_module(wgpu::include_wgsl!("bloom.wgsl"));

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mogra bloom bind group layout"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mogra bloom pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let downsample_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mogra bloom downsample pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
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
                module: &shader_module,
                entry_point: Some("fs_downsample"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::all(),
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let blend = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };

        let upsample_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mogra bloom downsample pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
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
                module: &shader_module,
                entry_point: Some("fs_upsample"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: Some(wgpu::BlendState {
                        color: blend,
                        alpha: blend,
                    }),
                    write_mask: wgpu::ColorWrites::all(),
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mogra bloom sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        BloomContext {
            bind_group_layout,
            downsample_pipeline,
            upsample_pipeline,
            sampler,
        }
    }
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Parameters {
    /// distance to closest neigbour to sample for downsampling
    delta: Vec2,

    /// distance to closest neigbour to sample for upsampling
    radius: f32,

    _pad: u32,
}

fn create_buffers(
    width: u32,
    height: u32,
    mips: u32,
    filter_radius: f32,
    device: &wgpu::Device,
) -> Vec<wgpu::Buffer> {
    let mut size = vec2(width as f32, height as f32);
    (0..mips)
        .into_iter()
        .map(|_| {
            let delta = 1.0 / size;
            size *= 0.5;
            let params = Parameters {
                delta,
                radius: filter_radius,
                _pad: 0,
            };
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mogra bloom parameters buffer"),
                contents: bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        })
        .collect()
}

fn create_input_texture(
    width: u32,
    height: u32,
    mips: u32,
    device: &wgpu::Device,
) -> Vec<wgpu::TextureView> {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let mip_level_count = size.max_mips(wgpu::TextureDimension::D2).min(mips);

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mogra bloom input texture"),
        size,
        mip_level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    (0..mip_level_count)
        .into_iter()
        .map(|base_mip_level| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("mogra bloom texture input view"),
                base_mip_level,
                mip_level_count: Some(1),
                ..Default::default()
            })
        })
        .collect()
}

fn create_bind_groups(
    views: &[wgpu::TextureView],
    sampler: &wgpu::Sampler,
    buffers: &[wgpu::Buffer],
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
) -> Vec<wgpu::BindGroup> {
    views
        .iter()
        .zip(buffers)
        .map(|(view, buffer)| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("mogra bloom bind group"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: buffer.as_entire_binding(),
                    },
                ],
            })
        })
        .collect()
}

struct IO {
    input_texture: wgpu::TextureView,
    downsample: Vec<(wgpu::BindGroup, wgpu::TextureView)>,
    upsample: Vec<(wgpu::BindGroup, wgpu::TextureView)>,
    last_bind_group: wgpu::BindGroup,
}

impl IO {
    fn new(width: u32, height: u32, buffers: &[wgpu::Buffer], ctx: &Context) -> Self {
        let mut views = create_input_texture(width, height, buffers.len() as u32, &ctx.device);
        let mut bind_groups = create_bind_groups(
            &views,
            &ctx.bloom.sampler,
            &buffers,
            &ctx.device,
            &ctx.bloom.bind_group_layout,
        );
        let mip_count = bind_groups.len();

        let mut downsample = Vec::with_capacity(mip_count - 1);
        for mip_level in 0..mip_count - 1 {
            downsample.push((bind_groups[mip_level].clone(), views[mip_level + 1].clone()));
        }

        let mut upsample = Vec::with_capacity(mip_count - 2);
        views.pop();
        for _ in 0..mip_count - 2 {
            upsample.push((bind_groups.pop().unwrap(), views.pop().unwrap()));
        }

        let input_texture = views.pop().unwrap();
        let last_bind_group = bind_groups.pop().unwrap();

        IO {
            input_texture,
            downsample,
            upsample,
            last_bind_group,
        }
    }
}
