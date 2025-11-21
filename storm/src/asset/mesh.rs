use std::{
    collections::HashMap,
    ops::DerefMut,
    sync::{Arc, Mutex},
};

use thiserror::Error;
use uuid::Uuid;

use crate::{
    Engine,
    environment::PREFILTER_MAP_MIP_COUNT,
    geometry::{Geometry, GeometryFlags, GeometryIndices},
    material::{AlphaMode, Material, MaterialFlags},
};

/// A unique id for a [mesh][Mesh]. A mesh will always have the same id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshId(Uuid);

/// A mesh describe a 3D object. It wraps a [Geometry] with a [Material].
#[derive(Clone)]
pub struct Mesh(Arc<MeshData>);

impl Mesh {
    /// Returns the mesh id. The id will never change.
    pub fn id(&self) -> MeshId {
        self.0.id
    }

    /// User-provided name.
    ///
    /// This method will block the current thread until it is able to acquire the name.
    /// When the returned value goes out of scope, the name is released, allowing other
    /// threads to aquire it.
    ///
    /// # Panics
    /// This function might panic when called if the name is already acquired by the current thread.
    pub fn name(&self) -> impl DerefMut<Target = String> {
        self.0.name.lock().unwrap_or_else(|err| {
            let mut inner = err.into_inner();
            *inner = String::new();
            inner
        })
    }

    /// Returns the number of morph target. A morph target is used to deform the mesh based on some
    /// scalar coefficients, called `weights`.
    pub fn morph_target_count(&self) -> usize {
        todo!()
    }

    /// The primitives that are part of this mesh. A primitive is a [`Geometry`] and [`Material`] pair and
    /// describe the shape and material (part) of the mesh.
    pub fn primitives(&self) -> &[MeshPrimitive] {
        todo!()
    }
}

/// Data contained in a [Mesh]. Private to this module.
struct MeshData {
    /// Unique id for the mesh. Will never change.
    id: MeshId,

    /// User-provided name.
    name: Mutex<String>,

    primitives: Vec<MeshPrimitive>,
}

/// A builder for [`Mesh`].
#[must_use]
#[derive(Default)]
pub struct MeshBuilder {
    name: String,
    primitives: Vec<(Geometry, Material)>,
}

