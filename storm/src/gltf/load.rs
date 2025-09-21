use std::{
    fs::File,
    io::{BufReader, Cursor, Read, Seek},
    path::Path,
};

use anyhow::{Context, Result, anyhow};
use bytemuck::{bytes_of_mut, cast_slice};
use data_url::{DataUrl, DataUrlError};
use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use image::{ImageFormat, ImageReader};

use super::transforms::*;
use crate::{
    DenseEntry, Id, Resources,
    geometry::GeometryBuilder,
    gltf::{
        AccessorComponentType, AccessorType, AccessorUsage, GlbChunk, GlbError, GltfEntity,
        GltfError, accessor::IteratorConsumer,
    },
    skin::SkinBuilder,
};

impl super::GltfAsset {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let read_failed_ctx = || format!("Failed to read asset from {path:?}");

        let mut file = File::open(path).with_context(read_failed_ctx)?;
        let parent = path
            .parent()
            .expect("a file should have a parent")
            .to_owned();

        let mut magic: u32 = 0;
        file.read_exact(bytes_of_mut(&mut magic))
            .with_context(read_failed_ctx)?;
        let mut json = if magic == super::GLTF {
            let mut reader = BufReader::new(file);

            let mut version: u32 = 0;
            reader
                .read_exact(bytes_of_mut(&mut version))
                .with_context(read_failed_ctx)?;
            if version != 2 {
                return Err(GlbError::UnknownVersion.into());
            }

            let mut length: u32 = 0;
            reader
                .read_exact(bytes_of_mut(&mut length))
                .with_context(read_failed_ctx)?;
            length -= super::GLB_HEADER_SIZE;
            let mut reader = reader.take(length as u64);

            let json = GlbChunk::from_reader(&mut reader, &read_failed_ctx)?;
            if json.chunk_type != super::JSON {
                return Err(GlbError::JsonChunkMissing.into());
            }
            let mut json: super::Gltf = serde_json::from_slice(&json.chunk_data)?;

            if let Some(buffer) = json.buffers.first_mut() {
                if buffer.uri.is_none() {
                    let bin = GlbChunk::from_reader(&mut reader, &read_failed_ctx)?;
                    if bin.chunk_type != super::BIN {
                        return Err(GlbError::BinChunkMissing.into());
                    }
                    buffer.bytes = bin.chunk_data;
                }
            }
            json
        } else {
            file.rewind().with_context(read_failed_ctx)?;

            let mut json = String::new();
            file.read_to_string(&mut json)
                .with_context(read_failed_ctx)?;

            serde_json::from_str(&json)?
        };

        for (idx, buffer) in json.buffers.iter_mut().enumerate() {
            if let Some(uri) = &buffer.uri {
                let path = parent.join(uri);

                let read_failed_ctx = || format!("Failed to read binary buffer from {path:?}");
                let mut file = File::open(&path).with_context(read_failed_ctx)?;
                file.read_to_end(&mut buffer.bytes)
                    .with_context(read_failed_ctx)?;
                if buffer.bytes.len() < buffer.byte_length {
                    return Err(
                        anyhow!("the byte lenght of the referenced resource must be greater than or equal to the buffer.byte_length property")
                    ).with_context(|| format!("Failed to load buffer {idx}"));
                }
            }
        }

