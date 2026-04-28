#![allow(dead_code)]

use glam::Vec3;

use crate::shape::ConvexShape;

pub(crate) fn gjk<S1: ConvexShape + ?Sized, S2: ConvexShape + ?Sized>(
    shape1: &S1,
    shape2: &S2,
) -> bool {
    gjk_tetrahedron(shape1, shape2).is_some()
}

pub(crate) fn gjk_tetrahedron<S1: ConvexShape + ?Sized, S2: ConvexShape + ?Sized>(
    shape1: &S1,
    shape2: &S2,
) -> Option<[SupportPoint; 4]> {
    let center1 = shape1.centroid();
    let center2 = shape2.centroid();
    let mut direction = if center1.abs_diff_eq(center2, 1e-4) {
        Vec3::X
    } else {
        center2 - center1
    };
    let a = SupportPoint::new(shape1, shape2, direction);

    direction = -a.difference;
    let mut simplex = Simplex::Point([a]);

    loop {
        if direction.abs_diff_eq(Vec3::ZERO, 1e-4) {
            direction = Vec3::X;
        }
        let a = SupportPoint::new(shape1, shape2, direction);

        if a.difference.dot(direction) < 0.0 {
            return None;
        }

        simplex = simplex.with(a);

        let contains_origin;
        (simplex, direction, contains_origin) = nearest_simplex(simplex);
        if contains_origin {
            if let Simplex::Tetrahedron(vertices) = simplex {
                return Some(vertices);
            } else {
                unreachable!();
            }
        }
    }
}

/// Returns:
/// - the simplex on `simplex` closest to the origin
/// - the direction toward the origin normal to the new simplex
/// - `true` iff `simplex` contains the origin.
fn nearest_simplex(simplex: Simplex) -> (Simplex, Vec3, bool) {
    match simplex {
        Simplex::Line(points) => nearest_line(points),
        Simplex::Triangle(points) => nearest_triangle(points),
        Simplex::Tetrahedron(points) => nearest_tetrahedron(points),
        Simplex::Point(_) => unreachable!(),
    }
}

fn nearest_line([b, a]: [SupportPoint; 2]) -> (Simplex, Vec3, bool) {
    let ab = b.difference - a.difference;
    let ao = -a.difference;

    let ab_normal = ab.cross(ao).cross(ab);
    (Simplex::Line([b, a]), ab_normal, false)
}

fn nearest_triangle([c, b, a]: [SupportPoint; 3]) -> (Simplex, Vec3, bool) {
    let ab = b.difference - a.difference;
    let ac = c.difference - a.difference;
    let ao = -a.difference;

    let abc_normal = ab.cross(ac);

    let ab_normal = ab.cross(abc_normal);
    if ab_normal.dot(ao) > 0.0 {
        return nearest_line([b, a]);
    }

    let ac_normal = abc_normal.cross(ac);
    if ac_normal.dot(ao) > 0.0 {
        return nearest_line([c, a]);
    }

    if abc_normal.dot(ao) > 0.0 {
        (Simplex::Triangle([c, b, a]), abc_normal, false)
    } else {
        (Simplex::Triangle([b, c, a]), -abc_normal, false)
    }
}

fn nearest_tetrahedron([d, c, b, a]: [SupportPoint; 4]) -> (Simplex, Vec3, bool) {
    let ab = b.difference - a.difference;
    let ac = c.difference - a.difference;
    let ad = d.difference - a.difference;
    let ao = -a.difference;

    let abc_normal = ab.cross(ac);
    if abc_normal.dot(ao) > 0.0 {
        return nearest_triangle([c, b, a]);
    }

    let acd_normal = ac.cross(ad);
    if acd_normal.dot(ao) > 0.0 {
        return nearest_triangle([d, c, a]);
    }

    let adb_normal = ad.cross(ab);
    if adb_normal.dot(ao) > 0.0 {
        return nearest_triangle([b, d, a]);
    }

    (Simplex::Tetrahedron([d, c, b, a]), ao, true)
}

#[derive(Debug)]
enum Simplex {
    Point([SupportPoint; 1]),
    Line([SupportPoint; 2]),
    Triangle([SupportPoint; 3]),
    Tetrahedron([SupportPoint; 4]),
}

impl Simplex {
    fn with(self, a: SupportPoint) -> Simplex {
        match self {
            Simplex::Point([b]) => Simplex::Line([b, a]),
            Simplex::Line([c, b]) => Simplex::Triangle([c, b, a]),
            Simplex::Triangle([d, c, b]) => Simplex::Tetrahedron([d, c, b, a]),
            Simplex::Tetrahedron(_) => unimplemented!(),
        }
    }
}

