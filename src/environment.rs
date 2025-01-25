use std::f32::consts::FRAC_PI_2;

use glam::{vec3, Mat4, Vec3};
use image::EncodableLayout;
use wgpu::util::DeviceExt;

const ENVIRONMENT_MAP_SIZE: u32 = 512;
const IRRADIANCE_MAP_SIZE: u32 = 256;

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
    vertex_buffer: wgpu::Buffer,
    environment_map_bind_group: wgpu::BindGroup,
    irradiance_map_bind_group: wgpu::BindGroup,
}

impl EnvironmentMap {
    pub fn from_equirectangular(
        equirectangular_map: &image::Rgba32FImage,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Self {
        let module = device.create_shader_module(wgpu::include_wgsl!("environment.wgsl"));

        let vertex_buffers = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vec3>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            }],
        }];

        let view_projection_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Environment view projection bind group layout"),
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

        let equirectangular_texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Environment equirectangular texture bind group layout"),
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

        let environment_map_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Environment map bind group layout"),
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

        let equirectangular_to_cubemap_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Environment equirectangular to cubemap pipeline layout"),
                bind_group_layouts: &[
                    &view_projection_bind_group_layout,
                    &equirectangular_texture_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let equirectangular_to_cubemap_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Environment map equirectangular to cubemap pipeline"),
                layout: Some(&equirectangular_to_cubemap_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_cube_view_projection"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &vertex_buffers,
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
                    module: &module,
                    entry_point: Some("fs_environment_map"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
                }),
                multiview: None,
                cache: None,
            });

        let irradiance_map_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Environment irrandiance map pipeline layout"),
                bind_group_layouts: &[
                    &view_projection_bind_group_layout,
                    &environment_map_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let irradiance_map_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Environment irradiance map pipeline"),
                layout: Some(&irradiance_map_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_cube_view_projection"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &vertex_buffers,
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
                    module: &module,
                    entry_point: Some("fs_skybox"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
                }),
                multiview: None,
                cache: None,
            });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Environment cube vertex buffer"),
            contents: bytemuck::cast_slice(POSITIONS),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let equirectangular_texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("Environment equirectangular texture"),
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

        let equirectangular_texture_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Environment equirectangular to cubemap texture bind group"),
                layout: &equirectangular_texture_bind_group_layout,
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

        // let environment_map_bytes = include_bytes!("../assets/environments/rgba8.ktx2");
        // let environment_map_reader = ktx2::Reader::new(environment_map_bytes).unwrap();

        // let mut image = Vec::with_capacity(environment_map_reader.data().len());
        // for level in environment_map_reader.levels() {
        //     image.extend_from_slice(level);
        // }
        // let header = environment_map_reader.header();

        // let environment_map_texture = device.create_texture_with_data(
        //     queue,
        //     &wgpu::TextureDescriptor {
        //         label: Some("Environment map texture"),
        //         size: wgpu::Extent3d {
        //             width: 256,
        //             height: 256,
        //             depth_or_array_layers: 6,
        //         },
        //         mip_level_count: header.level_count,
        //         sample_count: 1,
        //         dimension: wgpu::TextureDimension::D2,
        //         format: wgpu::TextureFormat::Rgba8Unorm,
        //         usage: wgpu::TextureUsages::TEXTURE_BINDING,
        //         view_formats: &[],
        //     },
        //     wgpu::util::TextureDataOrder::MipMajor,
        //     &image,
        // );

        let environment_map_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Environment map texture"),
            size: wgpu::Extent3d {
                width: ENVIRONMENT_MAP_SIZE,
                height: ENVIRONMENT_MAP_SIZE,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let environment_map_texture_view =
            environment_map_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("Environment map texture view"),
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            });

        let enviroment_map_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Environment map sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let environment_map_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Environment map bind group"),
            layout: &environment_map_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&environment_map_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&enviroment_map_sampler),
                },
            ],
        });

        let irradiance_map_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Environment diffuse irradiance texture"),
            size: wgpu::Extent3d {
                width: IRRADIANCE_MAP_SIZE,
                height: IRRADIANCE_MAP_SIZE,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let irradiance_map_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Environment irradiance map bind group"),
            layout: &environment_map_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &irradiance_map_texture.create_view(&wgpu::TextureViewDescriptor {
                            dimension: Some(wgpu::TextureViewDimension::Cube),
                            ..Default::default()
                        }),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&enviroment_map_sampler),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Environment map sampler command encoder"),
        });
        Self::create_cubemap(
            &environment_map_texture,
            &equirectangular_texture_bind_group,
            &view_projection_bind_group_layout,
            &vertex_buffer,
            &equirectangular_to_cubemap_pipeline,
            device,
            &mut encoder,
        );
        Self::create_cubemap(
            &irradiance_map_texture,
            &environment_map_bind_group,
            &view_projection_bind_group_layout,
            &vertex_buffer,
            &irradiance_map_pipeline,
            device,
            &mut encoder,
        );
        queue.submit([encoder.finish()]);

        let skybox_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Environment skybox pipeline layout"),
                bind_group_layouts: &[camera_bind_group_layout, &environment_map_bind_group_layout],
                push_constant_ranges: &[],
            });

        let skybox_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Environment skybox pipeline"),
            layout: Some(&skybox_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_cube_camera"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &vertex_buffers,
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
                module: &module,
                entry_point: Some("fs_skybox"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
            }),
            multiview: None,
            cache: None,
        });

        Self {
            skybox_pipeline,
            vertex_buffer,
            environment_map_bind_group,
            irradiance_map_bind_group,
        }
    }

    fn create_cubemap(
        target_texture: &wgpu::Texture,
        source_texture_bind_group: &wgpu::BindGroup,
        view_projection_bind_group_layout: &wgpu::BindGroupLayout,
        vertex_buffer: &wgpu::Buffer,
        pipeline: &wgpu::RenderPipeline,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let projection = Mat4::perspective_rh(FRAC_PI_2, 1.0, 0.1, 10.0);

        let views = [
            Mat4::look_to_lh(Vec3::ZERO, Vec3::X, -Vec3::Y),
            Mat4::look_to_lh(Vec3::ZERO, -Vec3::X, -Vec3::Y),
            Mat4::look_to_lh(Vec3::ZERO, Vec3::Y, Vec3::Z),
            Mat4::look_to_lh(Vec3::ZERO, -Vec3::Y, -Vec3::Z),
            Mat4::look_to_lh(Vec3::ZERO, Vec3::Z, -Vec3::Y),
            Mat4::look_to_lh(Vec3::ZERO, -Vec3::Z, -Vec3::Y),
        ];

        for base_array_layer in 0..6 {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Environment equirectangular to cubemap render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_texture.create_view(&wgpu::TextureViewDescriptor {
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
                    label: Some("Environment view projection buffer"),
                    contents: bytemuck::cast_slice(&[view_projection]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

            let view_projection_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Environment view projection bind group"),
                layout: view_projection_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: view_projection_buffer.as_entire_binding(),
                }],
            });

            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, Some(&view_projection_bind_group), &[]);
            render_pass.set_bind_group(1, Some(source_texture_bind_group), &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.draw(0..POSITIONS.len() as u32, 0..1);
        }
    }
}

pub trait DrawEnvironment {
    fn draw_environment(
        &mut self,
        environment: &EnvironmentMap,
        blur: bool,
        camera_bind_group: &wgpu::BindGroup,
    );
}

impl<'a> DrawEnvironment for wgpu::RenderPass<'a> {
    fn draw_environment(
        &mut self,
        environment: &EnvironmentMap,
        blur: bool,
        camera_bind_group: &wgpu::BindGroup,
    ) {
        self.set_pipeline(&environment.skybox_pipeline);
        self.set_bind_group(0, Some(camera_bind_group), &[]);
        if blur {
            self.set_bind_group(1, Some(&environment.irradiance_map_bind_group), &[]);
        } else {
            self.set_bind_group(1, Some(&environment.environment_map_bind_group), &[]);
        }
        self.set_vertex_buffer(0, environment.vertex_buffer.slice(..));
        self.draw(0..POSITIONS.len() as u32, 0..1);
    }
}
