use std::ops::{Index, IndexMut};

use glam::{Mat4, Quat, Vec3};
use thiserror::Error;

use crate::storage::{Id, Storage};

use super::mesh::{MeshId, MeshManager};

pub struct NodeManager {
    nodes: Storage<Node>,
}

impl NodeManager {
    pub fn new() -> Self {
        Self {
            nodes: Storage::new(),
        }
    }

    pub fn create(
        &mut self,
        node: &NodeDescriptor,
        meshes: &mut MeshManager,
        device: &wgpu::Device,
    ) -> Result<NodeId, NodeCreationError> {
        let local_matrix = match node.local_transform {
            Transform::Matrix(matrix) => matrix,
            Transform::TRS {
                translation,
                rotation,
                scale,
            } => Mat4::from_scale_rotation_translation(scale, rotation, translation),
        };

        let node_id = self.nodes.add(Node {
            local_transform: node.local_transform,
            global_transform: local_matrix,
            parent: node.parent,
            children: Vec::new(),
            mesh: node.mesh,
        });

        if let Some(parent_id) = node.parent {
            let parent = self
                .nodes
                .get_mut(parent_id)
                .ok_or(NodeCreationError::InvalidParent(parent_id))?;
            parent.children.push(node_id);
            self.nodes[node_id].global_transform = parent.global_transform * local_matrix;
        }

        if let Some(mesh) = node.mesh {
            let mesh = meshes
                .get_mut(mesh)
                .ok_or(NodeCreationError::InvalidMesh(mesh))?;
            mesh.add_node(node_id, device);
        }

        Ok(node_id)
    }
}

impl Index<NodeId> for NodeManager {
    type Output = Node;

    fn index(&self, index: NodeId) -> &Self::Output {
        &self.nodes[index]
    }
}

impl IndexMut<NodeId> for NodeManager {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        &mut self.nodes[index]
    }
}

pub struct Node {
    #[allow(dead_code)]
    local_transform: Transform,
    global_transform: Mat4,
    #[allow(dead_code)]
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    #[allow(dead_code)]
    mesh: Option<MeshId>,
}

pub type NodeId = Id<Node>;

impl Node {
    pub fn global_transform(&self) -> Mat4 {
        self.global_transform
    }
}

#[derive(Debug, Default)]
#[non_exhaustive]
pub struct NodeDescriptor {
    pub local_transform: Transform,
    pub parent: Option<NodeId>,
    pub mesh: Option<MeshId>,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum NodeCreationError {
    #[error("invalid parent node {0:?}")]
    InvalidParent(NodeId),
    #[error("invalid mesh {0:?}")]
    InvalidMesh(MeshId),
}

#[derive(Debug, Clone, Copy)]
pub enum Transform {
    Matrix(Mat4),
    TRS {
        translation: Vec3,
        rotation: Quat,
        scale: Vec3,
    },
}

impl Default for Transform {
    fn default() -> Self {
        Transform::Matrix(Mat4::IDENTITY)
    }
}
