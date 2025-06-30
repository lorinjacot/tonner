use std::{
    borrow::Cow,
    fmt::Display,
    fs::File,
    io::{BufReader, Cursor, Read},
    marker::PhantomData,
    num::NonZeroUsize,
    path::Path,
};

use bytemuck::{Pod, bytes_of_mut, cast_slice, from_bytes};
use glam::{Mat4, Quat, UVec4, Vec2, Vec3, Vec4, uvec4, vec2, vec3, vec4};
use image::{ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use thiserror::Error;

use crate::{DenseEntry, Id, Resources, geometry::MorphTargetBuilder};

#[derive(Error, Debug)]
pub enum GltfError {
    #[error("Failed to read the asset: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid binary gltf container: {0}")]
    Glb(#[from] GlbError),
    #[error("Failed to parse json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid {entity} index: {index}")]
    InvalidIndex { entity: GltfEntity, index: usize },
    #[error(
        "{usage} cannot have accessor with {accessor_type} of {component_type} (normalized: {normalized})"
    )]
    InvalidAccessorDataType {
        accessor_type: AccessorType,
        component_type: AccessorComponentType,
        normalized: bool,
        usage: AccessorUsage,
    },
    #[error("Failed to read external image: {0}")]
    Image(#[from] image::ImageError),
    #[error("Each image should have either an URI or reference a bufferView")]
    MissingImageContent,
    #[error("Each image referencing a bufferView should have its mimeType defined")]
    MissingImageMimeType,
    #[error("A dense accessor must have its bufferView defined")]
    MissingAccessorBufferView,
    #[error("Unsupported asset: {0}")]
    Unsupported(String),
}

#[derive(Debug)]
pub enum GltfEntity {
    Accessor,
    Buffer,
    BufferView,
    Image,
    Material,
    Mesh,
    Node,
    Sampler,
    Scene,
    Skin,
    Texture,
}

impl Display for GltfEntity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accessor => write!(f, "accessor"),
            Self::Buffer => write!(f, "buffer"),
            Self::BufferView => write!(f, "buffer view"),
            Self::Image => write!(f, "image"),
            Self::Material => write!(f, "material"),
            Self::Mesh => write!(f, "mesh"),
            Self::Node => write!(f, "node"),
            Self::Sampler => write!(f, "sampler"),
            Self::Scene => write!(f, "scene"),
            Self::Skin => write!(f, "skin"),
            Self::Texture => write!(f, "texture"),
        }
    }
}

#[derive(Debug)]
pub enum AccessorUsage {
    Indices,
    Position,
    Normal,
    Tangent,
    TexCoord,
    Color,
    Joints,
    Weights,
    MorphTargetPosition,
    MorphTargetNormal,
    MorphTargetTangent,
    MorphTargetTexCoord,
    MorphTargetColor,
}

impl Display for AccessorUsage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Indices => "Primitive indices",
            Self::Position => "Primitive position attribute",
            Self::Normal => "Primitive normal attribute",
            Self::Tangent => "Primitive tangent attribute",
            Self::TexCoord => "Primitive texture coordinate attribute",
            Self::Color => "Primitive color attribute",
            Self::Joints => "Primitive joints attribute",
            Self::Weights => "Primitive weights attribute",
            Self::MorphTargetPosition => "Primitive morph target position attribute",
            Self::MorphTargetNormal => "Primitive morph target normal attribute",
            Self::MorphTargetTangent => "Primitive morph target tangent attribute",
            Self::MorphTargetTexCoord => "Primitive morph target texture coordinate attribute",
            Self::MorphTargetColor => "Primitive morph target color attribute",
        })
    }
}

type Result<T> = std::result::Result<T, GltfError>;

#[derive(Error, Debug)]
pub enum GlbError {
    #[error("Binary glTF container version should be 2")]
    UnknownVersion,
    #[error("A glb asset must have a JSON chunk as first chunk")]
    JsonChunkMissing,
    #[error("This glb asset must have a BIN chunk as second chunk")]
    BinChunkMissing,
    #[error("Invalid chunk length")]
    InvalidChunkLength,
}

pub struct GltfAsset {
    json: Gltf,
    buffers: Vec<Vec<u8>>,
    default_material: Option<Material>,
}

const GLB_HEADER_SIZE: u32 = 3 * size_of::<u32>() as u32;
const MIN_CHUNK_SIZE: u32 = 2 * size_of::<u32>() as u32;
const ASCII_GLTF: u32 = 0x46546C67;
const ASCII_JSON: u32 = 0x4E4F534A;
const ASCII_BIN: u32 = 0x004E4942;

impl GltfAsset {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let mut magic: u32 = 0;
        reader.read_exact(bytes_of_mut(&mut magic))?;
        if magic != ASCII_GLTF {
            return Err(GltfError::Unsupported(
                "Only Binary glTF (.glb) asset are supported".to_string(),
            ));
        }

        let mut version: u32 = 0;
        reader.read_exact(bytes_of_mut(&mut version))?;
        if version != 2 {
            return Err(GlbError::UnknownVersion.into());
        }

        let mut length: u32 = 0;
        reader.read_exact(bytes_of_mut(&mut length))?;
        length -= GLB_HEADER_SIZE;

        let mut reader = reader.take(length as u64);

        if length < MIN_CHUNK_SIZE {
            return Err(GlbError::JsonChunkMissing.into());
        }
        let json = GlbChunk::from_reader(&mut reader)?;
        length -= MIN_CHUNK_SIZE + json.chunk_length;
        if json.chunk_type != ASCII_JSON {
            return Err(GlbError::JsonChunkMissing.into());
        }
        let json: Gltf = serde_json::from_slice(&json.chunk_data)?;

        let mut buffers = Vec::new();
        match json.buffers.first() {
            Some(buffer) if buffer.uri.is_none() => {
                if length < MIN_CHUNK_SIZE {
                    return Err(GlbError::BinChunkMissing.into());
                }
                let bin = GlbChunk::from_reader(&mut reader)?;
                if bin.chunk_type != ASCII_BIN {
                    return Err(GlbError::BinChunkMissing.into());
                }
                buffers.push(bin.chunk_data);
            }
            _ => (),
        }

