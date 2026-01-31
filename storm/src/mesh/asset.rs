use std::{
    hash::Hash,
    ops::DerefMut,
    sync::{Arc, Mutex},
};

use thiserror::Error;
use uuid::Uuid;

use crate::{
    Context,
    geometry::{Geometry, GeometryIndices},
    mesh::{
        PrimitivePipelineParameters,
        material::{AlphaMode, Material},
    },
};

/// A unique id for a [mesh][Mesh]. A mesh will always have the same id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshId(Uuid);

/// A mesh describe a 3D object. It wraps a [Geometry] with a [Material].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

    /// Returns the number of morph target. A morphfis used to deform the mesh based on some
    /// scalar coefficients, called `weights`.
    pub fn morph_target_count(&self) -> usize {
        self.0
            .primitives
            .first()
            .unwrap()
            .geometry
            .morph_target_count()
    }

    /// The primitives that are part of this mesh. A primitive is a [`Geometry`] and [`Material`] pair and
    /// describe the shape and material (part) of the mesh.
    pub fn primitives(&self) -> &[MeshPrimitive] {
        &self.0.primitives
    }
}

/// Data contained in a [Mesh]. Private to this module.
#[derive(Debug)]
struct MeshData {
    /// Unique id for the mesh. Will never change.
    id: MeshId,

    /// User-provided name.
    name: Mutex<String>,

    primitives: Vec<MeshPrimitive>,
}

impl PartialEq for MeshData {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for MeshData {}

impl Hash for MeshData {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
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
            name: name.into(),
            ..self
        }
    }

    /// Add a new [Geometry]-[Material] pair to the mesh. This function must be called at least once.
    pub fn primitive(
        mut self,
        geometry: impl Into<Geometry>,
        material: impl Into<Material>,
    ) -> Self {
        self.primitives.push((geometry.into(), material.into()));
        self
    }

    /// Create the mesh.
    pub fn build(self, ctx: &Context) -> Result<Mesh, MeshBuilderError> {
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

            let render_pipelines = ctx
                .mesh_ctx
                .get_or_create_render_pipeline(parameters, &ctx.device)
                .clone();

            let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Mesh primitive bind group"),
                layout: &ctx.mesh_ctx.primitive_bind_group_layout,
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
                id: MeshPrimitiveId(Uuid::new_v4()),
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
        Ok(Mesh(Arc::new(data)))
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
#[derive(Debug, Clone)]
pub struct MeshPrimitive {
    id: MeshPrimitiveId,
    geometry: Geometry,
    material: Material,
    render_pipelines: [wgpu::RenderPipeline; 2],
    bind_group: wgpu::BindGroup,
}

/// A unique id for [MeshPrimitive]. A mesh primitive has one and only one id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshPrimitiveId(Uuid);

impl MeshPrimitive {
    /// A mesh primitive has one and only one id.
    pub fn id(&self) -> MeshPrimitiveId {
        self.id
    }

    /// Returns the render pipelines. The first should be used when the model matrix has a positive determinant,
    /// and the second one is for negative determinant.
    ///
    /// TODO: add expected buffer & bind groups & render attachments.
    pub fn render_pipelines(&self) -> &[wgpu::RenderPipeline; 2] {
        &self.render_pipelines
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
        self.material.alpha_mode()
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
