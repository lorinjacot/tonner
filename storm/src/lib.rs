pub use asset::Asset;
pub use environment::Environment;
use environment::{EnvironmentBuilder, EnvironmentBuilderData};
pub use math::Transform;
pub use mesh::Mesh;
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

pub struct Storm {
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_texture_format: wgpu::TextureFormat,
    assets: SparseSet<Asset>,
    meshes: SparseSet<Mesh>,
    environments: SparseSet<Environment>,
    environment_builder_data: EnvironmentBuilderData,
    scenes: SparseSet<Scene>,
    scene: Option<Id<Scene>>,
    primitive_shader_module: wgpu::ShaderModule,
    render_bind_group_layout: wgpu::BindGroupLayout,
    skybox_bind_group_layout: wgpu::BindGroupLayout,
    skybox_pipeline: wgpu::RenderPipeline,
}

impl Storm {
    pub fn new(
        render_texture_format: wgpu::TextureFormat,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Self {
        let assets = SparseSet::new();
        let meshes = SparseSet::new();
        let environments = SparseSet::new();
        let environment_builder_data = EnvironmentBuilderData::new(&device);
        let scenes = SparseSet::new();

        let primitive_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("primitive.wgsl"));
        let render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render bind group layout"),
                entries: &[
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
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
            render_texture_format,
            assets,
            meshes,
            environments,
            environment_builder_data,
            scenes,
            scene: None,
            primitive_shader_module,
            render_bind_group_layout,
            skybox_bind_group_layout,
            skybox_pipeline,
        }
    }

    fn texture_builder(&self) -> TextureBuilder {
        TextureBuilder::new(self)
    }

    pub fn environment_builder<'a, 's>(&'s mut self) -> EnvironmentBuilder<'a, 's> {
        EnvironmentBuilder::new(self)
    }

    pub fn scene(&self, id: Id<Scene>) -> Option<&Scene> {
        self.scenes.get(id)
    }

    pub fn scenes(&self) -> std::slice::Iter<'_, Scene> {
        self.scenes.iter()
    }

    pub fn scenes_mut(&mut self) -> std::slice::IterMut<'_, Scene> {
        self.scenes.iter_mut()
    }

    pub fn active_scene(&self) -> Option<&Scene> {
        self.scene.map(|scene| &self.scenes[scene])
    }

    pub fn active_scene_mut(&mut self) -> Option<&mut Scene> {
        self.scene.map(|scene| &mut self.scenes[scene])
    }

    pub fn set_active_scene(&mut self, id: Option<Id<Scene>>) {
        self.scene = id;
    }
}
