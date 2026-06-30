use glam::DVec3;

use crate::{Transform, shape::Ball};

/// Information about a collision between two objects. This is returned by the narrow phase of the collision detection process.
#[derive(Debug, Clone)]
pub struct CollisionInfo {
    /// The penetration depth of the collision. This is the minimal distance that the two objects need to be separated to resolve the collision. If the two objects are exactly touching, this value is `0.0`.
    pub penetration_depth: f64,

    /// The normal of the collision. This is a unit vector that points from the first object to the second object. It is used to determine the direction of the collision response. It is expressed in world space (i.e., after applying the transforms of the objects).
    pub world_normal: DVec3,

    /// The contact points on the two objects. These are points on the surface of the objects that are in contact with each other. They are expressed in the local frame of each object (i.e., before applying the object's transform).
    pub local_contact_points: [DVec3; 2],
}

/// Returns `Some(CollisionInfo)` if the two balls collide when applying the given transforms, and `None` otherwise. If the two balls are exactly touching, this function returns `None`.
///
/// # Examples
///
/// Two balls that are colliding:
/// ```
/// # use glam::dvec3;
/// # use tonner::{Transform, shape::Ball, collision::narrow::collides_ball_ball};
/// let ball = Ball::UNIT;
/// let a = Transform::IDENTITY;
/// let b = Transform::from_translation(dvec3(1.0, 0.0, 0.0));
/// let collision_info = collides_ball_ball((&ball, &a), (&ball, &b)).unwrap();
/// assert_eq!(collision_info.penetration_depth, 1.0);
/// assert_eq!(collision_info.world_normal, dvec3(1.0, 0.0, 0.0));
/// assert_eq!(collision_info.local_contact_points[0], dvec3(1.0, 0.0, 0.0));
/// assert_eq!(collision_info.local_contact_points[1], dvec3(-1.0, 0.0, 0.0));
/// ```
///
/// Two balls that are touching:
/// ```
/// # use glam::dvec3;
/// # use tonner::{Transform, shape::Ball, collision::narrow::collides_ball_ball};
/// let ball = Ball::UNIT;
/// let a = Transform::IDENTITY;
/// let b = Transform::from_translation(dvec3(2.0, 0.0, 0.0));
/// assert!(collides_ball_ball((&ball, &a), (&ball, &b)).is_none());
/// ```
///
/// Two balls that are not colliding:
/// ```
/// # use glam::dvec3;
/// # use tonner::{Transform, shape::Ball, collision::narrow::collides_ball_ball};
/// let ball = Ball::UNIT;
/// let a = Transform::IDENTITY;
/// let b = Transform::from_translation(dvec3(3.0, 0.0, 0.0));
/// assert!(collides_ball_ball((&ball, &a), (&ball, &b)).is_none());
/// ```
pub fn collides_ball_ball(
    (ball0, transform0): (&Ball, &Transform),
    (ball1, transform1): (&Ball, &Transform),
) -> Option<CollisionInfo> {
    let dp = transform1.translation - transform0.translation;
    let radii_sum = ball0.radius() + ball1.radius();

    if dp.length_squared() >= radii_sum * radii_sum {
        return None;
    }

    let (world_normal, center_distance) = dp.normalize_and_length();
    let penetration_depth = radii_sum - center_distance;

    let local_contact_points = [
        transform0.rotation.conjugate() * world_normal * ball0.radius(),
        transform1.rotation.conjugate() * -world_normal * ball1.radius(),
    ];

    Some(CollisionInfo {
        penetration_depth,
        world_normal,
        local_contact_points,
    })
}
