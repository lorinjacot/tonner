use std::collections::HashMap;

pub use environment::Environment;
pub use math::Transform;
pub use scene::{Scene, SceneBuilder};
pub use scene::{camera, skin};
use storage::SparseSet;

pub use asset::geometry;

use crate::asset::geometry::GeometryManager;
use crate::asset::mesh::MeshManager;

mod asset;
mod environment;
mod gltf;
pub mod material;
pub mod math;
pub mod mesh;
mod scene;
mod storage;
mod texture;

/// This is the entry point of the crate.
/// To get started, create a new [Engine] using [EngineBuilder].
/// Once created, an engine can be used to create a [Scene].
/// The engine is also responsible to manage the resources shared between [Scene]s.
pub struct Engine {
    device: wgpu::Device,
    geometry_manager: GeometryManager,
    mesh_manager: MeshManager,
    resources: Resources,
}

impl Engine {
    /// Create an engine builder with default values.
    pub fn builder<'a>() -> EngineBuilder<'a> {
        EngineBuilder::default()
    }
}

/// A builder for [Engine].
#[must_use]
pub struct EngineBuilder<'a> {
    device: Option<(wgpu::Device, wgpu::Queue)>,
    compatible_surface: Option<&'a wgpu::Surface<'a>>,
    target_format: wgpu::TextureFormat,
}

impl<'a> Default for EngineBuilder<'a> {
    fn default() -> Self {
        Self {
            device: None,
            compatible_surface: None,
            target_format: wgpu::TextureFormat::Rgba8UnormSrgb,
        }
    }
}

impl<'a> EngineBuilder<'a> {
    /// Use an existing [wgpu::Device] and [wgpu::Queue].
    pub fn device(mut self, device: wgpu::Device, queue: wgpu::Queue) -> Self {
        self.device = Some((device, queue));
        self
    }

    /// Ensure the engine is compatible with this surface.
    pub fn compatible_surface(mut self, surface: &'a wgpu::Surface<'a>) -> Self {
        self.compatible_surface = Some(surface);
        self
    }

    /// Change the [wgpu::TextureFormat] of the rendering target.
    /// This setting controls the encoding of the rendered [Scene]s.
    pub fn target_format(mut self, target_format: wgpu::TextureFormat) -> Self {
        self.target_format = target_format;
        self
    }

    /// Build the [Engine].
    pub async fn build(self) -> Engine {
        let (device, queue) = match self.device {
            Some(device) => device,
            None => {
                let instance =
                    wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
                let adapter = instance
                    .request_adapter(&wgpu::RequestAdapterOptions::default())
                    .await
                    .expect("Failed to get wgpu adapter");
                adapter
                    .request_device(&wgpu::wgt::DeviceDescriptor::default())
                    .await
                    .expect("Failed to get wgpu device")
            }
        };

        let mut encoder = device.create_command_encoder(&wgpu::wgt::CommandEncoderDescriptor {
            label: Some("Engine builder command encoder"),
        });

        let engine = Engine {
            _scenes: HashMap::new(),
            resources: Resources::new(self.target_format, device, queue, &mut encoder),
        };

        engine.resources.queue.submit([encoder.finish()]);

        engine
    }
}

pub struct Resources {
    device: wgpu::Device,
    queue: wgpu::Queue,
    geometry_builder_data: geometry::GeometryBuilderData,
    geometries: storage::SparseSet<geometry::Geometry>,
    texture_builder_data: texture::TextureBuilderData,
    materials: material::MaterialManager,
    meshes: mesh::MeshManager,
    environments: storage::SparseSet<Environment>,
    environment_builder_data: environment::EnvironmentBuilderData,
    default_environmnent: Option<storage::Id<Environment>>,
    render_bind_group_layout: wgpu::BindGroupLayout,
    skybox_bind_group_layout: wgpu::BindGroupLayout,
    skybox_pipeline: wgpu::RenderPipeline,
    compose_bind_group_layout: wgpu::BindGroupLayout,
    compose_pipeline: wgpu::RenderPipeline,
    brightness_bind_group_layout: wgpu::BindGroupLayout,
    brightness_pipeline: wgpu::RenderPipeline,
    gaussian_blur_bind_group_layout: wgpu::BindGroupLayout,
    gaussian_blur_pipeline: wgpu::RenderPipeline,
    bloom_sampler: wgpu::Sampler,
    tone_mapping_bind_group_layout: wgpu::BindGroupLayout,
    tone_mapping_pipeline: wgpu::RenderPipeline,
}

