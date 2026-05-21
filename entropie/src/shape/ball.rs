use glam::Vec3;

use crate::{
    AABB, Transform,
    shape::{ConvexShape3D, Shape3D},
};

/// A 3d ball of radius `radius` centered at the origin.
#[derive(Debug, Clone, Copy)]
pub struct Ball {
    radius: f32,
}

impl Ball {
    /// A unit ball of radius `1.0`.
    /// 
    /// ## Example
    /// ```
    /// # use entropie::shape::Ball;
    /// let ball = Ball::UNIT;
    /// assert_eq!(ball.radius(), 1.0);
    /// ```
    pub const UNIT: Ball = Ball { radius: 1.0 };

    /// Creates a `Ball` from its radius.
    /// 
    /// ## Example
    /// ```
    /// # use entropie::shape::Ball;
    /// let ball = Ball::from_radius(2.5);
    /// assert_eq!(ball.radius(), 2.5);
    /// ```
    pub fn from_radius(radius: f32) -> Ball {
        Ball { radius }
    }

    /// Returns the radius of the ball.
    /// 
    /// ## Example
    /// ```
    /// # use entropie::shape::Ball;
    /// let ball = Ball::from_radius(2.5);
    /// assert_eq!(ball.radius(), 2.5);
    /// ```
    pub fn radius(&self) -> f32 {
        self.radius
    }

    /// Returns `true` iff `self` collides/overlaps with `other` when applying the given transforms.
    /// If the two balls are exactly touching, this function returns `false`.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use glam::vec3;
    /// # use entropie::{Transform, shape::Ball};
    /// let ball = Ball::UNIT;
    /// 
    /// let transform1 = Transform::IDENTITY;
    /// let transform2 = Transform::from_translation(vec3(1.0, 0.0, 0.0));
    /// let transform3 = Transform::from_translation(vec3(2.0, 0.0, 0.0));
    /// 
    /// assert!(ball.collides_ball(&transform1, &ball, &transform1));
    /// assert!(ball.collides_ball(&transform1, &ball, &transform2));
    /// assert!(!ball.collides_ball(&transform1, &ball, &transform3));
    /// ```
    pub fn collides_ball(
        &self,
        transform: &Transform,
        other: &Ball,
        other_transform: &Transform,
    ) -> bool {
        let center_distance = transform.translation - other_transform.translation;
        let radii_sum = self.radius + other.radius;
        center_distance.length_squared() < radii_sum * radii_sum
    }
}

impl Shape3D for Ball {
    fn aabb(&self, transform: &Transform) -> AABB {
        AABB::from_min_max(
            transform.translation - self.radius,
            transform.translation + self.radius,
        )
    }

    fn centroid(&self, transform: &Transform) -> Vec3 {
        transform.translation
    }
}

impl ConvexShape3D for Ball {
    fn support_point(&self, transform: &Transform, direction: Vec3) -> Vec3 {
        transform.translation + self.radius * direction.normalize_or(Vec3::X)
    }
}