/// 3d point living on the border of the Minkowsi difference.
#[derive(Debug, Clone)]
pub(crate) struct SupportPoint {
    pub(crate) point1: Vec3,
    pub(crate) point2: Vec3,
    pub(crate) difference: Vec3,
}

impl SupportPoint {
    pub fn new<S1: ConvexShape + ?Sized, S2: ConvexShape + ?Sized>(
        shape1: &S1,
        shape2: &S2,
        direction: Vec3,
    ) -> SupportPoint {
        let point1 = shape1.support_point(direction);
        let point2 = shape2.support_point(-direction);
        let difference = point1 - point2;
        SupportPoint {
            point1,
            point2,
            difference,
        }
    }
}

#[cfg(test)]
mod tests {
    use glam::vec3;

    use crate::shape::{AxisAlignedBox, Ball};

    use super::*;

    #[test]
    fn test_two_balls() {
        let origin = Ball {
            center: Vec3::ZERO,
            radius: 1.0,
        };

        assert!(gjk(&origin, &origin));

        let x = Ball {
            center: Vec3::X,
            radius: 1.0,
        };
        assert!(gjk(&x, &x));
        assert!(gjk(&origin, &x));

        let three_x = Ball {
            center: 3.0 * Vec3::X,
            radius: 1.0,
        };
        assert!(!gjk(&origin, &three_x));
        assert!(gjk(&x, &three_x));

        let random_center = Ball {
            center: vec3(-1.0312, 0.13312, 1.2),
            radius: 1.0,
        };
        assert!(gjk(&origin, &random_center));
        assert!(!gjk(&x, &random_center));

        let radius = 2.343;
        let random_radius = Ball {
            center: Vec3::Y * radius,
            radius,
        };
        assert!(gjk(&origin, &random_radius));
        assert!(gjk(&x, &random_radius));
        assert!(!gjk(&three_x, &random_radius));
    }

    #[test]
    fn test_two_axis_aligned_boxes() {
        let origin = AxisAlignedBox::from_center_dimension(Vec3::ZERO, 1.0, 1.0, 1.0);
        assert!(gjk(&origin, &origin));

        let top = AxisAlignedBox::from_center_dimension(0.5 * Vec3::Y, 1.0, 1.0, 1.0);
        assert!(gjk(&top, &origin));

        let top = AxisAlignedBox::from_center_dimension(Vec3::Y, 1.0, 1.0, 1.0);
        assert!(gjk(&top, &origin));

        let top = AxisAlignedBox::from_center_dimension(1.5 * Vec3::Y, 1.0, 1.0, 1.0);
        assert!(!gjk(&top, &origin));

        let range = [-1.5, -1.234, -1.0, -0.789, 0.0, 0.543, 1.0, 1.432, 1.5];
        for x in range {
            for y in range {
                for z in range {
                    let other = AxisAlignedBox::from_center_dimension(vec3(x, y, z), 1.0, 1.0, 1.0);

                    let collision_expected = x.abs() <= 1.0 && y.abs() <= 1.0 && z.abs() <= 1.0;
                    assert_eq!(collision_expected, gjk(&origin, &other));
                }
            }
        }
    }

    #[test]
    fn test_ball_axis_aligned_boxes() {
        let aab = AxisAlignedBox::from_center_dimension(Vec3::ZERO, 2.0, 2.0, 2.0);

        let mut ball = Ball {
            center: Vec3::ZERO,
            radius: 1.0,
        };
        assert!(gjk(&aab, &ball));

        ball.center = vec3(1.99, 0.0, 0.0);
        assert!(gjk(&aab, &ball));
        ball.center = vec3(2.0, 0.0, 0.0);
        assert!(gjk(&aab, &ball));
        ball.center = vec3(2.01, 0.0, 0.0);
        assert!(!gjk(&aab, &ball));

        ball.center = vec3(1.70, 1.70, 0.0);
        assert!(gjk(&aab, &ball));
        ball.center = vec3(1.71, 1.71, 0.0);
        assert!(!gjk(&aab, &ball));

        ball.center = vec3(1.57, 1.57, 1.57);
        assert!(gjk(&aab, &ball));
        ball.center = vec3(1.58, 1.58, 1.58);
        assert!(!gjk(&aab, &ball));
    }

    #[test]
    fn test_edge_cases() {
        // Minkowski point `a` exactly on origin
        let a = Ball {
            center: Vec3::X,
            radius: 1.0,
        };
        let b = Ball {
            center: 3.0 * Vec3::X,
            radius: 1.0,
        };
        assert!(gjk(&a, &b));
    }
}