impl Resources {
    pub fn new(
        render_texture_format: wgpu::TextureFormat,
        device: wgpu::Device,
        queue: wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Self {
        let render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render bind group layout"),
                entries: &[
                    // nodes
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // skins
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // camera
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // lights
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // irradiance map
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::Cube,
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
                    // prefilter map
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::Cube,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // BRDF LUT
                    wgpu::BindGroupLayoutEntry {
                        binding: 8,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 9,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

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

        let geometry_builder_data = geometry::GeometryBuilderData::new(&device);
        let geometries = SparseSet::new();

        let texture_builder_data = texture::TextureBuilderData::new(&device);

        let materials = material::MaterialManager::new(&device);
        let meshes = mesh::MeshManager::new(
            &device,
            &render_bind_group_layout,
            geometry_builder_data.bind_group_layout(),
            materials.bind_group_layout(),
        );

        let environments = SparseSet::new();
        let environment_builder_data =
            environment::EnvironmentBuilderData::new(&device, encoder, &skybox_bind_group_layout);

        let module = &device.create_shader_module(wgpu::include_wgsl!("skybox.wgsl"));
        let skybox_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Skybox pipeline layout"),
                bind_group_layouts: &[&render_bind_group_layout, &skybox_bind_group_layout],
                push_constant_ranges: &[],
            });
        let skybox_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skybox pipeline"),
            layout: Some(&skybox_pipeline_layout),
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
                module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba16Float.into()), None, None],
            }),
            multiview: None,
            cache: None,
        });

        let composer_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("compose.wgsl"));

        let compose_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Compose bind group layout"),
                entries: &[
                    // accumulation texture
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
                    // revealage texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let compose_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Compose pipeline layout"),
                bind_group_layouts: &[&compose_bind_group_layout],
                push_constant_ranges: &[],
            });

        const COMPOSE_BLEND: wgpu::BlendComponent = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let compose_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Compose pipeline"),
            layout: Some(&compose_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &composer_shader_module,
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
                module: &composer_shader_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: Some(wgpu::BlendState {
                        color: COMPOSE_BLEND,
                        alpha: COMPOSE_BLEND,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        let brightness_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("brightness.wgsl"));

        let brightness_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Brightness bind group layout"),
                entries: &[
                    // opaque texture
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
                ],
            });

        let brightness_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Brightness pipeline layout"),
                bind_group_layouts: &[&brightness_bind_group_layout],
                push_constant_ranges: &[],
            });

        let brightness_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Brightness pipeline"),
            layout: Some(&brightness_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &brightness_shader_module,
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
                module: &brightness_shader_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
            }),
            multiview: None,
            cache: None,
        });

        let gaussian_blur_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Gaussian blur bind group layout"),
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

        let gaussian_blur_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Gaussian blur pipeline layout"),
                bind_group_layouts: &[&gaussian_blur_bind_group_layout],
                push_constant_ranges: &[],
            });

        let module = &device.create_shader_module(wgpu::include_wgsl!("gaussian_blur.wgsl"));

        let gaussian_blur_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Gaussian blur pipeline"),
                layout: Some(&gaussian_blur_pipeline_layout),
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
                    targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
                }),
                multiview: None,
                cache: None,
            });

        let bloom_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Bloom texture samlper"),
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let tone_mapping_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
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
                ],
            });

        let tone_mapping_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Tone mapping pipeline layout"),
                bind_group_layouts: &[&tone_mapping_bind_group_layout],
                push_constant_ranges: &[],
            });

        let module = &device.create_shader_module(wgpu::include_wgsl!("tone_mapping.wgsl"));

        let tone_mapping_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Tone mapping pipeline"),
                layout: Some(&tone_mapping_pipeline_layout),
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
                    targets: &[Some(render_texture_format.into())],
                }),
                multiview: None,
                cache: None,
            });

        Self {
            device,
            queue,
            geometry_builder_data,
            geometries,
            texture_builder_data,
            materials,
            meshes,
            environments,
            environment_builder_data,
            default_environmnent: None,
            render_bind_group_layout,
            skybox_bind_group_layout,
            skybox_pipeline,
            compose_bind_group_layout,
            compose_pipeline,
            brightness_bind_group_layout,
            brightness_pipeline,
            gaussian_blur_bind_group_layout,
            gaussian_blur_pipeline,
            bloom_sampler,
            tone_mapping_bind_group_layout,
            tone_mapping_pipeline,
        }
    }

    pub fn mesh_builder(&mut self) -> mesh::MeshBuilder<'_> {
        mesh::MeshBuilder::new(self)
    }

    pub fn environment_builder<'a, 's>(&'s mut self) -> environment::EnvironmentBuilder<'a, 's> {
        environment::EnvironmentBuilder::new(self)
    }
}
