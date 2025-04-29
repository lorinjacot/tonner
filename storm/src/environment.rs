use std::f32::consts::FRAC_PI_2;

use bytemuck::cast_slice;
use glam::{Mat4, Vec3, vec3};
use image::DynamicImage;
use wgpu::util::DeviceExt;

use crate::{DenseEntry, Id, Resources, storage::SetEntry};

pub const CUBE_VERTICES: &[Vec3] = &[
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

pub const CUBE_INDICES: &[u16] = &[
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

pub(super) struct EnvironmentBuilderData {
    cube_vertex_buffer: wgpu::Buffer,
    cube_index_buffer: wgpu::Buffer,
    radiance_sampler: wgpu::Sampler,
    radiance_bind_group_layout: wgpu::BindGroupLayout,
    environment_map_sampler: wgpu::Sampler,
    view_projection_bind_groups: [wgpu::BindGroup; 6],
    equirectangular_to_cubemap_pipeline: wgpu::RenderPipeline,
    irradiance_pipeline: wgpu::RenderPipeline,
}

impl EnvironmentBuilderData {
    pub fn new(device: &wgpu::Device, skybox_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let cube_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Environment builder cube vertex buffer"),
            contents: cast_slice(CUBE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let cube_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Environment builder cube index buffer"),
            contents: cast_slice(CUBE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let radiance_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(&format!("Environment radiance sampler")),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let radiance_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Radiance bing group layout"),
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

        let environment_map_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Environment cubemap sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let view_projection_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Environment builder view projection bind group layout"),
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

        let projection = Mat4::perspective_rh(FRAC_PI_2, 1.0, 0.1, 10.0);
        let create_bind_group = |view| {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Environment builder view projection buffer"),
                contents: bytemuck::cast_slice(&[projection * view]),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Environment builder view projection bind group"),
                layout: &view_projection_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            })
        };

        let view_projection_bind_groups = [
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, Vec3::X, Vec3::Y)),
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, -Vec3::X, Vec3::Y)),
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, Vec3::Y, Vec3::Z)),
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, -Vec3::Y, -Vec3::Z)),
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, -Vec3::Z, Vec3::Y)), // the z-axis of wgpu is our -z
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, Vec3::Z, Vec3::Y)),
        ];

        let equirectangular_to_cubemap_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Equirectangular to cubemap pipeline layout"),
                bind_group_layouts: &[
                    &view_projection_bind_group_layout,
                    &radiance_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let module = &device.create_shader_module(wgpu::include_wgsl!("environment.wgsl"));

        let equirectangular_to_cubemap_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Equirectangular to cubemap pipeline"),
                layout: Some(&equirectangular_to_cubemap_pipeline_layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
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
                    entry_point: Some("fs_equirectangular_to_cubemap"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(ENVIRONMENT_MAP_FORMAT.into())],
                }),
                multiview: None,
                cache: None,
            });

        let irradiance_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Irradiance pipeline layout"),
                bind_group_layouts: &[
                    &view_projection_bind_group_layout,
                    &skybox_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let irradiance_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Irradiance pipeline"),
            layout: Some(&irradiance_pipeline_layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
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
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(IRRADIANCE_MAP_FORMAT.into())],
            }),
            multiview: None,
            cache: None,
        });

        Self {
            cube_vertex_buffer,
            cube_index_buffer,
            radiance_sampler,
            radiance_bind_group_layout,
            environment_map_sampler,
            view_projection_bind_groups,
            equirectangular_to_cubemap_pipeline,
            irradiance_pipeline,
        }
    }
}

pub struct Environment {
    id: Id<Environment>,
    pub name: String,
    skybox_bind_group: wgpu::BindGroup,
    irradiance_map: (wgpu::TextureView, wgpu::Sampler),
}

impl Environment {
    pub fn skybox_bind_group(&self) -> &wgpu::BindGroup {
        &self.skybox_bind_group
    }

    pub fn irradiance_map_view(&self) -> &wgpu::TextureView {
        &self.irradiance_map.0
    }

    pub fn irradiance_map_sampler(&self) -> &wgpu::Sampler {
        &self.irradiance_map.1
    }
}

impl DenseEntry for Environment {
    type Key = Environment;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

pub struct EnvironmentDescriptor {
    name: Option<String>,
    skybox_bind_group: wgpu::BindGroup,
    irradiance_map: (wgpu::TextureView, wgpu::Sampler),
}

impl SetEntry for Environment {
    type Descriptor = EnvironmentDescriptor;

