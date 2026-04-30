#![allow(dead_code)]

use std::{cmp::Reverse, collections::BinaryHeap};

use glam::{Vec3, vec3};

use crate::{gjk::SupportPoint, shape::ConvexShape};

#[derive(Debug)]
pub struct EpaEngine {
    max_iteration: usize,
    vertices: Vec<SupportPoint>,
    faces: Vec<Face>,
    priority_queue: BinaryHeap<Reverse<Entry>>,
    edges: Vec<AdjacentFace>,
    abs_epsilon: f32,
}

#[derive(Debug)]
pub struct EpaState<'a> {
    pub vertices: &'a mut Vec<SupportPoint>,
    pub faces: &'a mut Vec<Face>,
    edges: &'a mut Vec<AdjacentFace>,
    priority_queue: &'a mut BinaryHeap<Reverse<Entry>>,
}

impl<'a> EpaState<'a> {
    fn init(
        tetrahedron: [SupportPoint; 4],
        vertices: &'a mut Vec<SupportPoint>,
        faces: &'a mut Vec<Face>,
        edges: &'a mut Vec<AdjacentFace>,
        priority_queue: &'a mut BinaryHeap<Reverse<Entry>>,
    ) -> Self {
        vertices.clear();
        faces.clear();
        priority_queue.clear();

        vertices.extend(tetrahedron);
        faces.extend([
            Face::new(
                [3, 2, 1],
                [
                    AdjacentFace { index: 1, edge: 1 },
                    AdjacentFace { index: 3, edge: 1 },
                    AdjacentFace { index: 2, edge: 1 },
                ],
                &vertices,
            ),
            Face::new(
                [0, 2, 3],
                [
                    AdjacentFace { index: 3, edge: 2 },
                    AdjacentFace { index: 0, edge: 0 },
                    AdjacentFace { index: 2, edge: 0 },
                ],
                &vertices,
            ),
            Face::new(
                [0, 3, 1],
                [
                    AdjacentFace { index: 1, edge: 2 },
                    AdjacentFace { index: 0, edge: 2 },
                    AdjacentFace { index: 3, edge: 0 },
                ],
                &vertices,
            ),
            Face::new(
                [0, 1, 2],
                [
                    AdjacentFace { index: 2, edge: 2 },
                    AdjacentFace { index: 0, edge: 1 },
                    AdjacentFace { index: 1, edge: 0 },
                ],
                &vertices,
            ),
        ]);
        priority_queue.extend(faces.iter().enumerate().filter_map(|(index, face)| {
            face.closest_is_internal().then(|| {
                Reverse(Entry {
                    face: index,
                    distance: face.distance,
                })
            })
        }));

        EpaState {
            vertices,
            faces,
            edges,
            priority_queue,
        }
    }
}

impl EpaEngine {
    pub fn penetration_depth_details<S1: ConvexShape + ?Sized, S2: ConvexShape + ?Sized>(
        &mut self,
        shape1: &S1,
        shape2: &S2,
        tetrahedron: [SupportPoint; 4],
        steps: usize,
    ) -> EpaState<'_> {
        let mut state = EpaState::init(
            tetrahedron,
            &mut self.vertices,
            &mut self.faces,
            &mut self.edges,
            &mut self.priority_queue,
        );

        for _ in 0..steps {
            let entry = state.priority_queue.pop().unwrap();
            let closest_face = &mut state.faces[entry.0.face];
            if closest_face.obsolete {
                continue;
            }

            let closest_point = closest_face.closest;
            let support_point = SupportPoint::new(shape1, shape2, closest_point);
            if closest_point.dot(support_point.difference) / closest_point.length()
                - closest_point.length()
                <= self.abs_epsilon
            {
                break;
            }

            closest_face.obsolete = true;

            for adjacent_face in &closest_face.adjacents.clone() {
                silhouette(
                    adjacent_face,
                    &support_point,
                    &mut state.edges,
                    &mut state.faces,
                );
            }

            let support_index = state.vertices.len();
            state.vertices.push(support_point);

            let old_face_count = state.faces.len();
            let mut last_face_index = old_face_count + state.edges.len() - 1;
            for adjacent_face in state.edges.drain(..) {
                let current_face_index = state.faces.len();
                let face = &mut state.faces[adjacent_face.index];
                let adj = &mut face.adjacents[adjacent_face.edge];
                adj.index = current_face_index;
                adj.edge = 0;

                let face = Face::new(
                    [
                        face.vertex_indices[(adjacent_face.edge + 1) % 3],
                        face.vertex_indices[adjacent_face.edge],
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
                    &state.vertices,
                );

                if face.closest_is_internal() {
                    state.priority_queue.push(Reverse(Entry {
                        face: current_face_index,
                        distance: face.distance,
                    }));
                }

                state.faces.push(face);
                last_face_index = current_face_index;
            }
            state.faces[last_face_index].adjacents[1].index = old_face_count;
        }

        return state;
    }