        Ok(Self {
            json,
            buffers,
            default_material: None,
        })
    }

    pub fn load_scene(
        &mut self,
        scene_index: usize,
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
        render_width: u32,
        render_height: u32,
    ) -> Result<crate::Scene> {
        let scene = self
            .json
            .scenes
            .get(scene_index)
            .ok_or(GltfError::InvalidIndex {
                entity: GltfEntity::Scene,
                index: scene_index,
            })?;

        let name = scene.name.clone().unwrap_or_default();
        let root_nodes = scene.nodes.clone();

        let mut scene = crate::Scene::new(name, resources, encoder, render_width, render_height);

        for node in root_nodes {
            self.load_node(node, None, &mut scene, resources, encoder)?;
        }

        self.load_skins();

        self.load_animations();

        for node in &mut self.json.nodes {
            node.id = None;
        }

        Ok(scene)
    }

    fn load_node(
        &mut self,
        index: usize,
        parent: Option<Id<crate::Node>>,
        scene: &mut crate::Scene,
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<Id<crate::Node>> {
        let node = self.json.nodes.get(index).ok_or(GltfError::InvalidIndex {
            entity: GltfEntity::Node,
            index,
        })?;

        let mesh = match node.mesh {
            Some(index) => Some(self.get_or_load_mesh(index, resources, encoder)?),
            None => None,
        };

        let node = &mut self.json.nodes[index];
        let id = scene
            .node_builder()
            .name(node.name.clone())
            .parent(parent)
            .local_matrix(node.matrix.map(|m| Mat4::from_cols_array(&m)))
            .translation_rotation_scale(
                node.translation.map(|a| Vec3::from_array(a)),
                node.rotation.map(|a| Quat::from_array(a)),
                node.scale.map(|a| Vec3::from_array(a)),
            )
            .mesh(mesh)
            .weights(
                node.weights
                    .clone()
                    .or(self.json.meshes[index].weights.clone()),
            )
            .build(resources)
            .id();

        node.id = Some(id);
        if let Some(index) = node.skin {
            self.json
                .skins
                .get_mut(index)
                .ok_or(GltfError::InvalidIndex {
                    entity: GltfEntity::Skin,
                    index,
                })?
                .nodes
                .push(id);
        }

        let children = node.children.clone();
        for child in children {
            self.load_node(child, Some(id), scene, resources, encoder)?;
        }

        Ok(id)
    }

    fn load_skins(&mut self) {
        // todo!()
        println!("TODO: load_skins")
    }

    fn load_animations(&mut self) {
        // todo!()
        println!("TODO: load_animations");
    }

    fn get_or_load_mesh(
        &mut self,
        index: usize,
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<Id<crate::mesh::Mesh>> {
        let mesh = self.json.meshes.get(index).ok_or(GltfError::InvalidIndex {
            entity: GltfEntity::Mesh,
            index,
        })?;
        if let Some(id) = mesh.id {
            return Ok(id);
        }
        let name = mesh.name.clone();

        let mut primitives = Vec::with_capacity(mesh.primitives.len());
        for primitive in &mesh.primitives {
            if let Some(position) = primitive.attributes.position {
                let topology = match primitive.mode {
                    PrimitiveMode::Points => wgpu::PrimitiveTopology::PointList,
                    PrimitiveMode::LineStrip => wgpu::PrimitiveTopology::LineStrip,
                    PrimitiveMode::Lines => wgpu::PrimitiveTopology::LineList,
                    PrimitiveMode::Triangles => wgpu::PrimitiveTopology::TriangleList,
                    PrimitiveMode::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
                    _ => {
                        return Err(GltfError::Unsupported(format!(
                            "primitive topology type {} is not supported",
                            primitive.mode
                        )));
                    }
                };

                let accessor =
                    self.json
                        .accessors
                        .get(position)
                        .ok_or(GltfError::InvalidIndex {
                            entity: GltfEntity::Accessor,
                            index: position,
                        })?;

                let positions = match (accessor.type_, accessor.component_type, accessor.normalized)
                {
                    (AccessorType::Vec3, AccessorComponentType::Float, false) => self
                        .accessor_iter::<f32, 3>(position)?
                        .cloned()
                        .map(Vec3::from_array),
                    (accessor_type, component_type, normalized) => {
                        return Err(GltfError::InvalidAccessorDataType {
                            accessor_type,
                            component_type,
                            normalized,
                            usage: AccessorUsage::Position,
                        });
                    }
                };

                let mut builder = resources
                    .geometry_builder()
                    .positions(positions)
                    .topology(topology);

                if let Some(normal) = primitive.attributes.normal {
                    let accessor =
                        self.json
                            .accessors
                            .get(normal)
                            .ok_or(GltfError::InvalidIndex {
                                entity: GltfEntity::Accessor,
                                index: normal,
                            })?;

                    builder = match (accessor.type_, accessor.component_type, accessor.normalized) {
                        (AccessorType::Vec3, AccessorComponentType::Float, false) => builder
                            .normals(
                                self.accessor_iter::<f32, 3>(normal)?
                                    .cloned()
                                    .map(Vec3::from_array),
                            ),
                        (accessor_type, component_type, normalized) => {
                            return Err(GltfError::InvalidAccessorDataType {
                                accessor_type,
                                component_type,
                                normalized,
                                usage: AccessorUsage::Normal,
                            });
                        }
                    };
                }

                if let Some(tangent) = primitive.attributes.tangent {
                    let accessor =
                        self.json
                            .accessors
                            .get(tangent)
                            .ok_or(GltfError::InvalidIndex {
                                entity: GltfEntity::Accessor,
                                index: tangent,
                            })?;

                    builder = match (accessor.type_, accessor.component_type, accessor.normalized) {
                        (AccessorType::Vec4, AccessorComponentType::Float, false) => builder
                            .tangents(
                                self.accessor_iter::<f32, 4>(tangent)?
                                    .cloned()
                                    .map(Vec4::from_array),
                            ),
                        (accessor_type, component_type, normalized) => {
                            return Err(GltfError::InvalidAccessorDataType {
                                accessor_type,
                                component_type,
                                normalized,
                                usage: AccessorUsage::Tangent,
                            });
                        }
                    };
                }

                for tex_coord in [
                    primitive.attributes.tex_coord_0,
                    primitive.attributes.tex_coord_1,
                ] {
                    if let Some(tex_coord) = tex_coord {
                        let accessor =
                            self.json
                                .accessors
                                .get(tex_coord)
                                .ok_or(GltfError::InvalidIndex {
                                    entity: GltfEntity::Accessor,
                                    index: tex_coord,
                                })?;

                        builder =
                            match (accessor.type_, accessor.component_type, accessor.normalized) {
                                (AccessorType::Vec2, AccessorComponentType::Float, false) => {
                                    builder.tex_coords(
                                        self.accessor_iter::<f32, 2>(tex_coord)?
                                            .cloned()
                                            .map(Vec2::from_array),
                                    )
                                }
                                (AccessorType::Vec2, AccessorComponentType::UnsignedByte, true) => {
                                    builder.tex_coords(
                                        self.accessor_iter::<u8, 2>(tex_coord)?.map(u8x2_to_vec2),
                                    )
                                }
                                (
                                    AccessorType::Vec2,
                                    AccessorComponentType::UnsignedShort,
                                    true,
                                ) => builder.tex_coords(
                                    self.accessor_iter::<u16, 2>(tex_coord)?.map(u16x2_to_vec2),
                                ),
                                (accessor_type, component_type, normalized) => {
                                    return Err(GltfError::InvalidAccessorDataType {
                                        accessor_type,
                                        component_type,
                                        normalized,
                                        usage: AccessorUsage::TexCoord,
                                    });
                                }
                            };
                    }
                }

                if let Some(color) = primitive.attributes.color_0 {
                    let accessor =
                        self.json
                            .accessors
                            .get(color)
                            .ok_or(GltfError::InvalidIndex {
                                entity: GltfEntity::Accessor,
                                index: color,
                            })?;

                    builder = match (accessor.type_, accessor.component_type, accessor.normalized) {
                        (AccessorType::Vec3, AccessorComponentType::Float, false) => builder
                            .colors(
                                self.accessor_iter::<f32, 3>(color)?
                                    .cloned()
                                    .map(Vec3::from_array)
                                    .map(|v| v.extend(1.0)),
                            ),
                        (AccessorType::Vec3, AccessorComponentType::UnsignedByte, true) => builder
                            .colors(
                                self.accessor_iter::<u8, 3>(color)?
                                    .map(u8x3_to_vec3)
                                    .map(|v| v.extend(1.0)),
                            ),
                        (AccessorType::Vec3, AccessorComponentType::UnsignedShort, true) => builder
                            .colors(
                                self.accessor_iter::<u16, 3>(color)?
                                    .map(u16x3_to_vec3)
                                    .map(|v| v.extend(1.0)),
                            ),
                        (AccessorType::Vec4, AccessorComponentType::Float, false) => builder
                            .colors(
                                self.accessor_iter::<f32, 4>(color)?
                                    .cloned()
                                    .map(Vec4::from_array),
                            ),
                        (AccessorType::Vec4, AccessorComponentType::UnsignedByte, true) => {
                            builder.colors(self.accessor_iter::<u8, 4>(color)?.map(u8x4_to_vec4))
                        }
                        (AccessorType::Vec4, AccessorComponentType::UnsignedShort, true) => {
                            builder.colors(self.accessor_iter::<u16, 4>(color)?.map(u16x4_to_vec4))
                        }
                        (accessor_type, component_type, normalized) => {
                            return Err(GltfError::InvalidAccessorDataType {
                                accessor_type,
                                component_type,
                                normalized,
                                usage: AccessorUsage::Color,
                            });
                        }
                    };
                }

                if let Some(joints) = primitive.attributes.joints_0 {
                    let accessor =
                        self.json
                            .accessors
                            .get(joints)
                            .ok_or(GltfError::InvalidIndex {
                                entity: GltfEntity::Accessor,
                                index: joints,
                            })?;

                    builder = match (accessor.type_, accessor.component_type, accessor.normalized) {
                        (AccessorType::Vec4, AccessorComponentType::UnsignedByte, false) => {
                            builder.joints(self.accessor_iter::<u8, 4>(joints)?.map(u8x4_to_uvec4))
                        }
                        (AccessorType::Vec4, AccessorComponentType::UnsignedShort, false) => {
                            builder
                                .joints(self.accessor_iter::<u16, 4>(joints)?.map(u16x4_to_uvec4))
                        }
                        (accessor_type, component_type, normalized) => {
                            return Err(GltfError::InvalidAccessorDataType {
                                accessor_type,
                                component_type,
                                normalized,
                                usage: AccessorUsage::Joints,
                            });
                        }
                    };
                }

                if let Some(weights) = primitive.attributes.weights_0 {
                    let accessor =
                        self.json
                            .accessors
                            .get(weights)
                            .ok_or(GltfError::InvalidIndex {
                                entity: GltfEntity::Accessor,
                                index: weights,
                            })?;

                    builder = match (accessor.type_, accessor.component_type, accessor.normalized) {
                        (AccessorType::Vec4, AccessorComponentType::Float, false) => builder
                            .weights(
                                self.accessor_iter::<f32, 4>(weights)?
                                    .cloned()
                                    .map(Vec4::from_array),
                            ),
                        (AccessorType::Vec4, AccessorComponentType::UnsignedByte, true) => {
                            builder.weights(self.accessor_iter::<u8, 4>(weights)?.map(u8x4_to_vec4))
                        }
                        (AccessorType::Vec4, AccessorComponentType::UnsignedShort, true) => builder
                            .weights(self.accessor_iter::<u16, 4>(weights)?.map(u16x4_to_vec4)),
                        (accessor_type, component_type, normalized) => {
                            return Err(GltfError::InvalidAccessorDataType {
                                accessor_type,
                                component_type,
                                normalized,
                                usage: AccessorUsage::Weights,
                            });
                        }
                    };
                }

                for morph_target in &primitive.targets {
                    let mut target_builder = MorphTargetBuilder::new();

                    if let Some(position) = morph_target.position {
                        let accessor =
                            self.json
                                .accessors
                                .get(position)
                                .ok_or(GltfError::InvalidIndex {
                                    entity: GltfEntity::Accessor,
                                    index: position,
                                })?;

                        target_builder =
                            match (accessor.type_, accessor.component_type, accessor.normalized) {
                                (AccessorType::Vec3, AccessorComponentType::Float, false) => {
                                    target_builder.positions(
                                        self.accessor_iter::<f32, 3>(position)?
                                            .cloned()
                                            .map(Vec3::from_array),
                                    )
                                }
                                (accessor_type, component_type, normalized) => {
                                    return Err(GltfError::InvalidAccessorDataType {
                                        accessor_type,
                                        component_type,
                                        normalized,
                                        usage: AccessorUsage::MorphTargetPosition,
                                    });
                                }
                            };
                    }

                    if let Some(normal) = morph_target.normal {
                        let accessor =
                            self.json
                                .accessors
                                .get(normal)
                                .ok_or(GltfError::InvalidIndex {
                                    entity: GltfEntity::Accessor,
                                    index: normal,
                                })?;

                        target_builder =
                            match (accessor.type_, accessor.component_type, accessor.normalized) {
                                (AccessorType::Vec3, AccessorComponentType::Float, false) => {
                                    target_builder.normals(
                                        self.accessor_iter::<f32, 3>(normal)?
                                            .cloned()
                                            .map(Vec3::from_array),
                                    )
                                }
                                (accessor_type, component_type, normalized) => {
                                    return Err(GltfError::InvalidAccessorDataType {
                                        accessor_type,
                                        component_type,
                                        normalized,
                                        usage: AccessorUsage::MorphTargetNormal,
                                    });
                                }
                            };
                    }

                    if let Some(tangent) = morph_target.tangent {
                        let accessor =
                            self.json
                                .accessors
                                .get(tangent)
                                .ok_or(GltfError::InvalidIndex {
                                    entity: GltfEntity::Accessor,
                                    index: tangent,
                                })?;

                        target_builder =
                            match (accessor.type_, accessor.component_type, accessor.normalized) {
                                (AccessorType::Vec3, AccessorComponentType::Float, false) => {
                                    target_builder.tangents(
                                        self.accessor_iter::<f32, 3>(tangent)?
                                            .cloned()
                                            .map(Vec3::from_array),
                                    )
                                }
                                (accessor_type, component_type, normalized) => {
                                    return Err(GltfError::InvalidAccessorDataType {
                                        accessor_type,
                                        component_type,
                                        normalized,
                                        usage: AccessorUsage::MorphTargetTangent,
                                    });
                                }
                            };
                    }

                    for tex_coord in [morph_target.tex_coord_0, morph_target.tex_coord_1] {
                        if let Some(tex_coord) = tex_coord {
                            let accessor = self.json.accessors.get(tex_coord).ok_or(
                                GltfError::InvalidIndex {
                                    entity: GltfEntity::Accessor,
                                    index: tex_coord,
                                },
                            )?;

                            target_builder = match (
                                accessor.type_,
                                accessor.component_type,
                                accessor.normalized,
                            ) {
                                (AccessorType::Vec2, AccessorComponentType::Float, false) => {
                                    target_builder.tex_coords(
                                        self.accessor_iter::<f32, 2>(tex_coord)?
                                            .cloned()
                                            .map(Vec2::from_array),
                                    )
                                }
                                (AccessorType::Vec2, AccessorComponentType::Byte, true) => {
                                    target_builder.tex_coords(
                                        self.accessor_iter::<i8, 2>(tex_coord)?.map(i8x2_to_vec2),
                                    )
                                }
                                (AccessorType::Vec2, AccessorComponentType::Short, true) => {
                                    target_builder.tex_coords(
                                        self.accessor_iter::<i16, 2>(tex_coord)?.map(i16x2_to_vec2),
                                    )
                                }
                                (AccessorType::Vec2, AccessorComponentType::UnsignedByte, true) => {
                                    target_builder.tex_coords(
                                        self.accessor_iter::<u8, 2>(tex_coord)?.map(u8x2_to_vec2),
                                    )
                                }
                                (
                                    AccessorType::Vec2,
                                    AccessorComponentType::UnsignedShort,
                                    true,
                                ) => target_builder.tex_coords(
                                    self.accessor_iter::<u16, 2>(tex_coord)?.map(u16x2_to_vec2),
                                ),
                                (accessor_type, component_type, normalized) => {
                                    return Err(GltfError::InvalidAccessorDataType {
                                        accessor_type,
                                        component_type,
                                        normalized,
                                        usage: AccessorUsage::MorphTargetTexCoord,
                                    });
                                }
                            };
                        }
                    }

                    if let Some(color) = morph_target.color_0 {
                        let accessor =
                            self.json
                                .accessors
                                .get(color)
                                .ok_or(GltfError::InvalidIndex {
                                    entity: GltfEntity::Accessor,
                                    index: color,
                                })?;

                        builder =
                            match (accessor.type_, accessor.component_type, accessor.normalized) {
                                (AccessorType::Vec3, AccessorComponentType::Float, false) => {
                                    builder.colors(
                                        self.accessor_iter::<f32, 3>(color)?
                                            .cloned()
                                            .map(Vec3::from_array)
                                            .map(|v| v.extend(1.0)),
                                    )
                                }
                                (AccessorType::Vec3, AccessorComponentType::Byte, true) => builder
                                    .colors(
                                        self.accessor_iter::<i8, 3>(color)?
                                            .map(i8x3_to_vec3)
                                            .map(|v| v.extend(1.0)),
                                    ),
                                (AccessorType::Vec3, AccessorComponentType::Short, true) => builder
                                    .colors(
                                        self.accessor_iter::<i16, 3>(color)?
                                            .map(i16x3_to_vec3)
                                            .map(|v| v.extend(1.0)),
                                    ),
                                (AccessorType::Vec3, AccessorComponentType::UnsignedByte, true) => {
                                    builder.colors(
                                        self.accessor_iter::<u8, 3>(color)?
                                            .map(u8x3_to_vec3)
                                            .map(|v| v.extend(1.0)),
                                    )
                                }
                                (
                                    AccessorType::Vec3,
                                    AccessorComponentType::UnsignedShort,
                                    true,
                                ) => builder.colors(
                                    self.accessor_iter::<u16, 3>(color)?
                                        .map(u16x3_to_vec3)
                                        .map(|v| v.extend(1.0)),
                                ),
                                (AccessorType::Vec4, AccessorComponentType::Float, false) => {
                                    builder.colors(
                                        self.accessor_iter::<f32, 4>(color)?
                                            .cloned()
                                            .map(Vec4::from_array),
                                    )
                                }
                                (AccessorType::Vec4, AccessorComponentType::Byte, true) => builder
                                    .colors(self.accessor_iter::<i8, 4>(color)?.map(i8x4_to_vec4)),
                                (AccessorType::Vec4, AccessorComponentType::Short, true) => builder
                                    .colors(
                                        self.accessor_iter::<i16, 4>(color)?.map(i16x4_to_vec4),
                                    ),
                                (AccessorType::Vec4, AccessorComponentType::UnsignedByte, true) => {
                                    builder.colors(
                                        self.accessor_iter::<u8, 4>(color)?.map(u8x4_to_vec4),
                                    )
                                }
                                (
                                    AccessorType::Vec4,
                                    AccessorComponentType::UnsignedShort,
                                    true,
                                ) => builder.colors(
                                    self.accessor_iter::<u16, 4>(color)?.map(u16x4_to_vec4),
                                ),
                                (accessor_type, component_type, normalized) => {
                                    return Err(GltfError::InvalidAccessorDataType {
                                        accessor_type,
                                        component_type,
                                        normalized,
                                        usage: AccessorUsage::MorphTargetColor,
                                    });
                                }
                            };
                    }

                    builder = builder.morph_target(target_builder);
                }

                if let Some(indices) = primitive.indices {
                    let accessor =
                        self.json
                            .accessors
                            .get(indices)
                            .ok_or(GltfError::InvalidIndex {
                                entity: GltfEntity::Accessor,
                                index: indices,
                            })?;
                    let bytes = match &accessor.sparse {
                        Some(_sparse) => todo!("sparse accessor support"),
                        None => self.get_buffer_view(
                            accessor
                                .buffer_view
                                .ok_or(GltfError::MissingAccessorBufferView)?,
                        )?,
                    };
                    builder = match (accessor.type_, accessor.component_type, accessor.normalized) {
                        (AccessorType::Scalar, AccessorComponentType::UnsignedByte, false) => {
                            let indices: &[u8] = cast_slice(bytes);
                            builder.indices_u16(
                                indices.into_iter().map(|index| *index as u16).collect(),
                            )
                        }
                        (AccessorType::Scalar, AccessorComponentType::UnsignedShort, false) => {
                            builder.indices_u16(Cow::Borrowed(cast_slice(bytes)))
                        }
                        (AccessorType::Scalar, AccessorComponentType::UnsignedInt, false) => {
                            builder.indices_u32(Cow::Borrowed(cast_slice(bytes)))
                        }
                        (accessor_type, component_type, normalized) => {
                            return Err(GltfError::InvalidAccessorDataType {
                                accessor_type,
                                component_type,
                                normalized,
                                usage: AccessorUsage::Indices,
                            });
                        }
                    }
                }

                let geometry = builder.build(encoder).id();
                let material = primitive.material;
                primitives.push((geometry, material));
            }
        }

        let mut prims = Vec::with_capacity(primitives.len());
        for (geometry, material) in primitives {
            prims.push((
                geometry,
                self.get_or_load_material(material, resources, encoder)?,
            ));
        }

        let id = resources
            .mesh_builder()
            .name(name)
            .primitives(prims)
            .build()
            .id();

        self.json.meshes[index].id = Some(id);

        Ok(id)
    }

    fn get_buffer_view(&self, index: usize) -> Result<&[u8]> {
        let buffer_view = self
            .json
            .buffer_views
            .get(index)
            .ok_or(GltfError::InvalidIndex {
                entity: GltfEntity::BufferView,
                index,
            })?;
        let start = buffer_view.byte_offset;
        let end = start + buffer_view.byte_length;

        match self.buffers.get(buffer_view.buffer) {
            Some(bytes) => Ok(&bytes[start..end]),
            None => todo!("load buffer"),
        }
    }

    fn get_or_load_material(
        &mut self,
        index: Option<usize>,
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<Id<crate::material::Material>> {
        let material = match index {
            Some(index) => self
                .json
                .materials
                .get(index)
                .ok_or(GltfError::InvalidIndex {
                    entity: GltfEntity::Material,
                    index,
                })?,
            None => self.default_material.get_or_insert_with(Material::default),
        };

        if let Some(id) = material.id {
            return Ok(id);
        }

        let pbr = &material.pbr_metallic_roughness;

        let mut builder = crate::material::MaterialBuilder::default()
            .base_color_factor(pbr.base_color_factor)
            .metallic_factor(pbr.metallic_factor)
            .roughness_factor(pbr.roughness_factor)
            .emissive_factor(material.emissive_factor)
            .alpha_mode(match material.alpha_mode {
                AlphaMode::Opaque => crate::material::AlphaMode::Opaque,
                AlphaMode::Mask => crate::material::AlphaMode::Mask,
                AlphaMode::Blend => crate::material::AlphaMode::Blend,
            })
            .alpha_cutoff(material.alpha_cutoff)
            .double_sided(material.double_sided);

        let base_color_texture = pbr.base_color_texture.clone();
        let metallic_roughness_texture = pbr.metallic_roughness_texture.clone();
        let normal_texture = material.normal_texture.clone();
        let occlusion_texture = material.occlusion_texture.clone();
        let emissive_texture = material.emissive_texture.clone();

        if let Some(info) = base_color_texture {
            let id = self.get_or_load_texture(info.index, true, resources, encoder)?;
            builder = builder
                .base_color_texture(id)
                .base_color_tex_coord(info.tex_coord as u32);
        }

        if let Some(info) = metallic_roughness_texture {
            let id = self.get_or_load_texture(info.index, false, resources, encoder)?;
            builder = builder
                .metallic_roughness_texture(id)
                .metallic_roughness_tex_coord(info.tex_coord as u32);
        }

        if let Some(info) = normal_texture {
            let id = self.get_or_load_texture(info.index, false, resources, encoder)?;
            builder = builder
                .normal_texture(id)
                .normal_tex_coord(info.tex_coord as u32)
                .normal_scale(info.scale);
        }

        if let Some(info) = occlusion_texture {
            let id = self.get_or_load_texture(info.index, false, resources, encoder)?;
            builder = builder
                .occlusion_texture(id)
                .occlusion_tex_coord(info.tex_coord as u32)
                .occlusion_strength(info.strength);
        }

        if let Some(info) = emissive_texture {
            let id = self.get_or_load_texture(info.index, true, resources, encoder)?;
            builder = builder
                .emissive_texture(id)
                .emissive_tex_coord(info.tex_coord as u32);
        }

        let id = builder.build(resources).id();

        match index {
            Some(index) => &mut self.json.materials[index],
            None => self.default_material.as_mut().unwrap(),
        }
        .id = Some(id);

        Ok(id)
    }

    fn get_or_load_texture(
        &mut self,
        index: usize,
        srgb: bool,
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<Id<crate::material::Texture>> {
        let texture = self
            .json
            .textures
            .get(index)
            .ok_or(GltfError::InvalidIndex {
                entity: GltfEntity::Texture,
                index,
            })?;
        if let Some(id) = texture.id {
            return Ok(id);
        }

        let name = texture.name.clone();
        let sampler = texture.sampler;
        let source = texture.source.clone();

        let sampler = match sampler {
            Some(index) => Some(self.get_or_load_sampler(index, resources)?),
            None => None,
        };

        let source = self.get_or_load_image(
            source
                .ok_or_else(|| GltfError::Unsupported("Gltf texture with no source".to_string()))?,
            srgb,
            resources,
            encoder,
        )?;

        let id = crate::material::TextureBuilder::default()
            .name(name)
            .sampler(sampler)
            .texture(source)
            .build(resources)
            .id();
        self.json.textures[index].id = Some(id);
        Ok(id)
    }

    fn get_or_load_sampler(
        &mut self,
        index: usize,
        resources: &mut Resources,
    ) -> Result<wgpu::Sampler> {
        let sampler = self
            .json
            .samplers
            .get(index)
            .ok_or(GltfError::InvalidIndex {
                entity: GltfEntity::Sampler,
                index,
            })?;
        if let Some(sampler) = sampler.wgpu.clone() {
            return Ok(sampler);
        }

        let mag_filter = match sampler.mag_filter {
            MagFilter::Linear => wgpu::FilterMode::Linear,
            MagFilter::Nearest | MagFilter::None => wgpu::FilterMode::Nearest,
        };
        let (min_filter, mipmap_filter) = match sampler.min_filter {
            MinFilter::LinearMipmapNearest | MinFilter::Linear => {
                (wgpu::FilterMode::Linear, wgpu::FilterMode::Nearest)
            }
            MinFilter::LinearMipmapLinear => (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear),
            MinFilter::NearestMipmapNearest | MinFilter::Nearest | MinFilter::None => {
                (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest)
            }
            MinFilter::NearestMipmapLinear => (wgpu::FilterMode::Nearest, wgpu::FilterMode::Linear),
        };

        let sampler = resources.device.create_sampler(&wgpu::SamplerDescriptor {
            label: sampler.name.as_deref(),
            address_mode_u: wrapping_mode_to_address_mode(sampler.wrap_s),
            address_mode_v: wrapping_mode_to_address_mode(sampler.wrap_t),
            mag_filter,
            min_filter,
            mipmap_filter,
            ..Default::default()
        });
        self.json.samplers[index].wgpu = Some(sampler.clone());
        Ok(sampler)
    }

    fn get_or_load_image(
        &mut self,
        index: usize,
        srgb: bool,
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<wgpu::Texture> {
        let image = self.json.images.get(index).ok_or(GltfError::InvalidIndex {
            entity: GltfEntity::Image,
            index,
        })?;
        if let Some(image) = image.wgpu.clone() {
            return Ok(image);
        }

        let name = image.name.as_deref();
        let image = if let Some(uri) = &image.uri {
            if uri.starts_with("data:") {
                todo!("`data:`-URI support")
            } else {
                ImageReader::open(uri)?.decode()?
            }
        } else {
            let buffer_view = image.buffer_view.ok_or(GltfError::MissingImageContent)?;
            let format = match image.mime_type {
                ImageMimeType::ImageJpeg => ImageFormat::Jpeg,
                ImageMimeType::ImagePng => ImageFormat::Png,
                ImageMimeType::None => return Err(GltfError::MissingImageMimeType),
            };
            let buffer_view =
                self.json
                    .buffer_views
                    .get(buffer_view)
                    .ok_or(GltfError::InvalidIndex {
                        entity: GltfEntity::BufferView,
                        index: buffer_view,
                    })?;
            let bytes = match self.buffers.get(buffer_view.buffer) {
                Some(bytes) => bytes,
                None => todo!("load buffer into memory"),
            };

            let start = buffer_view.byte_offset;
            let end = start + buffer_view.byte_length;

            let reader = Cursor::new(&bytes[start..end]);

            ImageReader::with_format(reader, format).decode()?
        };

        let texture = crate::texture::TextureBuilder::default()
            .name(name)
            .from_dynamic_image(&image, srgb)
            // .generate_mips()
            .build(resources, encoder);
        self.json.images[index].wgpu = Some(texture.clone());
        Ok(texture)
    }

    fn accessor_iter<T: Pod + 'static, const N: usize>(
        &self,
        index: usize,
    ) -> Result<DenseAccessorIter<'_, T, N>> {
        let accessor = self
            .json
            .accessors
            .get(index)
            .ok_or(GltfError::InvalidIndex {
                entity: GltfEntity::Accessor,
                index,
            })?;
        if accessor.sparse.is_some() {
            todo!("sparse accessor support");
        }
        let buffer_view = accessor
            .buffer_view
            .ok_or(GltfError::MissingAccessorBufferView)?;
        let buffer_view =
            self.json
                .buffer_views
                .get(buffer_view)
                .ok_or(GltfError::InvalidIndex {
                    entity: GltfEntity::BufferView,
                    index: buffer_view,
                })?;

        let bytes = match self.buffers.get(buffer_view.buffer) {
            Some(bytes) => bytes,
            None => todo!("load buffer into memory"),
        };

        let byte_stride = buffer_view
            .byte_stride
            .map_or(size_of::<[T; N]>(), NonZeroUsize::get);
        let start = accessor.byte_offset + buffer_view.byte_offset;
        let end = start + accessor.count * byte_stride;

        Ok(DenseAccessorIter {
            bytes,
            start,
            end,
            byte_stride,
            data_type: PhantomData,
        })
    }
}

fn wrapping_mode_to_address_mode(wrapping_mode: WrappingMode) -> wgpu::AddressMode {
    match wrapping_mode {
        WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
        WrappingMode::Repeat => wgpu::AddressMode::Repeat,
        WrappingMode::None => wgpu::AddressMode::Repeat,
    }
}

fn i8x2_to_vec2(a: &[i8; 2]) -> Vec2 {
    (vec2(a[0] as f32, a[1] as f32) / 127.0).max(vec2(-1.0, -1.0))
}

fn i8x3_to_vec3(a: &[i8; 3]) -> Vec3 {
    (vec3(a[0] as f32, a[1] as f32, a[2] as f32) / 127.0).max(vec3(-1.0, -1.0, -1.0))
}

fn i8x4_to_vec4(a: &[i8; 4]) -> Vec4 {
    (vec4(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32) / 127.0)
        .max(vec4(-1.0, -1.0, -1.0, -1.0))
}

fn u8x2_to_vec2(a: &[u8; 2]) -> Vec2 {
    vec2(a[0] as f32, a[1] as f32) / 255.0
}

fn u8x3_to_vec3(a: &[u8; 3]) -> Vec3 {
    vec3(a[0] as f32, a[1] as f32, a[2] as f32) / 255.0
}

fn u8x4_to_vec4(a: &[u8; 4]) -> Vec4 {
    vec4(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32) / 255.0
}

fn i16x2_to_vec2(a: &[i16; 2]) -> Vec2 {
    (vec2(a[0] as f32, a[1] as f32) / 32767.0).max(vec2(-1.0, -1.0))
}

fn i16x3_to_vec3(a: &[i16; 3]) -> Vec3 {
    (vec3(a[0] as f32, a[1] as f32, a[2] as f32) / 32767.0).max(vec3(-1.0, -1.0, -1.0))
}

fn i16x4_to_vec4(a: &[i16; 4]) -> Vec4 {
    (vec4(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32) / 32767.0)
        .max(vec4(-1.0, -1.0, -1.0, -1.0))
}

fn u16x2_to_vec2(a: &[u16; 2]) -> Vec2 {
    vec2(a[0] as f32, a[1] as f32) / 65535.0
}

fn u16x3_to_vec3(a: &[u16; 3]) -> Vec3 {
    vec3(a[0] as f32, a[1] as f32, a[2] as f32) / 65535.0
}

fn u16x4_to_vec4(a: &[u16; 4]) -> Vec4 {
    vec4(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32) / 65535.0
}

fn u8x4_to_uvec4(a: &[u8; 4]) -> UVec4 {
    uvec4(a[0] as u32, a[1] as u32, a[2] as u32, a[3] as u32)
}

fn u16x4_to_uvec4(a: &[u16; 4]) -> UVec4 {
    uvec4(a[0] as u32, a[1] as u32, a[2] as u32, a[3] as u32)
}

struct DenseAccessorIter<'a, T: Pod + 'static, const N: usize> {
    bytes: &'a [u8],
    start: usize,
    end: usize,
    byte_stride: usize,
    data_type: PhantomData<[T; N]>,
}

impl<'a, T: Pod + 'static, const N: usize> Iterator for DenseAccessorIter<'a, T, N> {
    type Item = &'a [T; N];

    fn next(&mut self) -> Option<Self::Item> {
        let next_end = self.start + size_of::<[T; N]>();
        if next_end <= self.end {
            let next = from_bytes(&self.bytes[self.start..next_end]);
            self.start += self.byte_stride;
            Some(next)
        } else {
            None
        }
    }
}

struct GlbChunk {
    chunk_length: u32,
    chunk_type: u32,
    chunk_data: Vec<u8>,
}

impl GlbChunk {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let mut chunk_length: u32 = 0;
        reader.read_exact(bytes_of_mut(&mut chunk_length))?;

        let mut chunk_type: u32 = 0;
        reader.read_exact(bytes_of_mut(&mut chunk_type))?;

        let mut chunk_data = Vec::with_capacity(chunk_length as usize);
        reader
            .take(chunk_length as u64)
            .read_to_end(&mut chunk_data)?;

        if (chunk_data.len() as u32) < chunk_length {
            return Err(GlbError::InvalidChunkLength.into());
        }

        Ok(Self {
            chunk_length,
            chunk_type,
            chunk_data,
        })
    }
}

