use glam::{Mat4, Quat, Vec3};
use thiserror::Error;

use crate::{ecs::EntityId, scene_graph::SceneGraph};

/// A builder for camera.
#[must_use]
pub struct CameraBuilder {
    name: String,
    entity: EntityId,
    projection: Option<Projection>,
}

impl CameraBuilder {
    /// Creates a new camera build for the given entity.
    ///
    /// Attaches the camera to the entity node. The node global transform
    /// will be used as the view matrix. A node will be created if the entity
    /// does not have any node.
    pub fn new(entity: EntityId) -> Self {
        CameraBuilder {
            name: String::new(),
            entity,
            projection: None,
        }
    }

    /// Gives a name to the camera. The name is only used for UI and debugging.
    pub fn name(self, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
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
    pub fn build(self, scene_graph: &mut SceneGraph) -> Camera {
        if !scene_graph.contains(self.entity) {
            scene_graph.add(self.entity, None);
        }

        let projection = self
            .projection
            .unwrap_or_else(|| Projection::Perspective(PerspectiveProjection::default()));

        Camera {
            name: self.name,
            entity: self.entity,
            projection,
        }
    }
}

#[derive(Debug, Error)]
pub enum NewCameraError {
    #[error("invalid node: {0}")]
    InvalidNode(EntityId),
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

/// A camera is used to render a scene.
pub struct Camera {
    /// Name of the camera. Does not need to be unique. Used for GUI and debugging.
    pub name: String,
    /// The location of the camera. The camera will also move with this node.
    pub entity: EntityId,
    projection: Projection,
}

impl Camera {
    /// Create a new Camera. By default, the name is an empty string and uses
    /// the default [PerspectiveProjection].
    pub fn new(entity: EntityId) -> Self {
        let projection = Projection::Perspective(PerspectiveProjection::default());

        Camera {
            name: String::new(),
            entity,
            projection,
        }
    }

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
    ///
    /// ## Panics
    ///
    /// Panics if the entity does not have a scene graph node.
    pub fn unproject(
        &self,
        position: Vec3,
        viewport_aspect_ratio: f32,
        scene_graph: &mut SceneGraph,
    ) -> Vec3 {
        let view = scene_graph[self.entity].global_transformation();
        let projection = self.projection.matrix(viewport_aspect_ratio);
        (projection.inverse() * view).transform_point3(position)
    }

    pub fn projection_matrix(&self, viewport_aspect_ratio: f32) -> Mat4 {
        self.projection.matrix(viewport_aspect_ratio)
    }

    /// Modifies the local transformation of the camera entity such that its local z-axis is
    /// pointing toward `target`.
    ///
    /// ## Panics
    ///
    /// Panics if the entity does not have a scene graph node.
    pub fn look_at(&self, target: Vec3, scene_graph: &mut SceneGraph) {
        let node = &scene_graph[self.entity];
        let eye = node.global_transformation().transform_point3(Vec3::ZERO);
        let mut rotation = Quat::look_at_rh(eye, target, Vec3::Y).inverse();
        if let Some(parent) = node.parent() {
            let parent_rotation = scene_graph[parent]
                .global_transformation()
                .to_scale_rotation_translation()
                .1;
            rotation = parent_rotation.inverse() * rotation;
        }
        scene_graph.set_local_transformation(self.entity, None, rotation, None);
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
