use std::{collections::HashMap, ops::Index};

use crate::{
    DenseEntry, Id, Resources,
    environment::PREFILTER_MAP_MIP_COUNT,
    geometry::Geometry,
    material::{AlphaMode, Material},
    storage::SparseSet,
};

const ACCUMULATION_BLEND: wgpu::BlendComponent = wgpu::BlendComponent {
    src_factor: wgpu::BlendFactor::One,
    dst_factor: wgpu::BlendFactor::One,
    operation: wgpu::BlendOperation::Add,
};

const REVEALAGE_BLEND: wgpu::BlendComponent = wgpu::BlendComponent {
    src_factor: wgpu::BlendFactor::Zero,
    dst_factor: wgpu::BlendFactor::OneMinusSrc,
    operation: wgpu::BlendOperation::Add,
};

pub struct Mesh {
    id: Id<Mesh>,
    pub name: String,
    primitives: Vec<(Id<PrimitivePipeline>, Id<Geometry>, Id<Material>)>,
}

impl Mesh {
    pub fn primitives(&self) -> &[(Id<PrimitivePipeline>, Id<Geometry>, Id<Material>)] {
        &self.primitives
    }
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
        let manager = &mut self.resources.meshes;
        let id = manager.meshes.next_id();
        let mut morph_target_count = None;
        let primitives = self
            .primitives
            .into_iter()
            .map(|(geometry, material)| {
                let geometry = &self.resources.geometries[geometry];
                let material = &self.resources.materials[material];
                match morph_target_count {
                    Some(count) => assert_eq!(
                        count,
                        geometry.morph_target_count(),
                        "all primitives should have the same number of morph targets"
                    ),
                    None => morph_target_count = Some(geometry.morph_target_count()),
                }

                if material.has_normal_texture() {
                    assert!(geometry.has_tangents());
                }

                let vertex_buffer_layouts = vec![wgpu::VertexBufferLayout {
                    array_stride: 4,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Uint32],
                }];

                let (targets, depth_write_enabled) = match material.alpha_mode() {
                    AlphaMode::Opaque | AlphaMode::Mask => (
                        &[Some(wgpu::TextureFormat::Rgba16Float.into()), None, None],
                        true,
                    ),
                    AlphaMode::Blend => (
                        &[
                            None,
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: Some(wgpu::BlendState {
                                    color: ACCUMULATION_BLEND,
                                    alpha: ACCUMULATION_BLEND,
                                }),
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::R8Unorm,
                                blend: Some(wgpu::BlendState {
                                    color: REVEALAGE_BLEND,
                                    alpha: REVEALAGE_BLEND,
                                }),
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                        ],
                        false,
                    ),
                };

                let mut constants = HashMap::with_capacity(3);
                constants.insert(
                    "has_base_color_texture".to_string(),
                    bool_to_f64(material.has_base_color_texture()),
                );
                constants.insert(
                    "has_metallic_roughness_texture".to_string(),
                    bool_to_f64(material.has_metallic_roughness_texture()),
                );
                constants.insert(
                    "has_normal_texture".to_string(),
                    bool_to_f64(material.has_normal_texture()),
                );
                constants.insert(
                    "has_occlusion_texture".to_string(),
                    bool_to_f64(material.has_occlusion_texture()),
                );
                constants.insert(
                    "has_emissive_texture".to_string(),
                    bool_to_f64(material.has_emissive_texture()),
                );
                constants.insert(
                    "has_normal".to_string(),
                    geometry.has_normal() as u32 as f64,
                );
                constants.insert(
                    "alpha_mode".to_string(),
                    material.alpha_mode() as u32 as f64,
                );
                constants.insert(
                    "max_prefilter_map_mip".to_string(),
                    (PREFILTER_MAP_MIP_COUNT - 1) as f64,
                );

                let pipeline = match manager.primitive_pipelines.iter().find(|pipeline| {
                    pipeline.constants == constants
                        && pipeline.double_sided == material.double_sided()
                        && pipeline.topology == geometry.topology()
                }) {
                    Some(pipeline) => pipeline.id(),
                    None => {
                        let label = format!("Primitive pipeline");
                        let cull_mode = if material.double_sided() {
                            None
                        } else {
                            Some(wgpu::Face::Back)
                        };
                        let mut desc = wgpu::RenderPipelineDescriptor {
                            label: Some(&label),
                            layout: Some(&manager.primitive_pipeline_layout),
                            vertex: wgpu::VertexState {
                                module: &manager.primitive_shader_module,
                                entry_point: Some("vs_main"),
                                compilation_options: wgpu::PipelineCompilationOptions {
                                    constants: &constants,
                                    ..Default::default()
                                },
                                buffers: &vertex_buffer_layouts,
                            },
                            primitive: wgpu::PrimitiveState {
                                topology: geometry.topology(),
                                strip_index_format: None,
                                front_face: wgpu::FrontFace::Ccw,
                                cull_mode,
                                unclipped_depth: false,
                                polygon_mode: wgpu::PolygonMode::Fill,
                                conservative: false,
                            },
                            depth_stencil: Some(wgpu::DepthStencilState {
                                format: wgpu::TextureFormat::Depth24Plus,
                                depth_write_enabled,
                                depth_compare: wgpu::CompareFunction::Less,
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
                                    constants: &constants,
                                    ..Default::default()
                                },
                                targets,
                            }),
                            multiview: None,
                            cache: None,
                        };
                        let pipeline = self.resources.device.create_render_pipeline(&desc);
                        desc.primitive.front_face = wgpu::FrontFace::Cw;
                        let mirror_pipeline = self.resources.device.create_render_pipeline(&desc);

                        let id = manager.primitive_pipelines.next_id();
                        manager
                            .primitive_pipelines
                            .insert(PrimitivePipeline {
                                id,
                                pipeline,
                                mirror_pipeline,
                                constants,
                                double_sided: material.double_sided(),
                                topology: geometry.topology(),
                            })
                            .id()
                    }
                };