/// The root object for a glTF asset.
#[derive(Debug, Serialize, Deserialize)]
struct Gltf {
    /// Names of glTF extensions used in this asset.
    #[serde(rename = "extensionsUsed")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    extensions_used: Vec<String>,

    /// Names of glTF extensions required to properly load this asset.
    #[serde(rename = "extensionsRequired")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    extensions_required: Vec<String>,

    /// An array of accessors. An accessor is a typed view into a bufferView.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    accessors: Vec<Accessor>,

    /// An array of keyframe animations.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    animations: Vec<Animation>,

    /// Metadata about the glTF asset.
    asset: Asset,

    /// An array of buffers. A buffer points to binary geometry, animation, or skins.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    buffers: Vec<Buffer>,

    /// An array of bufferViews. A bufferView is a view into a buffer generally representing a subset of the buffer.
    #[serde(rename = "bufferViews")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    buffer_views: Vec<BufferView>,

    /// An array of cameras. A camera defines a projection matrix.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cameras: Vec<Camera>,

    /// An array of images. An image defines data used to create a texture.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    images: Vec<Image>,

    /// An array of materials. A material defines the appearance of a primitive.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    materials: Vec<Material>,

    /// An array of meshes. A mesh is a set of primitives to be rendered.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    meshes: Vec<Mesh>,

