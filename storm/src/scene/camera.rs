use glam::Mat4;

use crate::storage::{DenseEntry, Id};

use super::Node;

pub struct Camera {
    node: Id<Node>,
    pub name: String,
    pub projection: Projection,
}

impl Camera {
    pub(super) fn new(node: Id<Node>, desc: CameraDescriptor) -> Self {
        let name = desc.name.unwrap_or_else(|| node.to_string());
        Self {
            node,
            name,
            projection: desc.projection,
        }
    }
}

impl DenseEntry for Camera {
    type Key = Node;

    fn id(&self) -> Id<Self::Key> {
        self.node
    }
}

pub struct CameraDescriptor {
    pub name: Option<String>,
    pub projection: Projection,
}

pub enum Projection {
    Orthographic {
        x_mag: f32,
        y_mag: f32,
        z_far: f32,
        z_near: f32,
        zoom: f32,
    },
    Perspective {
        aspect_ratio: Option<f32>,
        y_fov: f32,
        z_far: Option<f32>,
        z_near: f32,
    },
}

impl Projection {
    pub fn matrix(&self, viewport_aspect_ratio: f32) -> Mat4 {
        match self {
            Projection::Orthographic {
                x_mag,
                y_mag,
                z_far,
                z_near,
                zoom,
            } => Mat4::orthographic_rh(
                -x_mag / zoom,
                x_mag / zoom,
                -y_mag / zoom,
                y_mag / zoom,
                *z_near,
                *z_far,
            ),
            Projection::Perspective {
                aspect_ratio,
                y_fov,
                z_far: Some(z_far),
                z_near,
            } => Mat4::perspective_rh(
                *y_fov,
                aspect_ratio.unwrap_or(viewport_aspect_ratio),
                *z_near,
                *z_far,
            ),
            Projection::Perspective {
                aspect_ratio,
                y_fov,
                z_far: None,
                z_near,
            } => Mat4::perspective_infinite_rh(
                *y_fov,
                aspect_ratio.unwrap_or(viewport_aspect_ratio),
                *z_near,
            ),
        }
    }
}
