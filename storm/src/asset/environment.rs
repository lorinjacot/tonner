use std::f32::consts::FRAC_PI_2;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};

use bytemuck::{bytes_of, cast_slice};
use glam::{Mat4, Vec3, vec3};
use image::DynamicImage;
use uuid::Uuid;
use wgpu::util::DeviceExt;

use crate::Context;
use crate::texture::TextureBuilder;

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

const PREFILTER_MAP_SIZE: u32 = 128;
const PREFILTER_MAP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const PREFILTER_MAP_MIP_COUNT: u32 = 5;

const BRDF_LUT_SIZE: u32 = 512;
const BRDF_LUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

/// Unique id for an environment. A environment has one and only one id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnvironmentId(Uuid);

#[derive(Debug, Clone)]
pub struct Environment(Arc<EnvironmentData>);

impl Environment {
    /// Returns the mesh id. The id will never change.
    pub fn id(&self) -> EnvironmentId {
        self.0.id
    }

    /// User-provided name.
    ///
    /// This method will block the current thread until it is able to acquire the name.
    /// When the returned value goes out of scope, the name is released, allowing other
    /// threads to aquire it.
    ///
    /// # Panics
    /// This function might panic when called if the name is already acquired by the current thread.
    pub fn name(&self) -> impl DerefMut<Target = String> {
        self.0.name.lock().unwrap_or_else(|err| {
            let mut inner = err.into_inner();
            *inner = String::new();
            inner
        })
    }

    pub fn skybox_bind_group(&self) -> &wgpu::BindGroup {
        &self.0.skybox_bind_group
    }

    pub fn irradiance_map_view(&self) -> &wgpu::TextureView {
        &self.0.irradiance_map.0
    }

    pub fn irradiance_map_sampler(&self) -> &wgpu::Sampler {
        &self.0.irradiance_map.1
    }

    pub fn prefilter_map_view(&self) -> &wgpu::TextureView {
        &self.0.prefilter_map.0
    }

    pub fn prefilter_map_sampler(&self) -> &wgpu::Sampler {
        &self.0.prefilter_map.1
    }

    pub fn brdf_lut_view(&self) -> &wgpu::TextureView {
        &self.0.brdf_lut.0
    }

    pub fn brdf_lut_sampler(&self) -> &wgpu::Sampler {
        &self.0.brdf_lut.1
    }
}

/// A builder for [`Environment`].
#[derive(Default)]
pub struct EnvironmentBuilder {
    name: String,
    radiance_image: Option<DynamicImage>,
}

