#![allow(dead_code)]

use std::{cmp::Reverse, collections::BinaryHeap, f32::consts::FRAC_PI_3};

use glam::{Mat3, Vec3};
use log::debug;

use crate::{gjk::SupportPoint, shape::ConvexShape};

#[derive(Debug)]
pub struct EpaEngine {
    max_iteration: usize,
    relative_tolerance: f32,
    tolerance_factor: f32,
    state: EpaState,
}

#[derive(Debug)]
pub struct EpaState {
    pub vertices: Vec<SupportPoint>,
    pub faces: Vec<Face>,
    priority_queue: BinaryHeap<Reverse<Entry>>,
    edges: Vec<AdjacentFace>,
    upper_bound: f32,
    pub closest_point: Vec3,
}

impl EpaState {
    fn reset(&mut self) {
        self.vertices.clear();
        self.faces.clear();
        self.priority_queue.clear();
        self.edges.clear();
        self.upper_bound = f32::INFINITY;
        self.closest_point = Vec3::ZERO;
    }
}

impl EpaEngine {
    pub fn penetration_depth_details<S1: ConvexShape + ?Sized, S2: ConvexShape + ?Sized>(
        &mut self,
        shape1: &S1,
        shape2: &S2,
        tetrahedron: [SupportPoint; 4],
        steps: usize,
    ) -> &EpaState {
        let s = &mut self.state;
        s.reset();

        for support_point in tetrahedron.into_iter() {
            if s.vertices
                .iter()
                .find(|v| {
                    v.difference
                        .abs_diff_eq(support_point.difference, f32::EPSILON)
                })
                .is_none()
            {
                s.vertices.push(support_point);
            }
        }
        if dbg!(s.vertices.len()) == 1 {
            // GJK returned a point -> this point must be the origin
            s.closest_point = Vec3::ZERO;
            return s;
        } else if s.vertices.len() == 2 {
            // GJK returned a line -> construct a hexahedron

            let a = s.vertices[0].difference;
            let b = s.vertices[1].difference;

            let dir = b - a;

            let mut axis = Vec3::X;
            let mut max = dir.x;
            if dir.y > max {
                max = dir.y;
                axis = Vec3::Y;
            }
            if dir.z > max {
                axis = Vec3::Z;
            }
            let v1 = dir.cross(axis);
            let r = Mat3::from_axis_angle(dir.normalize(), 2.0 * FRAC_PI_3);
            let v2 = r * v1;
            let v3 = r * v2;

            let c = SupportPoint::new(shape1, shape2, v1);
            let d = SupportPoint::new(shape1, shape2, v2);
            let e = SupportPoint::new(shape1, shape2, v3);

            s.vertices.extend([c, d, e]);

            #[rustfmt::skip]
            let faces = [
                Face::new([0, 3, 2], [
                    AdjacentFace { index: 1, edge: 2, },
                    AdjacentFace { index: 3, edge: 1, },
                    AdjacentFace { index: 2, edge: 0, },
                ], &s.vertices),
                Face::new([0, 4, 3], [
                    AdjacentFace { index: 2, edge: 2, },
                    AdjacentFace { index: 4, edge: 1, },
                    AdjacentFace { index: 0, edge: 0, },
                ], &s.vertices),
                Face::new([0, 2, 4], [
                    AdjacentFace { index: 0, edge: 2, },
                    AdjacentFace { index: 5, edge: 1, },
                    AdjacentFace { index: 1, edge: 0, },
                ], &s.vertices),
                Face::new([1, 2, 3], [
                    AdjacentFace { index: 5, edge: 2, },
                    AdjacentFace { index: 0, edge: 1, },
                    AdjacentFace { index: 4, edge: 0, },
                ], &s.vertices),
                Face::new([1, 3, 4], [
                    AdjacentFace { index: 3, edge: 2, },
                    AdjacentFace { index: 1, edge: 1, },
                    AdjacentFace { index: 5, edge: 0, },
                ], &s.vertices),
                Face::new([1, 4, 2], [
                    AdjacentFace { index: 4, edge: 2, },
                    AdjacentFace { index: 2, edge: 1, },
                    AdjacentFace { index: 3, edge: 0, },
                ], &s.vertices),
            ];

            // if the origin lives on the line, the hexahedron might not contain the origin
            for face in &faces[..3] {
                if face.normal.dot(a) >= 0.0 {
                    s.closest_point = Vec3::ZERO;
                    return s;
                }
            }
            for face in &faces[3..] {
                if face.normal.dot(b) >= 0.0 {
                    s.closest_point = Vec3::ZERO;
                    return s;
                }
            }

            s.faces.extend(faces);
        } else if s.vertices.len() == 3 {
            // GJK returned a triangle -> construct a tetrahedron

            // let a = s.vertices.pop().unwrap();
            // let b = s.vertices.pop().unwrap();
            // let c = s.vertices.pop().unwrap();

            todo!()
        } else {
            s.faces.extend([
                Face::new(
                    [3, 2, 1],
                    [
                        AdjacentFace { index: 1, edge: 1 },
                        AdjacentFace { index: 3, edge: 1 },
                        AdjacentFace { index: 2, edge: 1 },
                    ],
                    &s.vertices,
                ),
                Face::new(
                    [0, 2, 3],
                    [
                        AdjacentFace { index: 3, edge: 2 },
                        AdjacentFace { index: 0, edge: 0 },
                        AdjacentFace { index: 2, edge: 0 },
                    ],
                    &s.vertices,
                ),
                Face::new(
                    [0, 3, 1],
                    [
                        AdjacentFace { index: 1, edge: 2 },
                        AdjacentFace { index: 0, edge: 2 },
                        AdjacentFace { index: 3, edge: 0 },
                    ],
                    &s.vertices,
                ),
                Face::new(
                    [0, 1, 2],
                    [
                        AdjacentFace { index: 2, edge: 2 },
                        AdjacentFace { index: 0, edge: 1 },
                        AdjacentFace { index: 1, edge: 0 },
                    ],
                    &s.vertices,
                ),
            ]);
        }

        s.priority_queue
            .extend(s.faces.iter().enumerate().filter_map(|(index, face)| {
                face.closest_is_internal().then(|| {
                    Reverse(Entry {
                        face: index,
                        distance_squared: face.closest.length_squared(),
                    })
                })
            }));

        let mut current_step = 0;
        'expansion_loop: loop {
            if current_step == steps {
                debug!("EPA did not converge");
                break 'expansion_loop;
            }

            let Some(closest_face) = s.priority_queue.pop() else {
                debug!("EPA ran out of candidate faces");
                break 'expansion_loop;
            };

            let closest_face = &mut s.faces[closest_face.0.face];
            if closest_face.obsolete {
                continue 'expansion_loop;
            }

            s.closest_point = closest_face.closest;
            let support_point = SupportPoint::new(shape1, shape2, closest_face.normal);

            let dot = closest_face.normal.dot(support_point.difference);
            let distance_squared = dot * dot / closest_face.normal.length_squared();
            s.upper_bound = s.upper_bound.min(distance_squared);

            let close_enough =
                s.upper_bound <= self.tolerance_factor * closest_face.closest.length_squared();
            if close_enough {
                debug!("EPA successfully converged");
                break 'expansion_loop;
            }

            closest_face.obsolete = true;

            for adjacent_face in &closest_face.adjacents.clone() {
                silhouette(adjacent_face, &support_point, &mut s.edges, &mut s.faces);
            }

            let support_index = s.vertices.len();
            s.vertices.push(support_point);

            let old_face_count = s.faces.len();
            let mut last_face_index = old_face_count + s.edges.len() - 1;
            for adjacent_face in s.edges.drain(..) {
                let current_face_index = s.faces.len();
                let face = &mut s.faces[adjacent_face.index];
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
                    &s.vertices,
                );

                if face.affinely_dependent() {
                    debug!(
                        "EPA encountered an affinely dependent triangle: {:?}",
                        face.vertex_indices.map(|i| s.vertices[i].difference)
                    );
                    break 'expansion_loop;
                }

                if face.closest_is_internal()
                    && s.closest_point.length_squared() <= face.closest.length_squared()
                    && face.closest.length_squared() <= self.tolerance_factor * s.upper_bound
                {
                    s.priority_queue.push(Reverse(Entry {
                        face: current_face_index,
                        distance_squared: face.closest.length_squared(),
                    }));
                }

                s.faces.push(face);
                last_face_index = current_face_index;
            }
            s.faces[last_face_index].adjacents[1].index = old_face_count;