    /// An array of nodes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    nodes: Vec<Node>,

    /// An array of samplers. A sampler contains properties for texture filtering and wrapping modes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    samplers: Vec<Sampler>,

    /// The index of the default scene. This property **MUST NOT** be defined, when [scenes](Gltf::scenes) is undefined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    scene: Option<usize>,

    /// An array of scenes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    scenes: Vec<Scene>,

    /// An array of skins. A skin is defined by joints and matrices.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    skins: Vec<Skin>,

    /// An array of textures.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    textures: Vec<Texture>,
}

/// Metadata about the glTF asset.
#[derive(Debug, Serialize, Deserialize)]
struct Asset {
    /// A copyright message suitable for display to credit the content creator.
    copyright: Option<String>,

    /// Tool that generated this glTF model. Useful for debugging.
    generator: Option<String>,

    /// The glTF version in the form of `<major>.<minor>` that this asset targets.
    version: String,

    /// The minimum glTF version in the form of `<major>.<minor>` that this asset targets.
    /// This property **MUST NOT** be greater than the asset version.
    #[serde(rename = "minVersion")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    min_version: Option<String>,
}

/// A typed view into a buffer view that contains raw binary data.
#[derive(Debug, Serialize, Deserialize)]
struct Accessor {
    /// The index of the buffer view. When undefined, the accessor **MUST**
    /// be initialized with zeros; `sparse` property or extensions **MAY**
    /// override zeros with actual values.
    #[serde(rename = "bufferView")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    buffer_view: Option<usize>,