impl EnvironmentBuilder {
    /// Give a name for the environment. Useful for GUI and debugging.
    pub fn name(self, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..self
        }
    }

    /// Use an equarectangular map to generate the environment.
    pub fn equirectangular_map(self, radiance_image: impl Into<DynamicImage>) -> Self {
        Self {
            radiance_image: Some(radiance_image.into()),
            ..self
        }
    }

    /// Create the environment.
    pub fn build(self, ctx: &Context, encoder: &mut wgpu::CommandEncoder) -> Environment {
        let name = self.name;
        let environment_map_view = match self.radiance_image {
            Some(radiance_image) => {
                let radiance_texture = TextureBuilder::default()
                    .name(format!("{name} radiance texture").as_ref())
                    .from_dynamic_image(&radiance_image, false)
                    .build(ctx, encoder);
                let radiance_texture_view =
                    radiance_texture.create_view(&wgpu::TextureViewDescriptor {
                        label: Some(&format!("{name} radiance view")),
                        ..Default::default()
                    });

                let radiance_bind_group =
                    ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some(&format!("{name} randiance bind group")),
                        layout: &ctx.environment_ctx.radiance_bind_group_layout,
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
                                    &ctx.environment_ctx.radiance_sampler,
                                ),
                            },
                        ],
                    });

                let environment_cubemap_texture = TextureBuilder::default()
                    .name(Some(format!("{name} environment cubemap").as_ref()))
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
                    .generate_mips()
                    .build_callback(ctx, encoder, |environment_cubemap_texture, encoder| {
                        for face in 0..6 {
                            let environment_map_view = environment_cubemap_texture.create_view(
                                &wgpu::TextureViewDescriptor {
                                    label: Some("environmnent cubemap render view"),
                                    dimension: Some(wgpu::TextureViewDimension::D2),
                                    base_array_layer: face as u32,
                                    array_layer_count: Some(1),
                                    base_mip_level: 0,
                                    mip_level_count: Some(1),
                                    ..Default::default()
                                },
                            );

                            let mut render_pass =
                                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                    label: Some("Equirectangular to cubemap render pass"),
                                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                        view: &environment_map_view,
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

                            render_pass.set_pipeline(
                                &ctx.environment_ctx.equirectangular_to_cubemap_pipeline,
                            );
                            render_pass.set_vertex_buffer(
                                0,
                                ctx.environment_ctx.cube_vertex_buffer.slice(..),
                            );
                            render_pass.set_index_buffer(
                                ctx.environment_ctx.cube_index_buffer.slice(..),
                                wgpu::IndexFormat::Uint16,
                            );
                            render_pass.set_bind_group(
                                0,
                                &ctx.environment_ctx.view_projection_bind_groups[face],
                                &[],
                            );
                            render_pass.set_bind_group(1, &radiance_bind_group, &[]);
                            render_pass.draw_indexed(0..CUBE_VERTEX_COUNT, 0, 0..1);
                        }
                    });

                environment_cubemap_texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(&format!("{name} environment cubemap view")),
                    dimension: Some(wgpu::TextureViewDimension::Cube),
                    ..Default::default()
                })
            }
            None => {
                let radiance_texture = ctx.device.create_texture_with_data(
                    &ctx.queue,
                    &wgpu::TextureDescriptor {
                        label: Some("Default radiance texture"),
                        size: wgpu::Extent3d {
                            width: 1,
                            height: 1,
                            depth_or_array_layers: 6,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    },
                    wgpu::util::TextureDataOrder::LayerMajor,
                    &[u8::MAX; 6 * 4 * 1],
                );

                radiance_texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Default radiance texture view"),
                    dimension: Some(wgpu::TextureViewDimension::Cube),
                    ..Default::default()
                })
            }
        };

        let environment_map_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Environment map bind group"),
            layout: &ctx.environment_ctx.skybox_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&environment_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(
                        &ctx.environment_ctx.environment_map_sampler,
                    ),
                },
            ],
        });

        let irradiance_map = TextureBuilder::default()
            .name(Some(format!("{name} irradiance map").as_ref()))
            .empty(
                wgpu::Extent3d {
                    width: IRRADIANCE_MAP_SIZE,
                    height: IRRADIANCE_MAP_SIZE,
                    depth_or_array_layers: 6,
                },
                IRRADIANCE_MAP_FORMAT,
            )
            .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
            .build(ctx, encoder);

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

            render_pass.set_pipeline(&ctx.environment_ctx.irradiance_pipeline);
            render_pass.set_vertex_buffer(0, ctx.environment_ctx.cube_vertex_buffer.slice(..));
            render_pass.set_index_buffer(
                ctx.environment_ctx.cube_index_buffer.slice(..),
                wgpu::IndexFormat::Uint16,
            );
            render_pass.set_bind_group(
                0,
                &ctx.environment_ctx.view_projection_bind_groups[face],
                &[],
            );
            render_pass.set_bind_group(1, &environment_map_bind_group, &[]);
            render_pass.draw_indexed(0..CUBE_VERTEX_COUNT, 0, 0..1);
        }

        let irradiance_map_view = irradiance_map.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&format!("{name} irrandiance map view")),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        let irradiance_map_sampler = ctx.environment_ctx.environment_map_sampler.clone();

        let prefilter_map = TextureBuilder::default()
            .name(Some(format!("{name} prefilter map").as_ref()))
            .empty(
                wgpu::Extent3d {
                    width: PREFILTER_MAP_SIZE,
                    height: PREFILTER_MAP_SIZE,
                    depth_or_array_layers: 6,
                },
                PREFILTER_MAP_FORMAT,
            )
            .mip_level_count(PREFILTER_MAP_MIP_COUNT)
            .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
            .build(ctx, encoder);

        for mip in 0..PREFILTER_MAP_MIP_COUNT {
            let roughness = mip as f32 / (PREFILTER_MAP_MIP_COUNT - 1) as f32;
            let roughness_buffer =
                ctx.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("prefilter roughness buffer"),
                        contents: bytes_of(&roughness),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
            let roughness_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("prefilter roughness bind group"),
                layout: &ctx.environment_ctx.prefilter_roughness_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: roughness_buffer.as_entire_binding(),
                }],
            });

            for face in 0..6 {
                let prefilter_map_view = prefilter_map.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("prefilter map attachment render view"),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: face as u32,
                    array_layer_count: Some(1),
                    base_mip_level: mip,
                    mip_level_count: Some(1),
                    ..Default::default()
                });

                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("prefilter render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &prefilter_map_view,
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

                render_pass.set_pipeline(&ctx.environment_ctx.prefilter_pipeline);
                render_pass.set_vertex_buffer(0, ctx.environment_ctx.cube_vertex_buffer.slice(..));
                render_pass.set_index_buffer(
                    ctx.environment_ctx.cube_index_buffer.slice(..),
                    wgpu::IndexFormat::Uint16,
                );
                render_pass.set_bind_group(
                    0,
                    &ctx.environment_ctx.view_projection_bind_groups[face],
                    &[],
                );
                render_pass.set_bind_group(1, &environment_map_bind_group, &[]);
                render_pass.set_bind_group(2, &roughness_bind_group, &[]);
                render_pass.draw_indexed(0..CUBE_VERTEX_COUNT, 0, 0..1);
            }
        }

        let prefilter_map_view = prefilter_map.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&format!("{name} prefilter map view")),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        let prefilter_map_sampler = ctx.environment_ctx.environment_map_sampler.clone();

        let brdf_lut = (
            ctx.environment_ctx.brdf_lut_view.clone(),
            ctx.environment_ctx.brdf_lut_sampler.clone(),
        );

        let id = EnvironmentId(Uuid::new_v4());
        let data = EnvironmentData {
            id,
            name: Mutex::new(name),
            skybox_bind_group: environment_map_bind_group,
            irradiance_map: (irradiance_map_view, irradiance_map_sampler),
            prefilter_map: (prefilter_map_view, prefilter_map_sampler),
            brdf_lut,
        };
        Environment(Arc::new(data))
    }
}

