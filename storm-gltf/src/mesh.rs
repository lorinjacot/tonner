use std::{fmt::Display, path::Path};

use anyhow::{Context, Result, anyhow};
use bytemuck::cast_slice;
use glam::{UVec4, Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use storm::{GpuCommandQueue, geometry::GeometryBuilder, mesh::MeshBuilder};

use super::accessor::IteratorConsumer;
use crate::{
    AccessorUsage, GltfError,
    accessor::{AccessorComponentType, AccessorType},
};

/// A set of primitives to be rendered. Its global transform is defined by a node that references it.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Mesh {
    /// [Some] if already loaded.
    #[serde(skip)]
    loaded: Option<storm::mesh::Mesh>,

    /// An array of primitives, each defining geometry to be rendered.
    primitives: Vec<MeshPrimitive>,

    /// Array of weights to be applied to the morph targets. The number of array
    /// elements **MUST** match the number of morph targets.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    weights: Option<Vec<f32>>,

    /// The user-defined name of this object. This is not necessarily unique,
    /// e.g., an accessor and a buffer could have the same name, or two accessors
    /// could even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

impl Mesh {
    /// Array of weights to be applied to the morph targets. The number of array
    /// elements **MUST** match the number of morph targets.
    pub(super) fn weights(&self) -> &Option<Vec<f32>> {
        &self.weights
    }

    pub(super) fn load(
        &mut self,
        base_path: &Path,
        accessors: &[super::Accessor],
        materials: &mut [super::Material],
        default_material: &mut Option<storm::mesh::material::Material>,
        textures: &mut [super::Texture],
        samplers: &mut [super::Sampler],
        images: &mut [super::Image],
        buffer_views: &[super::BufferView],
        buffers: &[super::Buffer],
        gpu_command_queue: &mut GpuCommandQueue,
    ) -> anyhow::Result<storm::mesh::Mesh> {
        if let Some(mesh) = self.loaded.clone() {
            return Ok(mesh);
        }

        let name = self.name.clone();
        let mut mesh_builder = MeshBuilder::default().name(name.unwrap_or_default());

        for (idx, primitive) in self.primitives.iter().enumerate() {
            if let Some(position) = primitive.attributes.position {
                let primitive_ctx = || format!("Failed to load mesh.primitives[{idx}].");

                let material = match primitive.material {
                    Some(index) => materials
                        .get_mut(index)
                        .with_context(|| format!("primitive.material {index} is out of range."))
                        .with_context(primitive_ctx)?
                        .load(
                            base_path,
                            textures,
                            samplers,
                            buffer_views,
                            buffers,
                            images,
                            gpu_command_queue,
                        )
                        .with_context(|| format!("Failed to load primitive.material {index}."))
                        .with_context(primitive_ctx)?,
                    None => match default_material {
                        Some(material) => material.clone(),
                        None => {
                            let material = super::Material::default()
                                .load(
                                    base_path,
                                    textures,
                                    samplers,
                                    buffer_views,
                                    buffers,
                                    images,
                                    gpu_command_queue,
                                )
                                .context("Failed to load default material.")
                                .with_context(primitive_ctx)?;
                            *default_material = Some(material.clone());
                            material
                        }
                    },
                };

                let topology = match primitive.mode {
                    PrimitiveMode::Points => wgpu::PrimitiveTopology::PointList,
                    PrimitiveMode::LineStrip => wgpu::PrimitiveTopology::LineStrip,
                    PrimitiveMode::Lines => wgpu::PrimitiveTopology::LineList,
                    PrimitiveMode::Triangles => wgpu::PrimitiveTopology::TriangleList,
                    PrimitiveMode::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
                    mode => {
                        return Err(anyhow!("primitive topology type {mode} is not supported."))
                            .with_context(primitive_ctx);
                    }
                };
                let accessor = accessors
                    .get(position)
                    .with_context(|| {
                        format!("primitive.attributes.position {position} is out of range.")
                    })
                    .with_context(primitive_ctx)?;

                let normal_tex_coord = material.normal_tex_coord();
                let mut builder = GeometryBuilder::new(accessor.count(), primitive.targets.len())
                    .topology(topology);

                if let Some(normal_tex_coord) = normal_tex_coord {
                    builder = builder.normal_tex_coord(normal_tex_coord);
                }

                macro_rules! consume_attribute {
                    ($accessor:expr, $ty:ty $(, $transform:expr)? ; $load:ident => $register:ident, $ctx:expr) => {{
                        struct Consumer {
                            builder: GeometryBuilder,
                        }

                        impl<'a> IteratorConsumer<'a, $ty> for Consumer {
                            type Return = GeometryBuilder;

                            fn consume<I: Iterator<Item = $ty> + 'a>(
                                self,
                                iter: I,
                            ) -> Result<Self::Return> {
                                Ok(self.builder.$register(iter$(.map($transform))?)?)
                            }
                        }

                        builder = $accessor
                            .$load(buffer_views, buffers, Consumer { builder })
                            .with_context($ctx)
                            .with_context(primitive_ctx)?;
                    }};
                }

                macro_rules! load_attribute {
                    ($attr:ident: $ty:ty; $load:ident => $register:ident) => {
                        if let Some(accessor_idx) = primitive.attributes.$attr {
                            let accessor = accessors
                                .get(accessor_idx)
                                .with_context(|| {
                                    format!(
                                        concat!(
                                            "primitive.attributes.",
                                            stringify!($attr),
                                            " {} is out of range."
                                        ),
                                        accessor_idx
                                    )
                                })
                                .with_context(primitive_ctx)?;

                            let ctx = || {
                                format!(
                                    concat!(
                                        "Failed to load primitive.attributes.",
                                        stringify!($attr),
                                        " {}."
                                    ),
                                    accessor_idx
                                )
                            };

                            consume_attribute!(accessor, $ty; $load => $register, ctx)
                        }
                    };
                }

                let position_ctx = || format!("Failed to load attributes.position {}.", position);
                consume_attribute!(accessor, Vec3; iter_vec3 => positions, position_ctx);

                load_attribute!(normal: Vec3 ; iter_vec3 => normals);
                load_attribute!(tangent: Vec4 ; iter_vec4 => tangents);
                load_attribute!(tex_coord_0: Vec2 ; iter_vec2 => tex_coords_0);
                load_attribute!(tex_coord_1: Vec2 ; iter_vec2 => tex_coords_1);

                if let Some(accessor_idx) = primitive.attributes.color_0 {
                    let accessor = accessors
                        .get(accessor_idx)
                        .with_context(|| {
                            format!(
                                "primitive.attributes.color_0 {} is out of range.",
                                accessor_idx
                            )
                        })
                        .with_context(primitive_ctx)?;

                    let attribute_ctx = || {
                        format!(
                            "Failed to load primitive.attributes.color_0 {}.",
                            accessor_idx
                        )
                    };

                    if let AccessorType::Vec3 = accessor.type_() {
                        consume_attribute!(accessor, Vec3, |v| v.extend(1.0); iter_vec3 => colors_0, attribute_ctx);
                    } else {
                        consume_attribute!(accessor, Vec4; iter_vec4 => colors_0, attribute_ctx);
                    }
                }

                load_attribute!(joints_0: UVec4 ; iter_uvec4 => joints_0);
                load_attribute!(weights_0: Vec4 ; iter_vec4 => weights_0);

                for (target_idx, morph_target) in primitive.targets.iter().enumerate() {
                    let morph_target_ctx =
                        || format!("Failed to load primitive.target[{target_idx}].");

                    macro_rules! consume_morph_attribute {
                        ($accessor:expr, $ty:ty $(, $transform:expr)? ; $load:ident => $register:ident, $ctx:expr) => {{
                            struct Consumer {
                                builder: GeometryBuilder,
                                target_idx: usize,
                            }

                            impl<'a> IteratorConsumer<'a, $ty> for Consumer {
                                type Return = GeometryBuilder;

                                fn consume<I: Iterator<Item = $ty> + 'a>(
                                    self,
                                    iter: I,
                                ) -> Result<Self::Return> {
                                    Ok(self.builder.$register(self.target_idx, iter$(.map($transform))?)?)
                                }
                            }

                            builder = $accessor
                                .$load(buffer_views, buffers, Consumer { builder, target_idx })
                                .with_context($ctx)
                                .with_context(morph_target_ctx)
                                .with_context(primitive_ctx)?;
                        }};
                    }

                    macro_rules! load_morph_attribute {
                        ($attr:ident: $ty:ty; $load:ident => $register:ident) => {
                            if let Some(accessor_idx) = morph_target.$attr {
                                let accessor = accessors
                                                .get(accessor_idx).with_context(|| format!(
                                                    concat!(
                                                        "Morph targets[{}].",
                                                        stringify!($attr),
                                                        " {} is out of range."
                                                    ),
                                                    target_idx,
                                                    accessor_idx
                                                ))
                                                .with_context(morph_target_ctx)
                                                .with_context(primitive_ctx)?;

                                let accessor_ctx = || format!(
                                                            concat!(
                                                                "Failed to load targets[{}].", stringify!($attr), " {}."
                                                            ),
                                                            target_idx,
                                                            accessor_idx
                                                        );

                                consume_morph_attribute!(accessor, $ty; $load => $register, accessor_ctx);
                            }
                        };
                    }

                    load_morph_attribute!(position: Vec3; iter_vec3 => morph_target_positions);
                    load_morph_attribute!(normal: Vec3; iter_vec3 => morph_target_normals);
                    load_morph_attribute!(tangent: Vec3; iter_vec3 => morph_target_tangents);
                    load_morph_attribute!(tex_coord_0: Vec2; iter_vec2 => morph_target_tex_coords_0);
                    load_morph_attribute!(tex_coord_1: Vec2; iter_vec2 => morph_target_tex_coords_1);

                    if let Some(accessor_idx) = morph_target.color_0 {
                        let accessor = accessors
                            .get(accessor_idx)
                            .with_context(|| {
                                format!(
                                    "targets[{}].color_0 {} is out of range.",
                                    target_idx, accessor_idx
                                )
                            })
                            .with_context(morph_target_ctx)
                            .with_context(primitive_ctx)?;

                        let accessor_ctx = || {
                            format!(
                                "Failed to load targets[{}].color_0 {}.",
                                target_idx, accessor_idx
                            )
                        };

                        if let AccessorType::Vec3 = accessor.type_() {
                            consume_morph_attribute!(accessor, Vec3, |v| v.extend(1.0) ; iter_vec3 => morph_target_colors_0, accessor_ctx);
                        } else {
                            consume_morph_attribute!(accessor, Vec4 ; iter_vec4 => morph_target_colors_0, accessor_ctx);
                        }
                    }
                }

                if let Some(indices) = primitive.indices {
                    let accessor = accessors
                        .get(indices)
                        .with_context(|| format!("primitive.indices {indices} is out of range."))
                        .with_context(primitive_ctx)?;

                    let ctx = || format!("Failed to load primitive.indices {indices}.");

                    let bytes = accessor
                        .bytes_dense_tighly_packed(buffer_views, buffers)
                        .with_context(ctx)
                        .with_context(primitive_ctx)?;
                    builder = match (
                        accessor.type_(),
                        accessor.component_type(),
                        accessor.normalized(),
                    ) {
                        (AccessorType::Scalar, AccessorComponentType::UnsignedByte, false) => {
                            builder.indices_u16(bytes.iter().map(|index| *index as u16))
                        }
                        (AccessorType::Scalar, AccessorComponentType::UnsignedShort, false) => {
                            let indices: &[u16] = cast_slice(bytes);
                            builder.indices_u16(indices.iter().cloned())
                        }
                        (AccessorType::Scalar, AccessorComponentType::UnsignedInt, false) => {
                            let indices: &[u32] = cast_slice(bytes);
                            builder.indices_u32(indices.iter().cloned())
                        }
                        (accessor_type, component_type, normalized) => {
                            return Err(GltfError::InvalidAccessorDataType {
                                accessor_type,
                                component_type,
                                normalized,
                                usage: AccessorUsage::Indices,
                            })
                            .with_context(ctx);
                        }
                    }
                }

                let geometry = builder.build(gpu_command_queue.context()).unwrap();
                mesh_builder = mesh_builder.primitive(geometry, material);
            }
        }

        let mesh = mesh_builder.build(gpu_command_queue.context()).unwrap();
        self.loaded = Some(mesh.clone());
        Ok(mesh)
    }
}

