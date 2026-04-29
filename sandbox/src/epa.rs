#![allow(dead_code)]

use std::{cmp::Reverse, collections::BinaryHeap};

use glam::Vec3;

use crate::{gjk::SupportPoint, shape::ConvexShape};

#[derive(Debug)]
pub struct EpaEngine {
    max_iteration: usize,
    vertices: Vec<SupportPoint>,
    faces: Vec<Face>,
    priority_queue: BinaryHeap<Reverse<Entry>>,
    abs_epsilon: f32,
    edges: Vec<AdjacentFace>,
}

impl EpaEngine {
    pub fn penetration_depth<S1: ConvexShape + ?Sized, S2: ConvexShape + ?Sized>(
        &mut self,
        shape1: &S1,
        shape2: &S2,
        tetrahedron: [SupportPoint; 4],
    ) -> Vec3 {
        self.vertices.clear();
        self.faces.clear();
        self.priority_queue.clear();

        self.vertices.extend(tetrahedron);
        self.faces.extend([
            Face::new_init(
                [3, 2, 1],
                [
                    AdjacentFace { index: 1, edge: 1 },
                    AdjacentFace { index: 3, edge: 1 },
                    AdjacentFace { index: 2, edge: 1 },
                ],
                &self.vertices,
            ),
            Face::new_init(
                [0, 2, 3],
                [
                    AdjacentFace { index: 3, edge: 2 },
                    AdjacentFace { index: 0, edge: 0 },
                    AdjacentFace { index: 2, edge: 0 },
                ],
                &self.vertices,
            ),
            Face::new_init(
                [0, 3, 1],
                [
                    AdjacentFace { index: 1, edge: 2 },
                    AdjacentFace { index: 0, edge: 2 },
                    AdjacentFace { index: 3, edge: 0 },
                ],
                &self.vertices,
            ),
            Face::new_init(
                [0, 1, 2],
                [
                    AdjacentFace { index: 2, edge: 2 },
                    AdjacentFace { index: 0, edge: 1 },
                    AdjacentFace { index: 1, edge: 0 },
                ],
                &self.vertices,
            ),
        ]);
        self.priority_queue
            .extend(self.faces.iter().enumerate().filter_map(|(index, face)| {
                face.closest_is_internal().then(|| {
                    Reverse(Entry {
                        face: index,
                        distance: face.distance,
                    })
                })
            }));

        loop {
            let entry = self.priority_queue.pop().unwrap();
            let closest_face = &mut self.faces[entry.0.face];
            if closest_face.obsolete {
                continue;
            }

            let closest_point = closest_face.closest;
            let support_point = SupportPoint::new(shape1, shape2, closest_point);
            if closest_point.dot(support_point.difference) / closest_point.length()
                - closest_point.length()
                <= self.abs_epsilon
            {
                return closest_point;
            }

            closest_face.obsolete = true;

            for adjacent_face in &closest_face.adjacents {
                Self::silhouette(adjacent_face, &support_point, &mut self.edges);
            }

            self.vertices.push(support_point);
            let support_index = self.vertices.len();

            let old_face_count = self.faces.len();
            let mut last_face_index = old_face_count + self.edges.len() - 1;
            for adjacent_face in self.edges.drain(..) {
                let current_face_index = self.faces.len();
                let face = &mut self.faces[adjacent_face.index];
                let adj = &mut face.adjacents[adjacent_face.edge];
                adj.index = current_face_index;
                adj.edge = 0;

                let face = Face::new(
                    [
                        face.vertex_indices[adjacent_face.edge],
                        face.vertex_indices[adjacent_face.edge + 1 % 3],
                        support_index,
                    ],
                    [
                        adjacent_face,
                        AdjacentFace {
                            index: current_face_index + 1,
                            edge: 2,
                        },
                        AdjacentFace {
                            index: last_face_index,
                            edge: 1,
                        },
                    ],
                    &self.vertices,
                );

                if face.closest_is_internal() {
                    self.priority_queue.push(Reverse(Entry {
                        face: current_face_index,
                        distance: face.distance,
                    }));
                }

                self.faces.push(face);
                last_face_index = current_face_index;
            }
            self.faces[last_face_index].adjacents[2].index = old_face_count;
        }
    }

    fn silhouette(
        adjacent_face: &AdjacentFace,
        support_point: &SupportPoint,
        edges: &mut Vec<AdjacentFace>,
    ) {
        todo!()
    }
}