#[derive(Debug)]
struct EnvironmentData {
    id: EnvironmentId,
    name: Mutex<String>,
    skybox_bind_group: wgpu::BindGroup,
    irradiance_map: (wgpu::TextureView, wgpu::Sampler),
    prefilter_map: (wgpu::TextureView, wgpu::Sampler),
    brdf_lut: (wgpu::TextureView, wgpu::Sampler),
}

#[derive(Debug, Clone)]
pub(crate) struct EnvironmentContext {
    cube_vertex_buffer: wgpu::Buffer,
    cube_index_buffer: wgpu::Buffer,
    radiance_sampler: wgpu::Sampler,
    brdf_lut_view: wgpu::TextureView,
    brdf_lut_sampler: wgpu::Sampler,
    skybox_bind_group_layout: wgpu::BindGroupLayout,
    radiance_bind_group_layout: wgpu::BindGroupLayout,
    prefilter_roughness_bind_group_layout: wgpu::BindGroupLayout,
    environment_map_sampler: wgpu::Sampler,
    view_projection_bind_groups: [wgpu::BindGroup; 6],
    equirectangular_to_cubemap_pipeline: wgpu::RenderPipeline,
    irradiance_pipeline: wgpu::RenderPipeline,
    prefilter_pipeline: wgpu::RenderPipeline,
}

impl EnvironmentContext {
    pub fn new(device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder) -> Self {
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
        let constants = &[("prefilter_map_size", PREFILTER_MAP_SIZE as f64)];

        let equirectangular_to_cubemap_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Equirectangular to cubemap pipeline"),
                layout: Some(&equirectangular_to_cubemap_pipeline_layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants,
                        ..Default::default()
                    },
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
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants,
                        ..Default::default()
                    },
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
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants,
                    ..Default::default()
                },
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
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants,
                    ..Default::default()
                },
                targets: &[Some(IRRADIANCE_MAP_FORMAT.into())],
            }),
            multiview: None,
            cache: None,
        });

        let prefilter_roughness_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Roughness uniform bind group layout"),
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
            });

        let prefilter_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Prefilter pipeline layout"),
                bind_group_layouts: &[
                    &view_projection_bind_group_layout,
                    &skybox_bind_group_layout,
                    &prefilter_roughness_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let prefilter_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Prefilter pipeline"),
            layout: Some(&prefilter_pipeline_layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants,
                    ..Default::default()
                },
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
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants,
                    ..Default::default()
                },
                targets: &[Some(PREFILTER_MAP_FORMAT.into())],
            }),
            multiview: None,
            cache: None,
        });

        let brdf_lut_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("BRDF LUT pipeline layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        let brdf_lut_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BRDF LUT pipeline"),
            layout: Some(&brdf_lut_pipeline_layout),
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
            multiview: None,
            cache: None,
        });

        let brdf_lut_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("BRDF LUT texture"),
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

        let brdf_lut_view = brdf_lut_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("BRDF LUT texture view"),
            ..Default::default()
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("BRDF lut render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &brdf_lut_view,
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
            render_pass.set_pipeline(&brdf_lut_pipeline);
            render_pass.draw(0..3, 0..1);
        }

        let brdf_lut_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("BRDF LUT sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            cube_vertex_buffer,
            cube_index_buffer,
            radiance_sampler,
            brdf_lut_view,
            brdf_lut_sampler,
            skybox_bind_group_layout,
            radiance_bind_group_layout,
            prefilter_roughness_bind_group_layout,
            environment_map_sampler,
            view_projection_bind_groups,
            equirectangular_to_cubemap_pipeline,
            irradiance_pipeline,
            prefilter_pipeline,
        }
    }
}
