use glam::DVec3;

use crate::{
    AABB, Transform,
    collision::CollisionInfo,
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

/// Returns `true` iff the two balls collide when applying the given transforms.
/// If the two balls are exactly touching, this function returns `false`.
///
/// # Examples
///
/// ```
/// # use glam::dvec3;
/// # use tonner::{Transform, shape::Ball, shape::collides_2balls};
/// let ball = Ball::UNIT;
///
/// let transform1 = Transform::IDENTITY;
/// let transform2 = Transform::from_translation(dvec3(1.0, 0.0, 0.0));
/// let transform3 = Transform::from_translation(dvec3(2.0, 0.0, 0.0));
///
/// assert!(collides_2balls((&ball, &transform1), (&ball, &transform1)));
/// assert!(collides_2balls((&ball, &transform1), (&ball, &transform2)));
/// assert!(!collides_2balls((&ball, &transform1), (&ball, &transform3)));
/// ```
pub fn collides_2balls(
    (ball1, transform1): (&Ball, &Transform),
    (ball2, transform2): (&Ball, &Transform),
) -> bool {
    let center_distance = transform1.translation - transform2.translation;
    let radii_sum = ball1.radius + ball2.radius;
    center_distance.length_squared() < radii_sum * radii_sum
}

/// Returns the squared distance between the two balls when applying the given transforms.
/// If the two balls are touching or overlapping, this function returns `0.0`.
///
/// # Examples
/// ```
/// # use glam::dvec3;
/// # use tonner::{Transform, shape::Ball, shape::distance_2balls};
/// let ball = Ball::UNIT;
/// let transform1 = Transform::IDENTITY;
/// let transform2 = Transform::from_translation(dvec3(3.0, 0.0, 0.0));
/// assert_eq!(distance_2balls((&ball, &transform1), (&ball, &transform2)), 1.0);
/// ```
pub fn distance_2balls(
    (ball1, transform1): (&Ball, &Transform),
    (ball2, transform2): (&Ball, &Transform),
) -> f64 {
    let center_distance = transform1.translation - transform2.translation;
    let radii_sum = ball1.radius + ball2.radius;
    0.0f64.max(center_distance.length() - radii_sum)
}

/// Returns information about the collision between the two balls when applying the given transforms.
///
/// # Examples
/// ```
/// # use glam::dvec3;
/// # use tonner::{Transform, shape::Ball, shape::collision_info_2balls};
/// let ball = Ball::UNIT;
/// let transform1 = Transform::IDENTITY;
/// let transform2 = Transform::from_translation(dvec3(1.5, 0.0, 0.0));
/// let collision_info = collision_info_2balls((&ball, &transform1), (&ball, &transform2));
/// assert_eq!(collision_info.separating_vector, dvec3(0.5, 0.0, 0.0));
/// assert_eq!(collision_info.local_contact_points[0], dvec3(1.0, 0.0, 0.0));
/// assert_eq!(collision_info.local_contact_points[1], dvec3(-1.0, 0.0, 0.0));
/// ```
pub fn collision_info_2balls(
    (ball1, transform1): (&Ball, &Transform),
    (ball2, transform2): (&Ball, &Transform),
) -> CollisionInfo {
    let center_distance = transform1.translation - transform2.translation;
    let separating_dir = center_distance.normalize_or(DVec3::X);
    let local_contact_points = [
        -ball1.radius * separating_dir,
        ball2.radius * separating_dir,
    ];
    let separating_vector = transform1.translation + local_contact_points[0]
        - (transform2.translation + local_contact_points[1]);
    CollisionInfo {
        separating_vector,
        local_contact_points,
    }
}