impl Default for EpaEngine {
    fn default() -> Self {
        EpaEngine {
            max_iteration: 100,
            vertices: Vec::with_capacity(104),
            faces: Vec::with_capacity(104),
            priority_queue: BinaryHeap::with_capacity(104),
            abs_epsilon: 1e-6,
            edges: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct AdjacentFace {
    index: usize,
    edge: usize,
}

#[derive(Debug)]
struct Face {
    vertex_indices: [usize; 3],
    closest: Vec3,
    barycentric_coordinates: [f32; 3],
    distance: f32,
    adjacents: [AdjacentFace; 3],
    obsolete: bool,
}

impl Face {
    fn new_init(
        vertex_indices: [usize; 3],
        adjacents: [AdjacentFace; 3],
        vertices: &[SupportPoint],
    ) -> Face {
        Face {
            vertex_indices,
            closest: todo!(),
            barycentric_coordinates: todo!(),
            distance: todo!(),
            adjacents,
            obsolete: false,
        }
    }

    fn new(
        vertex_indices: [usize; 3],
        adjacents: [AdjacentFace; 3],
        vertices: &[SupportPoint],
    ) -> Face {
        todo!()
    }

    fn closest_is_internal(&self) -> bool {
        todo!()
    }
}

#[derive(Debug)]
struct Entry {
    face: usize,
    distance: f32,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Entry {}

impl Ord for Entry {
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

    // #[test]
    // fn test_two_balls() {
    //     let origin = Ball {
    //         center: Vec3::ZERO,
    //         radius: 1.0,
    //     };

    //     let tetrahedron = gjk_tetrahedron(&origin, &origin).unwrap();
    //     let separating_vector = epa(&origin, &origin, tetrahedron);
    //     assert!(
    //         (separating_vector.w - 2.0).abs() <= 0.1,
    //         "Expected 2.0, got {}",
    //         separating_vector.w
    //     );

    //     let x = Ball {
    //         center: Vec3::X,
    //         radius: 0.0,
    //     };

    //     let tetrahedron = gjk_tetrahedron(&x, &x).unwrap();
    //     let separating_vector = epa(&x, &x, tetrahedron);
    //     assert!(
    //         separating_vector.w.abs() <= 0.1,
    //         "Expected 0.0, got {}",
    //         separating_vector.w
    //     );

    //     let tetrahedron = gjk_tetrahedron(&origin, &x).unwrap();
    //     let separating_vector = epa(&origin, &x, tetrahedron);
    //     assert!(
    //         separating_vector.w.abs() <= 0.1,
    //         "Expected 0.0, got {}",
    //         separating_vector.w
    //     );

    //     // let x = Ball {
    //     //     center: Vec3::X,
    //     //     radius: 1.0,
    //     // };
    //     // assert!(gjk(&x, &x));
    //     // let tetrahedron = gjk_tetrahedron(&origin, &x).unwrap();
    //     // dbg!(epa(&origin, &x, tetrahedron));
    //     // assert!(gjk(&origin, &x));

    //     // let three_x = Ball {
    //     //     center: 3.0 * Vec3::X,
    //     //     radius: 1.0,
    //     // };
    //     // assert!(!gjk(&origin, &three_x));
    //     // assert!(gjk(&x, &three_x));

    //     // let random_center = Ball {
    //     //     center: vec3(-1.0312, 0.13312, 1.2),
    //     //     radius: 1.0,
    //     // };
    //     // assert!(gjk(&origin, &random_center));
    //     // assert!(!gjk(&x, &random_center));

    //     // let radius = 2.343;
    //     // let random_radius = Ball {
    //     //     center: Vec3::Y * radius,
    //     //     radius,
    //     // };
    //     // assert!(gjk(&origin, &random_radius));
    //     // assert!(gjk(&x, &random_radius));
    //     // assert!(!gjk(&three_x, &random_radius));
    // }

    // #[test]
    // fn test_ball_axis_aligned_boxes() {
    //     let aab = AxisAlignedBox::from_center_dimension(Vec3::ZERO, 2.0, 2.0, 2.0);

    //     let mut ball = Ball {
    //         center: Vec3::ZERO,
    //         radius: 1.0,
    //     };

    //     let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
    //     let separating_vector = epa(&aab, &ball, tetrahedron);
    //     assert!(
    //         (separating_vector.w - 2.0).abs() <= 1e-4,
    //         "Expected 1.0, got {}",
    //         separating_vector.w
    //     );

    //     ball.center = vec3(1.99, 0.0, 0.0);
    //     let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
    //     let separating_vector = epa(&aab, &ball, tetrahedron);
    //     assert_seperating_vector(ball.center.normalize(), 0.01, separating_vector);

    //     ball.center = vec3(2.0, 0.0, 0.0);
    //     let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
    //     let separating_vector = epa(&aab, &ball, tetrahedron);
    //     assert_seperating_vector(ball.center.normalize(), 0.0, separating_vector);

    //     ball.center = vec3(1.70, 1.70, 0.0);
    //     let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
    //     let separating_vector = epa(&aab, &ball, tetrahedron);
    //     assert_seperating_vector(ball.center.normalize(), 0.0101, separating_vector);

    //     ball.center = vec3(1.57, 1.57, 1.57);
    //     let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
    //     let separating_vector = epa(&aab, &ball, tetrahedron);
    //     assert_seperating_vector(ball.center.normalize(), 0.0127, separating_vector);
    // }

    // fn assert_seperating_vector(expected_direction: Vec3, expected_length: f32, actual: Vec4) {
    //     assert!(
    //         actual.truncate().abs_diff_eq(expected_direction, 1e-2),
    //         "Expected {expected_direction}, got {}",
    //         actual.truncate()
    //     );
    //     assert!(
    //         (actual.w - expected_length).abs() <= 1e-4,
    //         "Expected {expected_length}, got {}",
    //         actual.w
    //     );
    // }
}
