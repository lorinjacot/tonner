use std::{
    ops::{Deref, DerefMut},
    time::Duration,
};

use glam::{Vec3, vec3};
use numpy::{PyArray1, PyArrayMethods};
use pyo3::prelude::*;
use storm::scene_graph::{NodeId, SceneGraph};

use crate::ball::Ball;

const SUBSTEP_COUNT: usize = 10;
const G: Vec3 = vec3(0.0, 0.0, 0.0);

pub trait Constraint: Send + Sync {
    /// All entities impacted or impacting the contraint.
    fn entities(&self) -> &[NodeId];

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

struct Particle<'a> {
    ball: &'a mut Ball,
    node: NodeId,
    mass: f32,
    inverse_mass: f32,
    position: Vec3,
    previous_position: Vec3,
    velocity: Vec3,
}

pub fn update<'py, 'a, B: DerefMut<Target = Ball>, C: Deref<Target = dyn Constraint>>(
    py: Python<'py>,
    delta_time: Duration,
    scene_graph: &mut SceneGraph,
    balls: &mut [B],
    constraints: &[C],
) {
    let mut particles: Vec<_> = balls
        .iter_mut()
        .map(|b| {
            let v = b.velocity.bind(py).readonly();
            let v = v.as_array();
            let velocity = vec3(v[0] as f32, v[1] as f32, v[2] as f32);
            let ball = b.deref_mut();
            let node = ball.node().borrow(py).id();
            let mass = 1.0;
            let position = scene_graph[node]
                .global_transformation()
                .transform_point3(Vec3::ZERO);
            let previous_position = position;
            Particle {
                ball,
                node,
                mass,
                inverse_mass: 1.0 / mass,
                position,
                previous_position,
                velocity,
            }
        })
        .collect();

    let h = delta_time.as_secs_f32() / SUBSTEP_COUNT as f32;
    for _ in 0..SUBSTEP_COUNT {
        particles.iter_mut().for_each(|p| {
            p.previous_position = p.position;
            p.velocity += h * G;
            p.position += h * p.velocity;
        });

        for c in constraints {
            let nodes = c.entities();
            let mut particles: Vec<_> = particles
                .iter_mut()
                .filter(|p| nodes.contains(&p.node))
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

        particles.iter_mut().for_each(|p| {
            p.velocity = (p.position - p.previous_position) / h;
        });
    }

    particles.iter_mut().for_each(|p| {
        scene_graph
            .set_local_transformation(p.node, p.position, None, None)
            .unwrap();
        p.ball.velocity =
            PyArray1::from_iter(py, p.velocity.to_array().iter().map(|c| *c as f64)).unbind();
    });
}
