use std::{collections::HashMap, fmt::Display};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use thiserror::Error;
use uuid::Uuid;

use crate::{Scene, geometry::MAX_MORPH_TARGET_COUNT};

/// A unique id for a node. A node can only have one id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(Uuid);

impl Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeId({})", self.0)
    }
}

/// A builder for scene graph nodes.
#[derive(Default)]
pub struct NodeBuilder {
    name: Option<String>,
    parent: Option<NodeId>,
    translation: Option<Vec3>,
}

impl NodeBuilder {
    /// Set the node name.
    pub fn name(self, name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..self
        }
    }

    /// Set the node parent. A node without any parent will be added as a root node.
    pub fn parent(self, parent: impl Into<NodeId>) -> Self {
        Self {
            parent: Some(parent.into()),
            ..self
        }
    }

    /// Set the translation component of the node local transform. Defaults to [`Vec3::ZERO`].
    pub fn translation(self, translation: impl Into<Vec3>) -> Self {
        Self {
            translation: Some(translation.into()),
            ..self
        }
    }

    /// Build the node and add it to the scene.
    pub fn build(self, scene: &mut Scene) -> Result<NodeId, NodeBuilderError> {
        let id = NodeId(Uuid::new_v4());
        let local_matrix = Mat4::from_translation(self.translation.unwrap_or(Vec3::ZERO));
        let global_matrix = match self.parent {
            Some(parent) => {
                let parent_data = scene
                    .node_manager
                    .nodes
                    .get_mut(&parent)
                    .ok_or(NodeBuilderError::InvalidParentNode(parent))?;
                parent_data.children.push(id);
                parent_data.global_matrix * local_matrix
            }
            None => {
                scene.node_manager.root_nodes.push(id);
                local_matrix
            }
        };
        scene.node_manager.nodes.insert(
            id,
            NodeData {
                id,
                name: self.name,
                parent: self.parent,
                children: Vec::new(),
                local_matrix,
                global_matrix,
            },
        );
        scene.node_manager.buffer = None;

        Ok(id)
    }
}

#[derive(Debug, Error)]
pub enum NodeBuilderError {
    #[error("invalid parent node: {0}")]
    InvalidParentNode(NodeId),
}

pub(super) struct NodeManager {
    nodes: HashMap<NodeId, NodeData>,
    root_nodes: Vec<NodeId>,
    buffer: Option<wgpu::Buffer>,
}

impl NodeManager {
    pub(super) fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            root_nodes: Vec::new(),
            buffer: None,
        }
    }

    /// `true` if `node` is a valid id. `false` otherwise.
    pub(super) fn contains(&self, node: NodeId) -> bool {
        self.nodes.contains_key(&node)
    }

    /// Returns the node local matrix, or `None` if the node does not exists.
    ///
    /// The local matrix describe the transform between the node local coordinate system
    /// to the parent one.
    /// If the node has no parent, the local transform is equal to the global transform.
    pub(super) fn local_matrix(&self, node: NodeId) -> Option<Mat4> {
        self.nodes.get(&node).map(|data| data.local_matrix)
    }

    /// Returns the node global matrix, or `None` if the node does not exists.
    ///
    /// The global matrix describe the transform between the node local coordinate system
    /// to the world coordinate system.
    /// If the node has no parent, the global transform is equal to the local transform.
    pub(super) fn global_matrix(&self, node: NodeId) -> Option<Mat4> {
        self.nodes.get(&node).map(|data| data.global_matrix)
    }
}

struct NodeData {
    id: NodeId,
    name: Option<String>,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    local_matrix: Mat4,
    global_matrix: Mat4,
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct NodeUniform {
    matrix: Mat4,
    weights: [f32; MAX_MORPH_TARGET_COUNT],
    joint_offset: u32,
    _pad: [u32; 3],
}
