use dashmap::{DashMap, mapref::one::Ref};
use image::DynamicImage;

use crate::Context;

pub fn create_texture_from_image(
    label: Option<&str>,
    image: &DynamicImage,
    mip_level_count: u32,
    usage: wgpu::TextureUsages,
    ctx: &Context,
) -> wgpu::Texture {
    let size = wgpu::Extent3d {
        width: image.width(),
        height: image.height(),
        depth_or_array_layers: 1,
    };

    use DynamicImage as DI;
    let (image, format, bytes_per_pixel) = match image {
        DI::ImageRgb8(_) | DI::ImageLuma8(_) | DI::ImageLumaA8(_) => (
            &DI::ImageRgba8(image.to_rgba8()),
            wgpu::TextureFormat::Rgba8Unorm,
            4,
        ),
        DI::ImageRgba8(_) => (image, wgpu::TextureFormat::Rgba8Unorm, 4),
        DI::ImageRgb16(_) | DI::ImageLuma16(_) | DI::ImageLumaA16(_) => (
            &DI::ImageRgba16(image.to_rgba16()),
            wgpu::TextureFormat::Rgba16Unorm,
            8,
        ),
        DI::ImageRgba16(_) => (image, wgpu::TextureFormat::Rgba16Unorm, 8),
        DI::ImageRgb32F(_) => (
            &DI::ImageRgba32F(image.to_rgba32f()),
            wgpu::TextureFormat::Rgba32Float,
            32,
        ),
        DI::ImageRgba32F(_) => (image, wgpu::TextureFormat::Rgba32Float, 32),
        _ => unimplemented!(),
    };

    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label,
        size,
        mip_level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[format.add_srgb_suffix()],
    });

    ctx.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        image.as_bytes(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(bytes_per_pixel * size.width),
            rows_per_image: Some(size.height),
        },
        size,
    );

    texture
}

pub fn genenerate_mips(texture: &wgpu::Texture, ctx: &Context, encoder: &mut wgpu::CommandEncoder) {
    let pipeline = ctx.texture.mip_pipeline(texture.format(), &ctx.device);
    let views: Vec<_> = (0..texture.mip_level_count())
        .map(|mip| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("mogra mip texture view"),
                base_mip_level: mip,
                mip_level_count: Some(1),
                ..Default::default()
            })
        })
        .collect();
    for [src, dst] in views.array_windows() {
        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mogra mip bind group"),
            layout: &ctx.texture.mip_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(dst),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&ctx.texture.mip_sampler),
                },
            ],
        });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mogra mip render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: src,
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
        render_pass.set_pipeline(&pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

#[derive(Debug, Clone)]
pub struct TextureContext {
    shader_module: wgpu::ShaderModule,
    mip_bind_group_layout: wgpu::BindGroupLayout,
    mip_pipeline_layout: wgpu::PipelineLayout,
    mip_pipelines: DashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
    mip_sampler: wgpu::Sampler,
}

impl TextureContext {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader_module = device.create_shader_module(wgpu::include_wgsl!("texture.wgsl"));
        let mip_bind_group_layout =
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
        let mip_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mogra mip pipeline layout"),
            bind_group_layouts: &[Some(&mip_bind_group_layout)],
            immediate_size: 0,
        });
        let mip_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("migra mips sampler"),
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        TextureContext {
            shader_module,
            mip_bind_group_layout,
            mip_pipeline_layout,
            mip_pipelines: DashMap::new(),
            mip_sampler,
        }
    }

    fn mip_pipeline<'a>(
        &'a self,
        format: wgpu::TextureFormat,
        device: &wgpu::Device,
    ) -> Ref<'a, wgpu::TextureFormat, wgpu::RenderPipeline> {
        self.mip_pipelines
            .entry(format)
            .or_insert_with(|| {
                create_pip_pipeline(
                    format,
                    &self.mip_pipeline_layout,
                    device,
                    &self.shader_module,
                )
            })
            .downgrade()
    }
}

fn create_pip_pipeline(
    format: wgpu::TextureFormat,
    layout: &wgpu::PipelineLayout,
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mogra mips pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
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
            module,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(format.into())],
        }),
        multiview_mask: None,
        cache: None,
    })
}
