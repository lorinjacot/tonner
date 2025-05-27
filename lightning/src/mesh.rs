use std::{collections::HashMap, f32::consts::PI};

use bytemuck::{Pod, Zeroable, cast_slice};
use wgpu::util::DeviceExt;

use crate::{environment::PREFILTER_MAP_MIP_COUNT, resources::Resources};

use storm::geometry::{Geometry, IndexBuffer};
use storm::{DenseEntry, Id, Resources as ResourcesTrait};

pub struct Mesh {
    id: Id<Mesh>,
    pub name: String,
    pub(super) primitives: Vec<Primitive>,
}

impl DenseEntry for Mesh {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

#[must_use]
pub struct MeshBuilder<'r> {
    resources: &'r mut Resources,
    name: Option<String>,
    primitives: Vec<(Id<Geometry>, Id<Material>)>,
}

impl<'r> MeshBuilder<'r> {
    pub fn new(resources: &'r mut Resources) -> Self {
        Self {
            resources,
            name: None,
            primitives: Vec::new(),
        }
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn primitives(
        mut self,
        primitives: impl IntoIterator<Item = (Id<Geometry>, Id<Material>)>,
    ) -> Self {
        self.primitives.extend(primitives);
        self
    }

    pub fn build(self) -> &'r mut Mesh {
        let id = self.resources.meshes.next_id();
        let primitives = self
            .primitives
            .into_iter()
            .map(|(geometry, material)| {
                let geometry = &self.resources.geometries()[geometry];
                let material = &self.resources.materials[material];

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
                    bool_to_f64(material.has_base_color_texture),
                );
                constants.insert(
                    "has_metallic_roughness_texture".to_string(),
                    bool_to_f64(material.has_metallic_roughness_texture),
                );
                constants.insert(
                    "max_prefilter_map_mip".to_string(),
                    (PREFILTER_MAP_MIP_COUNT - 1) as f64,
                );
                let data = &self.resources.mesh_builder_data;
                let pipeline = self.resources.device().create_render_pipeline(
                    &wgpu::RenderPipelineDescriptor {
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
                    material: material.bind_group.clone(),
                }
            })
            .collect();
        let mesh = Mesh {
            id,
            name: self.name.unwrap_or_else(|| format!("Mesh {id}")),
            primitives,
        };
        self.resources.meshes.insert(mesh)
    }
}

pub struct Material {
    id: Id<Self>,
    bind_group: wgpu::BindGroup,
    has_base_color_texture: bool,
    has_metallic_roughness_texture: bool,
}

impl DenseEntry for Material {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
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

    pub fn build(self) -> &'r mut Material {
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
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Material uniform buffer"),
                    contents: cast_slice(&[self.uniform]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
        let bind_group = self
            .resources
            .device()
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
        let id = self.resources.materials.next_id();
        self.resources.materials.insert(Material {
            id,
            bind_group,
            has_base_color_texture,
            has_metallic_roughness_texture,
        })
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
