use glam::{Vec3, vec2, vec3};

use crate::{
    Context,
    geometry::{Geometry, GeometryBuilder},
};

/// A box geometry builder.
/// Based on [three.js `BoxGeometry`](https://threejs.org/docs/#BoxGeometry).
#[derive(Debug, Clone)]
pub struct BoxBuilder {
    name: String,
    width: f32,
    height: f32,
    depth: f32,
    translation: Vec3,
}

impl Default for BoxBuilder {
    fn default() -> Self {
        BoxBuilder {
            name: "Box".to_string(),
            width: 1.0,
            height: 1.0,
            depth: 1.0,
            translation: Vec3::ZERO,
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

    /// Applies `translation` to the resulting box. The resulting box will be
    /// centered on `translation`. Default is [`Vec3::ZERO`].
    pub fn translate(mut self, translation: impl Into<Vec3>) -> Self {
        self.translation = translation.into();
        self
    }

    /// Creates and returns the cylinder geometry.
    pub fn build(self, ctx: &Context) -> Geometry {
        let x = self.width / 2.0;
        let y = self.height / 2.0;
        let z = self.depth / 2.0;

        GeometryBuilder::new(6 * 4, 0)
            .name(self.name)
            .positions(
                [
                    // face 0
                    vec3(x, y, z),
                    vec3(x, y, -z),
                    vec3(x, -y, -z),
                    vec3(x, -y, z),
                    // face 1
                    vec3(-x, y, -z),
                    vec3(x, y, -z),
                    vec3(x, y, z),
                    vec3(-x, y, z),
                    // face 2
                    vec3(-x, y, z),
                    vec3(x, y, z),
                    vec3(x, -y, z),
                    vec3(-x, -y, z),
                    // face 3
                    vec3(-x, -y, -z),
                    vec3(x, -y, -z),
                    vec3(x, y, -z),
                    vec3(-x, y, -z),
                    // face 4
                    vec3(-x, -y, z),
                    vec3(x, -y, z),
                    vec3(x, -y, -z),
                    vec3(-x, -y, -z),
                    // face 5
                    vec3(-x, y, -z),
                    vec3(-x, y, z),
                    vec3(-x, -y, z),
                    vec3(-x, -y, -z),
                ]
                .into_iter()
                .map(|p| p + self.translation),
            )
            .unwrap()
            .normals([
                // face 0
                Vec3::X,
                Vec3::X,
                Vec3::X,
                Vec3::X,
                // face 1
                Vec3::Y,
                Vec3::Y,
                Vec3::Y,
                Vec3::Y,
                // face 2
                Vec3::Z,
                Vec3::Z,
                Vec3::Z,
                Vec3::Z,
                // face 3
                Vec3::NEG_Z,
                Vec3::NEG_Z,
                Vec3::NEG_Z,
                Vec3::NEG_Z,
                // face 4
                Vec3::NEG_Y,
                Vec3::NEG_Y,
                Vec3::NEG_Y,
                Vec3::NEG_Y,
                // face 5
                Vec3::NEG_X,
                Vec3::NEG_X,
                Vec3::NEG_X,
                Vec3::NEG_X,
            ])
            .unwrap()
            .tex_coords_0([
                // face 0
                vec2(0.5, 0.25),
                vec2(0.75, 0.25),
                vec2(0.75, 0.5),
                vec2(0.5, 0.5),
                // face 1
                vec2(0.25, 0.0),
                vec2(0.5, 0.0),
                vec2(0.5, 0.25),
                vec2(0.25, 0.25),
                // face 2
                vec2(0.25, 0.25),
                vec2(0.5, 0.25),
                vec2(0.5, 0.5),
                vec2(0.25, 0.5),
                // face 3
                vec2(0.25, 0.75),
                vec2(0.5, 0.75),
                vec2(0.5, 1.0),
                vec2(0.25, 1.0),
                // face 4
                vec2(0.25, 0.5),
                vec2(0.5, 0.5),
                vec2(0.5, 0.75),
                vec2(0.25, 0.75),
                // face 5
                vec2(0.0, 0.25),
                vec2(0.25, 0.25),
                vec2(0.25, 0.5),
                vec2(0.0, 0.5),
            ])
            .unwrap()
            .indices_u16(
                [
                    0, 1, 2, 2, 3, 0, // face 0
                    4, 5, 6, 6, 7, 4, // face 1
                    8, 9, 10, 10, 11, 8, // face 2
                    12, 13, 14, 14, 15, 12, // face 3
                    16, 17, 18, 18, 19, 16, // face 4
                    20, 21, 22, 22, 23, 20, // face 5
                ]
                .into_iter()
                .rev(),
            )
            .build(ctx)
            .unwrap()
    }
}