/// Geometry to be rendered with the given material.
#[derive(Debug, Serialize, Deserialize)]
struct MeshPrimitive {
    /// A plain JSON object, where each key corresponds to a mesh attribute semantic
    /// and each value is the index of the accessor containing attribute’s data.
    attributes: PrimitiveAttributes,

    /// The index of the accessor that contains the vertex indices. When this is undefined,
    /// the primitive defines non-indexed geometry. When defined, the accessor **MUST** have
    /// [SCALAR](AccessorType::Scalar) type and an unsigned integer component type.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    indices: Option<usize>,

    /// The index of the material to apply to this primitive when rendering.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    material: Option<usize>,

    /// The topology type of primitives to render.
    #[serde(default)]
    #[serde(skip_serializing_if = "PrimitiveMode::is_default")]
    mode: PrimitiveMode,

    /// An array of morph targets.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    targets: Vec<MorphTarget>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PrimitiveAttributes {
    /// Unitless XYZ vertex positions
    #[serde(rename = "POSITION")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<usize>,

    /// Normalized XYZ vertex normals
    #[serde(rename = "NORMAL")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    normal: Option<usize>,

    /// XYZW vertex tangents where the XYZ portion is normalized,
    /// and the W component is a sign value (-1 or +1) indicating
    /// handedness of the tangent basis
    #[serde(rename = "TANGENT")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    tangent: Option<usize>,

