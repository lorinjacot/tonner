use std::f32::consts::PI;

use glam::{vec2, vec3};
use thiserror::Error;

use crate::{
    Engine,
    asset::geometry::{Geometry, GeometryBuilder},
};

/// A sphere geometry builder.
/// Based on [three.js' `SphereGeometry`](https://threejs.org/docs/#api/en/geometries/SphereGeometry).
///
/// The geometry is created by sweeping and calculating vertexes around the Y axis (horizontal sweep)
/// and the Z axis (vertical sweep). Thus, incomplete spheres (akin to 'sphere slices') can be created
/// through the use of different values of `phi_start`, `phi_length`, `theta_start` and `theta_length`,
/// in order to define the points in which we start (or end) calculating those vertices.
#[must_use]
pub struct SphereBuilder {
    name: String,
    radius: f32,
    width_segments: usize,
    height_segments: usize,
    phi_start: f32,
    phi_length: f32,
    theta_start: f32,
    theta_length: f32,
}

impl SphereBuilder {
    /// Name. Default is "Sphere".
    pub fn name(self, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..self
        }
    }

    /// Sphere radius. Default is `1.0`.
    pub fn radius(self, radius: impl Into<f32>) -> Self {
        Self {
            radius: radius.into(),
            ..self
        }
    }

    /// Number of horizontal segments. Minimum value is `3`, and the default is `32`.
    pub fn width_segments(
        self,
        width_segments: impl Into<usize>,
    ) -> Result<Self, NotEnoughSegmentsError> {
        let width_segments = width_segments.into();
        if width_segments < 3 {
            return Err(NotEnoughSegmentsError {
                min: 3,
                actual: width_segments,
            });
        }
        Ok(Self {
            width_segments,
            ..self
        })
    }

    /// Number of vertical segments. Minimum value is `2`, and the default is `16`.
    pub fn height_segments(
        self,
        height_segments: impl Into<usize>,
    ) -> Result<Self, NotEnoughSegmentsError> {
        let height_segments = height_segments.into();
        if height_segments < 2 {
            return Err(NotEnoughSegmentsError {
                min: 2,
                actual: height_segments,
            });
        }
        Ok(Self {
            height_segments,
            ..self
        })
    }

    /// Specify horizontal starting angle. Default is `0.0`.
    pub fn phi_start(self, phi_start: impl Into<f32>) -> Self {
        Self {
            phi_start: phi_start.into(),
            ..self
        }
    }

    /// Specify horizontal sweep angle size. Default is `2.0 * PI`.
    pub fn phi_length(self, phi_length: impl Into<f32>) -> Self {
        Self {
            phi_length: phi_length.into(),
            ..self
        }
    }

    /// Specify vertical starting angle. Default is `0.0`.
    pub fn theta_start(self, theta_start: impl Into<f32>) -> Self {
        Self {
            theta_start: theta_start.into(),
            ..self
        }
    }

    /// Specify vertical sweep angle size. Default is `PI`.
    pub fn theta_length(self, theta_length: impl Into<f32>) -> Self {
        Self {
            theta_length: theta_length.into(),
            ..self
        }
    }

    pub fn build(self, engine: &mut Engine) -> Geometry {
        let theta_end = (self.theta_start + self.theta_length).min(PI);

        let row_count = self.height_segments + 1;
        let col_count = self.width_segments + 1;
        let mut index = 0u32;
        let mut grid = Vec::with_capacity(row_count);

        let vertex_count = (self.height_segments + 1) * (self.width_segments + 1);
        let mut positions = Vec::with_capacity(vertex_count);
        let mut normals = Vec::with_capacity(vertex_count);
        let mut uvs = Vec::with_capacity(vertex_count);

        for y in 0..=self.height_segments {
            let mut vertices_row = Vec::with_capacity(col_count);
            let v = y as f32 / self.height_segments as f32;

            // special case for the poles
            let u_offset = if y == 0 && self.theta_start == 0.0 {
                0.5 / self.width_segments as f32
            } else if y == self.height_segments && theta_end == PI {
                -0.5 / self.width_segments as f32
            } else {
                0.0
            };

            for x in 0..=self.width_segments {
                let u = x as f32 / self.width_segments as f32;

                let phi = self.phi_start + u * self.phi_length;
                let theta = self.theta_start + v * self.theta_length;

                let vertex = vec3(
                    -self.radius * phi.cos() * theta.sin(),
                    self.radius * theta.cos(),
                    self.radius * phi.sin() * theta.sin(),
                );

                positions.push(vertex);
                normals.push(vertex.normalize());
                uvs.push(vec2(u + u_offset, 1.0 - v));

                vertices_row.push(index);
                index += 1;
            }

            grid.push(vertices_row);
        }

        let mut index_count = (self.height_segments - 1) * self.width_segments * 6;
        if self.theta_start > 0.0 {
            index_count += self.width_segments * 3;
        }
        if theta_end < PI {
            index_count += self.width_segments * 3;
        }
        let mut indices = Vec::with_capacity(index_count);

        for y in 0..self.height_segments {
            for x in 0..self.width_segments {
                let a = grid[y][x + 1];
                let b = grid[y][x];
                let c = grid[y + 1][x];
                let d = grid[y + 1][x + 1];

                if y != 0 || self.theta_start > 0.0 {
                    indices.extend_from_slice(&[a, b, d]);
                }
                if y != self.height_segments - 1 || theta_end < PI {
                    indices.extend_from_slice(&[b, c, d]);
                }
            }
        }

        GeometryBuilder::new(vertex_count, 0)
            .name(self.name)
            .indices_u32(indices)
            .positions(positions)
            .unwrap()
            .normals(normals)
            .unwrap()
            .tex_coords_0(uvs)
            .unwrap()
            .build(engine)
            .unwrap()
    }
}

/// Error when [`SphereBuilder.width_segments`] or [`SphereBuilder.height_segments`] fail.
#[derive(Debug, Error)]
#[error("need {min} segments, got only {actual}")]
pub struct NotEnoughSegmentsError {
    pub min: usize,
    pub actual: usize,
}

impl Default for SphereBuilder {
    fn default() -> Self {
        Self {
            name: String::from("Sphere"),
            radius: 1.0,
            width_segments: 32,
            height_segments: 16,
            phi_start: 0.0,
            phi_length: 2.0 * PI,
            theta_start: 0.0,
            theta_length: PI,
        }
    }
}
