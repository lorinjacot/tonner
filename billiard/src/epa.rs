use std::{cmp::Reverse, collections::BinaryHeap};

use glam::{Vec3, Vec4};

use crate::{gjk::SupportPoint, shape::ConvexShape};

const MAX_ITERATION: usize = 100;

pub(crate) fn epa<S1: ConvexShape + ?Sized, S2: ConvexShape + ?Sized>(
    shape1: &S1,
    shape2: &S2,
    tetrahedron: [SupportPoint; 4],
) -> Vec4 {
    let mut vertices = Vec::from(tetrahedron);
    let mut faces = BinaryHeap::from(
        [[3, 2, 1], [3, 1, 0], [3, 0, 2], [2, 0, 1]]
            .map(|indices| Reverse(Face::from_vertex_indices(indices, &vertices))),
    );

    let mut unique_edges = Vec::new();
    for _ in 0..MAX_ITERATION {
        let closest_face = &faces.peek().unwrap().0;

        let support = SupportPoint::new(shape1, shape2, closest_face.normal);
        let distance_support = support.difference.dot(closest_face.normal);

        if (distance_support - closest_face.distance).abs() < 1e-4 {
            return closest_face.normal.extend(closest_face.distance);
        }

        // In order to keep the polyhedron convex, we need to remove all faces visible from `support`.
        // This creates an hole whose border is made up of all edges appearing in only one of the removed faces.
        faces.retain(|face| {
            if face.0.normal.dot(support.difference) > 0.0 {
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

    // min_normal.extend(min_distance)
    panic!()
}

#[derive(Debug)]
struct Face {
    indices: [usize; 3],
    normal: Vec3,
    distance: f32,
}

impl Face {
    fn from_vertex_indices(vertex_indices: [usize; 3], vertices: &[SupportPoint]) -> Face {
        let [a, b, c] = vertex_indices.map(|i| vertices[i].difference);
        let mut normal = (b - a).cross(c - a).try_normalize().unwrap_or_else(|| {
            dbg!(vertices);
            todo!("abc is a line or a point")
        });
        let mut distance = a.dot(normal);
        if distance < 0.0 {
            normal = -normal;
            distance = -distance;
        }

        Face {
            indices: vertex_indices,
            normal,
            distance,
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
        // let origin = Ball {
        //     center: Vec3::ZERO,
        //     radius: 1.0,
        // };

        // assert!(gjk(&origin, &origin));

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

        // let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        // dbg!(epa(&aab, &ball, tetrahedron));

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
        assert_seperating_vector(ball.center.normalize(), 0.01, separating_vector);

        // ball.center = vec3(1.57, 1.57, 1.57);
        // let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        // let separating_vector = epa(&aab, &ball, tetrahedron);
        // assert_seperating_vector(ball.center.normalize(), 0.01, separating_vector);
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
