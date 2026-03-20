use glam::Vec3;

use crate::shape::ConvexShape;

pub(crate) fn gjk<S1: ConvexShape + ?Sized, S2: ConvexShape + ?Sized>(shape1: &S1, shape2: &S2) -> bool {
    let direction = shape2.centroid() - shape1.centroid();
    let a = support(shape1, shape2, direction);

    let mut simplex = Simplex::Point([a]);
    let mut direction = -a;

    loop {
        let a = support(shape1, shape2, direction);

        if a.dot(direction) < 0.0 {
            // new point did not pass origin => no collision
            return false;
        }

        simplex.push(a);

        let contains_origin;
        (simplex, direction, contains_origin) = nearest_simplex(simplex);
        if contains_origin {
            return true;
        }
    }
}

fn support<S1: ConvexShape + ?Sized, S2: ConvexShape + ?Sized>(
    shape1: &S1,
    shape2: &S2,
    direction: Vec3,
) -> Vec3 {
    shape1.support_point(direction) - shape2.support_point(-direction)
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

fn nearest_line([b, a]: [Vec3; 2]) -> (Simplex, Vec3, bool) {
    let ab = b - a;
    let ao = -a;
    let direction = ab.cross(ao).cross(ab);
    (Simplex::Line([b, a]), direction, false)
}

fn nearest_triangle([c, b, a]: [Vec3; 3]) -> (Simplex, Vec3, bool) {
    let ab = b - a;
    let ac = c - a;
    let ao = -a;

    let abc = ab.cross(ac);

    let ab_normal = ab.cross(abc);
    if ab_normal.dot(ao) > 0.0 {
        return (Simplex::Line([b, a]), ab_normal, false);
    }

    let ac_normal = abc.cross(ac);
    if ac_normal.dot(ao) > 0.0 {
        return (Simplex::Line([c, a]), ac_normal, false);
    }

    if abc.dot(ac) > 0.0 {
        (Simplex::Triangle([c, b, a]), abc, false)
    } else {
        (Simplex::Triangle([c, b, a]), -abc, false)
    }
}

fn nearest_tetrahedron([d, c, b, a]: [Vec3; 4]) -> (Simplex, Vec3, bool) {
    let ab = b - a;
    let ac = c - a;
    let ad = d - a;
    let ao = -a;

    let mut abc = ab.cross(ac);
    if abc.dot(ad) > 0.0 {
        abc = -abc;
    }
    if abc.dot(ao) > 0.0 {
        return (Simplex::Triangle([c, b, a]), abc, false);
    }

    let mut abd = ab.cross(ad);
    if abd.dot(ac) > 0.0 {
        abd = -abd;
    }
    if abd.dot(ao) > 0.0 {
        return (Simplex::Triangle([d, b, a]), abd, false);
    }

    let mut acd = ac.cross(ad);
    if acd.dot(ab) > 0.0 {
        acd = -acd;
    }
    if acd.dot(ao) > 0.0 {
        return (Simplex::Triangle([d, c, a]), acd, false);
    }

    (Simplex::Tetrahedron([d, c, b, a]), ao, true)
}

enum Simplex {
    Point([Vec3; 1]),
    Line([Vec3; 2]),
    Triangle([Vec3; 3]),
    Tetrahedron([Vec3; 4]),
}

impl Simplex {
    fn push(&mut self, point: Vec3) {
        *self = match *self {
            Simplex::Point([a]) => Simplex::Line([a, point]),
            Simplex::Line([b, a]) => Simplex::Triangle([b, a, point]),
            Simplex::Triangle([c, b, a]) => Simplex::Tetrahedron([c, b, a, point]),
            Simplex::Tetrahedron(_) => unimplemented!(),
        }
    }
}