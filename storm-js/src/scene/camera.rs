#![allow(non_snake_case)]

use serde::Deserialize;
use tsify::Tsify;
use wasm_bindgen::prelude::*;

use crate::scene::{Scene, node::NodeId};

/// A unique id for a camera. A camera will always have the same id.
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub struct CameraId(pub(crate) storm::camera::CameraId);

#[wasm_bindgen]
#[derive(Default)]
pub struct CameraBuilder {
    node: Option<NodeId>,
    camera_type: Option<CameraType>,
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
            camera_type: Some(CameraType::Orthographic(orthographic)),
            ..self
        }
    }

    /// Defines a perspective camera. One and only one of {@link CameraBuilder.orthographic()}
    /// or {@link CameraBuilder.perspective()} is required.
    pub fn perspective(self, perspective: PerspectiveCamera) -> Self {
        Self {
            camera_type: Some(CameraType::Perspective(perspective)),
            ..self
        }
    }

    /// Build the camera or throws a {@link CameraBuilderError}.
    pub fn build(self, scene: &mut Scene) -> CameraId {
        let mut builder = storm::camera::CameraBuilder::default();
        if let Some(node) = self.node {
            builder = builder.node(node)
        }
        match self.camera_type {
            Some(CameraType::Perspective(PerspectiveCamera {
                aspectRatio,
                yfov,
                zfar,
                znear,
            })) => {
                builder = builder.perspective_projection(storm::camera::PerspectiveProjection {
                    aspect_ratio: aspectRatio,
                    y_fov: yfov,
                    z_near: znear,
                    z_far: zfar,
                });
            }
            Some(CameraType::Orthographic(OrthographicCamera {
                xmag,
                ymag,
                zfar,
                znear,
            })) => {
                builder = builder.orthographic_projection(storm::camera::OrthographicProjection {
                    x_mag: xmag,
                    y_mag: ymag,
                    z_far: zfar,
                    z_near: znear,
                });
            }
            None => (),
        }

        CameraId(builder.build(&mut scene.0))
    }
}

enum CameraType {
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
