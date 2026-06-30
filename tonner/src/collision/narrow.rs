use glam::{DMat3, DVec3};

use crate::{
    Transform,
    shape::{Ball, Box3D},
};

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
/// Colliding balls:
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
/// Touching balls:
/// ```
/// # use glam::dvec3;
/// # use tonner::{Transform, shape::Ball, collision::narrow::collides_ball_ball};
/// let ball = Ball::UNIT;
/// let a = Transform::IDENTITY;
/// let b = Transform::from_translation(dvec3(2.0, 0.0, 0.0));
/// assert!(collides_ball_ball((&ball, &a), (&ball, &b)).is_none());
/// ```
///
/// Non-colliding balls:
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

/// Returns `Some(CollisionInfo)` if the ball and box collide when applying the given transforms, and `None` otherwise. If the ball and box are exactly touching, this function returns `None`.
///
/// Based on the algorithm described in "Collision Detection in Interactive 3D Environments" by Gino van den Bergen, 2004, Section 3.2.2 "Sphere-Box Test".
///
/// # Examples
///
/// Colliding ball and box:
/// ```
/// # use glam::dvec3;
/// # use tonner::{Transform, shape::{Ball, Box3D}, collision::narrow::collides_ball_box};
/// let ball = Ball::UNIT;
/// let box_ = Box3D::from_dimensions(2.0, 2.0, 2.0);
/// let ball_transform = Transform::IDENTITY;
/// let box_transform = Transform::from_translation(dvec3(1.0, 0.0, 0.0));
/// let collision_info = collides_ball_box((&ball, &ball_transform), (&box_, &box_transform)).unwrap();
/// assert_eq!(collision_info.penetration_depth, 1.0);
/// assert_eq!(collision_info.world_normal, dvec3(1.0, 0.0, 0.0));
/// assert_eq!(collision_info.local_contact_points[0], dvec3(1.0, 0.0, 0.0));
/// assert_eq!(collision_info.local_contact_points[1], dvec3(-1.0, 0.0, 0.0));
/// ```
///
/// Touching ball and box:
/// ```
/// # use glam::dvec3;
/// # use tonner::{Transform, shape::{Ball, Box3D}, collision::narrow::collides_ball_box};
/// let ball = Ball::UNIT;
/// let box_ = Box3D::from_dimensions(2.0, 2.0, 2.0);
/// let ball_transform = Transform::IDENTITY;
/// let box_transform = Transform::from_translation(dvec3(2.0, 0.0, 0.0));
/// assert!(collides_ball_box((&ball, &ball_transform), (&box_, &box_transform)).is_none());
/// ```
///
/// Non-colliding ball and box:
/// ```
/// # use glam::dvec3;
/// # use tonner::{Transform, shape::{Ball, Box3D}, collision::narrow::collides_ball_box};
/// let ball = Ball::UNIT;
/// let box_ = Box3D::from_dimensions(2.0, 2.0, 2.0);
/// let ball_transform = Transform::IDENTITY;
/// let box_transform = Transform::from_translation(dvec3(3.0, 0.0, 0.0));
/// assert!(collides_ball_box((&ball, &ball_transform), (&box_, &box_transform)).is_none());
/// ```
pub fn collides_ball_box(
    (ball, ball_transform): (&Ball, &Transform),
    (box3d, box_transform): (&Box3D, &Transform),
) -> Option<CollisionInfo> {
    let relative_ball_center = box_transform.rotation.conjugate()
        * (ball_transform.translation - box_transform.translation);

    let closest_point_on_box = relative_ball_center.clamp(-box3d.halves(), box3d.halves());

    let v = closest_point_on_box - relative_ball_center;
    if v.length_squared() >= ball.radius() * ball.radius() {
        return None;
    }

    let (normal, distance) = v.normalize_and_length();
    let (ball_witness_point, box_witness_point, penetration_depth, local_normal) =
        if distance == 0.0 {
            // center of the ball is inside the box
            let delta = box3d.halves() - relative_ball_center.abs();

            let mut smallest_component = delta.x;
            let mut smallest_dir = relative_ball_center.x.signum() * DVec3::NEG_X;
            if delta.y < smallest_component {
                smallest_component = delta.y;
                smallest_dir = relative_ball_center.y.signum() * DVec3::NEG_Y;
            }
            if delta.z < smallest_component {
                smallest_component = delta.z;
                smallest_dir = relative_ball_center.z.signum() * DVec3::NEG_Z;
            }

            let box_witness_point = relative_ball_center - smallest_component * smallest_dir;
            let ball_witness_point = relative_ball_center + ball.radius() * smallest_dir;
            let penetration_depth = smallest_component + ball.radius();

            (
                ball_witness_point,
                box_witness_point,
                penetration_depth,
                smallest_dir,
            )
        } else {
            // center of the ball is outside the box
            let ball_witness_point = relative_ball_center + ball.radius() * normal;
            let penetration_depth = ball.radius() - distance;

            (
                ball_witness_point,
                closest_point_on_box,
                penetration_depth,
                normal,
            )
        };

    let world_normal = box_transform.rotation * local_normal;
    let ball_witness_point_world =
        box_transform.rotation * ball_witness_point + box_transform.translation;
    let ball_witness_point_local = ball_transform.rotation.conjugate()
        * (ball_witness_point_world - ball_transform.translation);

    Some(CollisionInfo {
        penetration_depth,
        world_normal,
        local_contact_points: [ball_witness_point_local, box_witness_point],
    })
}

