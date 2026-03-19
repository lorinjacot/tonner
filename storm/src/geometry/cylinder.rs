use std::f32::consts::PI;

use glam::{Vec2, Vec3, vec2, vec3};

use crate::{
    Context,
    geometry::{Geometry, GeometryBuilder},
};

/// A cylinder geometry builder.
/// Based on [three.js `ConeGeometry`](https://threejs.org/docs/#CylinderGeometry).
#[must_use]
pub struct CylinderBuilder {
    name: String,
    radius_top: f32,
    radius_bottom: f32,
    height: f32,
    radial_segments: usize,
    height_segments: usize,
    open_ended: bool,
    theta_start: f32,
    theta_length: f32,
}

impl Default for CylinderBuilder {
    fn default() -> Self {
        CylinderBuilder {
            name: "Cylinder".to_string(),
            radius_top: 1.0,
            radius_bottom: 1.0,
            height: 1.0,
            radial_segments: 32,
            height_segments: 1,
            open_ended: false,
            theta_start: 0.0,
            theta_length: 2.0 * PI,
        }
    }
}

impl CylinderBuilder {
    /// Name. Default is "Cylinder".
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Radius of the cylinder at the top. Default is `1.0`.
    pub fn radius_top(mut self, radius: f32) -> Self {
        self.radius_top = radius;
        self
    }

    /// Radius of the cylinder at the bottom. Default is `1.0`.
    pub fn radius_bottom(mut self, radius: f32) -> Self {
        self.radius_bottom = radius;
        self
    }

    /// Height of the cylinder. Default is `1.0`.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Number of segmented faces around the circumference of the cylinder. Default is `32`.
    pub fn radial_segments(mut self, segments: usize) -> Self {
        self.radial_segments = segments;
        self
    }

    /// Number of rows of faces along the height of the cylinder. Default is `1`.
    pub fn height_segments(mut self, segments: usize) -> Self {
        self.height_segments = segments;
        self
    }

    /// Whether the base of the cylinder is open or capped. Default is `false`.
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
    /// The default value results in a complete cylinder. Default is `2.0 * PI`.
    pub fn theta_length(mut self, theta_length: f32) -> Self {
        self.theta_length = theta_length;
        self
    }

