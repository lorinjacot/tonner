use bytemuck::cast_slice;
use wgpu::util::DeviceExt;

use crate::{DenseEntry, Id, Resources, storage::SetEntry};

pub struct Mesh {
    id: Id<Mesh>,
    pub name: String,
    pub(super) primitives: Vec<Primitive>,
}

pub struct MeshDescriptor {
    pub name: Option<String>,
    pub primitives: Vec<Primitive>,
}

impl DenseEntry for Mesh {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

impl SetEntry for Mesh {
    type Descriptor = MeshDescriptor;

    fn new(id: Id<Self::Key>, desc: Self::Descriptor) -> Self {
        Self {
            id,
            name: desc.name.unwrap_or_else(|| id.to_string()),
            primitives: desc.primitives,
        }
    }
}

#[must_use]
pub struct MeshBuilder<'r> {
    resources: &'r mut Resources,
    name: Option<String>,
    primitives: Vec<Primitive>,
}

impl<'r> MeshBuilder<'r> {
    pub fn new(resources: &'r mut Resources) -> Self {
        Self {
            resources,
            name: None,
            primitives: Vec::new(),
        }
    }

    pub fn name(mut self, name: Option<String>) -> Self {
        self.name = name;
        self
    }

    pub fn primitives(mut self, primitives: Vec<Primitive>) -> Self {
        self.primitives = primitives;
        self
    }

    pub fn build(self) -> &'r mut Mesh {
        self.resources.meshes.push(MeshDescriptor {
            name: self.name,
            primitives: self.primitives,
        })
    }
}

#[must_use]
pub struct PrimitiveBuilder<'a, 'r> {
    resources: &'r mut Resources,
    vertex_count: u32,
    indices: Indices<'a>,
    positions: Option<&'a [[f32; 3]]>,
    normals: Option<&'a [[f32; 3]]>,
    vertex_buffers: Vec<wgpu::Buffer>,
    vertex_buffer_layouts: Vec<VertexBufferLayout>,
}

impl<'a, 'r> PrimitiveBuilder<'a, 'r> {
    pub fn new(resources: &'r mut Resources) -> Self {
        Self {
            resources,
            vertex_count: 0,
            indices: Indices::None,
            positions: None,
            normals: None,
            vertex_buffers: Vec::with_capacity(2),
            vertex_buffer_layouts: Vec::with_capacity(2),
        }
    }

    pub fn vertex_count(mut self, vertex_count: u32) -> Self {
        self.vertex_count = vertex_count;
        self
    }

    pub fn indices(mut self, indices: Indices<'a>) -> Self {
        self.indices = indices;
        self
    }

    pub fn positions(mut self, positions: Option<&'a [[f32; 3]]>) -> Self {
        self.positions = positions;
        self
    }

    pub fn normals(mut self, normals: Option<&'a [[f32; 3]]>) -> Self {
        self.normals = normals;
        self
    }

    pub fn build(mut self) -> Primitive {
        if let Some(positions) = self.positions {
            self.create_vertex_buffer(
                "Position vertex buffer",
                cast_slice(positions),
                Attribute::Position,
            );
        }
        if let Some(normals) = self.normals {
            self.create_vertex_buffer(
                "Normal vertex buffer",
                cast_slice(normals),
                Attribute::Normal,
            );
        }

        let device = &self.resources.device;
        let data = &self.resources.mesh_builder_data;

        let mut vertex_buffer_layouts = Vec::with_capacity(1 + self.vertex_buffer_layouts.len());
        vertex_buffer_layouts.push(wgpu::VertexBufferLayout {
            array_stride: 4,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Uint32],
        });
        vertex_buffer_layouts.extend(self.vertex_buffer_layouts.iter().map(|layout| {
            wgpu::VertexBufferLayout {
                array_stride: layout.array_stride,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &layout.attributes,
            }
        }));
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("Primitive pipeline")),
            layout: Some(&data.primitive_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &data.primitive_shader_module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
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
                module: &data.primitive_shader_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(self.resources.render_texture_format.into())],
            }),
            multiview: None,
            cache: None,
        });

        let index_buffer = match self.indices {
            Indices::None => None,
            Indices::Slice(slice) => {
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Primitive index buffer"),
                    contents: cast_slice(slice),
                    usage: wgpu::BufferUsages::INDEX,
                });
                let format = wgpu::IndexFormat::Uint32;
                Some(IndexBuffer { buffer, format })
            }
        };

        let vertex_buffers = self.vertex_buffers;

        Primitive {
            pipeline,
            index_buffer,
            vertex_buffers,
            vertex_count: self.vertex_count,
        }
    }

    fn create_vertex_buffer(&mut self, name: &str, contents: &[u8], attribute: Attribute) {
        let device = &self.resources.device;
        self.vertex_buffers.push(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(name),
                contents,
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );
        self.vertex_buffer_layouts.push(VertexBufferLayout {
            array_stride: 3 * 4,
            attributes: vec![wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: attribute as u32,
            }],
        });
    }
}

pub enum Indices<'a> {
    None,
    Slice(&'a [u32]),
}

#[repr(u32)]
pub enum Attribute {
    Position = 1,
    Normal = 2,
}

struct VertexBufferLayout {
    array_stride: wgpu::BufferAddress,
    attributes: Vec<wgpu::VertexAttribute>,
}

#[derive(Clone)]
pub struct Primitive {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) index_buffer: Option<IndexBuffer>,
    pub(super) vertex_buffers: Vec<wgpu::Buffer>,
    pub(super) vertex_count: u32,
}

#[derive(Debug, Clone)]
pub(super) struct IndexBuffer {
    pub(super) buffer: wgpu::Buffer,
    pub(super) format: wgpu::IndexFormat,
}

pub(super) struct MeshBuilderData {
    primitive_pipeline_layout: wgpu::PipelineLayout,
    primitive_shader_module: wgpu::ShaderModule,
}

impl MeshBuilderData {
    pub fn new(device: &wgpu::Device, render_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let primitive_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(&format!("Primitive pipeline layout")),
                bind_group_layouts: &[render_bind_group_layout],
                push_constant_ranges: &[],
            });

        let primitive_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("primitive.wgsl"));

        Self {
            primitive_pipeline_layout,
            primitive_shader_module,
        }
    }
}
