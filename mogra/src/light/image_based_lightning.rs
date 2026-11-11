use glam::{Vec3, vec3};

use crate::Context;

const CUBE_VERTICES: &[Vec3] = &[
    // front face
    vec3(-1.0, 1.0, 1.0),
    vec3(-1.0, -1.0, 1.0),
    vec3(1.0, 1.0, 1.0),
    vec3(1.0, -1.0, 1.0),
    // right face
    vec3(1.0, 1.0, -1.0),
    vec3(1.0, 1.0, 1.0),
    vec3(1.0, -1.0, -1.0),
    vec3(1.0, -1.0, 1.0),
    // back face
    vec3(1.0, 1.0, -1.0),
    vec3(1.0, -1.0, -1.0),
    vec3(-1.0, 1.0, -1.0),
    vec3(-1.0, -1.0, -1.0),
    // left face
    vec3(-1.0, 1.0, 1.0),
    vec3(-1.0, 1.0, -1.0),
    vec3(-1.0, -1.0, 1.0),
    vec3(-1.0, -1.0, -1.0),
    // bottom face
    vec3(1.0, -1.0, 1.0),
    vec3(-1.0, -1.0, 1.0),
    vec3(1.0, -1.0, -1.0),
    vec3(-1.0, -1.0, -1.0),
    // top face
    vec3(-1.0, 1.0, 1.0),
    vec3(1.0, 1.0, 1.0),
    vec3(-1.0, 1.0, -1.0),
    vec3(1.0, 1.0, -1.0),
];

const CUBE_INDICES: &[u16] = &[
    0, 1, 2, 2, 1, 3, // front
    4, 5, 6, 6, 5, 7, // right
    8, 9, 10, 10, 9, 11, // back
    12, 13, 14, 14, 13, 15, // left
    16, 17, 18, 18, 17, 19, // bottom
    20, 21, 22, 22, 21, 23, // top
];

const CUBE_VERTEX_COUNT: u32 = 36;

const ENVIRONMENT_MAP_SIZE: u32 = 512;
const ENVIRONMENT_MAP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

const IRRADIANCE_MAP_SIZE: u32 = 32;
const IRRADIANCE_MAP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

const PREFILTER_MAP_SIZE: u32 = 128;
const PREFILTER_MAP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const PREFILTER_MAP_MIP_COUNT: u32 = 5;

const BRDF_LUT_SIZE: u32 = 512;
const BRDF_LUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

pub fn create_ibl_bind_group(
    irradiance_map: &wgpu::TextureView,
    prefilter_map: &wgpu::TextureView,
    ctx: &Context,
) -> wgpu::BindGroup {
    ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("mogra IBL bind group"),
        layout: &ctx.light.ibl_bind_group_layout,
        entries: &[
            // irradiance map
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(irradiance_map),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(&ctx.light.irradiance_map_sampler),
            },
            // prefilter map
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(prefilter_map),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::Sampler(&ctx.light.prefilter_map_sampler),
            },
            // BRDF LUT
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::TextureView(&ctx.light.brdf_lut_texture),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: wgpu::BindingResource::Sampler(&ctx.light.brdf_lut_sampler),
            },
        ],
    })
}

pub fn create_ibl_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mogra IBL bind group layout"),
        entries: &[
            // irradiance map
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::Cube,
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
            // prefilter map
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::Cube,
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
            // BRDF LUT
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
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
        ],
    })
}

pub fn create_irradiance_map_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("mogra radiance map sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    })
}

pub fn create_prefilter_map_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("mogra prefilter map sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    })
}

pub fn create_brdf_lut_texture(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    encoder: &mut wgpu::CommandEncoder,
) -> wgpu::TextureView {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mogra BRDF LUT pipeline layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });

    let constants = &[("prefilter_map_size", PREFILTER_MAP_SIZE as f64)];
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mogra BRDF LUT pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main_2d"),
            compilation_options: wgpu::PipelineCompilationOptions {
                constants,
                ..Default::default()
            },
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
            entry_point: Some("fs_brdf_lut"),
            compilation_options: wgpu::PipelineCompilationOptions {
                constants,
                ..Default::default()
            },
            targets: &[Some(BRDF_LUT_FORMAT.into())],
        }),
        multiview_mask: None,
        cache: None,
    });

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mogra BRDF LUT texture"),
        size: wgpu::Extent3d {
            width: BRDF_LUT_SIZE,
            height: BRDF_LUT_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: BRDF_LUT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("mogra BRDF LUT texture view"),
        ..Default::default()
    });

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("BRDF lut render pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
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
    render_pass.draw(0..3, 0..1);

    view
}

pub fn create_brdf_lut_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("mogra BRDF LUT sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    })
}

pub fn create_radiance_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("mogra radiance sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    })
}

pub fn create_environment_map_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("mogra environment cubemap sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    })
}
