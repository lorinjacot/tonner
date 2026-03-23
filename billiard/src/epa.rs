use glam::{Vec3, Vec4};

use crate::shape::ConvexShape;

pub(crate) fn epa<S1: ConvexShape + ?Sized, S2: ConvexShape + ?Sized>(
    shape1: &S1,
    shape2: &S2,
    tetrahedron: [Vec3; 4],
) -> Vec4 {
    let mut vertices = Vec::from(tetrahedron);
    let mut faces = Vec::from(
        [[3, 2, 1], [3, 1, 0], [3, 0, 2], [2, 0, 1]]
            .map(|indices| Face::from_vertex_indices(indices, &vertices)),
    );

    let mut edges_to_remove = Vec::new();
    loop {
        let mut min_distance = f32::INFINITY;
        let mut min_normal = Vec3::ZERO;

        for face in &faces {
            if face.distance < min_distance {
                min_distance = face.distance;
                min_normal = face.normal;
            }
        }

        let support = support(shape1, shape2, min_normal);
        let distance_support = support.dot(min_normal);

        if (distance_support - min_distance).abs() < 1e-4 {
            return min_normal.extend(min_distance);
        }

        for i in (0..faces.len()).rev() {
            let face = &faces[i];
            if face.normal.dot(support) > 0.0 {
                let face = faces.swap_remove(i);

                face.edges().into_iter().for_each(|(i, j)| {
                    match edges_to_remove
                        .iter()
                        .enumerate()
                        .find(|(_, edge)| **edge == (j, i))
                    {
                        Some((i, _)) => {
                            edges_to_remove.swap_remove(i);
                        }
                        None => edges_to_remove.push((i, j)),
                    }
                });
            }
        }

        let k = vertices.len();
        faces.extend(
            edges_to_remove
                .drain(..)
                .map(|(i, j)| Face::from_vertex_indices([i, j, k], &vertices)),
        );
        vertices.push(support);
    }
}

fn support<S1: ConvexShape + ?Sized, S2: ConvexShape + ?Sized>(
    shape1: &S1,
    shape2: &S2,
    direction: Vec3,
) -> Vec3 {
    shape1.support_point(direction) - shape2.support_point(-direction)
}

struct Face {
    indices: [usize; 3],
    normal: Vec3,
    distance: f32,
}

impl Face {
    fn from_vertex_indices(vertex_indices: [usize; 3], vertices: &[Vec3]) -> Face {
        let [a, b, c] = vertex_indices.map(|i| vertices[i]);
        let mut normal = (b - a)
            .cross(c - a)
            .try_normalize()
            .unwrap_or_else(|| todo!("abc is a line or a point"));
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
