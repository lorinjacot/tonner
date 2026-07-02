use glam::DVec3;

use crate::{
    AABB, Transform,
    shape::{ConvexShape3D, Shape3D},
};

/// A 3d ball of radius `radius` centered at the origin.
#[derive(Debug, Clone, Copy)]
pub struct Ball {
    radius: f64,
}

impl Ball {
    /// A ball of radius `1.0`.
    ///
    /// # Example
    /// ```
    /// # use tonner::shape::Ball;
    /// let ball = Ball::UNIT;
    /// assert_eq!(ball.radius(), 1.0);
    /// ```
    pub const UNIT: Ball = Ball { radius: 1.0 };

    /// Creates a `Ball` from its radius.
    ///
    /// # Example
    /// ```
    /// # use tonner::shape::Ball;
    /// let ball = Ball::from_radius(2.5);
    /// assert_eq!(ball.radius(), 2.5);
    /// ```
    pub fn from_radius(radius: f64) -> Ball {
        Ball { radius }
    }

    /// Returns the radius of the ball.
    ///
    /// # Example
    /// ```
    /// # use tonner::shape::Ball;
    /// let ball = Ball::from_radius(2.5);
    /// assert_eq!(ball.radius(), 2.5);
    /// ```
    pub fn radius(&self) -> f64 {
        self.radius
    }
}

impl Shape3D for Ball {
    fn aabb(&self, transform: &Transform) -> AABB {
        AABB::from_min_max(
            transform.translation - self.radius,
            transform.translation + self.radius,
        )
    }

    fn centroid(&self, transform: &Transform) -> DVec3 {
        transform.translation
    }
}

impl ConvexShape3D for Ball {
    fn support_point(&self, transform: &Transform, direction: DVec3) -> DVec3 {
        transform.translation + self.radius * direction.normalize_or(DVec3::X)
    }
}
