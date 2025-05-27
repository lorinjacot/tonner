use storm::geometry::GeometryManager;
use storm::{Id, storage::SparseSet};

use crate::Engine;
use crate::environment::{Environment, EnvironmentBuilder, EnvironmentBuilderData};
use crate::mesh::{Material, MaterialBuilder, Mesh, MeshBuilder, MeshBuilderData};
use crate::texture::{TextureBuilder, TextureBuilderData};

pub struct Resources {
    device: wgpu::Device,
    queue: wgpu::Queue,
    geometries: GeometryManager,
    pub render_texture_format: wgpu::TextureFormat,
    pub texture_builder_data: TextureBuilderData,
    pub materials: SparseSet<Material>,
    pub meshes: SparseSet<Mesh>,
    pub mesh_builder_data: MeshBuilderData,
    pub environments: SparseSet<Environment>,
    pub environment_builder_data: EnvironmentBuilderData,
    pub default_environmnent: Option<Id<Environment>>,
    pub render_bind_group_layout: wgpu::BindGroupLayout,
    pub skybox_bind_group_layout: wgpu::BindGroupLayout,
    pub skybox_pipeline: wgpu::RenderPipeline,
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
                    // camera
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
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
                        binding: 2,
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
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::Cube,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // prefilter map
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::Cube,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // BRDF LUT
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 8,
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

        let geometries = <GeometryManager as storm::GeometryManager<Engine>>::new(&device);

        let texture_builder_data = TextureBuilderData::new(&device);

        let materials = SparseSet::new();
        let meshes = SparseSet::new();
        let mesh_builder_data = MeshBuilderData::new(&device, &render_bind_group_layout);

        let environments = SparseSet::new();
        let environment_builder_data =
            EnvironmentBuilderData::new(&device, encoder, &skybox_bind_group_layout);

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
            render_texture_format,
            geometries,
            texture_builder_data,
            materials,
            meshes,
            mesh_builder_data,
            environments,
            environment_builder_data,
            default_environmnent: None,
            render_bind_group_layout,
            skybox_bind_group_layout,
            skybox_pipeline,
        }
    }

    pub fn material_builder(&mut self) -> MaterialBuilder {
        MaterialBuilder::new(self)
    }

    pub fn mesh_builder(&mut self) -> MeshBuilder {
        MeshBuilder::new(self)
    }

    pub fn texture_builder(&mut self) -> TextureBuilder {
        TextureBuilder::new(self)
    }

    pub fn environment_builder<'a, 's>(&'s mut self) -> EnvironmentBuilder<'a, 's> {
        EnvironmentBuilder::new(self)
    }
}

impl storm::Resources<Engine> for Resources {
    fn device(&self) -> &wgpu::Device {
        &self.device
    }

    fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    fn geometries(&self) -> &<Engine as storm::Storm>::GeometryManager {
        &self.geometries
    }

    fn geometries_mut(&mut self) -> &mut <Engine as storm::Storm>::GeometryManager {
        &mut self.geometries
    }
}
