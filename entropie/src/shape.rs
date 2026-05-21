use glam::Vec3;

pub use ball::Ball;
pub use box3d::Box3D;

use crate::{AABB, Transform};

mod ball;
mod box3d;

/// A shape is a representation of a subset of the 3D space. It is usually used to define the geometry of a 3d object.
/// It does not contain any information about the position or orientation of the object.
pub trait Shape3D {
    /// Smallest axis-aligned box containing the shape after applying the given transform.
    fn aabb(&self, transform: &Transform) -> AABB;

    /// Geometric center of the shape (arithmetic mean position of all points the shape) after applying the given transform.
    fn centroid(&self, transform: &Transform) -> Vec3;
}

/// A convex shape is a region of space where for all pair of points, the segment between them is entirely inside the shape.
pub trait ConvexShape3D: Shape3D {
    /// The point on shape which has the highest dot product with `direction` after applying the given transform.
    ///
    /// This corresponds to furthest point in the given direction that is still on the shape.
    /// In general this point is not unique but is always on the surface of the shape.
    ///
    /// `direction` can have any length, including `0.0`. In the latter case, any point one the surface of the shape can be returned.
    fn support_point(&self, transform: &Transform, direction: Vec3) -> Vec3;
}
