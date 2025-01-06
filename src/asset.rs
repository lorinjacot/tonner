use std::{collections::HashMap, path::Path};

use glam::{Mat4, Quat, Vec3};

use crate::{
    camera::Camera,
    scene::{MeshBuilder, MeshId, NodeDescriptor, NodeTransform, PrimitiveBuilder, Scene},
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
        targets: &[Option<wgpu::ColorTargetState>],
    ) -> Result<Scene, ()> {
        let mut scene = Scene::new(device, camera, targets);
        let mut mesh_mapping = HashMap::new();

        let mut nodes = Vec::with_capacity(gltf_scene.nodes().len());
        for node in gltf_scene.nodes() {
            nodes.push(self.create_node(&node, &mut scene, &mut mesh_mapping, device)?);
        }
        scene.create_node(nodes, device).or(Err(()))?;

        Ok(scene)
    }

    fn create_mesh(&self, gltf_mesh: &gltf::Mesh) -> MeshBuilder {
        let mut primitives = Vec::with_capacity(gltf_mesh.primitives().len());
        for primitive in gltf_mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&self.buffers.get(buffer.index())?));

            if let Some(positions) = reader.read_positions() {
                let positions: Vec<_> = positions
                    .map(|position| Vec3::from_array(position))
                    .collect();

                let indices: Option<Vec<_>> = reader.read_indices().map(|indices| match indices {
                    gltf::mesh::util::ReadIndices::U16(indices) => {
                        indices.map(|index| index as u32).collect()
                    }
                    gltf::mesh::util::ReadIndices::U32(indices) => indices.collect(),
                    gltf::mesh::util::ReadIndices::U8(indices) => {
                        indices.map(|index| index as u32).collect()
                    }
                });

                primitives.push(
                    PrimitiveBuilder::new()
                        .set_indices(indices)
                        .set_positions(positions),
                );
            }

            // PrimitiveBuilder::new()
            //     .set_attributes(attributes)
            //     .set_indices(buffer, format)
            //     .set_vertex_count(count)
        }

        MeshBuilder::new().set_primitives(primitives)
    }

    fn create_node(
        &self,
        gltf_node: &gltf::Node,
        scene: &mut Scene,
        meshes_mapping: &mut HashMap<usize, MeshId>,
        device: &wgpu::Device,
    ) -> Result<NodeDescriptor, ()> {
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
                    let mesh = self.create_mesh(&mesh);
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
