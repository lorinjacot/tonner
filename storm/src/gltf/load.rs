use std::{
    fs::File,
    io::{BufReader, Read, Seek},
    path::Path,
};

use anyhow::{Context, Result, anyhow};
use bytemuck::bytes_of_mut;
use glam::{Mat4, Quat, Vec3};

use crate::{
    DenseEntry, Id, Resources,
    gltf::{GlbChunk, GlbError, GltfEntity, GltfError, accessor::IteratorConsumer},
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
                if buffer.uri().is_none() {
                    let bin = GlbChunk::from_reader(&mut reader, &read_failed_ctx)?;
                    if bin.chunk_type != super::BIN {
                        return Err(GlbError::BinChunkMissing.into());
                    }
                    *buffer.bytes_mut() = bin.chunk_data;
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
            if let Some(uri) = &buffer.uri() {
                let path = parent.join(uri);

                let read_failed_ctx = || format!("Failed to read binary buffer from {path:?}");
                let mut file = File::open(&path).with_context(read_failed_ctx)?;
                file.read_to_end(buffer.bytes_mut())
                    .with_context(read_failed_ctx)?;
                if buffer.bytes().len() < buffer.byte_length() {
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

                impl<'a, 's> IteratorConsumer<'a, Mat4> for RegisterBindMatrices<'a, 's> {
                    type Return = SkinBuilder<'a, 's>;

                    fn consume<I: Iterator<Item = Mat4> + 'a>(
                        self,
                        iter: I,
                    ) -> Result<Self::Return> {
                        Ok(self.builder.inverse_bind_matrices(iter))
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

                builder =
                    accessor.iter_mat4(&self.json.buffer_views, &self.json.buffers, consumer)?;
            }

            let skin = builder.build().id();
            for node in nodes {
                scene.add_skin_to_node(skin, node);
            }
        }

        for animation in &self.json.animations {
            animation.load_into(
                &self.json.nodes,
                &self.json.accessors,
                &self.json.buffer_views,
                &self.json.buffers,
                &mut scene,
            )?;
        }

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
                    .map(|index| self.json.meshes[index].weights().clone())
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
