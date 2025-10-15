use std::collections::HashMap;

use image::DynamicImage;
use wgpu::util::DeviceExt;

use crate::Resources;

pub(super) struct TextureBuilderData {
    generate_mips_bind_group_layout: wgpu::BindGroupLayout,
    generate_mips_pipeline_layout: wgpu::PipelineLayout,
    shader_module: wgpu::ShaderModule,
    generate_mips_pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
    generate_mips_sampler: wgpu::Sampler,
}

impl TextureBuilderData {
    pub fn new(device: &wgpu::Device) -> Self {
        let generate_mips_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Generate mips bind group layout"),
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

        let generate_mips_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Generate mips pipeline layout"),
                bind_group_layouts: &[&generate_mips_bind_group_layout],
                push_constant_ranges: &[],
            });

        let shader_module = device.create_shader_module(wgpu::include_wgsl!("texture.wgsl"));

        let generate_mips_pipelines = HashMap::new();

        let generate_mips_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Generate mips sampler"),
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            generate_mips_bind_group_layout,
            generate_mips_pipeline_layout,
            shader_module,
            generate_mips_pipelines,
            generate_mips_sampler,
        }
    }
}

#[must_use]
pub struct TextureBuilder<'a> {
    name: Option<&'a str>,
    source: Source<'a>,
    mip_level_count: u32,
    generate_mips: bool,
    usage: wgpu::TextureUsages,
}

impl<'a> TextureBuilder<'a> {
    pub fn name(mut self, name: impl Into<Option<&'a str>>) -> Self {
        self.name = name.into();
        self
    }

    pub fn mip_level_count(mut self, mip_level_count: u32) -> Self {
        self.mip_level_count = mip_level_count;
        self
    }

    pub fn usage(mut self, usage: wgpu::TextureUsages) -> Self {
        self.usage = usage;
        self
    }

    pub fn empty(mut self, size: wgpu::Extent3d, format: wgpu::TextureFormat) -> Self {
        self.source = Source::Empty { size, format };
        self
    }

    pub fn from_dynamic_image(mut self, dynamic_image: &'a DynamicImage, srgb: bool) -> Self {
        self.source = Source::DynamicImage {
            dynamic_image,
            srgb,
        };
        self
    }

    pub fn generate_mips(mut self) -> Self {
        self.generate_mips = true;
        self
    }