                (pipeline, geometry.id(), material.id())
            })
            .collect();
        let mesh = Mesh {
            id,
            name: self.name.unwrap_or_else(|| format!("Mesh {id}")),
            primitives,
        };
        manager.meshes.insert(mesh)
    }
}

pub struct MeshManager {
    meshes: SparseSet<Mesh>,
    primitive_pipeline_layout: wgpu::PipelineLayout,
    primitive_shader_module: wgpu::ShaderModule,
    primitive_pipelines: SparseSet<PrimitivePipeline>,
}

impl MeshManager {
    pub fn new(
        device: &wgpu::Device,
        scene_bind_group_layout: &wgpu::BindGroupLayout,
        geometry_bind_group_layout: &wgpu::BindGroupLayout,
        material_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let primitive_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Primitive pipeline layout"),
                bind_group_layouts: &[
                    scene_bind_group_layout,
                    geometry_bind_group_layout,
                    material_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let primitive_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("primitive.wgsl"));

        Self {
            meshes: SparseSet::new(),
            primitive_pipeline_layout,
            primitive_shader_module,
            primitive_pipelines: SparseSet::new(),
        }
    }

    pub fn primitive_pipeline(
        &self,
        pipeline: Id<PrimitivePipeline>,
    ) -> Option<&PrimitivePipeline> {
        self.primitive_pipelines.get(pipeline)
    }
}

impl Index<Id<Mesh>> for MeshManager {
    type Output = Mesh;

    fn index(&self, index: Id<Mesh>) -> &Self::Output {
        &self.meshes[index]
    }
}

pub struct PrimitivePipeline {
    id: Id<Self>,
    pipeline: wgpu::RenderPipeline,
    mirror_pipeline: wgpu::RenderPipeline,
    constants: HashMap<String, f64>,
    double_sided: bool,
    topology: wgpu::PrimitiveTopology,
}

impl PrimitivePipeline {
    pub fn alpha_mode(&self) -> AlphaMode {
        match self.constants["alpha_mode"] {
            0.0 => AlphaMode::Opaque,
            1.0 => AlphaMode::Mask,
            2.0 => AlphaMode::Blend,
            _ => unreachable!(),
        }
    }

    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    pub fn mirror_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.mirror_pipeline
    }
}

impl DenseEntry for PrimitivePipeline {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

fn bool_to_f64(value: bool) -> f64 {
    if value { 1.0 } else { 0.0 }
}
