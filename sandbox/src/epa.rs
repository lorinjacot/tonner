#![allow(dead_code)]

use std::{cmp::Reverse, collections::BinaryHeap};

use glam::{Vec3, Vec4};
use log::debug;

use crate::{gjk::SupportPoint, shape::ConvexShape};

const MAX_ITERATION: usize = 100;

pub struct EpaResult {
    pub direction: Vec3,
    pub distance: f32,
    pub vertices: Vec<SupportPoint>,
    pub faces: BinaryHeap<Reverse<Face>>,
}

pub(crate) fn epa_dbg<S1: ConvexShape + ?Sized, S2: ConvexShape + ?Sized>(
    shape1: &S1,
    shape2: &S2,
    tetrahedron: [SupportPoint; 4],
    steps: usize,
) -> EpaResult {
    let mut vertices: Vec<SupportPoint> = Vec::with_capacity(4 + steps);
    tetrahedron.into_iter().for_each(|support_point| {
        if vertices
            .iter()
            .find(|v| v.difference.abs_diff_eq(support_point.difference, 1e-4))
            .is_none()
        {
            vertices.push(support_point);
        }
    });

    let mut faces = if vertices.len() == 4 {
        BinaryHeap::from(
            [[0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 3, 2]]
                .map(|indices| Reverse(Face::from_vertex_indices(indices, &vertices))),
        )
    } else if vertices.len() == 3 {
        BinaryHeap::from(
            [[0, 1, 2], [0, 2, 1]]
                .map(|indicies| Reverse(Face::from_vertex_indices(indicies, &vertices))),
        )
    } else {
        todo!()
    };

    let mut unique_edges = Vec::new();
    for _ in 0..steps {
        let closest_face = faces.pop().unwrap().0;

        let support = SupportPoint::new(shape1, shape2, closest_face.normal);
        let distance_support = support.difference.dot(closest_face.normal);

        if (distance_support - closest_face.distance).abs() < 1e-6 {
            break;
        }
        unique_edges.extend(closest_face.edges());

        // In order to keep the polyhedron convex, we need to remove all faces visible from `support`.
        // This creates a hole whose border is made up of all edges appearing in only one of the removed faces.
        faces.retain(|face| {
            let point_on_face = vertices[face.0.indices[0]].difference;
            if face.0.normal.dot(support.difference - point_on_face) > 0.0 {
                face.0.edges().into_iter().for_each(|(i, j)| {
                    match unique_edges
                        .iter()
                        .enumerate()
                        .find(|(_, edge)| **edge == (j, i))
                    {
                        Some((i, _)) => {
                            unique_edges.swap_remove(i);
                        }
                        None => unique_edges.push((i, j)),
                    }
                });

                false
            } else {
                true
            }
        });

        let k = vertices.len();
        vertices.push(support);
        faces.extend(
            unique_edges
                .drain(..)
                .map(|(i, j)| Reverse(Face::from_vertex_indices([i, j, k], &vertices))),
        );
    }

    let closest_face = &faces.peek().unwrap().0;

    EpaResult {
        direction: closest_face.normal,
        distance: closest_face.distance,
        vertices,
        faces,
    }
}

pub(crate) fn epa<S1: ConvexShape + ?Sized, S2: ConvexShape + ?Sized>(
    shape1: &S1,
    shape2: &S2,
    tetrahedron: [SupportPoint; 4],
) -> Vec4 {
    let res = epa_dbg(shape1, shape2, tetrahedron, MAX_ITERATION);

    res.direction.extend(res.distance)
}

#[derive(Debug)]
pub struct Face {
    pub indices: [usize; 3],
    pub normal: Vec3,
    pub distance: f32,
}

impl Face {
    fn from_vertex_indices(vertex_indices: [usize; 3], vertices: &[SupportPoint]) -> Face {
        let [a, b, c] = dbg!(vertex_indices.map(|i| vertices[i].difference));
        let normal = (b - a).cross(c - a).try_normalize().unwrap_or_else(|| {
            // handle degenerate triangles
            if a.abs_diff_eq(b, 1e-4) {
                if a.abs_diff_eq(c, 1e-4) {
                    let normal = a.normalize_or(Vec3::X);
                    debug!("degenerate face: Point({a}) => {normal}");
                    normal
                } else {
                    let ac = c - a;
                    let normal = ac
                        .cross(Vec3::X)
                        .normalize_or(ac.cross(Vec3::Y).normalize());
                    debug!("degenerate face: line({a},{c}) => {normal}");
                    normal
                }
            } else {
                let ab = b - a;
                let normal = ab
                    .cross(Vec3::X)
                    .normalize_or(ab.cross(Vec3::Y).normalize());
                debug!("degenerate face: line({a},{b}) => {normal}");
                normal
            }
        });
        let distance = a.dot(normal);
        if distance < 0.0 {
            dbg!(Face {
                indices: vertex_indices,
                normal: -normal,
                distance: -distance,
            })
        } else {
            dbg!(Face {
                indices: vertex_indices,
                normal,
                distance,
            })
        }
    }

