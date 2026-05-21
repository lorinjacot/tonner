use glam::Vec3;

/// An axis-aligned bounding box (AABB) is a 3D box whose faces are parallel to the coordinate axes.
/// It is defined by its minimum and maximum corners, which are the points with the smallest and largest coordinates respectively.
///
/// The AABB is a simple and efficient way to represent the bounding volume of a shape, and it is often used in collision detection algorithms.
pub struct AABB {
    min: Vec3,
    max: Vec3,
}

impl AABB {
    /// Creates an `AABB` from its minimum and maximum corners points.
    pub fn from_min_max(min: Vec3, max: Vec3) -> AABB {
        AABB { min, max }
    }

    /// Returns the corner with the smallest coordinates of the AABB.
    pub fn min(&self) -> Vec3 {
        self.min
    }

    /// Returns the corner with the largest coordinates of the AABB.
    pub fn max(&self) -> Vec3 {
        self.max
    }
}
