use std::{
    collections::HashMap,
    f32::consts::PI,
    marker::PhantomData,
    ops::{Index, IndexMut},
};

use storm::{
    DenseEntry, GeometryTrait, Id, IndexBuffer, Manager, ResourcesTrait,
    storage::{IntoIter, Iter, IterMut, SparseSet},
};

use crate::{
    MaterialTrait, MeshBuilderTrait, MeshManagerTrait, MeshTrait, ResourcesRendererTrait,
    StormRendererTrait,
};

pub struct Mesh {
    id: Id<Mesh>,
    pub name: String,
    pub primitives: Vec<Primitive>,
}

impl DenseEntry for Mesh {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

impl<Storm> MeshTrait<Storm> for Mesh where Storm: StormRendererTrait<Mesh = Self> {}

pub struct MeshManager<Storm>
where
    Storm: StormRendererTrait<MeshManager = Self>,
{
    meshes: SparseSet<Storm::Mesh>,
    primitive_pipeline_layout: wgpu::PipelineLayout,
    primitive_shader_module: wgpu::ShaderModule,
}

impl<Storm> Index<Id<Storm::Mesh>> for MeshManager<Storm>
where
    Storm: StormRendererTrait<MeshManager = Self>,
{
    type Output = Storm::Mesh;

    fn index(&self, index: Id<Storm::Mesh>) -> &Self::Output {
        &self.meshes[index]
    }
}

impl<Storm> IndexMut<Id<Storm::Mesh>> for MeshManager<Storm>
where
    Storm: StormRendererTrait<MeshManager = Self>,
{
    fn index_mut(&mut self, index: Id<Storm::Mesh>) -> &mut Self::Output {
        &mut self.meshes[index]
    }
}

impl<Storm> IntoIterator for MeshManager<Storm>
where
    Storm: StormRendererTrait<MeshManager = Self>,
{
    type Item = Storm::Mesh;
    type IntoIter = IntoIter<Storm::Mesh>;

    fn into_iter(self) -> Self::IntoIter {
        self.meshes.into_iter()
    }
}

impl<Storm> Manager<Storm::Mesh> for MeshManager<Storm>
where
    Storm: StormRendererTrait<MeshManager = Self>,
{
    type Iter<'a> = Iter<'a, Storm::Mesh>;
    type IterMut<'a> = IterMut<'a, Storm::Mesh>;

    fn get(&self, id: Id<Storm::Mesh>) -> Option<&Storm::Mesh> {
        self.meshes.get(id)
    }

    fn get_mut(&mut self, id: Id<Storm::Mesh>) -> Option<&mut Storm::Mesh> {
        self.meshes.get_mut(id)
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.meshes.iter()
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.meshes.iter_mut()
    }
}

impl<Storm> MeshManagerTrait<Storm> for MeshManager<Storm>
where
    Storm: StormRendererTrait<Mesh = Mesh, MeshManager = Self>,
{
    fn new(
        device: &wgpu::Device,
        scene_bind_group_layout: &wgpu::BindGroupLayout,
        material_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let primitive_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(&format!("Primitive pipeline layout")),
                bind_group_layouts: &[scene_bind_group_layout, &material_bind_group_layout],
                push_constant_ranges: &[],
            });

        let primitive_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("primitive.wgsl"));

        Self {
            meshes: SparseSet::new(),
            primitive_pipeline_layout,
            primitive_shader_module,
        }
    }
}

#[must_use]
pub struct MeshBuilder<'a, 'r, Storm>
where
    Storm: StormRendererTrait<MeshBuilder<'a, 'r> = Self>,
{
    encoder: PhantomData<&'a mut wgpu::CommandEncoder>,
    resources: &'r mut Storm::Resources,
    name: Option<String>,
    primitives: Vec<(Id<Storm::Geometry>, Id<Storm::Material>)>,
}

impl<'a, 'r, Storm> MeshBuilder<'a, 'r, Storm>
where
    Storm: StormRendererTrait<MeshBuilder<'a, 'r> = Self>,
{
    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn primitives(
        mut self,
        primitives: impl IntoIterator<Item = (Id<Storm::Geometry>, Id<Storm::Material>)>,
    ) -> Self {
        self.primitives.extend(primitives);
        self
    }
}

