use glam::{Vec2, Vec3};

use crate::{Context, geometry::Geometry};

/// A box geometry builder.
/// Based on [three.js `BoxGeometry`](https://threejs.org/docs/#BoxGeometry).
pub struct BoxBuilder {
    name: String,
    width: f32,
    height: f32,
    depth: f32,
    width_segments: usize,
    height_segments: usize,
    depth_segments: usize,
}

impl Default for BoxBuilder {
    fn default() -> Self {
        BoxBuilder {
            name: "Box".to_string(),
            width: 1.0,
            height: 1.0,
            depth: 1.0,
            width_segments: 1,
            height_segments: 1,
            depth_segments: 1,
        }
    }
}

impl BoxBuilder {
    /// Name. Default is "Box".
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// The width. That is, the length of the edges parallel to the X axis. Default is `1.0`.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The height. That is, the length of the edges parallel to the Y axis. Default is `1.0`.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// The depth. That is, the length of the edges parallel to the Z axis. Default is `1.0`.
    pub fn depth(mut self, depth: f32) -> Self {
        self.depth = depth;
        self
    }

    /// Number of segmented rectangular faces along the width of the sides. Default is `1.`.
    pub fn width_segments(mut self, segments: usize) -> Self {
        self.width_segments = segments;
        self
    }

    /// Number of segmented rectangular faces along the height of the sides. Default is `1.`.
    pub fn height_segments(mut self, segments: usize) -> Self {
        self.height_segments = segments;
        self
    }

    /// Number of segmented rectangular faces along the depth of the sides. Default is `1.`.
    pub fn depth_segments(mut self, segments: usize) -> Self {
        self.depth_segments = segments;
        self
    }

    /// Creates and returns the cylinder geometry.
    pub fn build(self, ctx: &Context) -> Geometry {
        let half_width = self.width / 2.0;
        let half_height = self.height / 2.0;
        let half_depth = self.depth / 2.0;

        todo!()
    }
}

fn generate_face(
    top_left_position: Vec3,
    top_left_uv: Vec2,
    top_right_position: Vec3,
    top_right_uv: Vec2,
    bottom_left_position: Vec3,
    bottom_left_uv: Vec2,
    horizontal_segments: usize,
    vertical_segments: usize,
    normal: Vec3,
    positions: &mut Vec<Vec3>,
    normals: &mut Vec<Vec3>,
    uvs: &mut Vec<Vec2>,
    indices: &mut Vec<u32>,
) {
    let base_index = positions.len() as u32;
    let delta_x = top_right_position - top_left_position;
    let delta_y = bottom_left_position - top_left_position;
    let delta_u = top_right_uv - top_left_uv;
    let delta_v = bottom_left_uv - top_left_uv;
    for x in 0..=horizontal_segments {
        for y in 0..=vertical_segments {
            let u = x as f32 / horizontal_segments as f32;
            let v = y as f32 / vertical_segments as f32;
            positions.push(top_left_position + u * delta_x + v * delta_y);
            normals.push(normal);
            uvs.push(top_left_uv + u * delta_u + v * delta_v);
        }
    }
    for x in 0..horizontal_segments {
        for y in 0..vertical_segments {
            let a = base_index + vertex_index(x, y, vertical_segments);
            let b = base_index + vertex_index(x, y + 1, vertical_segments);
            let c = base_index + vertex_index(x + 1, y + 1, vertical_segments);
            let d = base_index + vertex_index(x + 1, y, vertical_segments);

            indices.extend_from_slice(&[a, b, c, c, d, a]);
        }
    }
}

fn vertex_index(x: usize, y: usize, y_segments: usize) -> u32 {
    (x * (y_segments + 1) + y) as u32
}
