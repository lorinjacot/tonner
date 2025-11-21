use std::{collections::HashMap, fmt::Display};

use glam::Mat4;
use thiserror::Error;
use uuid::Uuid;

use crate::{Scene, scene::node::NodeId};

/// A unique id for a camera. A camera can only have one id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CameraId(Uuid);

impl Display for CameraId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CameraId({})", self.0)
    }
}

/// A builder for camera.
#[must_use]
#[derive(Default)]
pub struct CameraBuilder {
    name: Option<String>,
    node: Option<NodeId>,
    projection: Option<Projection>,
}

impl CameraBuilder {
    /// Gives a name to the camera. The name is only used for UI and debugging.
    pub fn name(self, name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..self
        }
    }

    /// Attaches the camera to the node. The node global transform
    /// will be used as the view matrix.
    ///
    /// If this function is never called, the camera will be attached to
    /// a new root node with identity transform.
    pub fn node(self, node: impl Into<NodeId>) -> Self {
        Self {
            node: Some(node.into()),
            ..self
        }
    }

    /// Set the projection matrix.
    pub fn orthographic_projection(self, projection: impl Into<OrthographicProjection>) -> Self {
        Self {
            projection: Some(Projection::Orthographic(projection.into())),
            ..self
        }
    }

    /// Set the projection matrix.
    pub fn perspective_projection(self, projection: impl Into<PerspectiveProjection>) -> Self {
        Self {
            projection: Some(Projection::Perspective(projection.into())),
            ..self
        }
    }

    /// Build the camera.
    pub fn build(self, _scene: &mut Scene) -> CameraId {
        todo!()
    }
}

#[derive(Debug, Error)]
pub enum NewCameraError {
    #[error("invalid node: {0}")]
    InvalidNode(NodeId),
}

/// In this projection mode, an object's size in the rendered image stays constant regardless of its distance from
/// the camera. This can be useful for rendering 2D scenes and UI elements, amongst other things.
pub struct OrthographicProjection {
    /// The floating-point horizontal magnification of the view. Default to `2.0`.
    pub x_mag: f32,

    /// The floating-point vertical magnification of the view. Default to `2.0`.
    pub y_mag: f32,

    /// The floating-point distance to the far clipping plane. Default to `2000.0`.
    pub z_far: f32,

    /// The floating-point distance to the near clipping plane. Default to `0.1`.
    pub z_near: f32,
}

impl Default for OrthographicProjection {
    fn default() -> Self {
        Self {
            x_mag: 2.0,
            y_mag: 2.0,
            z_far: 2000.0,
            z_near: 0.1,
        }
    }
}

/// This projection mode is designed to mimic the way the human eye sees. It is the most common projection
/// mode used for rendering a 3D scene.
pub struct PerspectiveProjection {
    /// The aspect ratio. If not provided, the target width / height will be used. Default is `None`.
    pub aspect_ratio: Option<f32>,

    /// The vertical field of view, from bottom to top of view, in radian. Default is `50°`.
    pub y_fov: f32,

    /// The camera's near plane. The valid range is greater than `0.0` and less than the
    /// current value of [`PerspectiveProjection::z_far`].
    ///
    /// Note that, unlike for the [OrthographicProjection], `0.0` is not a valid value for a
    /// perspective camera's near plane.
    ///
    /// Default is `0.1`.
    pub z_near: f32,

    /// The camera's far plane. Must be greater than the current value of [`PerspectiveProjection::z_near`].
    /// If `None`, an infinite perspective projection is used.
    ///
    /// Default is `None`.
    pub z_far: Option<f32>,
}

impl Default for PerspectiveProjection {
    fn default() -> Self {
        Self {
            aspect_ratio: None,
            y_fov: 50.0f32.to_radians(),
            z_far: None,
            z_near: 0.1,
        }
    }
}

struct CameraData {
    id: CameraId,
    name: String,
    node: NodeId,
    projection: Projection,
}

pub(super) struct CameraManager {
    cameras: HashMap<CameraId, CameraData>,
}

impl CameraManager {
    pub(super) fn new() -> Self {
        Self {
            cameras: HashMap::new(),
        }
    }

    /// Returns the node associated with the camera. The global transform of that node
    /// should be used as the camera view matrix. `None` if not camera is associated with the id.
    pub(super) fn node(&self, id: CameraId) -> Option<NodeId> {
        self.cameras.get(&id).map(|data| data.node)
    }

    /// Returns the projection matrix of the camera. `viewport_aspect_ration` should be the width over the height
    /// of the render target. Returns `None` if no camera is associated with the id.
    pub(super) fn projection_matrix(
        &self,
        id: CameraId,
        viewport_aspect_ratio: f32,
    ) -> Option<Mat4> {
        self.cameras
            .get(&id)
            .map(|data| data.projection.matrix(viewport_aspect_ratio))
    }
}

enum Projection {
    Orthographic(OrthographicProjection),
    Perspective(PerspectiveProjection),
}

impl Projection {
    fn matrix(&self, viewport_aspect_ratio: f32) -> Mat4 {
        match self {
            Projection::Orthographic(OrthographicProjection {
                x_mag,
                y_mag,
                z_far,
                z_near,
            }) => Mat4::orthographic_rh(-*x_mag, *x_mag, -*y_mag, *y_mag, *z_near, *z_far),
            Projection::Perspective(PerspectiveProjection {
                aspect_ratio,
                y_fov,
                z_near,
                z_far: Some(z_far),
            }) => Mat4::perspective_rh(
                *y_fov,
                aspect_ratio.unwrap_or(viewport_aspect_ratio),
                *z_near,
                *z_far,
            ),
            Projection::Perspective(PerspectiveProjection {
                aspect_ratio,
                y_fov,
                z_near,
                z_far: None,
            }) => Mat4::perspective_infinite_rh(
                *y_fov,
                aspect_ratio.unwrap_or(viewport_aspect_ratio),
                *z_near,
            ),
        }
    }
}
