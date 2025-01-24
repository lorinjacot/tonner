use std::f32::consts::FRAC_PI_2;

use glam::{vec3, Mat4, Vec3};
use image::EncodableLayout;
use wgpu::util::DeviceExt;

const ENVIRONMENT_MAP_SIZE: u32 = 512;

const POSITIONS: &[Vec3] = &[
    vec3(-1.0, 1.0, -1.0),
    vec3(-1.0, -1.0, -1.0),
    vec3(1.0, -1.0, -1.0),
    vec3(1.0, -1.0, -1.0),
    vec3(1.0, 1.0, -1.0),
    vec3(-1.0, 1.0, -1.0),
    vec3(-1.0, -1.0, 1.0),
    vec3(-1.0, -1.0, -1.0),
    vec3(-1.0, 1.0, -1.0),
    vec3(-1.0, 1.0, -1.0),
    vec3(-1.0, 1.0, 1.0),
    vec3(-1.0, -1.0, 1.0),
    vec3(1.0, -1.0, -1.0),
    vec3(1.0, -1.0, 1.0),
    vec3(1.0, 1.0, 1.0),
    vec3(1.0, 1.0, 1.0),
    vec3(1.0, 1.0, -1.0),
    vec3(1.0, -1.0, -1.0),
    vec3(-1.0, -1.0, 1.0),
    vec3(-1.0, 1.0, 1.0),
    vec3(1.0, 1.0, 1.0),
    vec3(1.0, 1.0, 1.0),
    vec3(1.0, -1.0, 1.0),
    vec3(-1.0, -1.0, 1.0),
    vec3(-1.0, 1.0, -1.0),
    vec3(1.0, 1.0, -1.0),
    vec3(1.0, 1.0, 1.0),
    vec3(1.0, 1.0, 1.0),
    vec3(-1.0, 1.0, 1.0),
    vec3(-1.0, 1.0, -1.0),
    vec3(-1.0, -1.0, -1.0),
    vec3(-1.0, -1.0, 1.0),
    vec3(1.0, -1.0, -1.0),
    vec3(1.0, -1.0, -1.0),
    vec3(-1.0, -1.0, 1.0),
    vec3(1.0, -1.0, 1.0),
];

pub struct EnvironmentMap {
    skybox_pipeline: wgpu::RenderPipeline,
    skybox_bind_group: wgpu::BindGroup,
}

impl EnvironmentMap {
    pub fn from_equirectangular(
        equirectangular_map: &image::Rgba32FImage,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Self {
        let equirectangular_to_cubemap_module =
            device.create_shader_module(wgpu::include_wgsl!("equirectangular_to_cubemap.wgsl"));

        let equirectangular_to_cubemap_view_projection_bind_group_layout = device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(
                    "Environment map equirectangular to cubemap view projection bind group layout",
                ),
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
            });