/// Returns `Some(CollisionInfo)` if the two boxes collide when applying the given transforms, and `None` otherwise. If the two boxes are exactly touching, this function returns `None`.
///
/// # Examples
///
/// Colliding boxes:
/// ```
/// # use glam::DVec3;
/// # use tonner::{Transform, shape::Box3D, collision::narrow::collides_box_box};
/// let box0 = Box3D::from_dimensions(2.0, 2.0, 2.0);
/// let box1 = Box3D::from_dimensions(2.0, 2.0, 2.0);
/// let transform0 = Transform::IDENTITY;
/// let transform1 = Transform::from_translation(DVec3::new(1.5, 0.0, 0.0));
/// let collision_info = collides_box_box((&box0, &transform0), (&box1, &transform1)).unwrap();
/// assert_eq!(collision_info.penetration_depth, 0.5);
/// assert_eq!(collision_info.world_normal, DVec3::new(1.0, 0.0, 0.0));
/// assert_eq!(collision_info.local_contact_points[0], DVec3::new(1.0, 0.0, 0.0));
/// assert_eq!(collision_info.local_contact_points[1], DVec3::new(-1.0, 0.0, 0.0));
/// ```
///
/// Touching boxes:
/// ```
/// # use glam::DVec3;
/// # use tonner::{Transform, shape::Box3D, collision::narrow::collides_box_box};
/// let box0 = Box3D::from_dimensions(2.0, 2.0, 2.0);
/// let box1 = Box3D::from_dimensions(2.0, 2.0, 2.0);
/// let transform0 = Transform::IDENTITY;
/// let transform1 = Transform::from_translation(DVec3::new(2.0, 0.0, 0.0));
/// assert!(collides_box_box((&box0, &transform0), (&box1, &transform1)).is_none());
/// ```
///
/// Non-colliding boxes:
/// ```
/// # use glam::DVec3;
/// # use tonner::{Transform, shape::Box3D, collision::narrow::collides_box_box};
/// let box0 = Box3D::from_dimensions(2.0, 2.0, 2.0);
/// let box1 = Box3D::from_dimensions(2.0, 2.0, 2.0);
/// let transform0 = Transform::IDENTITY;
/// let transform1 = Transform::from_translation(DVec3::new(3.0, 0.0, 0.0));
/// assert!(collides_box_box((&box0, &transform0), (&box1, &transform1)).is_none());
/// ```
pub fn collides_box_box(
    (box0, transform0): (&Box3D, &Transform),
    (box1, transform1): (&Box3D, &Transform),
) -> Option<CollisionInfo> {
    let rot0 = DMat3::from_quat(transform0.rotation);
    let rot1 = DMat3::from_quat(transform1.rotation);

    let rot0_t = rot0.transpose();
    let rot1_t = rot1.transpose();

    let mut min_penetration_depth = f64::INFINITY;
    let mut collision_normal = DVec3::X;
    let mut local_contact_points = [DVec3::ZERO, DVec3::ZERO];
    for axis in [
        rot0.x_axis,
        rot0.y_axis,
        rot0.z_axis,
        rot1.x_axis,
        rot1.y_axis,
        rot1.z_axis,
        rot0.x_axis.cross(rot1.x_axis),
        rot0.x_axis.cross(rot1.y_axis),
        rot0.x_axis.cross(rot1.z_axis),
        rot1.y_axis.cross(rot0.x_axis),
        rot1.y_axis.cross(rot0.y_axis),
        rot1.y_axis.cross(rot0.z_axis),
        rot1.z_axis.cross(rot0.x_axis),
        rot1.z_axis.cross(rot0.y_axis),
        rot1.z_axis.cross(rot0.z_axis),
    ] {
        if axis.length_squared() < 0.1 {
            continue;
        }
        let center0 = transform0.translation.dot(axis);
        let center1 = transform1.translation.dot(axis);
        let mut distance = center1 - center0;
        let normal = if distance < 0.0 {
            distance = -distance;
            -axis
        } else {
            axis
        };

        let radius0 = (rot0_t * axis).abs().dot(box0.halves());
        let radius1 = (rot1_t * axis).abs().dot(box1.halves());

        if distance >= radius0 + radius1 {
            return None;
        }

        let penetration_depth = radius0 + radius1 - distance;
        if penetration_depth < min_penetration_depth {
            min_penetration_depth = penetration_depth;
            collision_normal = normal;
            local_contact_points = [rot0_t * normal * radius0, rot1_t * -normal * radius1];
        }
    }

    Some(CollisionInfo {
        penetration_depth: min_penetration_depth,
        world_normal: collision_normal,
        local_contact_points,
    })
}

