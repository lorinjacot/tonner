use std::{array::from_fn, ops::Deref};

use bytemuck::{Pod, Zeroable, cast_slice};
use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

use crate::storage::{DenseEntry, Id};
use crate::{Resources, Transform, geometry::MAX_MORPH_TARGET_COUNT, mesh::Mesh};

use super::{Camera, PointLight, Scene, camera::CameraDescriptor, skin::Skin};

pub type NodeId = Id<Node>;

pub struct Node {
    id: Id<Node>,
    pub name: String,
    parent: Option<Id<Node>>,
    children: Vec<Id<Node>>,
    pub(super) local_transform: Transform,
    world_matrix: Mat4,
    mesh: Option<Id<Mesh>>,
    skin: Option<Id<Skin>>,
    pub(super) weights: Vec<f32>,
}

impl Node {
    pub fn parent(&self) -> Option<Id<Node>> {
        self.parent
    }

    pub fn children(&self) -> &[Id<Node>] {
        &self.children
    }

    pub fn local_transform(&self) -> &Transform {
        &self.local_transform
    }

    pub fn local_position(&self) -> Vec3 {
        self.local_transform.translation()
    }

    pub fn world_matrix(&self) -> Mat4 {
        self.world_matrix
    }

    pub fn world_position(&self) -> Vec3 {
        self.world_matrix.project_point3(Vec3::ZERO)
    }

    pub fn weights(&self) -> &[f32] {
        &self.weights
    }
}

pub struct NodeHandle<'a> {
    pub(super) id: Id<Node>,
    pub(super) scene: &'a mut Scene,
}

impl<'a> NodeHandle<'a> {
    pub(super) fn update_world_matrices_parent(&mut self, parent_matrix: Mat4) {
        let node = &mut self.scene[self.id];
        let world_matrix = parent_matrix * node.local_transform.matrix();
        node.world_matrix = world_matrix;
        let children = node.children.clone();
        for child in children {
            self.scene
                .node_handle(child)
                .update_world_matrices_parent(world_matrix);
        }
    }

    pub(super) fn update_world_matrices(&mut self) {
        let node = &self.scene[self.id];
        let world_matrix = match self.parent {
            Some(parent) => self.scene[parent].world_matrix * node.local_transform().matrix(),
            None => node.local_transform.matrix(),
        };
        let node = &mut self.scene[self.id];
        node.world_matrix = world_matrix;
        let children = node.children.clone();
        for child in children {
            self.scene
                .node_handle(child)
                .update_world_matrices_parent(world_matrix);
        }
    }

    pub fn set_local_transform(&mut self, transform: Transform) {
        let node = &mut self.scene[self.id];
        node.local_transform = transform;
        self.update_world_matrices();
    }

    pub fn set_mesh(&mut self, mesh: Option<Id<Mesh>>, resources: &Resources) {
        let node = &mut self.scene[self.id];
        if node.mesh == mesh.map(|mesh| mesh.id()) {
            return;
        }

        // remove old mesh
        self.scene[self.id].mesh.take().map(|old_mesh| {
            let nodes = &mut self.scene.meshes[old_mesh].nodes;
            let index = nodes.iter().position(|node| *node == self.id).unwrap();
            nodes.swap_remove(index);
        });

        if let Some(mesh) = mesh {
            self.scene
                .add_mesh_to_node_unchecked(mesh, self.id, resources);
        }
    }

    pub fn set_camera(&mut self, camera: Option<CameraDescriptor>) {
        match camera {
            Some(desc) => {
                self.scene.cameras.insert(Camera::new(self.id, desc));
            }
            None => {
                self.scene.cameras.remove(self.id);
            }
        }
    }
}

impl<'a> Deref for NodeHandle<'a> {
    type Target = Node;

    fn deref(&self) -> &Self::Target {
        &self.scene[self.id]
    }
}

/// A builder for scene graph nodes.
#[derive(Default)]
pub struct NodeBuilder {
    name: Option<String>,
    parent: Option<Id<Node>>,
    translation: Option<Vec3>,
}

impl NodeBuilder {
    /// Set the node name.
    pub fn name(self, name: impl Into<Option<String>>) -> Self {
        Self {
            name: name.into(),
            ..self
        }
    }

    /// Set the node parent. A node without any parent will be added as a root node.
    pub fn parent(self, parent: impl Into<Option<NodeId>>) -> Self {
        Self {
            parent: parent.into(),
            ..self
        }
    }

    pub fn translation(self, translation: impl Into<Option<Vec3>>) -> Self {
        Self {
            translation: translation.into(),
            ..self
        }
    }

    pub fn build(self, scene: &mut Scene) -> NodeId {
        let id = scene.nodes.next_id();
        let local_matrix = Mat4::from_translation(self.translation.unwrap_or(Vec3::ZERO));
        let world_matrix = match self.parent {
            Some(parent) => {
                let parent = &mut scene.nodes[parent];
                parent.children.push(id);
                parent.world_matrix * local_matrix
            }
            None => {
                scene.root_nodes.push(id);
                local_matrix
            }
        };
        scene.nodes_buffer = None;
        let id = scene.nodes.next_id();
        let mut local_transform = Transform::IDENTITY;
        local_transform.set_matrix(local_matrix);
        let node = Node {
            id,
            name: self.name.unwrap_or_else(|| format!("Node {id}")),
            parent: self.parent,
            children: Vec::new(),
            local_transform,
            world_matrix,
            mesh: None,
            weights: Vec::new(),
            skin: None,
        };
        scene.nodes.insert(node);

        id
    }
}

impl DenseEntry for Node {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

impl Scene {
    pub(super) fn update_nodes_buffer(&mut self) {
        let data: Vec<_> = self
            .nodes
            .iter()
            .map(|node| {
                let matrix = node.world_matrix();
                let mut weights_iter = node.weights().iter().copied();
                let weights = from_fn(|_| weights_iter.next().unwrap_or(0.0));
                let joint_offset = match node.skin {
                    Some(skin) => self.skins[skin].joint_offset(),
                    None => 0,
                };
                NodeUniform {
                    matrix,
                    weights,
                    joint_offset,
                    _pad: [0; 3],
                }
            })
            .collect();

        match &self.nodes_buffer {
            Some(buffer) => {
                self.queue.write_buffer(buffer, 0, cast_slice(&data));
            }
            None => {
                self.render_bind_group = None;
                let buffer = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("{}'s nodes buffer", self.name)),
                        contents: cast_slice(&data),
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    });
                self.nodes_buffer = Some(buffer);
            }
        }
    }

    pub fn add_skin_to_node(&mut self, skin: Id<Skin>, node: Id<Node>) {
        self.nodes[node].skin = Some(skin);
    }
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct NodeUniform {
    matrix: Mat4,
    weights: [f32; MAX_MORPH_TARGET_COUNT],
    joint_offset: u32,
    _pad: [u32; 3],
}