    pub fn build_callback(
        mut self,
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
        callback: impl FnOnce(&wgpu::Texture, &mut Resources, &mut wgpu::CommandEncoder),
    ) -> wgpu::Texture {
        let size = match self.source {
            Source::Empty { size, .. } => size,
            Source::DynamicImage { dynamic_image, .. } => wgpu::Extent3d {
                width: dynamic_image.width(),
                height: dynamic_image.height(),
                depth_or_array_layers: 1,
            },
        };

        if self.generate_mips {
            self.usage.insert(wgpu::TextureUsages::RENDER_ATTACHMENT);
            if self.mip_level_count == 1 {
                let max_size = size.width.max(size.height) as f32;
                self.mip_level_count = 1 + max_size.log2() as u32;
            }
        }

        let (texture, format) = match self.source {
            Source::Empty { format, .. } => {
                let texture = resources.device.create_texture(&wgpu::TextureDescriptor {
                    label: self.name,
                    size,
                    mip_level_count: self.mip_level_count,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: self.usage,
                    view_formats: &[],
                });
                (texture, format)
            }
            Source::DynamicImage {
                dynamic_image,
                srgb,
            } => {
                use DynamicImage::*;
                let (dynamic_image, mut format) = match dynamic_image {
                    ImageRgb8(_) => (
                        &ImageRgba8(dynamic_image.to_rgba8()),
                        wgpu::TextureFormat::Rgba8Unorm,
                    ),
                    ImageRgba8(_) => (dynamic_image, wgpu::TextureFormat::Rgba8Unorm),
                    ImageRgb16(_) => (
                        &ImageRgba16(dynamic_image.to_rgba16()),
                        wgpu::TextureFormat::Rgba16Unorm,
                    ),
                    ImageRgba16(_) => (dynamic_image, wgpu::TextureFormat::Rgba16Unorm),
                    ImageRgb32F(_) => (
                        &ImageRgba32F(dynamic_image.to_rgba32f()),
                        wgpu::TextureFormat::Rgba32Float,
                    ),
                    ImageRgba32F(_) => (dynamic_image, wgpu::TextureFormat::Rgba32Float),
                    _ => unimplemented!(),
                };
                if srgb {
                    format = format.add_srgb_suffix();
                }
                let texture = resources.device.create_texture_with_data(
                    &resources.queue,
                    &wgpu::TextureDescriptor {
                        label: self.name,
                        size,
                        mip_level_count: self.mip_level_count,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: self.usage,
                        view_formats: &[],
                    },
                    wgpu::util::TextureDataOrder::LayerMajor,
                    dynamic_image.as_bytes(),
                );
                (texture, format)
            }
        };

        callback(&texture, resources, encoder);

        if self.generate_mips {
            let pipeline = resources
                .texture_builder_data
                .generate_mips_pipelines
                .entry(format)
                .or_insert_with(|| {
                    resources
                        .device
                        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                            label: Some("Generate mips pipeline"),
                            layout: Some(
                                &resources.texture_builder_data.generate_mips_pipeline_layout,
                            ),
                            vertex: wgpu::VertexState {
                                module: &resources.texture_builder_data.shader_module,
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
                                module: &resources.texture_builder_data.shader_module,
                                entry_point: Some("fs_main"),
                                compilation_options: wgpu::PipelineCompilationOptions::default(),
                                targets: &[Some(format.into())],
                            }),
                            multiview: None,
                            cache: None,
                        })
                });

            for layer in 0..size.depth_or_array_layers {
                for mip_level in 1..self.mip_level_count {
                    let sample_view = texture.create_view(&wgpu::TextureViewDescriptor {
                        label: Some("Generate mips sample view"),
                        base_mip_level: mip_level - 1,
                        mip_level_count: Some(1),
                        base_array_layer: layer,
                        array_layer_count: Some(1),
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        ..Default::default()
                    });

                    let bind_group =
                        resources
                            .device
                            .create_bind_group(&wgpu::BindGroupDescriptor {
                                label: Some("Generate mips bind group"),
                                layout: &resources
                                    .texture_builder_data
                                    .generate_mips_bind_group_layout,
                                entries: &[
                                    wgpu::BindGroupEntry {
                                        binding: 0,
                                        resource: wgpu::BindingResource::TextureView(&sample_view),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 1,
                                        resource: wgpu::BindingResource::Sampler(
                                            &resources.texture_builder_data.generate_mips_sampler,
                                        ),
                                    },
                                ],
                            });

                    let render_view = texture.create_view(&wgpu::TextureViewDescriptor {
                        label: Some("Generate mips render view"),
                        base_mip_level: mip_level,
                        mip_level_count: Some(1),
                        base_array_layer: layer,
                        array_layer_count: Some(1),
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        ..Default::default()
                    });

                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Generate mips render pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &render_view,
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
                    render_pass.set_pipeline(pipeline);
                    render_pass.set_bind_group(0, &bind_group, &[]);
                    render_pass.draw(0..3, 0..1);
                }
            }
        }
        texture
    }

    pub fn build(
        self,
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
    ) -> wgpu::Texture {
        self.build_callback(resources, encoder, |_, _, _| ())
    }
}

impl<'a> Default for TextureBuilder<'a> {
    fn default() -> Self {
        Self {
            name: None,
            source: Source::Empty {
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                format: wgpu::TextureFormat::R8Unorm,
            },
            mip_level_count: 1,
            generate_mips: false,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
        }
    }
}

enum Source<'a> {
    Empty {
        size: wgpu::Extent3d,
        format: wgpu::TextureFormat,
    },
    DynamicImage {
        dynamic_image: &'a DynamicImage,
        srgb: bool,
    },
}