    fn edges(&self) -> [(usize, usize); 3] {
        [(0, 1), (1, 2), (2, 0)].map(|(i, j)| (self.indices[i], self.indices[j]))
    }
}

impl PartialEq for Face {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl PartialOrd for Face {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Face {}

impl Ord for Face {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.distance.total_cmp(&other.distance)
    }
}

#[cfg(test)]
mod tests {
    use glam::vec3;

    use crate::{
        gjk::gjk_tetrahedron,
        shape::{AxisAlignedBox, Ball},
    };

    use super::*;

    #[test]
    fn test_two_balls() {
        let origin = Ball {
            center: Vec3::ZERO,
            radius: 1.0,
        };

        let tetrahedron = gjk_tetrahedron(&origin, &origin).unwrap();
        let separating_vector = epa(&origin, &origin, tetrahedron);
        assert!(
            (separating_vector.w - 2.0).abs() <= 1e-4,
            "Expected 2.0, got {}",
            separating_vector.w
        );

        // let x = Ball {
        //     center: Vec3::X,
        //     radius: 1.0,
        // };
        // assert!(gjk(&x, &x));
        // let tetrahedron = gjk_tetrahedron(&origin, &x).unwrap();
        // dbg!(epa(&origin, &x, tetrahedron));
        // assert!(gjk(&origin, &x));

        // let three_x = Ball {
        //     center: 3.0 * Vec3::X,
        //     radius: 1.0,
        // };
        // assert!(!gjk(&origin, &three_x));
        // assert!(gjk(&x, &three_x));

        // let random_center = Ball {
        //     center: vec3(-1.0312, 0.13312, 1.2),
        //     radius: 1.0,
        // };
        // assert!(gjk(&origin, &random_center));
        // assert!(!gjk(&x, &random_center));

        // let radius = 2.343;
        // let random_radius = Ball {
        //     center: Vec3::Y * radius,
        //     radius,
        // };
        // assert!(gjk(&origin, &random_radius));
        // assert!(gjk(&x, &random_radius));
        // assert!(!gjk(&three_x, &random_radius));
    }

    #[test]
    fn test_ball_axis_aligned_boxes() {
        let aab = AxisAlignedBox::from_center_dimension(Vec3::ZERO, 2.0, 2.0, 2.0);

        let mut ball = Ball {
            center: Vec3::ZERO,
            radius: 1.0,
        };

        let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        let separating_vector = epa(&aab, &ball, tetrahedron);
        assert!(
            (separating_vector.w - 2.0).abs() <= 1e-4,
            "Expected 1.0, got {}",
            separating_vector.w
        );

        ball.center = vec3(1.99, 0.0, 0.0);
        let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        let separating_vector = epa(&aab, &ball, tetrahedron);
        assert_seperating_vector(ball.center.normalize(), 0.01, separating_vector);

        ball.center = vec3(2.0, 0.0, 0.0);
        let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        let separating_vector = epa(&aab, &ball, tetrahedron);
        assert_seperating_vector(ball.center.normalize(), 0.0, separating_vector);

        ball.center = vec3(1.70, 1.70, 0.0);
        let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        let separating_vector = epa(&aab, &ball, tetrahedron);
        assert_seperating_vector(ball.center.normalize(), 0.0101, separating_vector);

        ball.center = vec3(1.57, 1.57, 1.57);
        let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        let separating_vector = epa(&aab, &ball, tetrahedron);
        assert_seperating_vector(ball.center.normalize(), 0.0127, separating_vector);
    }

    fn assert_seperating_vector(expected_direction: Vec3, expected_length: f32, actual: Vec4) {
        assert!(
            actual.truncate().abs_diff_eq(expected_direction, 1e-2),
            "Expected {expected_direction}, got {}",
            actual.truncate()
        );
        assert!(
            (actual.w - expected_length).abs() <= 1e-4,
            "Expected {expected_length}, got {}",
            actual.w
        );
    }
}