    pub fn penetration_depth<S1: ConvexShape + ?Sized, S2: ConvexShape + ?Sized>(
        &mut self,
        shape1: &S1,
        shape2: &S2,
        tetrahedron: [SupportPoint; 4],
    ) -> (Vec3, f32) {
        self.vertices.clear();
        self.faces.clear();
        self.priority_queue.clear();

        self.vertices.extend(tetrahedron);
        self.faces.extend([
            Face::new(
                [3, 2, 1],
                [
                    AdjacentFace { index: 1, edge: 1 },
                    AdjacentFace { index: 3, edge: 1 },
                    AdjacentFace { index: 2, edge: 1 },
                ],
                &self.vertices,
            ),
            Face::new(
                [0, 2, 3],
                [
                    AdjacentFace { index: 3, edge: 2 },
                    AdjacentFace { index: 0, edge: 0 },
                    AdjacentFace { index: 2, edge: 0 },
                ],
                &self.vertices,
            ),
            Face::new(
                [0, 3, 1],
                [
                    AdjacentFace { index: 1, edge: 2 },
                    AdjacentFace { index: 0, edge: 2 },
                    AdjacentFace { index: 3, edge: 0 },
                ],
                &self.vertices,
            ),
            Face::new(
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

        let mut closest_point = Vec3::ZERO;
        for _ in 0..self.max_iteration {
            let entry = self.priority_queue.pop().unwrap();
            let closest_face = &mut self.faces[entry.0.face];
            if closest_face.obsolete {
                continue;
            }

            closest_point = closest_face.closest;
            let support_point = SupportPoint::new(shape1, shape2, closest_point);
            if closest_point.dot(support_point.difference) / closest_point.length()
                - closest_point.length()
                <= self.abs_epsilon
            {
                break;
            }

            closest_face.obsolete = true;

            for adjacent_face in &closest_face.adjacents.clone() {
                silhouette(
                    adjacent_face,
                    &support_point,
                    &mut self.edges,
                    &mut self.faces,
                );
            }

            let support_index = self.vertices.len();
            self.vertices.push(support_point);

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
                        face.vertex_indices[(adjacent_face.edge + 1) % 3],
                        face.vertex_indices[adjacent_face.edge],
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
            self.faces[last_face_index].adjacents[1].index = old_face_count;
        }

        return closest_point.normalize_and_length();
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

fn silhouette(
    adjacent_face: &AdjacentFace,
    support_point: &SupportPoint,
    edges: &mut Vec<AdjacentFace>,
    faces: &mut [Face],
) {
    let face = &mut faces[adjacent_face.index];
    if face.obsolete {
        return;
    }

    if face.closest.dot(support_point.difference) < face.closest.length_squared() {
        // face not visible from  `support_point`
        edges.push(*adjacent_face);
    } else {
        face.obsolete = true;

        [
            face.adjacents[(adjacent_face.edge + 1) % 3],
            face.adjacents[(adjacent_face.edge + 2) % 3],
        ]
        .iter()
        .for_each(|neighbor| {
            silhouette(neighbor, support_point, edges, faces);
        });
    }
}

#[derive(Debug, Clone, Copy)]
struct AdjacentFace {
    index: usize,
    edge: usize,
}

#[derive(Debug)]
pub struct Face {
    pub vertex_indices: [usize; 3],
    pub closest: Vec3,
    barycentric_coordinates: Vec3,
    distance: f32,
    adjacents: [AdjacentFace; 3],
    pub obsolete: bool,
}

impl Face {
    fn new(
        vertex_indices: [usize; 3],
        adjacents: [AdjacentFace; 3],
        vertices: &[SupportPoint],
    ) -> Face {
        let [p0, p1, p2] = vertex_indices.map(|idx| vertices[idx].difference);

        let d01_0 = (p1 - p0).dot(p1);
        let d01_1 = -(p1 - p0).dot(p0);

        let d02_0 = (p2 - p0).dot(p2);
        let d02_2 = -(p2 - p0).dot(p0);

        let d12_1 = (p2 - p1).dot(p2);
        let d12_2 = -(p2 - p1).dot(p1);

        let delta = vec3(
            d01_0 * d12_1 + d12_2 * (p1 - p0).dot(p2),
            d01_1 * d02_0 - d02_2 * (p1 - p0).dot(p2),
            d02_2 * d01_0 - d01_1 * (p2 - p0).dot(p1),
        );
        let denominator = delta.element_sum();
        let lambda = delta / denominator;

        let closest = lambda.x * p0 + lambda.y * p1 + lambda.z * p2;
        let distance = closest.length();

        Face {
            vertex_indices,
            closest,
            barycentric_coordinates: lambda,
            distance,
            adjacents,
            obsolete: false,
        }
    }

    fn closest_is_internal(&self) -> bool {
        self.barycentric_coordinates.cmpge(Vec3::ZERO).all()
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
    //     let result = epa(&origin, &origin, tetrahedron);
    //     assert!(
    //         (result.w - 2.0).abs() <= 0.1,
    //         "Expected 2.0, got {}",
    //         result.w
    //     );

    //     let x = Ball {
    //         center: Vec3::X,
    //         radius: 0.0,
    //     };

    //     let tetrahedron = gjk_tetrahedron(&x, &x).unwrap();
    //     let result = epa(&x, &x, tetrahedron);
    //     assert!(
    //         result.w.abs() <= 0.1,
    //         "Expected 0.0, got {}",
    //         result.w
    //     );

    //     let tetrahedron = gjk_tetrahedron(&origin, &x).unwrap();
    //     let result = epa(&origin, &x, tetrahedron);
    //     assert!(
    //         result.w.abs() <= 0.1,
    //         "Expected 0.0, got {}",
    //         result.w
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

    #[test]
    fn test_ball_axis_aligned_boxes() {
        let mut engine = EpaEngine::default();

        let aab = AxisAlignedBox::from_center_dimension(Vec3::ZERO, 2.0, 2.0, 2.0);

        let mut ball = Ball {
            center: Vec3::ZERO,
            radius: 1.0,
        };

        // let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        // let (_, distance) = engine.penetration_depth(&aab, &ball, tetrahedron);
        // assert!(
        //     (distance - 2.0).abs() <= 1e-4,
        //     "Expected 1.0, got {distance}",
        // );

        ball.center = vec3(1.99, 0.0, 0.0);
        let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        let result = engine.penetration_depth(&aab, &ball, tetrahedron);
        assert_seperating_vector(ball.center.normalize(), 0.01, result);

        ball.center = vec3(2.0, 0.0, 0.0);
        let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        let result = engine.penetration_depth(&aab, &ball, tetrahedron);
        assert_seperating_vector(ball.center.normalize(), 0.0, result);

        ball.center = vec3(1.70, 1.70, 0.0);
        let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        let result = engine.penetration_depth(&aab, &ball, tetrahedron);
        assert_seperating_vector(ball.center.normalize(), 0.0101, result);

        ball.center = vec3(1.57, 1.57, 1.57);
        let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        let result = engine.penetration_depth(&aab, &ball, tetrahedron);
        assert_seperating_vector(ball.center.normalize(), 0.0127, result);
    }

    fn assert_seperating_vector(
        expected_direction: Vec3,
        expected_distance: f32,
        (actual_direction, actual_distance): (Vec3, f32),
    ) {
        assert!(
            actual_direction.abs_diff_eq(expected_direction, 1e-2),
            "Expected {expected_direction}, got {}",
            actual_direction
        );
        assert!(
            (actual_distance - expected_distance).abs() <= 1e-4,
            "Expected {expected_distance}, got {actual_distance}",
        );
    }
}
