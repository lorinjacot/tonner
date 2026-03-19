use std::f32::consts::PI;

use crate::{
    Context,
    geometry::{CylinderBuilder, Geometry},
};

/// A cone geometry builder.
/// Based on [three.js `ConeGeometry`](https://threejs.org/docs/#ConeGeometry).
#[must_use]
pub struct ConeBuilder {
    name: String,
    radius: f32,
    height: f32,
    radial_segments: usize,
    height_segments: usize,
    open_ended: bool,
    theta_start: f32,
    theta_length: f32,
}

impl Default for ConeBuilder {
    fn default() -> Self {
        ConeBuilder {
            name: "Cone".to_string(),
            radius: 1.0,
            height: 1.0,
            radial_segments: 32,
            height_segments: 1,
            open_ended: false,
            theta_start: 0.0,
            theta_length: 2.0 * PI,
        }
    }
}

impl ConeBuilder {
    /// Name. Default is "Cone".
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Radius of the cone base. Default is `1.0`.
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Height of the cone. Default is `1.0`.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Number of segmented faces around the circumference of the cone. Default is `32`.
    pub fn radial_segments(mut self, segments: usize) -> Self {
        self.radial_segments = segments;
        self
    }

    /// Number of rows of faces along the height of the cone. Default is `1`.
    pub fn height_segments(mut self, segments: usize) -> Self {
        self.height_segments = segments;
        self
    }

    /// Whether the base of the cone is open or capped. Default is `false`.
    pub fn open_ended(mut self, open_ended: bool) -> Self {
        self.open_ended = open_ended;
        self
    }

    /// Start angle for first segment, in radians. Default is `0.0`.
    pub fn theta_start(mut self, theta_start: f32) -> Self {
        self.theta_start = theta_start;
        self
    }

    /// The central angle, often called theta, of the circular sector, in radians.
    /// The default value results in a complete cone. Default is `2.0 * PI`.
    pub fn theta_length(mut self, theta_length: f32) -> Self {
        self.theta_length = theta_length;
        self
    }

    pub fn build(self, ctx: &Context) -> Geometry {
        CylinderBuilder::default()
            .name(self.name)
            .radius_top(0.0)
            .radius_bottom(self.radius)
            .radial_segments(self.radial_segments)
            .height_segments(self.height_segments)
            .open_ended(self.open_ended)
            .theta_start(self.theta_start)
            .theta_length(self.theta_length)
            .build(ctx)
    }
}