    /// The offset relative to the start of the buffer view in bytes. This **MUST**
    /// be a multiple of the size of the component datatype. This property **MUST NOT**
    /// be defined when [bufferView](Accessor::buffer_view) is `undefined`.
    #[serde(rename = "byteOffset")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_0")]
    byte_offset: usize,

    /// The datatype of the accessor’s components. [UnsignedInt](AccessorComponentType::UnsignedInt)
    /// type **MUST NOT** be used for any accessor that is not referenced by
    /// [mesh.primitive.indices](MeshPrimitive::indices).
    #[serde(rename = "componentType")]
    component_type: AccessorComponentType,

    /// Specifies whether integer data values are normalized (`true`) to [0, 1] (for unsigned types)
    /// or to [-1, 1] (for signed types) when they are accessed. This property **MUST NOT** be set
    /// to `true` for accessors with [Float](AccessorComponentType::Float) or
    /// [UnsignedInt](AccessorComponentType::UnsignedInt) component type.
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    normalized: bool,

    /// The number of elements referenced by this accessor, not to be confused with the number of
    /// bytes or number of components.
    count: usize,

    /// Specifies if the accessor’s elements are scalars, vectors, or matrices.
    #[serde(rename = "type")]
    type_: AccessorType,

    /// Maximum value of each component in this accessor. Array elements
    /// **MUST** be treated as having the same data type as [componentType](Accessor::component_type).
    /// Both [min](Accessor::min) and [max](Accessor::max) arrays have the same length.
    /// The length is determined by the value of the type property;
    /// it can be 1, 2, 3, 4, 9, or 16.
    ///
    /// [normalized](Accessor::normalized) property has no effect on array values:
    /// they always correspond to the actual values stored in the buffer.
    /// When the accessor is sparse, this property **MUST** contain maximum
    /// values of accessor data with sparse substitution applied.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<Vec<f32>>,

    /// Minimum value of each component in this accessor. Array elements
    /// **MUST** be treated as having the same data type as [componentType](Accessor::component_type).
    /// Both [min](Accessor::min) and [max](Accessor::max) arrays have the same length.
    /// The length is determined by the value of the type property;
    /// it can be 1, 2, 3, 4, 9, or 16.
    ///
    /// [normalized](Accessor::normalized) property has no effect on array values:
    /// they always correspond to the actual values stored in the buffer.
    /// When the accessor is sparse, this property **MUST** contain maximum
    /// values of accessor data with sparse substitution applied.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<Vec<f32>>,

    /// Sparse storage of elements that deviate from their initialization value.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    sparse: Option<SparseAccessor>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could even
    /// have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// The datatype of the accessor’s components. [UnsignedInt](AccessorComponentType::UnsignedInt)
/// type **MUST NOT** be used for any accessor that is not referenced by
/// [mesh.primitive.indices](MeshPrimitive::indices).
#[derive(Debug, Clone, Copy, Serialize_repr, Deserialize_repr)]
#[repr(u32)]
pub enum AccessorComponentType {
    Byte = 5120,
    UnsignedByte = 5121,
    Short = 5122,
    UnsignedShort = 5123,
    UnsignedInt = 5125,
    Float = 5126,
}

impl Display for AccessorComponentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Byte => "byte",
            Self::UnsignedByte => "unsigned_byte",
            Self::Short => "short",
            Self::UnsignedShort => "unsigned_short",
            Self::UnsignedInt => "unsigned_int",
            Self::Float => "float",
        })
    }
}

/// Specifies if the accessor’s elements are scalars, vectors, or matrices.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AccessorType {
    #[serde(rename = "SCALAR")]
    Scalar,

    #[serde(rename = "VEC2")]
    Vec2,

    #[serde(rename = "VEC3")]
    Vec3,

    #[serde(rename = "VEC4")]
    Vec4,

    #[serde(rename = "MAT2")]
    Mat2,

    #[serde(rename = "MAT3")]
    Mat3,

    #[serde(rename = "MAT4")]
    Mat4,
}

impl Display for AccessorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Scalar => "scalar",
            Self::Vec2 => "vec2",
            Self::Vec3 => "vec3",
            Self::Vec4 => "vec4",
            Self::Mat2 => "mat2",
            Self::Mat3 => "mat3",
            Self::Mat4 => "mat4",
        })
    }
}

/// Sparse storage of accessor values that deviate from their initialization value.
#[derive(Debug, Serialize, Deserialize)]
struct SparseAccessor {
    /// Number of deviating accessor values stored in the sparse array.
    count: usize,

    /// An object pointing to a buffer view containing the indices of deviating
    /// accessor values. The number of indices is equal to [count](SparseAccessor::count).
    /// Indices **MUST** strictly increase.
    indices: SparseAccessorIndices,

    /// An object pointing to a buffer view containing the deviating accessor values.
    values: SparseAccessorValues,
}

/// An object pointing to a buffer view containing the indices of deviating accessor
/// values. The number of indices is equal to [accessor.sparse.count](SparseAccessor::count).
/// Indices **MUST** strictly increase.
#[derive(Debug, Serialize, Deserialize)]
struct SparseAccessorIndices {
    /// The index of the buffer view with sparse indices. The referenced buffer view
    /// **MUST NOT** have its [target](BufferView::target) or [byteStride](BufferView::byte_stride)
    /// properties defined. The buffer view and the optional [byteOffset](BufferView::byte_offset)
    /// **MUST** be aligned to the [componentType](SparseAccessorIndices::component_type) byte length.
    #[serde(rename = "bufferView")]
    buffer_view: usize,

