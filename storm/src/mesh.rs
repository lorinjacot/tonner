use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
};

use bytemuck::{Pod, Zeroable, cast_slice};
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
    tex_coords: BTreeMap<Attribute, TexCoords<'a>>,
    has_tex_coord_0: f64,
    has_tex_coord_1: f64,
    colors: BTreeMap<Attribute, Colors<'a>>,
    has_color_0: f64,
    vertex_buffers: Vec<wgpu::Buffer>,
    vertex_buffer_layouts: Vec<VertexBufferLayout>,
    material: Option<&'a Material>,
}

impl<'a, 'r> PrimitiveBuilder<'a, 'r> {
    pub fn new(resources: &'r mut Resources) -> Self {
        Self {
            resources,
            vertex_count: 0,
            indices: Indices::None,
            positions: None,
            normals: None,
            tex_coords: BTreeMap::new(),
            has_tex_coord_0: 0.0,
            has_tex_coord_1: 0.0,
            colors: BTreeMap::new(),
            has_color_0: 0.0,
            vertex_buffers: Vec::with_capacity(2),
            vertex_buffer_layouts: Vec::with_capacity(2),
            material: None,
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

    pub fn tex_coords(mut self, set: u32, tex_coords: TexCoords<'a>) -> Self {
        let attribute = match set {
            0 => {
                self.has_tex_coord_0 = 1.0;
                Attribute::TexCoord0
            }
            1 => {
                self.has_tex_coord_1 = 1.0;
                Attribute::TexCoord1
            }
            _ => panic!("only two texture coordinate sets supported"),
        };
        self.tex_coords.insert(attribute, tex_coords);
        self
    }

    pub fn colors(mut self, set: u32, colors: Colors<'a>) -> Self {
        let attribute = match set {
            0 => {
                self.has_color_0 = 1.0;
                Attribute::Color0
            }
            _ => panic!("only one color sets supported"),
        };
        self.colors.insert(attribute, colors);
        self
    }

    pub fn material(mut self, material: &'a Material) -> Self {
        self.material = Some(material);
        self
    }

    pub fn build(mut self) -> Primitive {
        let default_material;
        let material = match self.material {
            Some(material) => material,
            None => {
                default_material = self.resources.material_builder().build();
                &default_material
            }
        };

        let mut create_vertex_buffer =
            |name, contents, array_stride, format, attribute: Attribute| {
                let device = &self.resources.device;
                self.vertex_buffers.push(device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some(name),
                        contents,
                        usage: wgpu::BufferUsages::VERTEX,
                    },
                ));
                self.vertex_buffer_layouts.push(VertexBufferLayout {
                    array_stride,
                    attributes: vec![wgpu::VertexAttribute {
                        format,
                        offset: 0,
                        shader_location: attribute as u32,
                    }],
                });
            };

        if let Some(positions) = self.positions {
            create_vertex_buffer(
                "Position vertex buffer",
                cast_slice(positions),
                4 * 3,
                wgpu::VertexFormat::Float32x3,
                Attribute::Position,
            );
        }
        if let Some(normals) = self.normals {
            create_vertex_buffer(
                "Normal vertex buffer",
                cast_slice(normals),
                4 * 3,
                wgpu::VertexFormat::Float32x3,
                Attribute::Normal,
            );
        }
        for (attribute, tex_coords) in self.tex_coords.iter() {
            let (array_stride, contents, format) = match tex_coords {
                TexCoords::U8(slice) => (1 * 3, cast_slice(slice), wgpu::VertexFormat::Unorm8x2),
                TexCoords::U16(slice) => (2 * 3, cast_slice(slice), wgpu::VertexFormat::Unorm16x2),
                TexCoords::F32(slice) => (4 * 3, cast_slice(slice), wgpu::VertexFormat::Float32x2),
            };
            create_vertex_buffer(
                "Texture coordinate vertex buffer",
                contents,
                array_stride,
                format,
                *attribute,
            );
        }
        for (attribute, colors) in self.colors.iter() {
            let (array_stride, contents, format) = match colors {
                Colors::RgbaU8(slice) => (1 * 4, cast_slice(slice), wgpu::VertexFormat::Unorm8x4),
                Colors::RgbaU16(slice) => (2 * 4, cast_slice(slice), wgpu::VertexFormat::Unorm16x4),
                Colors::RgbaF32(slice) => (4 * 4, cast_slice(slice), wgpu::VertexFormat::Float32x4),
            };
            create_vertex_buffer(
                "Texture coordinate vertex buffer",
                contents,
                array_stride,
                format,
                *attribute,
            );
        }

        let find = |attribute: Attribute| {
            self.vertex_buffer_layouts
                .iter()
                .find(|layout| {
                    layout
                        .attributes
                        .iter()
                        .find(|vertex_attribute| {
                            vertex_attribute.shader_location == attribute as u32
                        })
                        .is_some()
                })
                .is_some()
        };
        let has_tex_coord_0 = find(Attribute::TexCoord0);
        let has_tex_coord_1 = find(Attribute::TexCoord1);
        let has_color_0 = find(Attribute::Color0);

        let mut create_vertex_buffer = |name, contents, format, attribute: Attribute| {
            let device = &self.resources.device;
            self.vertex_buffers.push(device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some(name),
                    contents,
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
            self.vertex_buffer_layouts.push(VertexBufferLayout {
                array_stride: 0,
                attributes: vec![wgpu::VertexAttribute {
                    format,
                    offset: 0,
                    shader_location: attribute as u32,
                }],
            });
        };

        if !has_tex_coord_0 {
            create_vertex_buffer(
                "Dummy tex_coord_0 vertex buffer",
                &[u8::MAX; 2],
                wgpu::VertexFormat::Unorm8x2,
                Attribute::TexCoord0,
            );
        }
        if !has_tex_coord_1 {
            create_vertex_buffer(
                "Dummy tex_coord_1 vertex buffer",
                &[u8::MAX; 2],
                wgpu::VertexFormat::Unorm8x2,
                Attribute::TexCoord1,
            );
        }
        if !has_color_0 {
            create_vertex_buffer(
                "Dummy color_0 vertex buffer",
                &[u8::MAX; 4],
                wgpu::VertexFormat::Unorm8x4,
                Attribute::Color0,
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

        let constants = &mut HashMap::with_capacity(3);
        constants.insert("has_tex_coord_0".to_string(), bool_to_f64(has_tex_coord_0));
        constants.insert("has_tex_coord_1".to_string(), bool_to_f64(has_tex_coord_1));
        constants.insert("has_color_0".to_string(), bool_to_f64(has_color_0));
        constants.insert(
            "has_base_color_texture".to_string(),
            bool_to_f64(material.has_base_color_texture),
        );
        constants.insert(
            "has_metallic_roughness_texture".to_string(),
            bool_to_f64(material.has_metallic_roughness_texture),
        );
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("Primitive pipeline")),
            layout: Some(&data.primitive_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &data.primitive_shader_module,
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
                module: &data.primitive_shader_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants,
                    ..Default::default()
                },
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
            material: material.bind_group.clone(),
        }
    }
}

pub enum Indices<'a> {
    None,
    Slice(&'a [u32]),
}

pub enum TexCoords<'a> {
    U8(Cow<'a, [[u8; 2]]>),
    U16(Cow<'a, [[u16; 2]]>),
    F32(Cow<'a, [[f32; 2]]>),
}

pub enum Colors<'a> {
    RgbaU8(Cow<'a, [[u8; 4]]>),
    RgbaU16(Cow<'a, [[u16; 4]]>),
    RgbaF32(Cow<'a, [[f32; 4]]>),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Attribute {
    Position = 1,
    Normal = 2,
    TexCoord0 = 4,
    TexCoord1 = 5,
    Color0 = 6,
}

struct VertexBufferLayout {
    array_stride: wgpu::BufferAddress,
    attributes: Vec<wgpu::VertexAttribute>,
}

pub struct Material {
    bind_group: wgpu::BindGroup,
    has_base_color_texture: bool,
    has_metallic_roughness_texture: bool,
}

#[must_use]
pub struct MaterialBuilder<'a, 'r> {
    resources: &'r mut Resources,
    base_color_texture: Option<&'a wgpu::TextureView>,
    base_color_sampler: Option<&'a wgpu::Sampler>,
    metallic_roughness_texture: Option<&'a wgpu::TextureView>,
    metallic_roughness_sampler: Option<&'a wgpu::Sampler>,
    uniform: MaterialUniform,
}

impl<'a, 'r> MaterialBuilder<'a, 'r> {
    pub fn new(resources: &'r mut Resources) -> Self {
        let uniform = MaterialUniform {
            base_color_factor: [1.0; 4],
            base_color_tex_coord: 0,
            metallic_factor: 1.0,
            roughness_factor: 1.0,
            metallic_roughness_tex_coord: 0,
        };

        Self {
            resources,
            base_color_texture: None,
            base_color_sampler: None,
            metallic_roughness_texture: None,
            metallic_roughness_sampler: None,
            uniform,
        }
    }

    pub fn base_color_factor(mut self, base_color_factor: [f32; 4]) -> Self {
        self.uniform.base_color_factor = base_color_factor;
        self
    }

    pub fn base_color_tex_coord(mut self, tex_coord: u32) -> Self {
        self.uniform.base_color_tex_coord = tex_coord;
        self
    }

    pub fn base_color_texture(mut self, texture: &'a wgpu::TextureView) -> Self {
        self.base_color_texture = Some(texture);
        self
    }

    pub fn base_color_sampler(mut self, sampler: &'a wgpu::Sampler) -> Self {
        self.base_color_sampler = Some(sampler);
        self
    }

    pub fn metallic_factor(mut self, metallic_factor: f32) -> Self {
        self.uniform.metallic_factor = metallic_factor;
        self
    }

    pub fn roughness_factor(mut self, roughness_factor: f32) -> Self {
        self.uniform.roughness_factor = roughness_factor;
        self
    }

    pub fn metallic_roughness_texture(mut self, texture: &'a wgpu::TextureView) -> Self {
        self.metallic_roughness_texture = Some(texture);
        self
    }

    pub fn metallic_roughness_sampler(mut self, sampler: &'a wgpu::Sampler) -> Self {
        self.metallic_roughness_sampler = Some(sampler);
        self
    }

    pub fn metallic_roughness_tex_coord(mut self, tex_coord: u32) -> Self {
        self.uniform.metallic_roughness_tex_coord = tex_coord;
        self
    }

    pub fn build(self) -> Material {
        let data = &self.resources.mesh_builder_data;
        let has_base_color_texture = self.base_color_texture.is_some();
        let has_metallic_roughness_texture = self.metallic_roughness_texture.is_some();
        let base_color_texture = self.base_color_texture.unwrap_or(&data.default_texture);
        let base_color_sampler = self.base_color_sampler.unwrap_or(&data.default_sampler);
        let metallic_roughness_texture = self
            .metallic_roughness_texture
            .unwrap_or(&data.default_texture);
        let metallic_roughness_sampler = self
            .metallic_roughness_sampler
            .unwrap_or(&data.default_sampler);

        let uniform_buffer =
            self.resources
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Material uniform buffer"),
                    contents: cast_slice(&[self.uniform]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
        let bind_group = self
            .resources
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Material bind gorup"),
                layout: &self.resources.mesh_builder_data.material_bind_group_layout,
                entries: &[
                    // Base color texture
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(base_color_texture),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(base_color_sampler),
                    },
                    // metallic roughness texture
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(metallic_roughness_texture),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(metallic_roughness_sampler),
                    },
                    // Material uniform
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                ],
            });
        Material {
            bind_group,
            has_base_color_texture,
            has_metallic_roughness_texture,
        }
    }
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct MaterialUniform {
    base_color_factor: [f32; 4],
    base_color_tex_coord: u32,
    metallic_factor: f32,
    roughness_factor: f32,
    metallic_roughness_tex_coord: u32,
}

#[derive(Clone)]
pub struct Primitive {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) index_buffer: Option<IndexBuffer>,
    pub(super) vertex_buffers: Vec<wgpu::Buffer>,
    pub(super) vertex_count: u32,
    pub(super) material: wgpu::BindGroup,
}

#[derive(Debug, Clone)]
pub(super) struct IndexBuffer {
    pub(super) buffer: wgpu::Buffer,
    pub(super) format: wgpu::IndexFormat,
}

pub(super) struct MeshBuilderData {
    material_bind_group_layout: wgpu::BindGroupLayout,
    primitive_pipeline_layout: wgpu::PipelineLayout,
    primitive_shader_module: wgpu::ShaderModule,
    default_texture: wgpu::TextureView,
    default_sampler: wgpu::Sampler,
}

impl MeshBuilderData {
    pub fn new(device: &wgpu::Device, render_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Material bind group layout"),
                entries: &[
                    // Base color texture
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
                    // metallic roughness texture
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
                    // Material Uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
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

        let primitive_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(&format!("Primitive pipeline layout")),
                bind_group_layouts: &[render_bind_group_layout, &material_bind_group_layout],
                push_constant_ranges: &[],
            });

        let primitive_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("primitive.wgsl"));

        let default_texture = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Material default texture"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Material default texture view"),
                ..Default::default()
            });

        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Material default sampler"),
            ..Default::default()
        });

        Self {
            material_bind_group_layout,
            primitive_pipeline_layout,
            primitive_shader_module,
            default_texture,
            default_sampler,
        }
    }
}

fn bool_to_f64(value: bool) -> f64 {
    if value { 1.0 } else { 0.0 }
}
