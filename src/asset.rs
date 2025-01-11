use std::{collections::HashMap, path::Path};

use glam::{Mat4, Quat, Vec3};
use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::{
    camera::Camera,
    scene::{
        MeshCreationError, MeshDescriptor, MeshId, NodeCreationError, NodeDescriptor,
        NodeTransform, PrimitiveAttributes, PrimitiveDescriptor, PrimitiveIndices, Scene,
    },
};

pub struct Asset {
    pub document: gltf::Document,
    buffers: Vec<gltf::buffer::Data>,
    _images: Vec<gltf::image::Data>,
}

impl Asset {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, gltf::Error> {
        let (document, buffers, images) = gltf::import(path)?;
        Ok(Self {
            document,
            buffers,
            _images: images,
        })
    }

    pub fn create_scene(
        &self,
        gltf_scene: gltf::Scene,
        device: &wgpu::Device,
        camera: Camera,
    ) -> Result<Scene, SceneCreationError> {
        let mut scene = Scene::new(device, camera);
        let mut mesh_mapping = HashMap::new();

        let mut nodes = Vec::with_capacity(gltf_scene.nodes().len());
        for node in gltf_scene.nodes() {
            nodes.push(self.create_node(&node, &mut scene, &mut mesh_mapping, device)?);
        }
        scene.create_node(nodes, device)?;

        Ok(scene)
    }

    fn create_mesh(
        &self,
        gltf_mesh: &gltf::Mesh,
        device: &wgpu::Device,
    ) -> Result<MeshDescriptor, SceneCreationError> {
        let mut primitives = Vec::with_capacity(gltf_mesh.primitives().len());
        for primitive in gltf_mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&self.buffers.get(buffer.index())?));

            if let Some(positions) = reader.read_positions() {
                let mut vertex_count = positions.len() as u32;

                let indices = match reader.read_indices() {
                    Some(indices) => match indices {
                        gltf::mesh::util::ReadIndices::U8(_) => {
                            return Err(SceneCreationError::Unsupported(
                                "Only u16 and u32 index format are supported".to_string(),
                            ))
                        }
                        gltf::mesh::util::ReadIndices::U16(indices) => {
                            let indices: Vec<_> = indices.collect();
                            vertex_count = indices.len() as u32;
                            Some(PrimitiveIndices {
                                buffer: device.create_buffer_init(
                                    &wgpu::util::BufferInitDescriptor {
                                        label: Some("Index buffer"),
                                        contents: bytemuck::cast_slice(&indices),
                                        usage: wgpu::BufferUsages::INDEX,
                                    },
                                ),
                                format: wgpu::IndexFormat::Uint16,
                            })
                        }
                        gltf::mesh::util::ReadIndices::U32(indices) => {
                            let indices: Vec<_> = indices.collect();
                            vertex_count = indices.len() as u32;
                            Some(PrimitiveIndices {
                                buffer: device.create_buffer_init(
                                    &wgpu::util::BufferInitDescriptor {
                                        label: Some("Index buffer"),
                                        contents: bytemuck::cast_slice(&indices),
                                        usage: wgpu::BufferUsages::INDEX,
                                    },
                                ),
                                format: wgpu::IndexFormat::Uint32,
                            })
                        }
                    },
                    None => None,
                };

                let normals = reader
                    .read_normals()
                    .ok_or(SceneCreationError::Unsupported(
                        "Attributes NORMALS is required".to_string(),
                    ))?;

                let attributes: Vec<_> = positions
                    .zip(normals)
                    .map(|(position, normal)| PrimitiveAttributes { position, normal })
                    .collect();

                let attributes = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Attributes buffer"),
                    contents: bytemuck::cast_slice(&attributes),
                    usage: wgpu::BufferUsages::VERTEX,
                });

                primitives.push(PrimitiveDescriptor {
                    vertex_count,
                    indices,
                    attributes,
                });
            }
        }

        Ok(MeshDescriptor { primitives })
    }

    fn create_node(
        &self,
        gltf_node: &gltf::Node,
        scene: &mut Scene,
        meshes_mapping: &mut HashMap<usize, MeshId>,
        device: &wgpu::Device,
    ) -> Result<NodeDescriptor, SceneCreationError> {
        let local_transform = match gltf_node.transform() {
            gltf::scene::Transform::Decomposed {
                translation,
                rotation,
                scale,
            } => NodeTransform::TRS {
                translation: Vec3::from_array(translation),
                rotation: Quat::from_array(rotation),
                scale: Vec3::from_array(scale),
            },
            gltf::scene::Transform::Matrix { matrix } => {
                NodeTransform::Matrix(Mat4::from_cols_array_2d(&matrix))
            }
        };

        let mesh = match gltf_node.mesh() {
            Some(mesh) => Some(match meshes_mapping.entry(mesh.index()) {
                std::collections::hash_map::Entry::Occupied(entry) => *entry.get(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let mesh = self.create_mesh(&mesh, device)?;
                    *entry.insert(scene.create_mesh(mesh, device)?)
                }
            }),
            None => None,
        };

        let mut children = Vec::with_capacity(gltf_node.children().len());
        for child in gltf_node.children() {
            children.push(self.create_node(&child, scene, meshes_mapping, device)?);
        }

        Ok(NodeDescriptor {
            local_transform,
            children,
            mesh,
        })
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SceneCreationError {
    #[error(transparent)]
    NodeCreationError(#[from] NodeCreationError),
    #[error(transparent)]
    MeshCreationError(#[from] MeshCreationError),
    #[error("unsupported: {0}")]
    Unsupported(String),
}