    /// The offset relative to the start of the buffer view in bytes.
    #[serde(rename = "byteOffset")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_0")]
    byte_offset: usize,

    /// The indices data type.
    #[serde(rename = "componentType")]
    component_type: SparseAccessorComponentType,
}

/// The indices data type.
#[derive(Debug, Serialize_repr, Deserialize_repr)]
#[repr(u32)]
enum SparseAccessorComponentType {
    UnsignedByte = 5121,
    UnsignedShort = 5123,
    UnsignedInt = 5125,
}

/// An object pointing to a buffer view containing the deviating accessor values.
/// The number of elements is equal to [accessor.sparse.count](SparseAccessor::count)
/// times number of components. The elements have the same component type as the base
/// accessor. The elements are tightly packed. Data **MUST** be aligned following the
/// same rules as the base accessor.
#[derive(Debug, Serialize, Deserialize)]
struct SparseAccessorValues {
    /// The index of the bufferView with sparse values. The referenced buffer
    /// view **MUST NOT** have its [target](BufferView::target) or
    /// [byteStride](BufferView::byte_stride) properties defined.
    #[serde(rename = "bufferView")]
    buffer_view: usize,

    /// The offset relative to the start of the
    /// [buffer_view](SparseAccessorValues::buffer_view) in bytes.
    #[serde(rename = "byteOffset")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_0")]
    byte_offset: usize,
}

/// A keyframe animation.
#[derive(Debug, Serialize, Deserialize)]
struct Animation {
    /// An array of animation channels. An animation channel combines an
    /// animation sampler with a target property being animated. Different
    /// channels of the same animation **MUST NOT** have the same targets.
    channels: Vec<AnimationChannel>,

    /// An array of animation samplers. An animation sampler combines timestamps
    /// with a sequence of output values and defines an interpolation algorithm.
    samplers: Vec<AnimationSampler>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// An animation channel combines an animation sampler with a target property being animated.
#[derive(Debug, Serialize, Deserialize)]
struct AnimationChannel {
    /// The index of a sampler in this animation used to compute the value for the target,
    /// e.g., a node’s translation, rotation, or scale (TRS).
    sampler: usize,

    /// The descriptor of the animated property.
    target: AnimationTarget,
}

/// The descriptor of the animated property.
#[derive(Debug, Serialize, Deserialize)]
struct AnimationTarget {
    /// The index of the node to animate. When undefined, the animated object
    /// **MAY** be defined by an extension.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    node: Option<usize>,

    /// The name of the node’s TRS property to animate, or the "weights" of
    /// the Morph Targets it instantiates. For the [Translation](AnimationTargetPath::Translation)
    /// property, the values that are provided by the sampler are the translation along
    /// the X, Y, and Z axes. For the [Rotation](AnimationTargetPath::Rotation) property,
    /// the values are a quaternion in the order (x, y, z, w), where w is the scalar.
    /// For the [Scale](AnimationTargetPath::Scale) property, the values are the scaling
    /// factors along the X, Y, and Z axes.
    path: AnimationTargetPath,
}

/// The name of the node’s TRS property to animate, or the "weights" of
/// the Morph Targets it instantiates. For the [Translation](AnimationTargetPath::Translation)
/// property, the values that are provided by the sampler are the translation along
/// the X, Y, and Z axes. For the [Rotation](AnimationTargetPath::Rotation) property,
/// the values are a quaternion in the order (x, y, z, w), where w is the scalar.
/// For the [Scale](AnimationTargetPath::Scale) property, the values are the scaling
/// factors along the X, Y, and Z axes.
#[derive(Debug, Serialize, Deserialize)]
enum AnimationTargetPath {
    #[serde(rename = "translation")]
    Translation,

    #[serde(rename = "rotation")]
    Rotation,

    #[serde(rename = "scale")]
    Scale,

    #[serde(rename = "weights")]
    Weights,
}

/// An animation sampler combines timestamps with a sequence of output values and defines an interpolation algorithm.
#[derive(Debug, Serialize, Deserialize)]
struct AnimationSampler {
    /// The index of an accessor containing keyframe timestamps. The accessor **MUST** be of scalar type with
    /// floating-point components. The values represent time in seconds with `time[0] >= 0.0`, and strictly
    /// increasing values, i.e., `time[n + 1] > time[n]`.
    input: usize,

    /// Interpolation algorithm.
    #[serde(default)]
    #[serde(skip_serializing_if = "AnimationInterpolation::is_default")]
    interpolation: AnimationInterpolation,

    /// The index of an accessor, containing keyframe output values.
    output: usize,
}

/// Interpolation algorithm.
#[derive(Debug, Default, Serialize, Deserialize)]
enum AnimationInterpolation {
    /// The animated values are linearly interpolated between keyframes.
    /// When targeting a rotation, spherical linear interpolation (slerp)
    /// **SHOULD** be used to interpolate quaternions. The number of
    /// output elements **MUST** equal the number of input elements.
    #[default]
    #[serde(rename = "LINEAR")]
    Linear,

    /// The animated values remain constant to the output of the first keyframe,
    /// until the next keyframe. The number of output elements **MUST** equal the
    /// number of input elements.
    #[serde(rename = "STEP")]
    Step,

    /// The animation’s interpolation is computed using a cubic spline with
    /// specified tangents. The number of output elements **MUST** equal three
    /// times the number of input elements. For each input element, the output
    /// stores three elements, an in-tangent, a spline vertex, and an out-tangent.
    /// There **MUST** be at least two keyframes when using this interpolation.
    #[serde(rename = "CUBICSPLINE")]
    Cubicspline,
}

impl AnimationInterpolation {
    fn is_default(&self) -> bool {
        match self {
            AnimationInterpolation::Linear => true,
            _ => false,
        }
    }
}

/// A buffer points to binary geometry, animation, or skins.
#[derive(Debug, Serialize, Deserialize)]
struct Buffer {
    /// The URI (or IRI) of the buffer. Relative paths are relative to
    /// the current glTF asset. Instead of referencing an external file,
    /// this field **MAY** contain a `data:`-URI.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    uri: Option<String>,

    /// The length of the buffer in bytes.
    #[serde(rename = "byteLength")]
    byte_length: usize,

    /// The user-defined name of this object. This is not necessarily unique,
    /// e.g., an accessor and a buffer could have the same name, or two accessors
    /// could even have the same name.
    name: Option<String>,
}

/// A view into a buffer generally representing a subset of the buffer.
#[derive(Debug, Serialize, Deserialize)]
struct BufferView {
    /// The index of the buffer.
    buffer: usize,

    /// The offset into the buffer in bytes.
    #[serde(rename = "byteOffset")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_0")]
    byte_offset: usize,

    /// The length of the bufferView in bytes.
    #[serde(rename = "byteLength")]
    byte_length: usize,

    /// The stride, in bytes, between vertex attributes. When this is not
    /// defined, data is tightly packed. When two or more accessors use the
    /// same buffer view, this field **MUST** be defined.
    #[serde(rename = "byteStride")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    byte_stride: Option<NonZeroUsize>,

    /// The hint representing the intended GPU buffer type to use with this buffer view.
    #[serde(default)]
    #[serde(skip_serializing_if = "BufferViewTarget::is_none")]
    target: BufferViewTarget,

    /// The user-defined name of this object. This is not necessarily unique,
    /// e.g., an accessor and a buffer could have the same name, or two accessors
    /// could even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Default, Serialize_repr, Deserialize_repr)]
#[repr(u32)]
enum BufferViewTarget {
    #[default]
    None = 0,
    ArrayBuffer = 34962,
    ElementArrayBuffer = 34963,
}

impl BufferViewTarget {
    fn is_none(&self) -> bool {
        match self {
            BufferViewTarget::None => true,
            _ => false,
        }
    }
}

///A camera’s projection. A node **MAY** reference a camera to apply a transform to place the camera in the scene.
#[derive(Debug, Serialize, Deserialize)]
struct Camera {
    /// An orthographic camera containing properties to create an orthographic projection matrix.
    /// This property **MUST NOT** be defined when [perspective](Camera::perspective) is defined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    orthographic: Option<OrthographicCamera>,

    /// A perspective camera containing properties to create a perspective projection matrix.
    /// This property **MUST NOT** be defined when [orthographic](Camera::orthographic) is defined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    perspective: Option<PerspectiveCamera>,

    /// Specifies if the camera uses a perspective or orthographic projection.
    /// Based on this, either the camera’s [perspective](Camera::perspective)
    /// or [orthographic](Camera::orthographic) property **MUST** be defined.
    #[serde(rename = "type")]
    type_: CameraType,

    /// The user-defined name of this object. This is not necessarily unique,
    /// e.g., an accessor and a buffer could have the same name, or two accessors
    /// could even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// An orthographic camera containing properties to create an orthographic projection matrix.
#[derive(Debug, Serialize, Deserialize)]
struct OrthographicCamera {
    /// The floating-point horizontal magnification of the view. This value **MUST NOT**
    /// be equal to zero. This value **SHOULD NOT** be negative.
    xmag: f32,

    /// The floating-point vertical magnification of the view. This value **MUST NOT**
    /// be equal to zero. This value **SHOULD NOT** be negative.
    ymag: f32,

    /// The floating-point distance to the far clipping plane. This value **MUST NOT**
    /// be equal to zero. zfar **MUST** be greater than znear.
    zfar: f32,

    /// The floating-point distance to the near clipping plane.
    znear: f32,
}

/// A perspective camera containing properties to create a perspective projection matrix.
#[derive(Debug, Serialize, Deserialize)]
struct PerspectiveCamera {
    /// The floating-point aspect ratio of the field of view. When undefined, the aspect
    /// ratio of the rendering viewport **MUST** be used.
    #[serde(rename = "aspectRatio")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    aspect_ratio: Option<f32>,