#[cfg(test)]
mod tests {
    use glam::DQuat;

    use super::*;

    #[test]
    fn test_collides_ball_box() {
        let ball = Ball::UNIT;
        let box_ = Box3D::from_dimensions(2.0, 2.0, 2.0);

        let box_transform = Transform::IDENTITY;

        // no collision
        let ball_transform = Transform::from_translation(DVec3::new(3.0, 0.0, 0.0));
        assert!(collides_ball_box((&ball, &ball_transform), (&box_, &box_transform)).is_none());

        // touching
        let ball_transform = Transform::from_translation(DVec3::new(2.0, 0.0, 0.0));
        assert!(collides_ball_box((&ball, &ball_transform), (&box_, &box_transform)).is_none());

        // colliding, ball center outside box
        let ball_transform = Transform::from_translation(DVec3::new(1.5, 0.0, 0.0));
        let collision_info =
            collides_ball_box((&ball, &ball_transform), (&box_, &box_transform)).unwrap();
        assert_eq!(collision_info.penetration_depth, 0.5);
        assert_eq!(collision_info.world_normal, DVec3::new(-1.0, 0.0, 0.0));
        assert_eq!(
            collision_info.local_contact_points[0],
            DVec3::new(-1.0, 0.0, 0.0)
        );
        assert_eq!(
            collision_info.local_contact_points[1],
            DVec3::new(1.0, 0.0, 0.0)
        );

        // colliding, ball center inside box
        let ball_transform = Transform::from_translation(DVec3::new(0.5, 0.0, 0.0));
        let collision_info =
            collides_ball_box((&ball, &ball_transform), (&box_, &box_transform)).unwrap();
        assert_eq!(collision_info.penetration_depth, 1.5);
        assert_eq!(collision_info.world_normal, DVec3::new(-1.0, 0.0, 0.0));
        assert_eq!(
            collision_info.local_contact_points[0],
            DVec3::new(-1.0, 0.0, 0.0)
        );
        assert_eq!(
            collision_info.local_contact_points[1],
            DVec3::new(1.0, 0.0, 0.0)
        );

        // with translation and rotation
        let box_transform = Transform {
            translation: DVec3::new(1.0, 2.0, 3.0),
            rotation: glam::DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2),
        };
        let ball_transform = Transform {
            translation: DVec3::new(1.5, 2.0, 3.0),
            rotation: glam::DQuat::IDENTITY,
        };
        let collision_info =
            collides_ball_box((&ball, &ball_transform), (&box_, &box_transform)).unwrap();
        assert_eq!(collision_info.penetration_depth, 1.5);
        assert!(
            collision_info
                .world_normal
                .abs_diff_eq(DVec3::new(-1.0, 0.0, 0.0), 1e-6)
        );
        assert_eq!(
            collision_info.local_contact_points[0],
            DVec3::new(-1.0, 0.0, 0.0)
        );
        assert_eq!(
            collision_info.local_contact_points[1],
            DVec3::new(0.0, 0.0, 1.0)
        );
    }

    #[test]
    fn test_collides_box_box() {
        let box0 = Box3D::from_dimensions(2.0, 2.0, 2.0);
        let box1 = Box3D::from_dimensions(2.0, 2.0, 2.0);

        let transform0 = Transform::IDENTITY;
        let transform1 = Transform::from_translation(DVec3::new(1.5, 0.0, 0.0));

        let collision_info = collides_box_box((&box0, &transform0), (&box1, &transform1)).unwrap();
        assert_eq!(collision_info.penetration_depth, 0.5);
        assert_eq!(collision_info.world_normal, DVec3::new(1.0, 0.0, 0.0));
        assert_eq!(
            collision_info.local_contact_points[0],
            DVec3::new(1.0, 0.0, 0.0)
        );
        assert_eq!(
            collision_info.local_contact_points[1],
            DVec3::new(-1.0, 0.0, 0.0)
        );

        let transform0 =
            Transform::from_rotation(DQuat::from_rotation_y(std::f64::consts::FRAC_PI_4));

        let collision_info = collides_box_box((&box0, &transform0), (&box1, &transform1)).unwrap();
        assert!((collision_info.penetration_depth - (std::f64::consts::SQRT_2 - 0.5)).abs() < 1e-6);
        assert_eq!(collision_info.world_normal, DVec3::new(1.0, 0.0, 0.0));
        assert!(
            collision_info.local_contact_points[0].abs_diff_eq(DVec3::new(1.0, 0.0, 1.0), 1e-6)
        );
        assert_eq!(
            collision_info.local_contact_points[1],
            DVec3::new(-1.0, 0.0, 0.0)
        );
    }
}
