use glam::{DVec3, dvec3};

use crate::{
    AABB, Transform,
    shape::{ConvexShape3D, Shape3D},
};

/// A 3d Box of dimensions `x`, `y` and `z` centered at the origin and aligned with the coordinate axes.
#[derive(Debug, Clone, Copy)]
pub struct Box3D {
    halves: DVec3,
}

impl Box3D {
    /// Creates a `Box3D` from its side lengths `x`, `y` and `z`.
    ///
    /// ## Example
    /// ```
    /// # use tonner::shape::Box3D;
    /// let box_ = Box3D::from_dimensions(2.0, 4.0, 6.0);
    /// assert_eq!(box_.x(), 2.0);
    /// assert_eq!(box_.y(), 4.0);
    /// assert_eq!(box_.z(), 6.0);
    /// ```
    pub fn from_dimensions(x: f64, y: f64, z: f64) -> Box3D {
        Box3D {
            halves: dvec3(x, y, z) / 2.0,
        }
    }

    /// Returns the side length of the Box3D in the `x` direction.
    ///
    /// ## Example
    /// ```
    /// # use tonner::shape::Box3D;
    /// let box_ = Box3D::from_dimensions(2.0, 4.0, 6.0);
    /// assert_eq!(box_.x(), 2.0);
    /// ```
    pub fn x(&self) -> f64 {
        self.halves.x * 2.0
    }

    /// Returns the side length of the Box3D in the `y` direction.
    ///
    /// ## Example
    /// ```
    /// # use tonner::shape::Box3D;
    /// let box_ = Box3D::from_dimensions(2.0, 4.0, 6.0);
    /// assert_eq!(box_.y(), 4.0);
    /// ```
    pub fn y(&self) -> f64 {
        self.halves.y * 2.0
    }

    /// Returns the side length of the Box3D in the `z` direction.
    ///
    /// ## Example
    /// ```
    /// # use tonner::shape::Box3D;
    /// let box_ = Box3D::from_dimensions(2.0, 4.0, 6.0);
    /// assert_eq!(box_.z(), 6.0);
    /// ```
    pub fn z(&self) -> f64 {
        self.halves.z * 2.0
    }

    /// Returns the half side lengths of the Box3D in the `x`, `y` and `z` directions.
    /// 
    /// ## Example
    /// ```
    /// # use tonner::shape::Box3D;
    /// let box_ = Box3D::from_dimensions(2.0, 4.0, 6.0);
    /// assert_eq!(box_.halves(), glam::dvec3(1.0, 2.0, 3.0));
    /// ```
    pub fn halves(&self) -> DVec3 {
        self.halves
    }
}

impl Shape3D for Box3D {
    fn aabb(&self, transform: &Transform) -> AABB {
        let halves = (transform.rotation * self.halves).abs();
        AABB::from_min_max(
            transform.translation - halves,
            transform.translation + halves,
        )
    }

    fn centroid(&self, transform: &Transform) -> DVec3 {
        transform.translation
    }
}

impl ConvexShape3D for Box3D {
    fn support_point(&self, transform: &Transform, direction: DVec3) -> DVec3 {
        let halves = (transform.rotation * self.halves).abs();
        transform.translation + direction.signum() * halves
    }
}

#[cfg(test)]
mod tests {
    use glam::DQuat;

    use super::*;

    #[test]
    fn aabb() {
        let box_ = Box3D::from_dimensions(2.0, 4.0, 6.0);
        let transform = Transform::IDENTITY;

        let aabb = box_.aabb(&transform);
        assert_eq!(aabb.min(), dvec3(-1.0, -2.0, -3.0));
        assert_eq!(aabb.max(), dvec3(1.0, 2.0, 3.0));

        let transform = Transform::from_translation(dvec3(10.0, 20.0, 30.0));
        let aabb = box_.aabb(&transform);
        assert_eq!(aabb.min(), dvec3(9.0, 18.0, 27.0));
        assert_eq!(aabb.max(), dvec3(11.0, 22.0, 33.0));

        let transform =
            Transform::from_rotation(DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2));
        let aabb = box_.aabb(&transform);
        assert_approx_eq_vec3(aabb.min(), dvec3(-3.0, -2.0, -1.0), 1e-6);
        assert_approx_eq_vec3(aabb.max(), dvec3(3.0, 2.0, 1.0), 1e-6);
    }

    #[test]
    fn support_point() {
        let box_ = Box3D::from_dimensions(2.0, 4.0, 6.0);
        let transform = Transform::IDENTITY;

        let dir = dvec3(1.0, 1e-6, 1e-6);
        let support_point = box_.support_point(&transform, dir);
        assert_approx_eq_vec3(support_point, dvec3(1.0, 2.0, 3.0), 1e-6);

        let dir = dvec3(-1e-6, -1.0, -1e-6);
        let support_point = box_.support_point(&transform, dir);
        assert_approx_eq_vec3(support_point, dvec3(-1.0, -2.0, -3.0), 1e-6);

        let transform = Transform {
            translation: dvec3(10.0, 20.0, 30.0),
            rotation: DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2),
        };

        let dir = dvec3(1.0, 1e-6, 1e-6);
        let support_point = box_.support_point(&transform, dir);
        assert_approx_eq_vec3(support_point, dvec3(13.0, 22.0, 31.0), 1e-6);

        let dir = dvec3(-1e-6, -1.0, -1e-6);
        let support_point = box_.support_point(&transform, dir);
        assert_approx_eq_vec3(support_point, dvec3(7.0, 18.0, 29.0), 1e-6);
    }

    #[rustfmt::skip]
    fn assert_approx_eq_vec3(a: DVec3, b: DVec3, epsilon: f64) {
        assert!((a.x - b.x).abs() < epsilon, "assertion `a.x ≈ b.x` failed: {} ≈! {}", a.x, b.x);
        assert!((a.y - b.y).abs() < epsilon, "assertion `a.y ≈ b.y` failed: {} ≈! {}", a.y, b.y);
        assert!((a.z - b.z).abs() < epsilon, "assertion `a.z ≈ b.z` failed: {} ≈! {}", a.z, b.z);
    }
}
