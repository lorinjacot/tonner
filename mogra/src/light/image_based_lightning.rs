use std::f32::consts::FRAC_PI_2;

use bytemuck::{bytes_of, cast_slice};
use glam::{Mat4, Vec3, vec3};
use wgpu::util::DeviceExt;

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

const IRRADIANCE_MAP_SIZE: u32 = 32;
const IRRADIANCE_MAP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

const PREFILTER_MAP_SIZE: u32 = 128;
const PREFILTER_MAP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const PREFILTER_MAP_MIP_COUNT: usize = 5;

const BRDF_LUT_SIZE: u32 = 512;
const BRDF_LUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

const COMPILATION_OPTIONS: wgpu::PipelineCompilationOptions = wgpu::PipelineCompilationOptions {
    constants: &[("prefilter_map_size", PREFILTER_MAP_SIZE as f64)],
    zero_initialize_workgroup_memory: false,
};

pub fn create_irradiance_map(
    skybox_bind_group: &wgpu::BindGroup,
    ctx: &Context,
    encoder: &mut wgpu::CommandEncoder,
) -> wgpu::TextureView {
    let irradiance_map = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mogra irradiance map texture"),
        size: wgpu::Extent3d {
            width: IRRADIANCE_MAP_SIZE,
            height: IRRADIANCE_MAP_SIZE,
            depth_or_array_layers: 6,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: IRRADIANCE_MAP_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    for face in 0..6 {
        let view = irradiance_map.create_view(&wgpu::TextureViewDescriptor {
            label: Some("mogra irrandiance map face view"),
            base_array_layer: face,
            array_layer_count: Some(1),
            ..Default::default()
        });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mogra irradiance render pass"),
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

        render_pass.set_pipeline(&ctx.light.irradiance_pipeline);
        render_pass.set_vertex_buffer(0, ctx.light.cube_vertex_buffer.slice(..));
        render_pass.set_index_buffer(
            ctx.light.cube_index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.set_bind_group(
            0,
            &ctx.light.view_projection_bind_groups[face as usize],
            &[],
        );
        render_pass.set_bind_group(1, skybox_bind_group, &[]);
        render_pass.draw_indexed(0..CUBE_VERTEX_COUNT, 0, 0..1);
    }

    irradiance_map.create_view(&wgpu::TextureViewDescriptor {
        label: Some("mogra irrandiance map cube view"),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    })
}

pub fn create_prefilter_map(
    skybox_bind_group: &wgpu::BindGroup,
    ctx: &Context,
    encoder: &mut wgpu::CommandEncoder,
) -> wgpu::TextureView {
    let prefilter_map = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mogra prefilter map texture"),
        size: wgpu::Extent3d {
            width: PREFILTER_MAP_SIZE,
            height: PREFILTER_MAP_SIZE,
            depth_or_array_layers: 6,
        },
        mip_level_count: PREFILTER_MAP_MIP_COUNT as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: PREFILTER_MAP_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    for face in 0..6 {
        for mip in 0..PREFILTER_MAP_MIP_COUNT {
            let view = prefilter_map.create_view(&wgpu::TextureViewDescriptor {
                label: Some("mogra prefilter map face view"),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: face as u32,
                array_layer_count: Some(1),
                base_mip_level: mip as u32,
                mip_level_count: Some(1),
                ..Default::default()
            });

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mogra prefilter render pass"),
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

            render_pass.set_pipeline(&ctx.light.prefilter_map_pipeline);
            render_pass.set_vertex_buffer(0, ctx.light.cube_vertex_buffer.slice(..));
            render_pass.set_index_buffer(
                ctx.light.cube_index_buffer.slice(..),
                wgpu::IndexFormat::Uint16,
            );
            render_pass.set_bind_group(0, &ctx.light.view_projection_bind_groups[face], &[]);
            render_pass.set_bind_group(1, skybox_bind_group, &[]);
            render_pass.set_bind_group(2, &ctx.light.roughness_bind_groups[mip], &[]);
            render_pass.draw_indexed(0..CUBE_VERTEX_COUNT, 0, 0..1);
        }
    }

    prefilter_map.create_view(&wgpu::TextureViewDescriptor {
        label: Some("mogra prefilter map cube view"),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    })
}

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
                binding: 0,
                resource: wgpu::BindingResource::TextureView(irradiance_map),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&ctx.light.irradiance_map_sampler),
            },
            // prefilter map
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(prefilter_map),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&ctx.light.prefilter_map_sampler),
            },
            // BRDF LUT
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&ctx.light.brdf_lut_texture),
            },
            wgpu::BindGroupEntry {
                binding: 5,
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

pub fn create_view_projection_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mogra view projection bind group layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

pub fn create_view_projection_bind_groups(
    layout: &wgpu::BindGroupLayout,
    device: &wgpu::Device,
) -> [wgpu::BindGroup; 6] {
    let projection = Mat4::perspective_rh(FRAC_PI_2, 1.0, 0.1, 10.0);
    let create_bind_group = |view| {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Environment builder view projection buffer"),
            contents: bytemuck::cast_slice(&[projection * view]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Environment builder view projection bind group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    };

    [
        create_bind_group(Mat4::look_to_rh(Vec3::ZERO, Vec3::X, Vec3::Y)),
        create_bind_group(Mat4::look_to_rh(Vec3::ZERO, -Vec3::X, Vec3::Y)),
        create_bind_group(Mat4::look_to_rh(Vec3::ZERO, Vec3::Y, Vec3::Z)),
        create_bind_group(Mat4::look_to_rh(Vec3::ZERO, -Vec3::Y, -Vec3::Z)),
        create_bind_group(Mat4::look_to_rh(Vec3::ZERO, -Vec3::Z, Vec3::Y)), // the z-axis of wgpu is our -z
        create_bind_group(Mat4::look_to_rh(Vec3::ZERO, Vec3::Z, Vec3::Y)),
    ]
}

pub fn create_prefilter_roughness_bind_group_layout(
    device: &wgpu::Device,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mogra roughness bind group layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

pub fn create_roughness_bind_groups(
    layout: &wgpu::BindGroupLayout,
    device: &wgpu::Device,
) -> [wgpu::BindGroup; PREFILTER_MAP_MIP_COUNT] {
    std::array::from_fn(|mip| {
        let roughness = mip as f32 / (PREFILTER_MAP_MIP_COUNT - 1) as f32;
        let roughness_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mogra roughness buffer"),
            contents: bytes_of(&roughness),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mogra roughness bind group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: roughness_buffer.as_entire_binding(),
            }],
        })
    })
}