        let equirectangular_to_cubemap_texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Environment map equirectangular to cubemap texture bind group layout"),
                entries: &[
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                ],
            });

        let equirectangular_to_cubemap_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Environment map equirectangular to cubemap pipeline layout"),
                bind_group_layouts: &[
                    &equirectangular_to_cubemap_view_projection_bind_group_layout,
                    &equirectangular_to_cubemap_texture_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let equirectangular_to_cubemap_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Environment map equirectangular to cubemap pipeline"),
                layout: Some(&equirectangular_to_cubemap_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &equirectangular_to_cubemap_module,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vec3>() as u64,
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
                    module: &equirectangular_to_cubemap_module,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
                }),
                multiview: None,
                cache: None,
            });

        let positions_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Environment map positions buffer"),
            contents: bytemuck::cast_slice(POSITIONS),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let projection = Mat4::perspective_rh(FRAC_PI_2, 1.0, 0.1, 10.0);
        let views = [
            Mat4::look_at_rh(Vec3::ZERO, Vec3::X, -Vec3::Y),
            Mat4::look_at_rh(Vec3::ZERO, -Vec3::X, -Vec3::Y),
            Mat4::look_at_rh(Vec3::ZERO, -Vec3::Y, -Vec3::Z),
            Mat4::look_at_rh(Vec3::ZERO, Vec3::Y, Vec3::Z),
            Mat4::look_at_rh(Vec3::ZERO, Vec3::Z, -Vec3::Y),
            Mat4::look_at_rh(Vec3::ZERO, -Vec3::Z, -Vec3::Y),
        ];

        let equirectangular_texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("Environment map equirectangular texture"),
                size: wgpu::Extent3d {
                    width: equirectangular_map.width(),
                    height: equirectangular_map.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            equirectangular_map.as_bytes(),
        );
        let equirectangular_texture_view =
            equirectangular_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let equirectangular_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Environment map equirectangular sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let equirectangular_to_cubemap_texture_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Environment map equirectangar to cubemap texture bind group"),
                layout: &equirectangular_to_cubemap_texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&equirectangular_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&equirectangular_sampler),
                    },
                ],
            });

        let cubemap_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Environment map cubemap texture"),
            size: wgpu::Extent3d {
                width: ENVIRONMENT_MAP_SIZE,
                height: ENVIRONMENT_MAP_SIZE,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let cubemap_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Environment map cubemap sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Environment map equirectangular to cubemap command encoder"),
        });
        for base_array_layer in 0..6 {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Environment map equirectangular to cubemap render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &cubemap_texture.create_view(&wgpu::TextureViewDescriptor {
                        label: Some(
                            "Environment map cubemap texture view for equirectangular to cubemap",
                        ),
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        base_array_layer,
                        array_layer_count: Some(1),
                        ..Default::default()
                    }),
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

            let view_projection = projection * views[base_array_layer as usize];
            let view_projection_buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Environment map view projection buffer"),
                    contents: bytemuck::cast_slice(&[view_projection]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

            let view_projection_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Environment map equirectangar to cubemap view projection bind group"),
                layout: &equirectangular_to_cubemap_view_projection_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: view_projection_buffer.as_entire_binding(),
                }],
            });

            render_pass.set_pipeline(&equirectangular_to_cubemap_pipeline);
            render_pass.set_bind_group(
                0,
                Some(&view_projection_bind_group),
                &[],
            );
            render_pass.set_bind_group(
                1,
                Some(&equirectangular_to_cubemap_texture_bind_group),
                &[],
            );
            render_pass.set_vertex_buffer(0, positions_buffer.slice(..));
            render_pass.draw(0..POSITIONS.len() as u32, 0..1);
        }
        queue.submit([encoder.finish()]);

        let skybox_module = device.create_shader_module(wgpu::include_wgsl!("skybox.wgsl"));

        let skybox_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Skybox bind group layout"),
                entries: &[
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
                ],
            });

        let skybox_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Skybox pipeline layout"),
                bind_group_layouts: &[&skybox_bind_group_layout, camera_bind_group_layout],
                push_constant_ranges: &[],
            });

        let skybox_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skybox pipeline"),
            layout: Some(&skybox_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &skybox_module,
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
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &skybox_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
            }),
            multiview: None,
            cache: None,
        });

        let skybox_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skybox bind group"),
            layout: &skybox_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&cubemap_texture.create_view(
                        &wgpu::TextureViewDescriptor {
                            dimension: Some(wgpu::TextureViewDimension::Cube),
                            ..Default::default()
                        },
                    )),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&cubemap_sampler),
                },
            ],
        });

        Self {
            skybox_pipeline,
            skybox_bind_group,
        }
    }
}

pub trait DrawEnvironment {
    fn draw_environment(
        &mut self,
        environment: &EnvironmentMap,
        camera_bind_group: &wgpu::BindGroup,
    );
}

impl<'a> DrawEnvironment for wgpu::RenderPass<'a> {
    fn draw_environment(
        &mut self,
        environment: &EnvironmentMap,
        camera_bind_group: &wgpu::BindGroup,
    ) {
        self.set_pipeline(&environment.skybox_pipeline);
        self.set_bind_group(0, Some(&environment.skybox_bind_group), &[]);
        self.set_bind_group(1, Some(camera_bind_group), &[]);
        self.draw(0..3, 0..1);
    }
}