    /// ST texture coordinates
    #[serde(rename = "TEXCOORD_0")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    tex_coord_0: Option<usize>,

    /// ST texture coordinates
    #[serde(rename = "TEXCOORD_1")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    tex_coord_1: Option<usize>,

    /// RGB or RGBA vertex color linear multiplier
    #[serde(rename = "COLOR_0")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    color_0: Option<usize>,

    /// Ondices of the skin joints
    #[serde(rename = "JOINTS_0")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    joints_0: Option<usize>,

    /// How strongly the skin joint influences the vertex
    #[serde(rename = "WEIGHTS_0")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    weights_0: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MorphTarget {
    /// XYZ vertex position displacements
    #[serde(rename = "POSITION")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<usize>,

    /// XYZ vertex normal displacements
    #[serde(rename = "NORMAL")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    normal: Option<usize>,

    /// XYZ vertex tangent displacements
    #[serde(rename = "TANGENT")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    tangent: Option<usize>,

    /// ST texture coordinate displacements
    #[serde(rename = "TEXCOORD_0")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    tex_coord_0: Option<usize>,

    /// ST texture coordinate displacements
    #[serde(rename = "TEXCOORD_1")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    tex_coord_1: Option<usize>,

    /// RGB or RGBA color deltas
    #[serde(rename = "COLOR_n")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    color_0: Option<usize>,
}

/// The topology type of primitives to render.
#[derive(Debug, Clone, Copy, Default, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
enum PrimitiveMode {
    Points = 0,
    Lines = 1,
    LineLoop = 2,
    LineStrip = 3,
    #[default]
    Triangles = 4,
    TriangleStrip = 5,
    TriangleFan = 6,
}

impl PrimitiveMode {
    fn is_default(&self) -> bool {
        match self {
            PrimitiveMode::Triangles => true,
            _ => false,
        }
    }
}

impl Display for PrimitiveMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Points => "points",
                Self::Lines => "lines",
                Self::LineLoop => "line loop",
                Self::LineStrip => "line strip",
                Self::Triangles => "triangles",
                Self::TriangleStrip => "triangle strip",
                Self::TriangleFan => "triangle fan",
            }
        )
    }
}
