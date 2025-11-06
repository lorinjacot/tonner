#![allow(non_snake_case)]

use serde::Deserialize;
use thiserror::Error;
use tsify::Tsify;
use wasm_bindgen::prelude::*;

use crate::scene::{Scene, node::NodeId};

/// A unique id for a camera. A camera will always have the same id.
#[wasm_bindgen]
pub struct CameraId;

#[wasm_bindgen]
#[derive(Default)]
pub struct CameraBuilder {
    node: Option<NodeId>,
    camera_type: CameraType,
}

#[wasm_bindgen]
impl CameraBuilder {
    /// Create a builder with default parameters.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attaches the camera to a node. Required.
    pub fn node(self, node: NodeId) -> Self {
        Self {
            node: Some(node),
            ..self
        }
    }

    /// Defines an orthographic camera. One and only one of {@link CameraBuilder.orthographic()}
    /// or {@link CameraBuilder.perspective()} is required.
    pub fn orthographic(self, orthographic: OrthographicCamera) -> Self {
        Self {
            camera_type: CameraType::Orthographic(orthographic),
            ..self
        }
    }

    /// Defines a perspective camera. One and only one of {@link CameraBuilder.orthographic()}
    /// or {@link CameraBuilder.perspective()} is required.
    pub fn perspective(self, perspective: PerspectiveCamera) -> Self {
        Self {
            camera_type: CameraType::Perspective(perspective),
            ..self
        }
    }

    /// Build the camera or throws a {@link CameraBuilderError}.
    pub fn build(self, _scene: &mut Scene) -> Result<CameraId, CameraBuilderError> {
        todo!()
    }
}

#[derive(Default)]
enum CameraType {
    #[default]
    None,
    Orthographic(OrthographicCamera),
    Perspective(PerspectiveCamera),
}

/// Orthographic camera properties.
#[derive(Debug, Deserialize, Tsify)]
#[tsify(from_wasm_abi)]
pub struct OrthographicCamera {
    pub xmag: f32,
    pub ymag: f32,
    pub zfar: f32,
    pub znear: f32,
}

/// Perspective camera properties.
#[derive(Debug, Deserialize, Tsify)]
#[tsify(from_wasm_abi)]
pub struct PerspectiveCamera {
    pub aspectRatio: Option<f32>,
    pub yfov: f32,
    pub zfar: Option<f32>,
    pub znear: f32,
}

/// Error when {@link CameraBuilder.build()} fails.
#[wasm_bindgen]
#[derive(Debug, Error)]
pub enum CameraBuilderError {
    #[error("node is not set")]
    NodeNotSet,
    #[error("invalid node")]
    InvalidNode,
    #[error("camera type is not set")]
    CameraTypeNodSet,
}