    /// The floating-point vertical field of view in radians. This value **SHOULD** be less than π.
    yfov: f32,

    /// The floating-point distance to the far clipping plane. When defined, `zfar` **MUST** be greater
    /// than [znear](PerspectiveCamera::znear). If `zfar` is undefined, client implementations **SHOULD**
    /// use infinite projection matrix.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    zfar: Option<f32>,

    /// The floating-point distance to the near clipping plane.
    znear: f32,
}

/// Specifies if the camera uses a perspective or orthographic projection.
/// Based on this, either the camera’s [perspective](Camera::perspective)
/// or [orthographic](Camera::orthographic) property **MUST** be defined.
#[derive(Debug, Serialize, Deserialize)]
enum CameraType {
    #[serde(rename = "perspective")]
    Perspective,

    #[serde(rename = "orthographic")]
    Orthographic,
}

/// Image data used to create a texture. Image **MAY** be referenced by an URI (or IRI) or a buffer view index.
#[derive(Debug, Serialize, Deserialize)]
struct Image {
    /// wgpu texture, if the resource has been loaded.
    #[serde(skip)]
    wgpu: Option<wgpu::Texture>,

    /// The URI (or IRI) of the image. Relative paths are relative to the current glTF asset.
    /// Instead of referencing an external file, this field **MAY** contain a `data:`-URI.
    /// This field **MUST NOT** be defined when [bufferView](Image::buffer_view) is defined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    uri: Option<String>,

    /// The image’s media type. This field **MUST** be defined when
    /// [buffer_view](Image::buffer_view) is defined.
    #[serde(rename = "mimeType")]
    #[serde(default)]
    #[serde(skip_serializing_if = "ImageMimeType::is_none")]
    mime_type: ImageMimeType,

    /// The index of the [BufferView] that contains the image.
    /// This field **MUST NOT** be defined when [uri](Image::uri) is defined.
    #[serde(rename = "bufferView")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    buffer_view: Option<usize>,

    /// The user-defined name of this object. This is not necessarily unique,
    /// e.g., an accessor and a buffer could have the same name, or two accessors
    /// could even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// The image’s media type. This field **MUST** be defined when
/// [bufferView](Image::buffer_view) is defined.
#[derive(Debug, Default, Serialize, Deserialize)]
enum ImageMimeType {
    #[default]
    None,

    #[serde(rename = "image/jpeg")]
    ImageJpeg,

    #[serde(rename = "image/png")]
    ImagePng,
}

impl ImageMimeType {
    fn is_none(&self) -> bool {
        match self {
            ImageMimeType::None => true,
            _ => false,
        }
    }
}

/// The material appearance of a primitive.
#[derive(Debug, Serialize, Deserialize)]
struct Material {
    /// Storm storage id, if the resource has been loaded.
    #[serde(skip)]
    id: Option<Id<crate::material::Material>>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,

    /// A set of parameter values that are used to define the metallic-roughness material
    /// model from Physically Based Rendering (PBR) methodology. When undefined, all the
    /// default values of [PbrMetallicRoughness] **MUST** apply.
    #[serde(rename = "pbrMetallicRoughness")]
    #[serde(default)]
    #[serde(skip_serializing_if = "PbrMetallicRoughness::is_default")]
    pbr_metallic_roughness: PbrMetallicRoughness,

    /// The tangent space normal texture. The texture encodes RGB components with linear
    /// transfer function. Each texel represents the XYZ components of a normal vector in
    /// tangent space. The normal vectors use the convention +X is right and +Y is up. +Z
    /// points toward the viewer. If a fourth component (A) is present, it **MUST** be ignored.
    /// When undefined, the material does not have a tangent space normal texture.
    #[serde(rename = "normalTexture")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    normal_texture: Option<NormalTextureInfo>,

    /// The occlusion texture. The occlusion values are linearly sampled from the R channel.
    /// Higher values indicate areas that receive full indirect lighting and lower values
    /// indicate no indirect lighting. If other channels are present (GBA), they **MUST** be ignored
    /// for occlusion calculations. When undefined, the material does not have an occlusion texture.
    #[serde(rename = "occlusionTexture")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    occlusion_texture: Option<OcclusionTextureInfo>,

    /// The emissive texture. It controls the color and intensity of the light being emitted by the material. $
    /// This texture contains RGB components encoded with the sRGB transfer function. If a fourth component (A) is present,
    /// it **MUST** be ignored. When undefined, the texture **MUST** be sampled as having 1.0 in RGB components.
    #[serde(rename = "emissiveTexture")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    emissive_texture: Option<TextureInfo>,

    /// The factors for the emissive color of the material. This value defines
    /// linear multipliers for the sampled texels of the emissive texture.
    #[serde(rename = "emissiveFactor")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_3x00")]
    emissive_factor: [f32; 3],

    /// The material’s alpha rendering mode enumeration specifying the interpretation of the alpha value of the base color.
    #[serde(rename = "alphaMode")]
    #[serde(default)]
    #[serde(skip_serializing_if = "AlphaMode::is_default")]
    alpha_mode: AlphaMode,

    /// Specifies the cutoff threshold when in [MASK](AlphaMode::Mask). If the alpha value is
    /// greater than or equal to this value then it is rendered as fully opaque, otherwise,
    /// it is rendered as fully transparent. A value greater than 1.0 will render the entire
    /// material as fully transparent. This value **MUST** be ignored for other alpha modes.
    /// When [alphaMode](Material::alpha_mode) is not defined, this value **MUST NOT** be defined.
    #[serde(rename = "alphaCutoff")]
    #[serde(default = "default_05")]
    #[serde(skip_serializing_if = "is_05")]
    alpha_cutoff: f32,

    /// Specifies whether the material is double sided. When this value is false,
    /// back-face culling is enabled. When this value is true, back-face culling is
    /// disabled and double-sided lighting is enabled. The back-face **MUST** have
    /// its normals reversed before the lighting equation is evaluated.
    #[serde(rename = "doubleSided")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    double_sided: bool,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            id: None,
            name: None,
            pbr_metallic_roughness: PbrMetallicRoughness::default(),
            normal_texture: None,
            occlusion_texture: None,
            emissive_texture: None,
            emissive_factor: Default::default(),
            alpha_mode: AlphaMode::default(),
            alpha_cutoff: 0.5,
            double_sided: false,
        }
    }
}

/// A set of parameter values that are used to define the metallic-roughness material model
/// from Physically-Based Rendering (PBR) methodology.
#[derive(Debug, Serialize, Deserialize)]
struct PbrMetallicRoughness {
    /// The factors for the base color of the material. This value defines linear multipliers
    /// for the sampled texels of the base color texture.
    #[serde(rename = "baseColorFactor")]
    #[serde(default = "default_4x10")]
    #[serde(skip_serializing_if = "is_4x10")]
    base_color_factor: [f32; 4],

    /// The base color texture. The first three components (RGB) **MUST** be encoded with the
    /// sRGB transfer function. They specify the base color of the material. If the fourth
    /// component (A) is present, it represents the linear alpha coverage of the material.
    /// Otherwise, the alpha coverage is equal to 1.0. The [material.alphaMode](Material::alpha_mode)
    /// property specifies how alpha is interpreted. The stored texels **MUST NOT** be premultiplied.
    /// When undefined, the texture **MUST** be sampled as having `1.0` in all components.
    #[serde(rename = "baseColorTexture")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    base_color_texture: Option<TextureInfo>,

    /// The factor for the metalness of the material. This value defines a linear multiplier
    /// or the sampled metalness values of the metallic-roughness texture.
    #[serde(rename = "metallicFactor")]
    #[serde(default = "default_10")]
    #[serde(skip_serializing_if = "is_10")]
    metallic_factor: f32,

    /// The factor for the roughness of the material. This value defines a linear multiplier
    /// for the sampled roughness values of the metallic-roughness texture.
    #[serde(rename = "roughnessFactor")]
    #[serde(default = "default_10")]
    #[serde(skip_serializing_if = "is_10")]
    roughness_factor: f32,

    /// The metallic-roughness texture. The metalness values are sampled from the B channel.
    /// The roughness values are sampled from the G channel. These values **MUST** be encoded
    /// with a linear transfer function. If other channels are present (R or A), they **MUST**
    /// be ignored for metallic-roughness calculations. When undefined, the texture **MUST**
    /// be sampled as having 1.0 in G and B components.
    #[serde(rename = "metallicRoughnessTexture")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    metallic_roughness_texture: Option<TextureInfo>,
}

impl PbrMetallicRoughness {
    fn is_default(&self) -> bool {
        self.base_color_factor == [1.0; 4]
            && self.base_color_texture.is_none()
            && self.metallic_factor == 1.0
            && self.roughness_factor == 1.0
            && self.metallic_roughness_texture.is_none()
    }
}

impl Default for PbrMetallicRoughness {
    fn default() -> Self {
        PbrMetallicRoughness {
            base_color_factor: [1.0; 4],
            base_color_texture: None,
            metallic_factor: 1.0,
            roughness_factor: 1.0,
            metallic_roughness_texture: None,
        }
    }
}

/// Reference to a texture.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TextureInfo {
    /// The index of the texture.
    index: usize,

    /// This integer value is used to construct a string in the format `TEXCOORD_<set index>`
    /// which is a reference to a key in [mesh.primitives.attributes](MeshPrimitive::attributes)
    /// (e.g. a value of `0` corresponds to [TEXCOORD_0](PrimitiveAttributes::tex_coord_0)).
    /// A mesh primitive **MUST** have the corresponding texture coordinate attributes for
    /// the material to be applicable to it.
    #[serde(rename = "texCoord")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_0")]
    tex_coord: usize,
}

/// Reference to a texture.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NormalTextureInfo {
    /// The index of the texture.
    index: usize,

