use std::f32::consts::FRAC_PI_2;

use glam::{Quat, Vec3};

use crate::{
    Context,
    geometry::{ConeBuilder, CylinderBuilder, Geometry},
};

pub struct ArrowParts {
    pub head: Geometry,
    pub body: Geometry,
}

/// A geometry builder for an arrow shape, consisting of a cylindrical body and a conical head. The arrow is oriented along the negative z-axis, with its base at the origin.
///
/// The arrow is defined by the following parameters:
/// - `length`: Total length of the arrow (head + body). Default is `1.0`.
/// - `head_length`: Length of the arrow head. Default is `0.2`.
/// - `head_radius`: Radius of the base of the arrow head cone. Default is `0.1`.
/// - `body_radius`: Radius of the arrow body cylinder. Default is `0.05`.
/// - `radial_segments`: Number of segmented faces around the circumference of the arrow. Default is `32`.
#[must_use]
pub struct ArrowBuilder {
    name: String,
    length: f32,
    head_length: f32,
    head_radius: f32,
    body_radius: f32,
    radial_segments: usize,
    translation: Vec3,
    rotation: Quat,
}

impl Default for ArrowBuilder {
    fn default() -> Self {
        ArrowBuilder {
            name: "Arrow".to_string(),
            length: 1.0,
            head_length: 0.2,
            head_radius: 0.1,
            body_radius: 0.05,
            radial_segments: 32,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        }
    }
}

impl ArrowBuilder {
    /// Name. Default is "Arrow".
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Total length of the arrow (head + body). Default is `1.0`.
    pub fn length(mut self, length: f32) -> Self {
        self.length = length;
        self
    }

    /// Length of the arrow head. Default is `0.2`.
    pub fn head_length(mut self, head_length: f32) -> Self {
        self.head_length = head_length;
        self
    }

    /// Radius of the base of the arrow head cone. Default is `0.1`.
    pub fn head_radius(mut self, head_radius: f32) -> Self {
        self.head_radius = head_radius;
        self
    }

    /// Radius of the arrow body cylinder. Default is `0.05`.
    pub fn body_radius(mut self, body_radius: f32) -> Self {
        self.body_radius = body_radius;
        self
    }

    /// Number of segmented faces around the circumference of the arrow. Default is `32`.
    pub fn radial_segments(mut self, segments: usize) -> Self {
        self.radial_segments = segments;
        self
    }

    /// Translation of the arrow. Default is no translation (origin).
    ///
    /// By default, the base of the arrow is at the origin. Setting a translation will move the entire arrow by the specified vector.
    pub fn translate(mut self, translation: Vec3) -> Self {
        self.translation = translation;
        self
    }

    /// Rotation of the arrow. Default is no rotation (identity).
    ///
    /// By default, the arrow is oriented along the negative z-axis. Setting a rotation will rotate the entire arrow by the specified quaternion.
    pub fn rotate(mut self, rotation: Quat) -> Self {
        self.rotation = rotation;
        self
    }

    /// Builds the arrow geometry. The arrow is oriented along the negative z-axis, with its base at the origin.
    pub fn build(self, ctx: &Context) -> ArrowParts {
        let base_length = self.length - self.head_length;

        let body = CylinderBuilder::default()
            .name(format!("{} Body", self.name))
            .radius_top(self.body_radius)
            .radius_bottom(self.body_radius)
            .height(base_length)
            .radial_segments(self.radial_segments)
            .translate(Vec3::Y * base_length / 2.0 + self.translation)
            .rotate(self.rotation * Quat::from_rotation_x(-FRAC_PI_2))
            .build(ctx);

        let head = ConeBuilder::default()
            .name(format!("{} Head", self.name))
            .radius(self.head_radius)
            .height(self.head_length)
            .radial_segments(self.radial_segments)
            .translate(Vec3::Y * (base_length + self.head_length / 2.0) + self.translation)
            .rotate(self.rotation * Quat::from_rotation_x(-FRAC_PI_2))
            .build(ctx);

        ArrowParts { head, body }
    }
}