pub fn create_prefilter_map_pipeline(
    module: &wgpu::ShaderModule,
    view_projection_bind_group_layout: &wgpu::BindGroupLayout,
    skybox_bind_group_layout: &wgpu::BindGroupLayout,
    roughness_bind_group_layout: &wgpu::BindGroupLayout,
    device: &wgpu::Device,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mogra prefilter map pipeline layout"),
        bind_group_layouts: &[
            Some(view_projection_bind_group_layout),
            Some(skybox_bind_group_layout),
            Some(roughness_bind_group_layout),
        ],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mpgra prefilter map pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            compilation_options: COMPILATION_OPTIONS,
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: size_of::<Vec3>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                }],
            }],
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
            entry_point: Some("fs_prefilter"),
            compilation_options: COMPILATION_OPTIONS,
            targets: &[Some(PREFILTER_MAP_FORMAT.into())],
        }),
        multiview_mask: None,
        cache: None,
    })
}

pub fn create_irradiance_pipeline(
    view_projection_bind_group_layout: &wgpu::BindGroupLayout,
    cubemap_bind_group_layout: &wgpu::BindGroupLayout,
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mogra irradiance pipeline layout"),
        bind_group_layouts: &[
            Some(view_projection_bind_group_layout),
            Some(cubemap_bind_group_layout),
        ],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mogra irradiance pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            compilation_options: COMPILATION_OPTIONS,
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: size_of::<Vec3>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                }],
            }],
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
            entry_point: Some("fs_irradiance"),
            compilation_options: COMPILATION_OPTIONS,
            targets: &[Some(IRRADIANCE_MAP_FORMAT.into())],
        }),
        multiview_mask: None,
        cache: None,
    })
}

pub fn create_cube_vertex_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("mogra cube vertex buffer"),
        contents: cast_slice(CUBE_VERTICES),
        usage: wgpu::BufferUsages::VERTEX,
    })
}

pub fn create_cube_index_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("mogra cube index buffer"),
        contents: cast_slice(CUBE_INDICES),
        usage: wgpu::BufferUsages::INDEX,
    })
}