impl MeshBuilder {
    /// Gives a name to the mesh. Used for GUI and debugging.
    pub fn name(self, name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..self
        }
    }

    /// Add a new [Geometry] [Material] to the mesh. This function must be called at least once.
    pub fn primitive(
        mut self,
        geometry: impl Into<Geometry>,
        material: impl Into<Material>,
    ) -> Self {
        self.primitives.push((geometry.into(), material.into()));
        self
    }

    /// Create the mesh.
    pub fn build(self, engine: &mut Engine) -> Result<Mesh, MeshBuilderError> {
        let mut primitives = Vec::with_capacity(self.primitives.len());
        let morph_target_count = self
            .primitives
            .first()
            .ok_or(MeshBuilderError::NoPrimitive)?
            .0
            .morph_target_count();
        for (geometry, material) in self.primitives {
            if morph_target_count != geometry.morph_target_count() {
                return Err(MeshBuilderError::InvalidMorphTargetCount);
            }
            if material.has_normal_texture() && !geometry.has_tangent() {
                return Err(MeshBuilderError::NormalTextureWithoutTangent);
            }

            let parameters = PrimitivePipelineParameters {
                geometry_flags: geometry.flags(),
                topology: geometry.topology(),
                material_flags: material.flags(),
                alpha_mode: material.alpha_mode(),
                double_sided: material.double_sided(),
            };

            let render_pipelines = engine
                .mesh_manager
                .get_or_create_render_pipeline(parameters, &engine.device)
                .clone();

            let bind_group = engine.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Mesh primitive bind group"),
                layout: &engine.mesh_manager.primitive_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: geometry.vertex_buffer().as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: material.buffer().as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(
                            material.base_color_texture_view(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(
                            material.base_color_texture_sampler(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(
                            material.metallic_roughness_texture_view(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(
                            material.metallic_roughness_texture_sampler(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::TextureView(
                            material.normal_texture_view(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: wgpu::BindingResource::Sampler(material.normal_texture_sampler()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: wgpu::BindingResource::TextureView(
                            material.occlusion_texture_view(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 9,
                        resource: wgpu::BindingResource::Sampler(
                            material.occlusion_texture_sampler(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 10,
                        resource: wgpu::BindingResource::TextureView(
                            material.emissive_texture_view(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 11,
                        resource: wgpu::BindingResource::Sampler(
                            material.emissive_texture_sampler(),
                        ),
                    },
                ],
            });

            primitives.push(MeshPrimitive {
                geometry,
                material,
                render_pipelines,
                bind_group,
            });
        }

        let id = MeshId(Uuid::new_v4());
        let data = MeshData {
            id,
            name: Mutex::new(self.name),
            primitives,
        };
        let mesh = Mesh(Arc::new(data));
        engine.mesh_manager.meshes.insert(id, mesh.clone());

        Ok(mesh)
    }
}

/// Error when [`MeshBuilder::build`] fails.
#[derive(Debug, Error)]
pub enum MeshBuilderError {
    #[error("cannot create a mesh with no primitive")]
    NoPrimitive,
    #[error("primitive geometries with different morph target count")]
    InvalidMorphTargetCount,
    #[error("cannot use a material containing a normal texture with a geometry without tangents")]
    NormalTextureWithoutTangent,
}

/// A primitive is a [`Geometry`], [`Material`] pair. A [`Mesh`] is described as a list of primitives.
pub struct MeshPrimitive {
    geometry: Geometry,
    material: Material,
    render_pipelines: [wgpu::RenderPipeline; 2],
    bind_group: wgpu::BindGroup,
}

impl MeshPrimitive {
    /// Returns the render pipelines. The first should be used when the model matrix has a positive determinant,
    /// and the second one is for negative determinant.
    ///
    /// TODO: add expected buffer & bind groups & render attachments.
    pub fn render_pipelines(&self) -> &[wgpu::RenderPipeline; 2] {
        &self.render_pipeline
    }

    /// Returns the primitive bind group:
    /// ```wgsl
    /// @group(1) @binding(0) var<storage, read> geometry: GeometryStorage;
    /// @group(1) @binding(1) var<uniform> material_uniform: MaterialUniform;
    /// @group(1) @binding(2) var base_color_texture: texture_2d<f32>;
    /// @group(1) @binding(3) var base_color_sampler: sampler;
    /// @group(1) @binding(4) var metallic_roughness_texture: texture_2d<f32>;
    /// @group(1) @binding(5) var metallic_roughness_sampler: sampler;
    /// @group(1) @binding(6) var normal_texture: texture_2d<f32>;
    /// @group(1) @binding(7) var normal_sampler: sampler;
    /// @group(1) @binding(8) var occlusion_texture: texture_2d<f32>;
    /// @group(1) @binding(9) var occlusion_sampler: sampler;
    /// @group(1) @binding(10) var emissive_texture: texture_2d<f32>;
    /// @group(1) @binding(11) var emissive_sampler: sampler;
    ///
    /// struct GeometryStorage {
    ///     vertex_count: u32,
    ///     target_count: u32,
    ///     attributes: array<Attribute>,
    /// }
    ///
    /// struct Attribute {
    ///     position: vec3<f32>,
    ///     normal: vec3<f32>,
    ///     tangent: vec4<f32>,
    ///     tex_coord_0: vec2<f32>,
    ///     tex_coord_1: vec2<f32>,
    ///     color_0: vec4<f32>,
    ///     joints_0: vec4<u32>,
    ///     weights_0: vec4<f32>,
    /// }
    ///
    /// struct MaterialUniform {
    ///     base_color_factor: vec4<f32>,
    ///     base_color_tex_coord: u32,
    ///     metallic_factor: f32,
    ///     roughness_factor: f32,
    ///     metallic_roughness_tex_coord: u32,
    ///     normal_scale: f32,
    ///     normal_tex_coord: u32,
    ///     occlusion_strength: f32,
    ///     occlusion_tex_coord: u32,
    ///     emissive_factor: vec3<f32>,
    ///     emissive_tex_coord: u32,
    ///     alpha_cutoff: f32,
    /// }
    /// ```
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Describe how to interpret the `alpha` channel of the rendered primitive.
    pub fn alpha_mode(&self) -> AlphaMode {
        todo!()
    }

    /// Return indices data if the primitive has some. Indices are a way to use the same
    /// geometry vertix in multiple triangles.
    pub fn indices(&self) -> &Option<GeometryIndices> {
        self.geometry.indices()
    }

    /// The number of vertices that describe the primitive geometry. If th geometry is indexed,
    /// this number is usually smaller than the index count.
    pub fn vertex_count(&self) -> usize {
        self.geometry.vertex_count()
    }
}

/// A container for all [meshes][Mesh]. This type is used to create, query and delete meshes.
pub(crate) struct MeshManager {
    meshes: HashMap<MeshId, Mesh>,
    primitive_shader_module: wgpu::ShaderModule,
    primitive_bind_group_layout: wgpu::BindGroupLayout,
    primitive_pipelines: HashMap<PrimitivePipelineParameters, wgpu::RenderPipeline>,
}

impl MeshManager {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let primitive_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("primitive.wgsl"));

        let primitive_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Primitive bind group layout"),
                entries: &[
                    // geometry
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
                    // material uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // base color texture view
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
                    // base color texture sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // metallic roughness texture view
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // metallic roughness texture sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // normal texture view
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // normal texture sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // occlusion texture view
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
                    // occlusion texture sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 9,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // emissive texture view
                    wgpu::BindGroupLayoutEntry {
                        binding: 10,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // emissive texture sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 11,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        Self {
            meshes: HashMap::new(),
            primitive_shader_module,
            primitive_bind_group_layout,
            primitive_pipelines: HashMap::new(),
        }
    }

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

    fn get_or_create_render_pipeline(
        &mut self,
        parameters: PrimitivePipelineParameters,
        device: &wgpu::Device,
    ) -> &wgpu::RenderPipeline {
        self.primitive_pipelines
            .entry(parameters)
            .or_insert_with(|| {
                let module = &self.primitive_shader_module;

                let constants = &[
                    ("geometry_flags", parameters.geometry_flags.bits() as f64),
                    ("geometry_flags", parameters.material_flags.bits() as f64),
                    ("alpha_mode", parameters.alpha_mode as u32 as f64),
                    (
                        "max_prefilter_map_mip",
                        (PREFILTER_MAP_MIP_COUNT - 1) as f64,
                    ),
                ];

                let cull_mode = if parameters.double_sided {
                    None
                } else {
                    Some(wgpu::Face::Back)
                };

                let (targets, depth_write_enabled) = match parameters.alpha_mode {
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
                                    color: Self::ACCUMULATION_BLEND,
                                    alpha: Self::ACCUMULATION_BLEND,
                                }),
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::R8Unorm,
                                blend: Some(wgpu::BlendState {
                                    color: Self::REVEALAGE_BLEND,
                                    alpha: Self::REVEALAGE_BLEND,
                                }),
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                        ],
                        false,
                    ),
                };

                let mut desc = wgpu::RenderPipelineDescriptor {
                    label: Some("Primitive (normal) render pass"),
                    layout: Some(self.primitive_bind_group_layout),
                    vertex: wgpu::VertexState {
                        module,
                        entry_point: Some("vs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions {
                            constants,
                            zero_initialize_workgroup_memory: true,
                        },
                        buffers: &[wgpu::VertexBufferLayout {
                            array_stride: 4,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &wgpu::vertex_attr_array![0 => Uint32],
                        }],
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: parameters.topology,
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
                        module,
                        entry_point: Some("fs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions {
                            constants,
                            zero_initialize_workgroup_memory: true,
                        },
                        targets,
                    }),
                    multiview: None,
                    cache: None,
                };

                let normal = device.create_render_pipeline(&desc);

                desc.primitive.front_face = wgpu::FrontFace::Cw;
                let mirrored = device.create_render_pipeline(&desc);

                [normal, mirrored]
            })
    }
}

const PRIMITIVE_OVERRIDE_COUNT: usize = 8;

#[derive(PartialEq, Eq, Hash)]
struct PrimitivePipelineParameters {
    geometry_flags: GeometryFlags,
    topology: wgpu::PrimitiveTopology,
    material_flags: MaterialFlags,
    alpha_mode: AlphaMode,
    double_sided: bool,
}
