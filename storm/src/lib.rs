pub use asset::{environment, geometry, material, mesh};
pub use scene::{Scene, SceneBuilder};
pub use scene::{animation, camera, light, mesh_instance, node, skin};

use environment::EnvironmentManager;
use geometry::GeometryManager;
use material::MaterialManager;
use mesh::MeshManager;
use texture::TextureBuilderData;

mod asset {
    pub mod environment;
    pub mod geometry;
    pub mod material;
    pub mod mesh;
}
// mod gltf;
pub mod math;
pub mod render_target;
mod scene;
mod texture;

/// This is the entry point of the crate.
/// To get started, create a new [Engine] using [EngineBuilder].
/// Once created, an engine can be used to create a [Scene].
/// The engine is also responsible to manage the resources shared between [Scene]s.
pub struct Engine {
    device: wgpu::Device,
    queue: wgpu::Queue,
    geometry_manager: GeometryManager,
    material_manager: MaterialManager,
    mesh_manager: MeshManager,
    texture_builder_data: TextureBuilderData,
    environment_manager: EnvironmentManager,
    render_bind_group_layout: wgpu::BindGroupLayout,
    skybox_bind_group_layout: wgpu::BindGroupLayout,
    compose_bind_group_layout: wgpu::BindGroupLayout,
    brightness_bind_group_layout: wgpu::BindGroupLayout,
    gaussian_blur_bind_group_layout: wgpu::BindGroupLayout,
    tone_mapping_bind_group_layout: wgpu::BindGroupLayout,
    tone_mapping_shader_module: wgpu::ShaderModule,
    tone_mapping_pipeline_layout: wgpu::PipelineLayout,
    bloom_sampler: wgpu::Sampler,
    skybox_pipeline: wgpu::RenderPipeline,
    brightness_pipeline: wgpu::RenderPipeline,
    gaussian_blur_pipeline: wgpu::RenderPipeline,
}

impl Engine {
    /// Convenience function to create a command encoder.
    pub fn encoder(&self, label: Option<&str>) -> wgpu::CommandEncoder {
        self.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label })
    }

    /// At the end of the frame, call this function to submit commands.
    pub fn submit_commands(&self, command_buffer: wgpu::CommandBuffer) {
        self.queue.submit([command_buffer]);
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
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

        let geometry_manager = GeometryManager::new();
        let material_manager = MaterialManager::new(&device);
        let mesh_manager = MeshManager::new(&render_bind_group_layout, &device);
        let texture_builder_data = TextureBuilderData::new(&device);
        let environment_manager =
            EnvironmentManager::new(&skybox_bind_group_layout, &device, &mut encoder);

        let skybox_shader_module = device.create_shader_module(wgpu::include_wgsl!("skybox.wgsl"));
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
                module: &skybox_shader_module,
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
                depth_write_enabled: true,
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
                module: &skybox_shader_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[
                    Some(wgpu::TextureFormat::Rgba16Float.into()),
                    Some(wgpu::TextureFormat::Rgba16Float.into()),
                ],
            }),
            multiview: None,
            cache: None,
        });

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

        let gaussian_blur_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("gaussian_blur.wgsl"));

        let gaussian_blur_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Gaussian blur pipeline"),
                layout: Some(&gaussian_blur_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &gaussian_blur_shader_module,
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
                    module: &gaussian_blur_shader_module,
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

        let tone_mapping_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("tone_mapping.wgsl"));

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

        let engine = Engine {
            device,
            queue,
            geometry_manager,
            material_manager,
            mesh_manager,
            texture_builder_data,
            environment_manager,
            render_bind_group_layout,
            skybox_bind_group_layout,
            compose_bind_group_layout,
            brightness_bind_group_layout,
            gaussian_blur_bind_group_layout,
            tone_mapping_bind_group_layout,
            tone_mapping_shader_module,
            tone_mapping_pipeline_layout,
            bloom_sampler,
            skybox_pipeline,
            brightness_pipeline,
            gaussian_blur_pipeline,
        };

        engine.queue.submit([encoder.finish()]);

        engine
    }
}