    fn new(id: Id<Self::Key>, desc: Self::Descriptor) -> Self {
        let name = desc.name.unwrap_or_else(|| id.to_string());
        Self {
            id,
            name,
            skybox_bind_group: desc.skybox_bind_group,
            irradiance_map: desc.irradiance_map,
        }
    }
}

pub struct EnvironmentBuilder<'a, 'r> {
    resources: &'r mut Resources,
    name: Option<String>,
    source: Source<'a>,
}

impl<'a, 'r> EnvironmentBuilder<'a, 'r> {
    pub fn new(resources: &'r mut Resources) -> Self {
        Self {
            resources,
            name: None,
            source: Source::None,
        }
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn from_equirectangular_map(mut self, radiance_image: &'a DynamicImage) -> Self {
        self.source = Source::EquirectangularMap(radiance_image);
        self
    }

    pub fn build(self, encoder: &'a mut wgpu::CommandEncoder) -> &'r mut Environment {
        let name = self.name.as_ref().map_or("", |name| name);
        let data = &self.resources.environment_builder_data;
        let environment_map_view = match self.source {
            Source::None => panic!("no environment support"),
            Source::EquirectangularMap(radiance_image) => {
                let radiance_texture = self
                    .resources
                    .texture_builder()
                    .name(&format!("{name} radiance texture"))
                    .from_dynamic_image(radiance_image, false)
                    .build(encoder);
                let radiance_texture_view =
                    radiance_texture.create_view(&wgpu::TextureViewDescriptor {
                        label: Some(&format!("{name} radiance view")),
                        ..Default::default()
                    });
                let radiance_bind_group =
                    self.resources
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some(&format!("{name} randiance bind group")),
                            layout: &data.radiance_bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(
                                        &radiance_texture_view,
                                    ),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::Sampler(
                                        &data.radiance_sampler,
                                    ),
                                },
                            ],
                        });

                let environment_cubemap_texture = self
                    .resources
                    .texture_builder()
                    .name(&format!("{name} environment cubemap"))
                    .empty(
                        wgpu::Extent3d {
                            width: ENVIRONMENT_MAP_SIZE,
                            height: ENVIRONMENT_MAP_SIZE,
                            depth_or_array_layers: 6,
                        },
                        ENVIRONMENT_MAP_FORMAT,
                    )
                    .usage(
                        wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                    )
                    .build(encoder);

                for face in 0..6 {
                    let environment_map_view =
                        environment_cubemap_texture.create_view(&wgpu::TextureViewDescriptor {
                            label: Some("environmnent cubemap render view"),
                            dimension: Some(wgpu::TextureViewDimension::D2),
                            base_array_layer: face as u32,
                            array_layer_count: Some(1),
                            ..Default::default()
                        });

                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Equirectangular to cubemap render pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &environment_map_view,
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

                    render_pass.set_pipeline(&data.equirectangular_to_cubemap_pipeline);
                    render_pass.set_vertex_buffer(0, data.cube_vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        data.cube_index_buffer.slice(..),
                        wgpu::IndexFormat::Uint16,
                    );
                    render_pass.set_bind_group(0, &data.view_projection_bind_groups[face], &[]);
                    render_pass.set_bind_group(1, &radiance_bind_group, &[]);
                    render_pass.draw_indexed(0..CUBE_VERTEX_COUNT, 0, 0..1);
                }

                environment_cubemap_texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(&format!("{name} environment cubemap view")),
                    dimension: Some(wgpu::TextureViewDimension::Cube),
                    ..Default::default()
                })
            }
        };

        let environment_map_bind_group =
            self.resources
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Environment map bind group"),
                    layout: &self.resources.skybox_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&environment_map_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(
                                &self
                                    .resources
                                    .environment_builder_data
                                    .environment_map_sampler,
                            ),
                        },
                    ],
                });

        let irradiance_map = self
            .resources
            .texture_builder()
            .name(&format!("{name} irradiance map"))
            .empty(
                wgpu::Extent3d {
                    width: IRRADIANCE_MAP_SIZE,
                    height: IRRADIANCE_MAP_SIZE,
                    depth_or_array_layers: 6,
                },
                IRRADIANCE_MAP_FORMAT,
            )
            .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
            .build(encoder);

        for face in 0..6 {
            let irrandiance_map_view = irradiance_map.create_view(&wgpu::TextureViewDescriptor {
                label: Some("irradiance map attachment render view"),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: face as u32,
                array_layer_count: Some(1),
                ..Default::default()
            });

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("irradiance render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &irrandiance_map_view,
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

            render_pass.set_pipeline(&data.irradiance_pipeline);
            render_pass.set_vertex_buffer(0, data.cube_vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(data.cube_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.set_bind_group(0, &data.view_projection_bind_groups[face], &[]);
            render_pass.set_bind_group(1, &environment_map_bind_group, &[]);
            render_pass.draw_indexed(0..CUBE_VERTEX_COUNT, 0, 0..1);
        }

        let irradiance_map_view = irradiance_map.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&format!("{name} irrandiance map view")),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        let irradiance_map_sampler = self
            .resources
            .environment_builder_data
            .environment_map_sampler
            .clone();

        let irradiance_map_bind_group =
            self.resources
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Environment map bind group"),
                    layout: &self.resources.skybox_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&irradiance_map_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&irradiance_map_sampler),
                        },
                    ],
                });

        self.resources.environments.push(EnvironmentDescriptor {
            name: self.name,
            skybox_bind_group: irradiance_map_bind_group,
            irradiance_map: (irradiance_map_view, irradiance_map_sampler),
        })
    }
}

enum Source<'a> {
    None,
    EquirectangularMap(&'a DynamicImage),
}