impl<'a, 'r, Storm> MeshBuilderTrait<'a, 'r, Storm> for MeshBuilder<'a, 'r, Storm>
where
    Storm: StormRendererTrait<
            Mesh = Mesh,
            MeshManager = MeshManager<Storm>,
            MeshBuilder<'a, 'r> = Self,
        >,
{
    fn new(resources: &'r mut <Storm>::Resources, _encoder: &'a mut wgpu::CommandEncoder) -> Self {
        Self {
            encoder: PhantomData,
            resources,
            name: None,
            primitives: Vec::new(),
        }
    }

    fn build(self) -> &'r mut <Storm as StormRendererTrait>::Mesh {
        let manager = self.resources.meshes();
        let primitives = self
            .primitives
            .into_iter()
            .map(|(geometry, material)| {
                let geometry = &self.resources.geometries()[geometry];
                let material = &self.resources.materials()[material];

                let geometry_layouts = geometry.vertex_buffer_layouts();

                let mut vertex_buffer_layouts = Vec::with_capacity(1 + geometry_layouts.len());
                vertex_buffer_layouts.push(wgpu::VertexBufferLayout {
                    array_stride: 4,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Uint32],
                });
                vertex_buffer_layouts.extend(geometry_layouts);

                let constants = &mut HashMap::with_capacity(3);
                constants.insert(
                    "has_base_color_texture".to_string(),
                    bool_to_f64(material.has_base_color_texture()),
                );
                constants.insert(
                    "has_metallic_roughness_texture".to_string(),
                    bool_to_f64(material.has_metallic_roughness_texture()),
                );
                constants.insert(
                    "max_prefilter_map_mip".to_string(),
                    // (PREFILTER_MAP_MIP_COUNT - 1) as f64,
                    4.0,
                );
                let pipeline = self.resources.device().create_render_pipeline(
                    &wgpu::RenderPipelineDescriptor {
                        label: Some(&format!("Primitive pipeline")),
                        layout: Some(&manager.primitive_pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: &manager.primitive_shader_module,
                            entry_point: Some("vs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions {
                                constants,
                                ..Default::default()
                            },
                            buffers: &vertex_buffer_layouts,
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
                            module: &manager.primitive_shader_module,
                            entry_point: Some("fs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions {
                                constants,
                                ..Default::default()
                            },
                            targets: &[Some(self.resources.render_texture_format().into())],
                        }),
                        multiview: None,
                        cache: None,
                    },
                );

                let index_buffer = geometry.indices().clone();
                let vertex_buffers = geometry.vertex_buffer().into();
                let vertex_count = geometry.vertex_count();

                Primitive {
                    pipeline,
                    index_buffer,
                    vertex_buffers,
                    vertex_count,
                    material: material.bind_group().clone(),
                }
            })
            .collect();
        let id = manager.meshes.next_id();
        self.resources.meshes_mut().meshes.insert(Mesh {
            id,
            name: self.name.unwrap_or_else(|| format!("Mesh {id}")),
            primitives,
        })
    }
}

#[derive(Clone)]
pub struct Primitive {
    pub pipeline: wgpu::RenderPipeline,
    pub index_buffer: Option<IndexBuffer>,
    pub vertex_buffers: Vec<wgpu::Buffer>,
    pub vertex_count: u32,
    pub material: wgpu::BindGroup,
}

fn bool_to_f64(value: bool) -> f64 {
    if value { 1.0 } else { 0.0 }
}

pub struct SphereDescriptor {
    /// Sphere radius. Default is `1.0`.
    pub radius: f32,
    /// Number of horizontal segments. Minimum value is `3`, and the default is `32`.
    pub width_segments: usize,
    /// Number of vertical segments. Minimum value is `2`, and the default is `16`.
    pub height_segments: usize,
    /// Specify horizontal starting angle. Default is `0.0`.
    pub phi_start: f32,
    /// Specify horizontal sweep angle size. Default is `2.0 * PI`.
    pub phi_length: f32,
    /// Specify vertical starting angle. Default is `0.0`.
    pub theta_start: f32,
    /// Specify vertical sweep angle size. Default is `PI`.
    pub theta_length: f32,
}

impl Default for SphereDescriptor {
    fn default() -> Self {
        Self {
            radius: 1.0,
            width_segments: 32,
            height_segments: 16,
            phi_start: 0.0,
            phi_length: 2.0 * PI,
            theta_start: 0.0,
            theta_length: PI,
        }
    }
}
