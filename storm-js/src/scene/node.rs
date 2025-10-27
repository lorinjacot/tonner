use glam_js::Vec3;
use thiserror::Error;
use wasm_bindgen::prelude::*;

use crate::scene::Scene;

/// A unique identifier for a node.
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub struct NodeId(storm::NodeId);

impl From<storm::NodeId> for NodeId {
    fn from(value: storm::NodeId) -> Self {
        Self(value)
    }
}

impl From<NodeId> for storm::NodeId {
    fn from(value: NodeId) -> Self {
        value.0
    }
}

/// A builder for scene graphs node.
#[wasm_bindgen]
#[derive(Default)]
pub struct NodeBuilder {
    name: Option<String>,
    parent: Option<storm::NodeId>,
    translation: Option<glam::Vec3>,
}

#[wasm_bindgen]
impl NodeBuilder {
    /// Create a new node with default parameters:
    /// - No name;
    /// - No parent node;
    /// - No translation.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(self, name: String) -> Self {
        Self {
            name: Some(name),
            ..self
        }
    }

    pub fn parent(self, parent: NodeId) -> Self {
        Self {
            parent: Some(parent.into()),
            ..self
        }
    }

    pub fn translation(self, translation: Vec3) -> Self {
        Self {
            translation: Some(translation.into()),
            ..self
        }
    }

    /// Build the node.
    pub fn build(self, scene: &mut Scene) -> Result<NodeId, NodeBuilderError> {
        Ok(NodeId(
            storm::NodeBuilder::default()
                .name(self.name)
                .parent(self.parent)
                .translation(self.translation)
                .build(&mut scene.0),
        ))
    }
}

/// Error when {@link NodeBuilder.build()} fails.
#[wasm_bindgen]
#[derive(Debug, Error)]
pub enum NodeBuilderError {
    #[error("failed to found parent node in the scene")]
    InvalidParent,
}