    /// This integer value is used to construct a string in the format `TEXCOORD_<set index>`
    /// which is a reference to a key in [mesh.primitives.attributes](MeshPrimitive::attributes)
    /// (e.g. a value of `0` corresponds to [TEXCOORD_0](PrimitiveAttributes::tex_coord_0)).
    /// A mesh primitive **MUST** have the corresponding texture coordinate attributes for
    /// the material to be applicable to it.
    #[serde(rename = "texCoord")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_0")]
    tex_coord: usize,

    /// The scalar parameter applied to each normal vector of the texture. This value scales
    /// the normal vector in X and Y directions using the formula:
    /// `scaledNormal = normalize(<sampled normal texture value> * 2.0 - 1.0) * vec3(<normal scale>, <normal scale>, 1.0)`.
    #[serde(default = "default_10")]
    #[serde(skip_serializing_if = "is_10")]
    scale: f32,
}

/// Reference to a texture.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OcclusionTextureInfo {
    /// The index of the texture.
    index: usize,

    /// This integer value is used to construct a string in the format `TEXCOORD_<set index>`
    /// which is a reference to a key in [mesh.primitives.attributes](MeshPrimitive::attributes)
    /// (e.g. a value of `0` corresponds to [TEXCOORD_0](PrimitiveAttributes::tex_coord_0)).
    /// A mesh primitive **MUST** have the corresponding texture coordinate attributes for
    /// the material to be applicable to it.
    #[serde(rename = "texCoord")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_0")]
    tex_coord: usize,

    /// A scalar parameter controlling the amount of occlusion applied. A value of `0.0` means no occlusion.
    /// A value of `1.0` means full occlusion. This value affects the final occlusion value as:
    /// `1.0 + strength * (<sampled occlusion texture value> - 1.0)`.
    #[serde(default = "default_10")]
    #[serde(skip_serializing_if = "is_10")]
    strength: f32,
}

/// The material’s alpha rendering mode enumeration specifying the interpretation of the alpha value of the base color.
#[derive(Debug, Default, Serialize, Deserialize)]
enum AlphaMode {
    /// The alpha value is ignored, and the rendered output is fully opaque.
    #[default]
    #[serde(rename = "OPAQUE")]
    Opaque,

    /// The rendered output is either fully opaque or fully transparent depending
    /// on the alpha value and the specified [alphaCutoff](Material::alpha_cutoff) value;
    /// the exact appearance of the edges **MAY** be subject to implementation-specific
    /// techniques such as “Alpha-to-Coverage”.
    #[serde(rename = "MASK")]
    Mask,

    /// The alpha value is used to composite the source and destination areas.
    /// The rendered output is combined with the background using the normal painting operation
    /// (i.e. the Porter and Duff over operator).
    #[serde(rename = "BLEND")]
    Blend,
}

impl AlphaMode {
    fn is_default(&self) -> bool {
        match self {
            AlphaMode::Opaque => true,
            _ => false,
        }
    }
}

/// A set of primitives to be rendered. Its global transform is defined by a node that references it.
#[derive(Debug, Serialize, Deserialize)]
struct Mesh {
    /// Storm storage id, if the resource has been loaded.
    #[serde(skip)]
    id: Option<Id<crate::mesh::Mesh>>,

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
    #[serde(rename = "COLOR_n")]
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

/// A node in the node hierarchy. When the node contains [skin](Node::skin),
/// all [mesh.primitives](Mesh::primitives) **MUST** contain [JOINTS_0](PrimitiveAttributes::joints_0)
/// and [WEIGHTS_0](PrimitiveAttributes::weights_0) attributes.
/// A node **MAY** have either a `matrix` or any combination of
/// [translation](Node::translation)/[rotation](Node::rotation)/[scale](Node::scale)
/// (TRS) properties. TRS properties are converted to matrices and postmultiplied in the
/// `T * R * S` order to compose the transformation matrix; first the scale is applied to
/// the vertices, then the rotation, and then the translation. If none are provided, the
/// transform is the identity. When a node is targeted for animation (referenced by an
/// [animation.channel.target](AnimationChannel::target)), [matrix](Node::matrix)
/// **MUST NOT** be present.
#[derive(Debug, Serialize, Deserialize)]
struct Node {
    /// Storm storage id, if the resource has been loaded. Cleared once the scene has been loaded.
    #[serde(skip)]
    id: Option<Id<crate::Node>>,

    /// The index of the camera referenced by this node.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    camera: Option<usize>,

    /// The indices of this node's children.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<usize>,

    /// The index of the skin referenced by this node.
    /// When a skin is referenced by a node within a scene,
    /// all joints used by the skin **MUST** belong to the same scene.
    /// When defined, [mesh](Node::mesh) **MUST** also be defined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    skin: Option<usize>,

    /// A floating-point 4x4 transformation matrix stored in column-major order.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    matrix: Option<[f32; 16]>,

    /// The index of the mesh in this node.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    mesh: Option<usize>,

    /// The node’s unit quaternion rotation in the order (x, y, z, w), where w is the scalar.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    rotation: Option<[f32; 4]>,

    /// The node’s non-uniform scale, given as the scaling factors along the x, y, and z axes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    scale: Option<[f32; 3]>,

    /// The node’s translation along the x, y, and z axes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    translation: Option<[f32; 3]>,

    /// The weights of the instantiated morph target. The number of array elements
    /// **MUST** match the number of morph targets of the referenced mesh.
    /// When defined, [mesh](Node::mesh) **MUST** also be defined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    weights: Option<Vec<f32>>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// Texture sampler properties for filtering and wrapping modes.
#[derive(Debug, Serialize, Deserialize)]
struct Sampler {
    /// wgpu sampler, if the resource has been loaded.
    #[serde(skip)]
    wgpu: Option<wgpu::Sampler>,

    /// Magnification filter.
    #[serde(rename = "magFilter")]
    #[serde(default)]
    #[serde(skip_serializing_if = "MagFilter::is_none")]
    mag_filter: MagFilter,

    /// Minification filter.
    #[serde(rename = "minFilter")]
    #[serde(default)]
    #[serde(skip_serializing_if = "MinFilter::is_none")]
    min_filter: MinFilter,

    /// S (U) wrapping mode. All valid values correspond to WebGL enums.
    #[serde(rename = "wrapS")]
    #[serde(default)]
    #[serde(skip_serializing_if = "WrappingMode::is_none")]
    wrap_s: WrappingMode,

    /// T (V) wrapping mode.
    #[serde(rename = "wrapT")]
    #[serde(default)]
    #[serde(skip_serializing_if = "WrappingMode::is_none")]
    wrap_t: WrappingMode,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// Magnification filter.
#[derive(Debug, Default, Serialize_repr, Deserialize_repr)]
#[repr(u32)]
enum MagFilter {
    #[default]
    None = 0,
    Nearest = 9728,
    Linear = 9729,
}

impl MagFilter {
    fn is_none(&self) -> bool {
        match self {
            MagFilter::None => true,
            _ => false,
        }
    }
}

/// Minification filter.
#[derive(Debug, Default, Serialize_repr, Deserialize_repr)]
#[repr(u32)]
enum MinFilter {
    #[default]
    None = 0,
    Nearest = 9728,
    Linear = 9729,
    NearestMipmapNearest = 9984,
    LinearMipmapNearest = 9985,
    NearestMipmapLinear = 9986,
    LinearMipmapLinear = 9987,
}

impl MinFilter {
    fn is_none(&self) -> bool {
        match self {
            MinFilter::None => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize_repr, Deserialize_repr)]
#[repr(u32)]
enum WrappingMode {
    #[default]
    None = 0,
    ClampToEdge = 33071,
    MirroredRepeat = 33648,
    Repeat = 10497,
}

impl WrappingMode {
    fn is_none(&self) -> bool {
        match self {
            WrappingMode::None => true,
            _ => false,
        }
    }
}

/// The root nodes of a scene.
#[derive(Debug, Serialize, Deserialize)]
struct Scene {
    /// The indices of each root node.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    nodes: Vec<usize>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

// Joints and matrices defining a skin.
#[derive(Debug, Serialize, Deserialize)]
struct Skin {
    /// Nodes using this skin. Cleared once the scene has been loaded.
    #[serde(skip)]
    nodes: Vec<Id<crate::Node>>,

    /// The index of the accessor containing the floating-point 4x4 inverse-bind matrices.
    /// Its [accessor.count](Accessor::count) property **MUST** be greater than or equal to
    /// the number of elements of the joints array. When undefined, each matrix is a 4x4
    /// identity matrix.
    #[serde(rename = "inverseBindMatrices")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    inverse_bind_matrices: Option<usize>,

    /// The index of the node used as a skeleton root. The node **MUST** be the closest common
    /// root of the joints hierarchy or a direct or indirect parent node of the closest common root.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    skeleton: Option<usize>,

    /// Indices of skeleton nodes, used as joints in this skin.
    joints: Vec<usize>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// A texture and its sampler.
#[derive(Debug, Serialize, Deserialize)]
struct Texture {
    /// Storm storage id, if the resource has been loaded.
    #[serde(skip)]
    id: Option<Id<crate::material::Texture>>,

    /// The index of the sampler used by this texture. When undefined, a sampler
    /// with repeat wrapping and auto filtering **SHOULD** be used.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    sampler: Option<usize>,

    /// The index of the image used by this texture. When undefined, an extension or
    /// other mechanism **SHOULD** supply an alternate texture source, otherwise behavior is undefined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<usize>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

fn is_0(value: &usize) -> bool {
    *value == 0
}

fn is_false(value: &bool) -> bool {
    *value == false
}

fn is_3x00(value: &[f32; 3]) -> bool {
    *value == [0.0; 3]
}

fn default_05() -> f32 {
    0.5
}

fn is_05(value: &f32) -> bool {
    *value == 0.5
}

fn default_10() -> f32 {
    1.0
}

fn is_10(value: &f32) -> bool {
    *value == 1.0
}

fn default_4x10() -> [f32; 4] {
    [1.0; 4]
}

fn is_4x10(value: &[f32; 4]) -> bool {
    *value == [1.0; 4]
}
