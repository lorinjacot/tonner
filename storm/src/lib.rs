pub use asset::open_gltf;
pub use environment::Environment;
use environment::{EnvironmentBuilder, EnvironmentBuilderData};
pub use math::Transform;
use mesh::MeshBuilderData;
pub use mesh::{Material, MaterialBuilder, Mesh, MeshBuilder, PrimitiveBuilder};
pub use scene::camera;
pub use scene::{Node, NodeBuilder, NodeHandle, Scene};
use storage::SparseSet;
pub use storage::{DenseEntry, Id};
use texture::TextureBuilder;

mod asset;
mod environment;
pub mod math;
mod mesh;
mod scene;
mod storage;
mod texture;

pub struct Resources {
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_texture_format: wgpu::TextureFormat,
    meshes: SparseSet<Mesh>,
    mesh_builder_data: MeshBuilderData,
    environments: SparseSet<Environment>,
    environment_builder_data: EnvironmentBuilderData,
    render_bind_group_layout: wgpu::BindGroupLayout,
    skybox_bind_group_layout: wgpu::BindGroupLayout,
    skybox_pipeline: wgpu::RenderPipeline,
}

impl Resources {
    pub fn new(
        render_texture_format: wgpu::TextureFormat,
        device: wgpu::Device,
        queue: wgpu::Queue,
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
                    // irradiance map
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

        let meshes = SparseSet::new();
        let mesh_builder_data = MeshBuilderData::new(&device, &render_bind_group_layout);

        let environments = SparseSet::new();
        let environment_builder_data =
            EnvironmentBuilderData::new(&device, &skybox_bind_group_layout);

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
            meshes,
            mesh_builder_data,
            environments,
            environment_builder_data,
            render_bind_group_layout,
            skybox_bind_group_layout,
            skybox_pipeline,
        }
    }

    pub fn material_builder(&mut self) -> MaterialBuilder {
        MaterialBuilder::new(self)
    }

    pub fn primitive_builder(&mut self) -> PrimitiveBuilder {
        PrimitiveBuilder::new(self)
    }

    pub fn mesh_builder(&mut self) -> MeshBuilder {
        MeshBuilder::new(self)
    }

    fn texture_builder(&self) -> TextureBuilder {
        TextureBuilder::new(self)
    }

    pub fn environment_builder<'a, 's>(&'s mut self) -> EnvironmentBuilder<'a, 's> {
        EnvironmentBuilder::new(self)
    }
}
