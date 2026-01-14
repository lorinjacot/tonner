use glam::{Mat4, Vec3};
use thiserror::Error;

use crate::{
    node::NodeBuilder,
    scene::{Scene, node::NodeId},
};

/// A builder for camera.
#[must_use]
#[derive(Default)]
pub struct CameraBuilder {
    name: String,
    node: Option<NodeId>,
    projection: Option<Projection>,
}

impl CameraBuilder {
    /// Gives a name to the camera. The name is only used for UI and debugging.
    pub fn name(self, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
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
    pub fn build(self, scene: &mut Scene) -> Camera {
        let node = self.node.unwrap_or_else(|| {
            NodeBuilder::default()
                .name(&self.name)
                .build(scene)
                .unwrap()
        });

        let projection = self
            .projection
            .unwrap_or_else(|| Projection::Perspective(PerspectiveProjection::default()));

        Camera {
            name: self.name,
            node,
            projection,
        }
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

    pub zoom: f32,
}

impl Default for OrthographicProjection {
    fn default() -> Self {
        Self {
            x_mag: 2.0,
            y_mag: 2.0,
            z_far: 2000.0,
            z_near: 0.1,
            zoom: 1.0,
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

    pub zoom: f32,
}

impl Default for PerspectiveProjection {
    fn default() -> Self {
        Self {
            aspect_ratio: None,
            y_fov: 50.0f32.to_radians(),
            z_far: None,
            z_near: 0.1,
            zoom: 1.0,
        }
    }
}

/// A camera is used to render a [Scene].
pub struct Camera {
    /// Name of the camera. Does not need to be unique. Used for GUI and debugging.
    pub name: String,
    /// The location of the camera. The camera will also move with this node.
    pub node: NodeId,
    projection: Projection,
}

impl Camera {
    /// - If self is an orthographic camera, returns the orthographic projection data;
    /// - Returns `None` otherwise.
    pub fn orthographic_projection(&self) -> Option<&OrthographicProjection> {
        match &self.projection {
            Projection::Orthographic(projection) => Some(projection),
            _ => None,
        }
    }

    pub fn is_orthographic(&self) -> bool {
        self.orthographic_projection().is_some()
    }

    /// - If self is an perspective camera, returns the perspective projection data;
    /// - Returns `None` otherwise.
    pub fn perspective_projection(&self) -> Option<&PerspectiveProjection> {
        match &self.projection {
            Projection::Perspective(projection) => Some(projection),
            _ => None,
        }
    }

    pub fn is_perspective(&self) -> bool {
        self.perspective_projection().is_some()
    }

    /// Transform a position from the camera's normalized device coordinate space into world space.
    pub fn unproject(&self, position: Vec3, viewport_aspect_ratio: f32, scene: &Scene) -> Vec3 {
        let view = scene.node_manager.global_matrix(self.node).unwrap();
        let projection = self.projection.matrix(viewport_aspect_ratio);
        (projection.inverse() * view).transform_point3(position)
    }

    /// Returns the current zoom of the camera, if available. The following camera have zooming capabilities:
    /// - Orthographic
    /// - Perspective
    pub fn zoom(&self) -> Option<f32> {
        match self.projection {
            Projection::Orthographic(OrthographicProjection { zoom, .. }) => Some(zoom),
            Projection::Perspective(PerspectiveProjection { zoom, .. }) => Some(zoom),
        }
    }

    /// Modifies the current zoom of the camera, if available. The following camera have zooming capabilities:
    /// - Orthographic
    /// - Perspective
    pub fn set_zoom(&mut self, zoom: f32) -> Result<(), ()> {
        match &mut self.projection {
            Projection::Orthographic(OrthographicProjection { zoom: mut_zoom, .. }) => {
                *mut_zoom = zoom;
                Ok(())
            }
            Projection::Perspective(PerspectiveProjection { zoom: mut_zoom, .. }) => {
                *mut_zoom = zoom;
                Ok(())
            }
            _ => Err(()),
        }
    }

    pub fn projection_matrix(&self, viewport_aspect_ratio: f32) -> Mat4 {
        self.projection.matrix(viewport_aspect_ratio)
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
                zoom,
            }) => Mat4::orthographic_rh(-*x_mag, *x_mag, -*y_mag, *y_mag, *z_near, *z_far),
            Projection::Perspective(PerspectiveProjection {
                aspect_ratio,
                y_fov,
                z_near,
                z_far: Some(z_far),
                zoom,
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
                zoom,
            }) => Mat4::perspective_infinite_rh(
                *y_fov,
                aspect_ratio.unwrap_or(viewport_aspect_ratio),
                *z_near,
            ),
        }
    }
}
