#![allow(non_snake_case)]

use glam_js::Vec3;
use thiserror::Error;
use wasm_bindgen::prelude::*;

use crate::{asset::mesh::Mesh, scene::Scene};

/// A unique identifier for a node.
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub struct NodeId(storm::node::NodeId);

impl From<storm::node::NodeId> for NodeId {
    fn from(value: storm::node::NodeId) -> Self {
        Self(value)
    }
}

impl From<NodeId> for storm::node::NodeId {
    fn from(value: NodeId) -> Self {
        value.0
    }
}

/// A builder for scene graphs node.
#[wasm_bindgen]
pub struct NodeBuilder {
    name: String,
    parent: Option<NodeId>,
    translation: Vec3,
}

#[wasm_bindgen]
impl NodeBuilder {
    /// Create a new node with default parameters:
    /// - No name;
    /// - No parent node;
    /// - No translation.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            name: String::new(),
            parent: None,
            translation: Vec3::ZERO(),
        }
    }

    pub fn name(self, name: String) -> Self {
        Self { name, ..self }
    }

    pub fn parent(self, parent: NodeId) -> Self {
        Self {
            parent: Some(parent),
            ..self
        }
    }

    pub fn translation(self, translation: Vec3) -> Self {
        Self {
            translation,
            ..self
        }
    }

    /// Build the node.
    pub fn build(self, scene: &mut Scene) -> Result<NodeId, NodeBuilderError> {
        let mut builder = storm::node::NodeBuilder::default()
            .name(self.name)
            .translation(self.translation);
        if let Some(parent) = self.parent {
            builder = builder.parent(parent);
        }
        Ok(NodeId(builder.build(&mut scene.0).map_err(
            |e| match e {
                storm::node::NodeBuilderError::InvalidParentNode(_) => {
                    NodeBuilderError::InvalidParent
                }
            },
        )?))
    }
}

/// Error when {@link NodeBuilder.build()} fails.
#[wasm_bindgen]
#[derive(Debug, Error)]
pub enum NodeBuilderError {
    #[error("failed to found parent node in the scene")]
    InvalidParent,
}

#[wasm_bindgen]
impl Scene {
    /// This function is used to add a mesh to the scene.
    /// The mesh will be rendered at the node's location.
    /// To be precise, the local space of the mesh will match the node's one.
    /// A single mesh can be attached to multiple nodes.
    pub fn attachMeshToNode(
        &mut self,
        _mesh: Mesh,
        _node: NodeId,
    ) -> Result<(), AttachMeshToNodeError> {
        todo!()
    }

    /// This function removes one instance of the mesh. Other instances of the same mesh are left untouched.
    pub fn detachMeshFromNode(&mut self, _mesh: Mesh, _node: NodeId) {
        todo!()
    }
}

/// Error when {@link Scene.attachMeshToNode()} fails.
#[wasm_bindgen]
#[derive(Debug, Error)]
pub enum AttachMeshToNodeError {
    #[error("invalid node")]
    InvalidNode,
}
