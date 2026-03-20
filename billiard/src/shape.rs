use glam::{Vec3, vec3};

use crate::ball;

#[derive(Debug, Clone, Copy)]
pub struct AxisAlignedBox {
    min: Vec3,
    max: Vec3,
}

impl Shape for AxisAlignedBox {
    fn bounding_box(&self) -> AxisAlignedBox {
        *self
    }

    fn centroid(&self) -> Vec3 {
        self.max.midpoint(self.max)
    }
}

impl ConvexShape for AxisAlignedBox {
    fn support_point(&self, direction: Vec3) -> Vec3 {
        vec3(
            if direction.x.is_sign_positive() {
                self.max.x
            } else {
                self.min.x
            },
            if direction.y.is_sign_positive() {
                self.max.y
            } else {
                self.min.y
            },
            if direction.z.is_sign_positive() {
                self.max.z
            } else {
                self.min.z
            },
        )
    }
}

pub trait Shape {
    /// Smallest axis-aligned box containing the shape.
    fn bounding_box(&self) -> AxisAlignedBox;

    /// Geometric center of the shape (arithmetic mean position of all points the shape).
    fn centroid(&self) -> Vec3;
}

/// A convex shape is a region of space where for all pair of points,
/// the segment between them is entirely inside the shape.
pub trait ConvexShape: Shape {
    /// The point on shape which has the highest dot product with `direction`.
    ///
    /// This corresponds to furthest point in the given direction that is still on the shape.
    /// In general this point is not unique.
    ///
    /// `direction` can have any stricly positive length.
    fn support_point(&self, direction: Vec3) -> Vec3;

    fn collides(&self, other: &impl ConvexShape) -> bool {
        let mut direction = other.centroid() - self.centroid();

        let a = self.support_point(direction) - other.support_point(-direction);

        let mut simplex = Simplex::Point(a);
        direction = -a;

        loop {
            let a = self.support_point(direction) - other.support_point(-direction);
            if a.dot(direction) < 0.0 {
                return false;
            }

            simplex.push(a);

            match simplex {
                Simplex::Line(b, a) => {
                    let ab = b - a;
                    let ao = -a;
                    let normal = ab.cross(ao).cross(ab);
                    direction = ab.cross(ao).cross(ab)
                }
                Simplex::Triangle(c, b, a) => {
                    let ab = b - a;
                    let ac = c - a;
                    let ao = -a;
                    let ab_normal = ac.cross(ab).cross(ab);
                    let ac_normal = ab.cross(ac).cross(ac);
                    if ab_normal.dot(ao) > 0.0 {
                        simplex = Simplex::Line(b, a);
                        direction = ab_normal;
                    } else if ac_normal.dot(ao) > 0.0 {
                        simplex = Simplex::Line(c, a);
                        direction = ac_normal;
                    } else {
                        return true;
                    }
                }
                Simplex::Tetrahedron(d, c, b, a) => {
                    
                    let ao = -a;
                }
                Simplex::Point(_) => unreachable!()
            }
        }
    }
}

enum Simplex {
    Point(Vec3),
    Line(Vec3, Vec3),
    Triangle(Vec3, Vec3, Vec3),
    Tetrahedron(Vec3, Vec3, Vec3, Vec3),
}

impl Simplex {
    fn push(&mut self, point: Vec3) {
        *self = match *self {
            Simplex::Point(a) => Simplex::Line(a, point),
            Simplex::Line(a, b) => Simplex::Triangle(a, b, point),
            Simplex::Triangle(a, b, c) => Simplex::Tetrahedron(a, b, c, point),
            Simplex::Tetrahedron(_, _, _, _) => unimplemented!(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Ball {
    center: Vec3,
    radius: f32,
}

impl Shape for Ball {
    fn bounding_box(&self) -> AxisAlignedBox {
        AxisAlignedBox {
            min: self.center - self.radius,
            max: self.center + self.radius,
        }
    }

    fn centroid(&self) -> Vec3 {
        self.center
    }
}

impl ConvexShape for Ball {
    fn support_point(&self, direction: Vec3) -> Vec3 {
        self.center + direction.normalize() * self.radius
    }
}
