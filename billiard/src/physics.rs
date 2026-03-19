use std::{ops::Deref, time::Duration};

use glam::{Vec3, vec3};
use pyo3::prelude::*;
use storm::scene_graph::{NodeId, SceneGraph};

use crate::ball::Ball;

const SUBSTEP_COUNT: usize = 10;
const G: Vec3 = vec3(0.0, -1.0, 0.0);

pub trait Constraint: Send + Sync {
    /// All entities impacted or impacting the contraint.
    fn entities(&self) -> &[NodeId];

    /// Value of the constraint for the given positions. The order and length of `positions` depends on the last
    /// [`Constraint::entities()`] return.
    fn value(&self, positions: &[Vec3]) -> f32;

    /// Gradient of the constraint for the given positions. The order and length of `positions` depends on the last
    /// [`Constraint::entities()`] return.
    fn gradient(&self, positions: &[Vec3]) -> Vec<Vec3>;
}

pub fn update<'py, 'a, B: Deref<Target = Ball>, C: Deref<Target = dyn Constraint>>(
    py: Python<'py>,
    delta_time: Duration,
    scene_graph: &mut SceneGraph,
    balls: &[B],
    constraints: &[C],
) {
    let ids: Vec<_> = balls.iter().map(|b| b.node().borrow(py).id()).collect();
    let mut positions: Vec<_> = ids
        .iter()
        .map(|id| {
            scene_graph[*id]
                .global_transformation()
                .transform_point3(Vec3::ZERO)
        })
        .collect();
    // let mut velocities: Vec<_> = balls.iter().map(|b| b.)
    for _ in 0..SUBSTEP_COUNT {
        for c in constraints {
            let positions: Vec<_> = c
                .entities()
                .iter()
                .map(|node| scene_graph.get(*node).unwrap().local_translation())
                .collect();
            let value = c.value(&positions);
            let grad = c.gradient(&positions);
        }
    }
}
