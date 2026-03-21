use glam::{Vec3, vec3};

use crate::gjk::gjk;

#[derive(Debug, Clone, Copy)]
pub struct AxisAlignedBox {
    min: Vec3,
    max: Vec3,
}

impl AxisAlignedBox {
    pub fn from_min_max(min: Vec3, max: Vec3) -> AxisAlignedBox {
        AxisAlignedBox { min, max }
    }

    /// Creates an `AxisAlignedBox` from its center, its width, height and depth.
    ///
    /// `width`, `height` and `depth` must be larger or equal to `0.0`. They represent the length of the side parallel
    /// to the x, y and respectively z-axis.
    pub fn from_center_dimension(
        center: Vec3,
        width: f32,
        height: f32,
        depth: f32,
    ) -> AxisAlignedBox {
        let half = vec3(width, height, depth) / 2.0;
        AxisAlignedBox {
            min: center - half,
            max: center + half,
        }
    }
}

impl Shape for AxisAlignedBox {
    fn bounding_box(&self) -> AxisAlignedBox {
        *self
    }

    fn centroid(&self) -> Vec3 {
        self.min.midpoint(self.max)
    }
}

impl ConvexShape for AxisAlignedBox {
    fn support_point(&self, direction: Vec3) -> Vec3 {
        vec3(
            if direction.x.is_sign_positive() {
                self.max.x
            } else {
                self.min.x
            },
            if direction.y.is_sign_positive() {
                self.max.y
            } else {
                self.min.y
            },
            if direction.z.is_sign_positive() {
                self.max.z
            } else {
                self.min.z
            },
        )
    }
}

pub trait Shape {
    /// Smallest axis-aligned box containing the shape.
    fn bounding_box(&self) -> AxisAlignedBox;

    /// Geometric center of the shape (arithmetic mean position of all points the shape).
    fn centroid(&self) -> Vec3;
}

/// A convex shape is a region of space where for all pair of points,
/// the segment between them is entirely inside the shape.
pub trait ConvexShape: Shape {
    /// The point on shape which has the highest dot product with `direction`.
    ///
    /// This corresponds to furthest point in the given direction that is still on the shape.
    /// In general this point is not unique.
    ///
    /// `direction` can have any stricly positive length.
    fn support_point(&self, direction: Vec3) -> Vec3;

    fn collides(&self, other: &impl ConvexShape) -> bool {
        gjk(self, other)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Ball {
    pub center: Vec3,
    pub radius: f32,
}

impl Shape for Ball {
    fn bounding_box(&self) -> AxisAlignedBox {
        AxisAlignedBox {
            min: self.center - self.radius,
            max: self.center + self.radius,
        }
    }

    fn centroid(&self) -> Vec3 {
        self.center
    }
}

impl ConvexShape for Ball {
    fn support_point(&self, direction: Vec3) -> Vec3 {
        self.center + direction.normalize() * self.radius
    }
}
