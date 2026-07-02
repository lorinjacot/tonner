use std::{collections::HashMap, ops::Deref, time::Duration};

use glam::{Vec3, vec3};
use numpy::{PyArray1, PyArrayMethods};
use pyo3::prelude::*;
use tempete::{ecs::EntityId, scene_graph::SceneGraph};

const SUBSTEP_COUNT: usize = 10;

pub trait Force: Send + Sync {
    /// All entities impacted or impacting the force.
    fn entities(&self) -> &[EntityId];

    /// Force (in N) to apply on the entities. for the given positions.
    /// The order and length the returned `Vec` and of `positions` must match
    /// [`Force::entities()`].
    fn value(&self, positions: &[Vec3], velocities: &[Vec3]) -> Vec<Vec3>;
}

pub trait Constraint: Send + Sync {
    /// All entities impacted or impacting the contraint.
    fn entities(&self) -> &[EntityId];

    /// Value of the constraint for the given positions. The order and length of `positions` depends on the last
    /// [`Constraint::entities()`] return.
    fn value(&self, positions: &[Vec3]) -> f32;

    /// Gradient of the constraint for the given positions. The order and length of `positions` depends on the last
    /// [`Constraint::entities()`] return.
    fn gradient(&self, positions: &[Vec3]) -> Vec<Vec3>;

    fn alpha(&self) -> f32 {
        0.0
    }
}

struct ContactConstraint {
    entities: [EntityId; 2],
    radii_sum: f32,
}

impl ContactConstraint {
    fn separating_vector(&self, positions: &[Vec3]) -> (Vec3, f32) {
        let dp = positions[1] - positions[0];
        let (dir, dist) = dp.normalize_and_length();

        if dist < self.radii_sum {
            (dir, self.radii_sum - dist)
        } else {
            (Vec3::ZERO, 0.0)
        }
    }
}

impl Constraint for ContactConstraint {
    fn entities(&self) -> &[EntityId] {
        &self.entities
    }

    fn value(&self, positions: &[Vec3]) -> f32 {
        let (_, penetration_depth) = self.separating_vector(positions);
        penetration_depth
    }

    fn gradient(&self, positions: &[Vec3]) -> Vec<Vec3> {
        let (separating_vector, _) = self.separating_vector(positions);
        let dir = separating_vector.normalize_or_zero();
        vec![dir, -dir]
    }
}

struct Particle<'a> {
    ball: &'a mut crate::ball::Ball,
    mass: f32,
    inverse_mass: f32,
    position: Vec3,
    previous_position: Vec3,
    velocity: Vec3,
}

pub fn update<'py, 'a, F: Deref<Target = dyn Force>, C: Deref<Target = dyn Constraint>>(
    py: Python<'py>,
    delta_time: Duration,
    scene_graph: &mut SceneGraph,
    balls: impl IntoIterator<Item = &'a mut crate::ball::Ball>,
    forces: &[F],
    constraints: &[C],
) {
    let mut particles: HashMap<_, _> = balls
        .into_iter()
        .map(|ball| {
            let node = ball.node().borrow(py).entity();
            let mass = 1.0;
            let position = scene_graph[node]
                .global_transformation()
                .transform_point3(Vec3::ZERO);
            let v = ball.velocity.bind(py).readonly();
            let v = v.as_array();
            let velocity = vec3(v[0] as f32, v[1] as f32, v[2] as f32);
            let previous_position = position;
            let particle = Particle {
                ball,
                mass,
                inverse_mass: 1.0 / mass,
                position,
                previous_position,
                velocity,
            };
            (node, particle)
        })
        .collect();

    let mut contact_constraints = Vec::with_capacity(particles.len());

    let h = delta_time.as_secs_f32() / SUBSTEP_COUNT as f32;
    for _ in 0..SUBSTEP_COUNT {
        for f in forces {
            let entities = f.entities();
            let positions: Vec<_> = entities
                .iter()
                .map(|node| particles[node].position)
                .collect();
            let velocities: Vec<_> = entities
                .iter()
                .map(|node| particles[node].velocity)
                .collect();
            let force = f.value(&positions, &velocities);
            entities.iter().zip(force).for_each(|(node, force)| {
                particles.get_mut(node).unwrap().velocity += h * force;
            })
        }

        for p in particles.values_mut() {
            p.previous_position = p.position;
            p.position += h * p.velocity;
        }

        for (&entity1, particle1) in &particles {
            for (&entity2, particle2) in &particles {
                if entity1 < entity2 {
                    let dp = particle2.position - particle1.position;
                    let radii_sum = (particle1.ball.radius + particle2.ball.radius) as f32;
                    if dp.length_squared() < radii_sum * radii_sum {
                        contact_constraints.push(ContactConstraint {
                            entities: [entity1, entity2],
                            radii_sum,
                        });
                    }
                }
            }
        }

        for c in constraints {
            let nodes = c.entities();
            let mut particles: Vec<_> = particles
                .iter_mut()
                .filter(|(node, _)| nodes.contains(*node))
                .map(|(_, p)| p)
                .collect();

            let positions: Vec<_> = particles.iter().map(|p| p.position).collect();

            let loss = c.value(&positions);
            if loss.abs() > 1e-6 {
                let total_inverse_mass: f32 = particles.iter().map(|p| p.inverse_mass).sum();
                let lambda = -loss / (total_inverse_mass + c.alpha() / (h * h));

                let gradients = c.gradient(&positions);
                particles.iter_mut().zip(gradients).for_each(|(p, grad)| {
                    let impulse = lambda * grad.normalize();
                    p.position += impulse / p.mass;
                });
            }
        }
        for c in contact_constraints.drain(..) {
            let nodes = c.entities();
            let mut particles: Vec<_> = particles
                .iter_mut()
                .filter(|(node, _)| nodes.contains(*node))
                .map(|(_, p)| p)
                .collect();

            let positions: Vec<_> = particles.iter().map(|p| p.position).collect();

            let loss = c.value(&positions);
            if loss.abs() > 1e-6 {
                let total_inverse_mass: f32 = particles.iter().map(|p| p.inverse_mass).sum();
                let lambda = -loss / (total_inverse_mass + c.alpha() / (h * h));

                let gradients = c.gradient(&positions);
                particles.iter_mut().zip(gradients).for_each(|(p, grad)| {
                    let impulse = lambda * grad.normalize();
                    p.position += impulse / p.mass;
                });
            }
        }

        particles.values_mut().for_each(|p| {
            p.velocity = (p.position - p.previous_position) / h;
        });
    }

    for (node, p) in &mut particles {
        scene_graph.set_local_transformation(*node, p.position, None, None);
        p.ball.velocity =
            PyArray1::from_iter(py, p.velocity.to_array().iter().map(|c| *c as f64)).unbind();
    }
}