            current_step += 1;
        }

        s
    }

    pub fn penetration_depth<S1: ConvexShape + ?Sized, S2: ConvexShape + ?Sized>(
        &mut self,
        shape1: &S1,
        shape2: &S2,
        tetrahedron: [SupportPoint; 4],
    ) -> (Vec3, f32) {
        let state = self.penetration_depth_details(shape1, shape2, tetrahedron, self.max_iteration);
        state.closest_point.normalize_and_length()
    }
}

impl Default for EpaEngine {
    fn default() -> Self {
        let relative_tolerance = f32::EPSILON;

        EpaEngine {
            max_iteration: 100,
            relative_tolerance,
            tolerance_factor: (1.0 + relative_tolerance) * (1.0 + relative_tolerance),
            state: EpaState {
                vertices: Vec::with_capacity(104),
                faces: Vec::with_capacity(104),
                priority_queue: BinaryHeap::with_capacity(104),
                edges: Vec::new(),
                closest_point: Vec3::ZERO,
                upper_bound: f32::INFINITY,
            },
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

    if face.normal.dot(support_point.difference - face.closest) < 0.0 {
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
pub struct AdjacentFace {
    pub index: usize,
    pub edge: usize,
}

#[derive(Debug)]
pub struct Face {
    pub vertex_indices: [usize; 3],
    pub normal: Vec3,
    pub closest: Vec3,
    pub closest_is_internal: bool,
    pub adjacents: [AdjacentFace; 3],
    pub obsolete: bool,
}

impl Face {
    fn new(
        vertex_indices: [usize; 3],
        adjacents: [AdjacentFace; 3],
        vertices: &[SupportPoint],
    ) -> Face {
        let [a, b, c] = vertex_indices.map(|idx| vertices[idx].difference);

        let ab = b - a;
        let bc = c - b;
        let ca = a - c;

        let normal = ab.cross(bc);
        let closest = a.project_onto(normal);

        let external = ab.cross(normal).dot(-a) > 0.0
            || bc.cross(normal).dot(-b) > 0.0
            || ca.cross(normal).dot(-a) > 0.0;

        Face {
            vertex_indices,
            normal,
            closest,
            closest_is_internal: !external,
            adjacents,
            obsolete: false,
        }
    }

    fn affinely_dependent(&self) -> bool {
        self.normal.length_squared() <= f32::EPSILON * f32::EPSILON
    }

    pub fn closest_is_internal(&self) -> bool {
        self.closest_is_internal
    }
}

#[derive(Debug)]
struct Entry {
    face: usize,
    distance_squared: f32,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.distance_squared == other.distance_squared
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
        self.distance_squared.total_cmp(&other.distance_squared)
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
        let mut engine = EpaEngine::default();

        let origin = Ball {
            center: Vec3::ZERO,
            radius: 1.0,
        };

        // let tetrahedron = gjk_tetrahedron(&origin, &origin).unwrap();
        // let (_, distance) = engine.penetration_depth(&origin, &origin, tetrahedron);
        // assert!(
        //     (distance - 2.0).abs() <= 0.1,
        //     "Expected 2.0, got {distance}",
        // );

        let x = Ball {
            center: Vec3::X,
            radius: 0.0,
        };

        let tetrahedron = gjk_tetrahedron(&x, &x).unwrap();
        let result = engine.penetration_depth(&x, &x, tetrahedron);
        assert!(result.1.abs() <= 0.1, "Expected 0.0, got {}", result.0);

        let tetrahedron = gjk_tetrahedron(&origin, &x).unwrap();
        let result = engine.penetration_depth(&origin, &x, tetrahedron);
        assert!(result.1.abs() <= 0.1, "Expected 0.0, got {}", result.0);

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
        let mut engine = EpaEngine::default();

        let aab = AxisAlignedBox::from_center_dimension(Vec3::ZERO, 2.0, 2.0, 2.0);

        let mut ball = Ball {
            center: Vec3::ZERO,
            radius: 1.0,
        };

        let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        let (_, distance) = engine.penetration_depth(&aab, &ball, tetrahedron);
        assert_seperating_distance(2.0, distance);

        ball.center = vec3(1.99, 0.0, 0.0);
        let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        let result = engine.penetration_depth(&aab, &ball, tetrahedron);
        assert_seperating_vector(ball.center.normalize(), 0.01, result);

        ball.center = vec3(2.0, 0.0, 0.0);
        let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        let (_, distance) = engine.penetration_depth(&aab, &ball, tetrahedron);
        assert_seperating_distance(0.0, distance);

        ball.center = vec3(1.70, 1.70, 0.0);
        let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        let result = engine.penetration_depth(&aab, &ball, tetrahedron);
        assert_seperating_vector(ball.center.normalize(), 0.0101, result);

        ball.center = vec3(1.57, 1.57, 1.57);
        let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        let result = engine.penetration_depth(&aab, &ball, tetrahedron);
        assert_seperating_vector(ball.center.normalize(), 0.0127, result);
    }

    fn assert_seperating_distance(expected: f32, actual: f32) {
        assert!(
            (actual - expected).abs() <= 1e-3,
            "Expected {expected}, got {actual}",
        );
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
        assert_seperating_distance(expected_distance, actual_distance);
    }
}