        Ok(Self {
            parent,
            json,
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

        let mut data = Vec::with_capacity(self.json.skins.len());
        for skin in &mut self.json.skins {
            if !skin.nodes.is_empty() {
                let mut joints = Vec::with_capacity(skin.joints.len());
                for &index in skin.joints.iter() {
                    joints.push(self.json.nodes.get(index).and_then(|node| node.id).ok_or(
                        GltfError::InvalidIndex {
                            entity: GltfEntity::Node,
                            index,
                        },
                    )?);
                }
                data.push((
                    std::mem::take(&mut skin.nodes),
                    joints,
                    skin.inverse_bind_matrices,
                ));
            }
        }

        for (nodes, joints, inverse_bind_matrices) in data {
            let mut builder = scene.skin_builder().nodes(joints);
            if let Some(inverse_bind_matrices) = inverse_bind_matrices {
                struct RegisterBindMatrices<'a, 's> {
                    builder: SkinBuilder<'a, 's>,
                }

                impl<'a, 's> IteratorConsumer<'a, &'a [f32; 4 * 4]> for RegisterBindMatrices<'a, 's> {
                    type Return = SkinBuilder<'a, 's>;

                    fn consume<I: Iterator<Item = &'a [f32; 4 * 4]> + 'a>(
                        self,
                        iter: I,
                    ) -> Result<Self::Return> {
                        Ok(self
                            .builder
                            .inverse_bind_matrices(iter.map(Mat4::from_cols_array)))
                    }
                }

                let consumer = RegisterBindMatrices { builder };
                let accessor = self
                    .json
                    .accessors
                    .get(inverse_bind_matrices)
                    .with_context(|| {
                        format!("inverse_bind_matrices {inverse_bind_matrices} is out of range")
                    })?;

                builder = accessor.iter_unchecked(
                    &self.json.buffer_views,
                    &self.json.buffers,
                    consumer,
                )?;
            }

            let skin = builder.build().id();
            for node in nodes {
                scene.add_skin_to_node(skin, node);
            }
        }

        // 'anim: for animation in &self.json.animations {
        //     let mut node_morph_targets_count_channel = Vec::new();
        //     'channel: for channel in &animation.channels {
        //         let node = match channel.target.node {
        //             Some(node) => node,
        //             None => continue 'channel,
        //         };
        //         match self
        //             .json
        //             .nodes
        //             .get(node)
        //             .ok_or(GltfError::InvalidIndex {
        //                 entity: GltfEntity::Node,
        //                 index: node,
        //             })?
        //             .id
        //         {
        //             Some(id) => {
        //                 let morph_targets_count = scene[id].weights().len();
        //                 node_morph_targets_count_channel.push((id, morph_targets_count, channel));
        //             }
        //             None => {
        //                 continue 'anim;
        //             }
        //         }
        //     }
        //     let mut channels = Vec::with_capacity(node_morph_targets_count_channel.len());
        //     for (node, morph_targets_count, channel) in node_morph_targets_count_channel {
        //         let sampler =
        //             animation
        //                 .samplers
        //                 .get(channel.sampler)
        //                 .ok_or(GltfError::InvalidIndex {
        //                     entity: GltfEntity::AnimationSampler,
        //                     index: channel.sampler,
        //                 })?;
        //         let inputs = self
        //             .accessor_iter::<f32, 1>(sampler.input)?
        //             .map(|t| t[0])
        //             .collect();
        //         let interpolation = match sampler.interpolation {
        //             super::AnimationInterpolation::Step => animation::Interpolation::Step,
        //             super::AnimationInterpolation::Linear => animation::Interpolation::Linear,
        //             super::AnimationInterpolation::Cubicspline => {
        //                 animation::Interpolation::CubicSpline
        //             }
        //         };
        //         let accessor =
        //             self.json
        //                 .accessors
        //                 .get(sampler.output)
        //                 .ok_or(GltfError::InvalidIndex {
        //                     entity: GltfEntity::Accessor,
        //                     index: sampler.output,
        //                 })?;
        //         let outputs = match (
        //             channel.target.path,
        //             accessor.type_,
        //             accessor.component_type,
        //             accessor.normalized,
        //         ) {
        //             (
        //                 super::AnimationTargetPath::Translation,
        //                 AccessorType::Vec3,
        //                 AccessorComponentType::Float,
        //                 false,
        //             ) => animation::Outputs::Translations(
        //                 self.accessor_iter::<f32, 3>(sampler.output)?
        //                     .cloned()
        //                     .collect(),
        //             ),
        //             (
        //                 super::AnimationTargetPath::Rotation,
        //                 AccessorType::Vec4,
        //                 AccessorComponentType::Float,
        //                 false,
        //             ) => animation::Outputs::Rotations(
        //                 self.accessor_iter::<f32, 4>(sampler.output)?
        //                     .cloned()
        //                     .collect(),
        //             ),
        //             (
        //                 super::AnimationTargetPath::Rotation,
        //                 AccessorType::Vec4,
        //                 AccessorComponentType::Byte,
        //                 true,
        //             ) => animation::Outputs::Rotations(
        //                 self.accessor_iter::<i8, 4>(sampler.output)?
        //                     .map(i8x4_to_f32x4)
        //                     .collect(),
        //             ),
        //             (
        //                 super::AnimationTargetPath::Rotation,
        //                 AccessorType::Vec4,
        //                 AccessorComponentType::UnsignedByte,
        //                 true,
        //             ) => animation::Outputs::Rotations(
        //                 self.accessor_iter::<u8, 4>(sampler.output)?
        //                     .map(u8x4_to_f32x4)
        //                     .collect(),
        //             ),
        //             (
        //                 super::AnimationTargetPath::Rotation,
        //                 AccessorType::Vec4,
        //                 AccessorComponentType::Short,
        //                 true,
        //             ) => animation::Outputs::Rotations(
        //                 self.accessor_iter::<i16, 4>(sampler.output)?
        //                     .map(i16x4_to_f32x4)
        //                     .collect(),
        //             ),
        //             (
        //                 super::AnimationTargetPath::Rotation,
        //                 AccessorType::Vec4,
        //                 AccessorComponentType::UnsignedShort,
        //                 true,
        //             ) => animation::Outputs::Rotations(
        //                 self.accessor_iter::<u16, 4>(sampler.output)?
        //                     .map(u16x4_to_f32x4)
        //                     .collect(),
        //             ),
        //             (
        //                 super::AnimationTargetPath::Scale,
        //                 AccessorType::Vec3,
        //                 AccessorComponentType::Float,
        //                 false,
        //             ) => animation::Outputs::Scales(
        //                 self.accessor_iter::<f32, 3>(sampler.output)?
        //                     .cloned()
        //                     .collect(),
        //             ),
        //             (
        //                 super::AnimationTargetPath::Weights,
        //                 AccessorType::Scalar,
        //                 AccessorComponentType::Float,
        //                 false,
        //             ) => animation::Outputs::Weights(
        //                 self.accessor_iter::<f32, 1>(sampler.output)?
        //                     .map(|w| w[0])
        //                     .collect(),
        //                 morph_targets_count,
        //             ),
        //             (
        //                 super::AnimationTargetPath::Weights,
        //                 AccessorType::Scalar,
        //                 AccessorComponentType::Byte,
        //                 true,
        //             ) => animation::Outputs::Weights(
        //                 self.accessor_iter::<i8, 1>(sampler.output)?
        //                     .map(i8x1_to_f32)
        //                     .collect(),
        //                 morph_targets_count,
        //             ),
        //             (
        //                 super::AnimationTargetPath::Weights,
        //                 AccessorType::Scalar,
        //                 AccessorComponentType::UnsignedByte,
        //                 true,
        //             ) => animation::Outputs::Weights(
        //                 self.accessor_iter::<u8, 1>(sampler.output)?
        //                     .map(u8x1_to_f32)
        //                     .collect(),
        //                 morph_targets_count,
        //             ),
        //             (
        //                 super::AnimationTargetPath::Weights,
        //                 AccessorType::Scalar,
        //                 AccessorComponentType::Short,
        //                 true,
        //             ) => animation::Outputs::Weights(
        //                 self.accessor_iter::<i16, 1>(sampler.output)?
        //                     .map(i16x1_to_f32)
        //                     .collect(),
        //                 morph_targets_count,
        //             ),
        //             (
        //                 super::AnimationTargetPath::Weights,
        //                 AccessorType::Scalar,
        //                 AccessorComponentType::UnsignedShort,
        //                 true,
        //             ) => animation::Outputs::Weights(
        //                 self.accessor_iter::<u16, 1>(sampler.output)?
        //                     .map(u16x1_to_f32)
        //                     .collect(),
        //                 morph_targets_count,
        //             ),
        //             (path, accessor_type, component_type, normalized) => {
        //                 return Err(GltfError::InvalidAccessorDataType {
        //                     accessor_type,
        //                     component_type,
        //                     normalized,
        //                     usage: AccessorUsage::AnimationOutpus { path },
        //                 })
        //                 .with_context(|| format!("Failed to load animation"));
        //             }
        //         };
        //         channels.push(animation::Channel {
        //             node,
        //             inputs,
        //             interpolation,
        //             outputs,
        //         });
        //     }
        //     scene
        //         .animation_builder()
        //         .name(animation.name.clone())
        //         .repeat()
        //         .channels(channels)
        //         .build();
        // }

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
    ) -> anyhow::Result<Id<crate::Node>> {
        let node = self.json.nodes.get(index).ok_or(GltfError::InvalidIndex {
            entity: GltfEntity::Node,
            index,
        })?;

        let mesh = match node.mesh {
            Some(index) => Some(
                self.json
                    .meshes
                    .get_mut(index)
                    .ok_or(anyhow!("node.mesh {index} is out of range"))?
                    .load(
                        &self.parent,
                        &self.json.accessors,
                        &mut self.json.materials,
                        &mut self.default_material,
                        &mut self.json.textures,
                        &mut self.json.samplers,
                        &mut self.json.images,
                        &self.json.buffer_views,
                        &self.json.buffers,
                        resources,
                        encoder,
                    )
                    .with_context(|| format!("Failed to load node.mesh {index}"))?,
            ),
            None => None,
        };

        let node = &mut self.json.nodes[index];
        let mut builder = scene.node_builder().name(node.name.clone()).parent(parent);
        builder = match &node.matrix {
            Some(matrix) => builder.local_matrix(Mat4::from_cols_array(matrix)),
            None => builder.translation_rotation_scale(
                node.translation.map_or(Vec3::ZERO, Vec3::from_array),
                node.rotation.map_or(Quat::IDENTITY, Quat::from_array),
                node.scale.map_or(Vec3::ONE, Vec3::from_array),
            ),
        };
        let id = builder
            .mesh(mesh)
            .weights(
                node.weights.clone().or(node
                    .mesh
                    .map(|index| self.json.meshes[index].weights.clone())
                    .flatten()),
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
}

impl GlbChunk {
    fn from_reader<R: Read>(reader: &mut R, read_failed_ctx: &impl Fn() -> String) -> Result<Self> {
        let mut chunk_length: u32 = 0;
        reader
            .read_exact(bytes_of_mut(&mut chunk_length))
            .with_context(read_failed_ctx)?;

        let mut chunk_type: u32 = 0;
        reader
            .read_exact(bytes_of_mut(&mut chunk_type))
            .with_context(read_failed_ctx)?;

        let mut chunk_data = Vec::with_capacity(chunk_length as usize);
        reader
            .take(chunk_length as u64)
            .read_to_end(&mut chunk_data)
            .with_context(read_failed_ctx)?;

        if (chunk_data.len() as u32) < chunk_length {
            return Err(GlbError::InvalidChunkLength.into());
        }

        Ok(Self {
            chunk_type,
            chunk_data,
        })
    }
}

impl super::BufferView {
    pub(super) fn bytes<'a>(&self, buffers: &'a [super::Buffer]) -> Result<&'a [u8]> {
        let buffer = buffers
            .get(self.buffer)
            .ok_or_else(|| anyhow!("buffer_view.buffer {} is out of range", self.buffer))?;

        let start = self.byte_offset;
        let end = start + self.byte_length;

        buffer
            .bytes
            .get(start..end)
            .with_context(|| format!("buffer_view.buffer {} is too short", self.buffer))
    }
}

impl super::Image {
    fn load(
        &mut self,
        srgb: bool,
        parent: &Path,
        buffer_views: &[super::BufferView],
        buffers: &[super::Buffer],
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<wgpu::Texture> {
        if let Some(image) = &self.wgpu {
            return Ok(image.clone());
        }

        let name = self.name.as_deref();
        let image = if let Some(uri) = &self.uri {
            match DataUrl::process(&uri) {
                Ok(url) => {
                    let mime_type = url.mime_type();
                    let (body, _fragment) = url.decode_to_vec()?;
                    ImageReader::with_format(
                        Cursor::new(body),
                        match (mime_type.type_.as_str(), mime_type.subtype.as_str()) {
                            ("image", "png") => ImageFormat::Png,
                            ("image", "jpeg") => ImageFormat::Jpeg,
                            (type_, subtype) => {
                                anyhow::bail!("Unsupported image format {type_}/{subtype}")
                            }
                        },
                    )
                    .decode()?
                }
                Err(DataUrlError::NoComma) => anyhow::bail!("Invalid data url"),
                Err(DataUrlError::NotADataUrl) => {
                    let path = parent.join(uri);
                    ImageReader::open(&path)
                        .with_context(|| format!("Failed to open image at {path:?}"))?
                        .decode()?
                }
            }
        } else {
            let buffer_view = self.buffer_view.ok_or(anyhow!(
                "one of image.uri or image.buffer_view must be defined'"
            ))?;
            let format = match self.mime_type {
                super::ImageMimeType::ImageJpeg => ImageFormat::Jpeg,
                super::ImageMimeType::ImagePng => ImageFormat::Png,
                super::ImageMimeType::None => anyhow::bail!(
                    "image.mime_type must be defined when image.buffer_view is defined"
                ),
            };
            let buffer_view = buffer_views
                .get(buffer_view)
                .ok_or(anyhow!("image.buffer_view is out of range"))?;
            let bytes = match buffers.get(buffer_view.buffer) {
                Some(buffer) => &buffer.bytes,
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
        self.wgpu = Some(texture.clone());
        Ok(texture)
    }
}

impl super::Material {
    fn load(
        &mut self,
        parent: &Path,
        textures: &mut [super::Texture],
        samplers: &mut [super::Sampler],
        buffer_views: &[super::BufferView],
        buffers: &[super::Buffer],
        images: &mut [super::Image],
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<Id<crate::material::Material>> {
        if let Some(id) = self.id {
            return Ok(id);
        }

        let pbr = &self.pbr_metallic_roughness;

        let mut builder = crate::material::MaterialBuilder::default()
            .base_color_factor(pbr.base_color_factor)
            .metallic_factor(pbr.metallic_factor)
            .roughness_factor(pbr.roughness_factor)
            .emissive_factor(self.emissive_factor)
            .alpha_mode(match self.alpha_mode {
                super::AlphaMode::Opaque => crate::material::AlphaMode::Opaque,
                super::AlphaMode::Mask => crate::material::AlphaMode::Mask,
                super::AlphaMode::Blend => crate::material::AlphaMode::Blend,
            })
            .alpha_cutoff(self.alpha_cutoff)
            .double_sided(self.double_sided);

        if let Some(info) = &pbr.base_color_texture {
            let id = textures
                .get_mut(info.index)
                .ok_or(anyhow!(
                    "material.base_color_texture {} is out of range",
                    info.index
                ))?
                .load(
                    true,
                    parent,
                    samplers,
                    images,
                    buffer_views,
                    buffers,
                    resources,
                    encoder,
                )
                .with_context(|| {
                    format!("Failed to load material.base_color_texture {}", info.index)
                })?;
            builder = builder
                .base_color_texture(id)
                .base_color_tex_coord(info.tex_coord as u32);
        }

        if let Some(info) = &pbr.metallic_roughness_texture {
            let id = textures
                .get_mut(info.index)
                .ok_or(anyhow!(
                    "material.metallic_roughness_texture {} is out of range",
                    info.index
                ))?
                .load(
                    false,
                    parent,
                    samplers,
                    images,
                    buffer_views,
                    buffers,
                    resources,
                    encoder,
                )
                .with_context(|| {
                    format!(
                        "Failed to load material.metallic_roughness_texture {}",
                        info.index
                    )
                })?;
            builder = builder
                .metallic_roughness_texture(id)
                .metallic_roughness_tex_coord(info.tex_coord as u32);
        }

        if let Some(info) = &self.normal_texture {
            let id = textures
                .get_mut(info.index)
                .ok_or(anyhow!(
                    "material.normal_texture {} is out of range",
                    info.index
                ))?
                .load(
                    false,
                    parent,
                    samplers,
                    images,
                    buffer_views,
                    buffers,
                    resources,
                    encoder,
                )
                .with_context(|| {
                    format!("Failed to load material.normal_texture {}", info.index)
                })?;
            builder = builder
                .normal_texture(id)
                .normal_tex_coord(info.tex_coord as u32)
                .normal_scale(info.scale);
        }

        if let Some(info) = &self.occlusion_texture {
            let id = textures
                .get_mut(info.index)
                .ok_or(anyhow!(
                    "material.occlusion_texture {} is out of range",
                    info.index
                ))?
                .load(
                    true,
                    parent,
                    samplers,
                    images,
                    buffer_views,
                    buffers,
                    resources,
                    encoder,
                )
                .with_context(|| {
                    format!("Failed to load material.occlusion_texture {}", info.index)
                })?;
            builder = builder
                .occlusion_texture(id)
                .occlusion_tex_coord(info.tex_coord as u32)
                .occlusion_strength(info.strength);
        }

        if let Some(info) = &self.emissive_texture {
            let id = textures
                .get_mut(info.index)
                .ok_or(anyhow!(
                    "material.emissive_texture {} is out of range",
                    info.index
                ))?
                .load(
                    true,
                    parent,
                    samplers,
                    images,
                    buffer_views,
                    buffers,
                    resources,
                    encoder,
                )
                .with_context(|| {
                    format!("Failed to load material.emissive_texture {}", info.index)
                })?;
            builder = builder
                .emissive_texture(id)
                .emissive_tex_coord(info.tex_coord as u32);
        }

        let id = builder.build(resources).id();
        self.id = Some(id);
        Ok(id)
    }
}

impl super::Mesh {
    fn load(
        &mut self,
        parent: &Path,
        accessors: &[super::Accessor],
        materials: &mut [super::Material],
        default_material: &mut Option<Id<crate::material::Material>>,
        textures: &mut [super::Texture],
        samplers: &mut [super::Sampler],
        images: &mut [super::Image],
        buffer_views: &[super::BufferView],
        buffers: &[super::Buffer],
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<Id<crate::mesh::Mesh>> {
        if let Some(id) = self.id {
            return Ok(id);
        }

        let name = self.name.clone();
        let mut primitives = Vec::with_capacity(self.primitives.len());
        for (idx, primitive) in self.primitives.iter().enumerate() {
            if let Some(position) = primitive.attributes.position {
                let primitive_ctx = || format!("Failed to load mesh.primitives[{idx}]");

                let material = match primitive.material {
                    Some(index) => materials
                        .get_mut(index)
                        .ok_or(anyhow!("primitive.material {index} is out of range"))
                        .with_context(primitive_ctx)?
                        .load(
                            parent,
                            textures,
                            samplers,
                            buffer_views,
                            buffers,
                            images,
                            resources,
                            encoder,
                        )
                        .with_context(|| format!("Failed to load primitive.material {index}"))
                        .with_context(primitive_ctx)?,
                    None => match default_material {
                        Some(id) => *id,
                        None => {
                            let id = super::Material::default()
                                .load(
                                    parent,
                                    textures,
                                    samplers,
                                    buffer_views,
                                    buffers,
                                    images,
                                    resources,
                                    encoder,
                                )
                                .context("Failed to load default material")
                                .with_context(primitive_ctx)?;
                            *default_material = Some(id);
                            id
                        }
                    },
                };

                let topology = match primitive.mode {
                    super::PrimitiveMode::Points => wgpu::PrimitiveTopology::PointList,
                    super::PrimitiveMode::LineStrip => wgpu::PrimitiveTopology::LineStrip,
                    super::PrimitiveMode::Lines => wgpu::PrimitiveTopology::LineList,
                    super::PrimitiveMode::Triangles => wgpu::PrimitiveTopology::TriangleList,
                    super::PrimitiveMode::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
                    mode => {
                        return Err(anyhow!("primitive topology type {mode} is not supported"))
                            .with_context(primitive_ctx);
                    }
                };
                let accessor = accessors
                    .get(position)
                    .with_context(|| {
                        format!("primitive.attributes.position {position} is out of range")
                    })
                    .with_context(primitive_ctx)?;

                let normal_tex_coord = resources.materials[material].normal_tex_coord();
                let mut builder = GeometryBuilder::new(accessor.count(), primitive.targets.len())
                    .topology(topology);

                if let Some(normal_tex_coord) = normal_tex_coord {
                    builder = builder.normal_tex_coord(normal_tex_coord);
                }

                macro_rules! dim {
                    (Vec2) => {
                        2
                    };
                    (Vec3) => {
                        3
                    };
                    (Vec4) => {
                        4
                    };
                }

                macro_rules! rust_type {
                    (Byte) => {
                        i8
                    };
                    (UnsignedByte) => {
                        u8
                    };
                    (Short) => {
                        i16
                    };
                    (UnsignedShort) => {
                        u16
                    };
                    (UnsignedInt) => {
                        u32
                    };
                }

                macro_rules! transform {
                    (Vec2, Byte, true) => {
                        i8x2_to_vec2
                    };
                    (Vec2, Short, true) => {
                        i16x2_to_vec2
                    };
                    (Vec2, UnsignedByte, true) => {
                        u8x2_to_vec2
                    };
                    (Vec2, UnsignedShort, true) => {
                        u16x2_to_vec2
                    };
                    (Vec3, Byte, true) => {
                        i8x3_to_vec3
                    };
                    (Vec3, Short, true) => {
                        i16x3_to_vec3
                    };
                    (Vec3, UnsignedByte, true) => {
                        u8x3_to_vec3
                    };
                    (Vec3, UnsignedShort, true) => {
                        u16x3_to_vec3
                    };
                    (Vec4, Byte, true) => {
                        i8x4_to_vec4
                    };
                    (Vec4, Short, true) => {
                        i16x4_to_vec4
                    };
                    (Vec4, UnsignedByte, true) => {
                        u8x4_to_vec4
                    };
                    (Vec4, UnsignedShort, true) => {
                        u16x4_to_vec4
                    };
                    (Vec4, UnsignedByte, false) => {
                        u8x4_to_uvec4
                    };
                    (Vec4, UnsignedShort, false) => {
                        u16x4_to_uvec4
                    };
                }

                macro_rules! case {
                    ($method:ident, (
                        $type_:ident,
                        Float,
                        false
                        $(, extend($value:literal))?
                    ), $accessor:ident, $accessor_ctx:ident) => {{
                        struct Consumer {
                            builder: GeometryBuilder,
                        }

                        impl<'a> IteratorConsumer<'a, &'a [f32; dim!($type_)]> for Consumer {
                            type Return = GeometryBuilder;

                            fn consume<I: Iterator<Item = &'a [f32; dim!($type_)]> + 'a>(self, iter: I) -> Result<Self::Return> {
                                Ok(self.builder.$method(
                                    iter
                                        .cloned()
                                        .map($type_::from_array)
                                        $(.map(|v| v.extend($value)))?,
                                ))
                            }
                        }

                        Consumer {
                            builder
                        }
                    }};
                    ($method:ident, (
                        $type_:ident,
                        $component:ident,
                        $normalized:ident
                        $(, extend($value:literal))?
                    ), $accessor:ident, $accessor_ctx:ident) => {{
                        struct Consumer {
                            builder: GeometryBuilder,
                        }

                        impl<'a> IteratorConsumer<'a, &'a [rust_type!($component); dim!($type_)]> for Consumer {
                            type Return = GeometryBuilder;

                            fn consume<I: Iterator<Item = &'a [rust_type!($component); dim!($type_)]> + 'a>(self, iter: I) -> Result<Self::Return> {
                                Ok(self.builder.$method(
                                    iter
                                        .map(transform!($type_, $component, $normalized))
                                        $(.map(|v| v.extend($value)))?,
                                ))
                            }
                        }

                        Consumer {
                            builder
                        }
                    }};
                }

                macro_rules! load_attribute {
                    (
                        $attr:expr,
                        $method:ident, [
                            $((
                                $type_:ident,
                                $component:ident,
                                $normalized:ident
                                $(, extend($value:literal))?
                            ),)+
                        ]
                    ) => {
                        if let Some(accessor_idx) = $attr {
                            let accessor = accessors
                                            .get(accessor_idx).with_context(|| format!(
                                                "{} {accessor_idx} is out of range",
                                                stringify!($attr),
                                            )).with_context(primitive_ctx)?;

                            let accessor_ctx = || format!(
                                                        "Failed to load {} {accessor_idx}",
                                                        stringify!($attr),
                                                    );

                            builder = match (accessor.type_(), accessor.component_type(), accessor.normalized()) {
                                $(
                                    (AccessorType::$type_, AccessorComponentType::$component, $normalized) => {
                                        accessor.iter_unchecked(buffer_views, buffers,
                                             case!(
                                                $method,
                                                (
                                                    $type_,
                                                    $component,
                                                    $normalized
                                                    $(, extend($value))?
                                                ),
                                                accessor,
                                                accessor_ctx
                                            )
                                        )
                                            .with_context(accessor_ctx)
                                            .with_context(primitive_ctx)?
                                    }
                                )+
                                (accessor_type, component_type, normalized) => {
                                    let normalized = if normalized {
                                        "normalized "
                                    } else {
                                        ""
                                    };
                                    return Err(anyhow!(
                                        "{accessor_type} of {normalized}{component_type} cannot be used for {}",
                                        stringify!($attr),
                                    ))
                                        .with_context(accessor_ctx)
                                        .with_context(primitive_ctx);
                                }
                            };
                        }
                    };
                }

                load_attribute!(
                    primitive.attributes.position,
                    positions,
                    [(Vec3, Float, false),]
                );
                load_attribute!(
                    primitive.attributes.normal,
                    normals,
                    [(Vec3, Float, false),]
                );
                load_attribute!(
                    primitive.attributes.tangent,
                    tangents,
                    [(Vec4, Float, false),]
                );
                load_attribute!(
                    primitive.attributes.tex_coord_0,
                    tex_coords_0,
                    [
                        (Vec2, Float, false),
                        (Vec2, UnsignedByte, true),
                        (Vec2, UnsignedShort, true),
                    ]
                );
                load_attribute!(
                    primitive.attributes.tex_coord_1,
                    tex_coords_1,
                    [
                        (Vec2, Float, false),
                        (Vec2, UnsignedByte, true),
                        (Vec2, UnsignedShort, true),
                    ]
                );
                load_attribute!(
                    primitive.attributes.color_0,
                    colors_0,
                    [
                        (Vec3, Float, false, extend(1.0)),
                        (Vec3, UnsignedByte, true, extend(1.0)),
                        (Vec3, UnsignedShort, true, extend(1.0)),
                        (Vec4, Float, false),
                        (Vec4, UnsignedByte, true),
                        (Vec4, UnsignedShort, true),
                    ]
                );
                load_attribute!(
                    primitive.attributes.joints_0,
                    joints_0,
                    [(Vec4, UnsignedByte, false), (Vec4, UnsignedShort, false),]
                );
                load_attribute!(
                    primitive.attributes.weights_0,
                    weights_0,
                    [
                        (Vec4, Float, false),
                        (Vec4, UnsignedByte, true),
                        (Vec4, UnsignedShort, true),
                    ]
                );

                for (target_idx, morph_target) in primitive.targets.iter().enumerate() {
                    let morph_target_ctx =
                        || format!("Failed to load primitive.target[{target_idx}]");

                    macro_rules! case {
                        ($method:ident, (
                            $type_:ident,
                            Float,
                            false
                            $(, extend($value:literal))?
                        ), $accessor:ident, $accessor_ctx:ident) => {{
                            struct Consumer {
                                builder: GeometryBuilder,
                                target_idx: usize,
                            }

                            impl<'a> IteratorConsumer<'a, &'a [f32; dim!($type_)]> for Consumer {
                                type Return = GeometryBuilder;

                                fn consume<I: Iterator<Item = &'a [f32; dim!($type_)]> + 'a>(self, iter: I) -> Result<Self::Return> {
                                    Ok(self.builder.$method(
                                        self.target_idx,
                                        iter
                                            .cloned()
                                            .map($type_::from_array)
                                            $(.map(|v| v.extend($value)))?,
                                    ))
                                }
                            }

                            Consumer {
                                builder,
                                target_idx,
                            }
                        }};
                        ($method:ident, (
                            $type_:ident,
                            $component:ident,
                            $normalized:ident
                            $(, extend($value:literal))?
                        ), $accessor:ident, $accessor_ctx:ident) => {{
                            struct Consumer {
                                builder: GeometryBuilder,
                                target_idx: usize,
                            }

                            impl<'a> IteratorConsumer<'a, &'a [rust_type!($component); dim!($type_)]> for Consumer {
                                type Return = GeometryBuilder;

                                fn consume<I: Iterator<Item = &'a [rust_type!($component); dim!($type_)]> + 'a>(self, iter: I) -> Result<Self::Return> {
                                    Ok(self.builder.$method(
                                        self.target_idx,
                                        iter
                                            .map(transform!($type_, $component, $normalized))
                                            $(.map(|v| v.extend($value)))?,
                                    ))
                                }
                            }

                            Consumer {
                                builder,
                                target_idx,
                            }
                        }};
                    }

                    macro_rules! load_attribute {
                        ($attr:expr, $method:ident, [
                            $((
                                $type_:ident,
                                $component:ident,
                                $normalized:ident
                                $(, extend($value:literal))?
                            ),)+
                        ]) => {
                            if let Some(accessor_idx) = $attr {
                                let accessor = accessors
                                                .get(accessor_idx).with_context(|| format!(
                                                    "{} {accessor_idx} is out of range",
                                                    stringify!($attr),
                                                ))
                                                .with_context(morph_target_ctx)
                                                .with_context(primitive_ctx)?;

                                let accessor_ctx = || format!(
                                                            "Failed to load {} {accessor_idx}",
                                                            stringify!($attr),
                                                        );

                                builder = match (accessor.type_(), accessor.component_type(), accessor.normalized()) {
                                    $(
                                        (AccessorType::$type_, AccessorComponentType::$component, $normalized) => {
                                            accessor.iter_unchecked(buffer_views, buffers,
                                                case!($method, (
                                                    $type_,
                                                    $component,
                                                    $normalized
                                                    $(, extend($value))?
                                                ), accessor, accessor_ctx),
                                            )
                                                .with_context(accessor_ctx)
                                                .with_context(morph_target_ctx)
                                                .with_context(primitive_ctx)?
                                        }
                                    )+
                                    (accessor_type, component_type, normalized) => {
                                        let normalized = if normalized {
                                            "normalized "
                                        } else {
                                            ""
                                        };
                                        return Err(anyhow!(
                                            "{accessor_type} of {normalized}{component_type} cannot be used for {}",
                                            stringify!($attr),
                                        ))
                                            .with_context(accessor_ctx)
                                            .with_context(morph_target_ctx)
                                            .with_context(primitive_ctx);
                                    }
                                };
                            }
                        };
                    }

                    load_attribute!(
                        morph_target.position,
                        morph_target_positions,
                        [(Vec3, Float, false),]
                    );
                    load_attribute!(
                        morph_target.normal,
                        morph_target_normals,
                        [(Vec3, Float, false),]
                    );
                    load_attribute!(
                        morph_target.tangent,
                        morph_target_tangents,
                        [(Vec3, Float, false),]
                    );
                    load_attribute!(
                        morph_target.tex_coord_0,
                        morph_target_tex_coords_0,
                        [
                            (Vec2, Float, false),
                            (Vec2, Byte, true),
                            (Vec2, Short, true),
                            (Vec2, UnsignedByte, true),
                            (Vec2, UnsignedShort, true),
                        ]
                    );
                    load_attribute!(
                        morph_target.tex_coord_1,
                        morph_target_tex_coords_1,
                        [
                            (Vec2, Float, false),
                            (Vec2, Byte, true),
                            (Vec2, Short, true),
                            (Vec2, UnsignedByte, true),
                            (Vec2, UnsignedShort, true),
                        ]
                    );
                    load_attribute!(
                        morph_target.color_0,
                        morph_target_colors_0,
                        [
                            (Vec3, Float, false, extend(0.0)),
                            (Vec3, Byte, true, extend(0.0)),
                            (Vec3, Short, true, extend(0.0)),
                            (Vec3, UnsignedByte, true, extend(0.0)),
                            (Vec3, UnsignedShort, true, extend(0.0)),
                            (Vec4, Float, false),
                            (Vec4, Byte, true),
                            (Vec4, Short, true),
                            (Vec4, UnsignedByte, true),
                            (Vec4, UnsignedShort, true),
                        ]
                    );
                }

                if let Some(indices) = primitive.indices {
                    let accessor = accessors.get(indices).ok_or_else(|| {
                        anyhow!("mesh.primitives[{idx}].indices {indices} is out of range")
                    })?;

                    let ctx = || format!("Failed to load mesh.primitives[{idx}].indices {indices}");

                    let bytes = accessor
                        .bytes_dense_tighly_packed(buffer_views, buffers)
                        .with_context(ctx)?;
                    builder = match (
                        accessor.type_(),
                        accessor.component_type(),
                        accessor.normalized(),
                    ) {
                        (AccessorType::Scalar, AccessorComponentType::UnsignedByte, false) => {
                            let indices: Vec<_> = bytes.iter().map(|index| *index as u16).collect();
                            builder.indices_u16(&indices, resources)
                        }
                        (AccessorType::Scalar, AccessorComponentType::UnsignedShort, false) => {
                            builder.indices_u16(cast_slice(bytes), resources)
                        }
                        (AccessorType::Scalar, AccessorComponentType::UnsignedInt, false) => {
                            builder.indices_u32(cast_slice(bytes), resources)
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

                let geometry = builder.build(resources, encoder).id();
                primitives.push((geometry, material));
            }
        }

        let id = resources
            .mesh_builder()
            .name(name)
            .primitives(primitives)
            .build()
            .id();

        self.id = Some(id);

        Ok(id)
    }
}

impl super::Sampler {
    fn load(&mut self, resources: &mut Resources) -> anyhow::Result<wgpu::Sampler> {
        if let Some(sampler) = &self.wgpu {
            return Ok(sampler.clone());
        }

        let mag_filter = match self.mag_filter {
            super::MagFilter::Linear => wgpu::FilterMode::Linear,
            super::MagFilter::Nearest | super::MagFilter::None => wgpu::FilterMode::Nearest,
        };
        let (min_filter, mipmap_filter) = match self.min_filter {
            super::MinFilter::LinearMipmapNearest | super::MinFilter::Linear => {
                (wgpu::FilterMode::Linear, wgpu::FilterMode::Nearest)
            }
            super::MinFilter::LinearMipmapLinear => {
                (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear)
            }
            super::MinFilter::NearestMipmapNearest
            | super::MinFilter::Nearest
            | super::MinFilter::None => (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest),
            super::MinFilter::NearestMipmapLinear => {
                (wgpu::FilterMode::Nearest, wgpu::FilterMode::Linear)
            }
        };

        let sampler = resources.device.create_sampler(&wgpu::SamplerDescriptor {
            label: self.name.as_deref(),
            address_mode_u: wrapping_mode_to_address_mode(self.wrap_s),
            address_mode_v: wrapping_mode_to_address_mode(self.wrap_t),
            mag_filter,
            min_filter,
            mipmap_filter,
            ..Default::default()
        });
        self.wgpu = Some(sampler.clone());
        Ok(sampler)
    }
}

fn wrapping_mode_to_address_mode(wrapping_mode: super::WrappingMode) -> wgpu::AddressMode {
    match wrapping_mode {
        super::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        super::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
        super::WrappingMode::Repeat => wgpu::AddressMode::Repeat,
        super::WrappingMode::None => wgpu::AddressMode::Repeat,
    }
}

impl super::Texture {
    fn load(
        &mut self,
        srgb: bool,
        parent: &Path,
        samplers: &mut [super::Sampler],
        images: &mut [super::Image],
        buffer_views: &[super::BufferView],
        buffers: &[super::Buffer],
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<Id<crate::material::Texture>> {
        if let Some(id) = self.id {
            return Ok(id);
        }

        let name = self.name.clone();
        let sampler = self.sampler;
        let source = self.source.ok_or(anyhow!("image.source must be defined"))?;

        let sampler = match sampler {
            Some(index) => Some(
                samplers
                    .get_mut(index)
                    .ok_or(anyhow!("texture.sampler {index} is out of range"))?
                    .load(resources)
                    .with_context(|| format!("Failed to load texture.sampler {index}"))?,
            ),
            None => None,
        };

        let source = images
            .get_mut(source)
            .ok_or(anyhow!("texture.image {source} is out of range"))?
            .load(srgb, parent, buffer_views, buffers, resources, encoder)
            .with_context(|| format!("Failed to load texture.image {source}"))?;

        let id = crate::material::TextureBuilder::default()
            .name(name)
            .sampler(sampler)
            .texture(source)
            .build(resources)
            .id();
        self.id = Some(id);
        Ok(id)
    }
}