    /// Creates and returns the cylinder geometry.
    pub fn build(self, ctx: &Context) -> Geometry {
        let mut vertex_count = self.torso_vertex_count();
        let mut index_count = self.torso_index_count();
        if !self.open_ended {
            if self.radius_top > 0.0 {
                vertex_count += self.cap_vertex_count();
                index_count += self.cap_index_count();
            }
            if self.radius_bottom > 0.0 {
                vertex_count += self.cap_vertex_count();
                index_count += self.cap_index_count();
            }
        }

        let mut positions = Vec::with_capacity(vertex_count);
        let mut normals = Vec::with_capacity(vertex_count);
        let mut uvs = Vec::with_capacity(vertex_count);
        let mut indices = Vec::with_capacity(index_count);

        self.generate_torso(&mut positions, &mut normals, &mut uvs, &mut indices);

        if !self.open_ended {
            if self.radius_top > 0.0 {
                self.generate_cap(&mut positions, &mut normals, &mut uvs, &mut indices, true);
            }
            if self.radius_bottom > 0.0 {
                self.generate_cap(&mut positions, &mut normals, &mut uvs, &mut indices, false);
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
            .build(ctx)
            .unwrap()
    }

    fn torso_vertex_count(&self) -> usize {
        (self.height_segments + 1) * (self.radial_segments + 1)
    }

    fn torso_vertex_index(&self, y: usize, x: usize) -> u32 {
        (y * (self.radial_segments + 1) + x) as u32
    }

    fn generate_torso(
        &self,
        positions: &mut Vec<Vec3>,
        normals: &mut Vec<Vec3>,
        uvs: &mut Vec<Vec2>,
        indices: &mut Vec<u32>,
    ) {
        let index_start = positions.len() as u32;
        self.generate_torse_vertices(positions, normals, uvs);
        self.generate_torso_indices(indices, index_start);
    }

    fn generate_torse_vertices(
        &self,
        positions: &mut Vec<Vec3>,
        normals: &mut Vec<Vec3>,
        uvs: &mut Vec<Vec2>,
    ) {
        let half_height = self.height / 2.0;
        let slope = (self.radius_bottom - self.radius_top) / self.height;

        for y in 0..=self.height_segments {
            let v = y as f32 / self.height_segments as f32;
            let radius = v * (self.radius_bottom - self.radius_top) + self.radius_top;

            for x in 0..=self.radial_segments {
                let u = x as f32 / self.radial_segments as f32;

                let theta = u * self.theta_length + self.theta_start;
                let (sin_theta, cos_theta) = theta.sin_cos();

                positions.push(vec3(
                    radius * sin_theta,
                    -v * self.height + half_height,
                    radius * cos_theta,
                ));

                normals.push(vec3(sin_theta, slope, cos_theta).normalize());

                uvs.push(vec2(u, 1.0 - v));
            }
        }
    }

    fn torso_index_count(&self) -> usize {
        // overcounts if `radius_top` or `radius_bottom` are `0.0`
        3 * self.height_segments * self.radial_segments
    }

    fn generate_torso_indices(&self, indices: &mut Vec<u32>, index_start: u32) {
        for x in 0..self.radial_segments {
            for y in 0..self.height_segments {
                let a = index_start + self.torso_vertex_index(y, x);
                let b = index_start + self.torso_vertex_index(y + 1, x);
                let c = index_start + self.torso_vertex_index(y + 1, x + 1);
                let d = index_start + self.torso_vertex_index(y, x + 1);

                if self.radius_top > 0.0 || y != 0 {
                    indices.extend_from_slice(&[a, b, d]);
                }
                if self.radius_bottom > 0.0 || y != self.height_segments - 1 {
                    indices.extend_from_slice(&[b, c, d]);
                }
            }
        }
    }

    fn cap_vertex_count(&self) -> usize {
        self.radial_segments + 2
    }

    fn cap_index_count(&self) -> usize {
        3 * (self.radial_segments + 1)
    }

    fn generate_cap(
        &self,
        positions: &mut Vec<Vec3>,
        normals: &mut Vec<Vec3>,
        uvs: &mut Vec<Vec2>,
        indices: &mut Vec<u32>,
        top: bool,
    ) {
        let center_index = positions.len() as u32;
        self.generate_cap_vertices(positions, normals, uvs, top);

        if top {
            for x in 1..=self.radial_segments {
                let i = center_index + x as u32;
                indices.extend_from_slice(&[i, i + 1, center_index]);
            }
        } else {
            for x in 1..=self.radial_segments {
                let i = center_index + x as u32;
                indices.extend_from_slice(&[i + 1, i, center_index]);
            }
        }
    }

    fn generate_cap_vertices(
        &self,
        positions: &mut Vec<Vec3>,
        normals: &mut Vec<Vec3>,
        uvs: &mut Vec<Vec2>,
        top: bool,
    ) {
        let half_height = self.height / 2.0;

        let (radius, sign) = if top {
            (self.radius_top, 1.0)
        } else {
            (self.radius_bottom, -1.0)
        };

        // first we generate the center vertex data of the cap.
        positions.push(vec3(0.0, half_height * sign, 0.0));
        normals.push(vec3(0.0, sign, 0.0));
        uvs.push(vec2(0.5, 0.5));

        // now we generate the surrounding vertices, normals and uvs
        for x in 0..=self.radial_segments {
            let u = x as f32 / self.radial_segments as f32;
            let theta = u * self.theta_length + self.theta_start;

            let (sin_theta, cos_theta) = theta.sin_cos();

            positions.push(vec3(
                radius * sin_theta,
                half_height * sign,
                radius * cos_theta,
            ));

            normals.push(vec3(0.0, sign, 0.0));

            uvs.push(vec2(cos_theta * 0.5 + 0.5, sin_theta * 0.5 * sign + 0.5));
        }
    }
}
