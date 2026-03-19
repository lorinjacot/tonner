use std::time::Duration;

use glam::Vec3;
use pyo3::prelude::*;
use storm::scene_graph::NodeId;

use crate::ball::Ball;

const SUBSTEP_COUNT: usize = 10;

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

pub fn update<'py, 'a>(
    py: Python<'py>,
    delta_time: Duration,
    balls: impl IntoIterator<Item = &'a Ball>,
    constraints: impl IntoIterator<Item = &'a dyn Constraint>,
) {
    
}
