use std::{fmt::Debug, time::Duration};

use glam::Vec3;
use sparse_keyed::{Key, KeyRegistry, SecondaryMap};

pub use aabb::AABB;
pub use particle::ParticleBuilder;
pub use transform::Transform;

mod aabb;
pub mod collision;
pub mod constraint;
mod particle;
pub mod shape;
mod transform;

#[derive(Debug, Clone)]
struct LinearData {
    position: Vec3,
    previous_position: Vec3,
    velocity: Vec3,
    inverse_mass: f32,
    force: Vec3,
}

#[derive(Debug, Clone)]
pub struct State {
    bodies: KeyRegistry,
    particles: SecondaryMap<()>,
    linear_data: SecondaryMap<LinearData>,
    time: Duration,
}

impl State {
    pub fn new() -> Self {
        State {
            bodies: KeyRegistry::new(),
            particles: SecondaryMap::new(),
            linear_data: SecondaryMap::new(),
            time: Duration::ZERO,
        }
    }

    pub fn is_particle(&self, body: BodyId) -> bool {
        self.particles.contains(body.0)
    }

    pub fn position(&self, body: BodyId) -> Option<Vec3> {
        self.linear_data.get(body.0).map(|data| data.position)
    }

    pub fn position_mut(&mut self, body: BodyId) -> Option<&mut Vec3> {
        self.linear_data
            .get_mut(body.0)
            .map(|data| &mut data.position)
    }

    pub fn velocity(&self, body: BodyId) -> Option<Vec3> {
        self.linear_data.get(body.0).map(|data| data.velocity)
    }

    pub fn velocity_mut(&mut self, body: BodyId) -> Option<&mut Vec3> {
        self.linear_data
            .get_mut(body.0)
            .map(|data| &mut data.velocity)
    }

    pub fn mass(&self, body: BodyId) -> Option<f32> {
        self.linear_data
            .get(body.0)
            .map(|data| data.inverse_mass.recip())
    }

    pub fn inverse_mass(&self, body: BodyId) -> Option<f32> {
        self.linear_data.get(body.0).map(|data| data.inverse_mass)
    }

    pub fn inverse_mass_mut(&mut self, body: BodyId) -> Option<&mut f32> {
        self.linear_data
            .get_mut(body.0)
            .map(|data| &mut data.inverse_mass)
    }

    pub fn force(&mut self, body: BodyId) -> Option<Vec3> {
        self.linear_data.get(body.0).map(|data| data.force)
    }

    pub fn force_mut(&mut self, body: BodyId) -> Option<&mut Vec3> {
        self.linear_data.get_mut(body.0).map(|data| &mut data.force)
    }
}

impl Default for State {
    fn default() -> Self {
        State::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyId(Key);

#[derive(Debug, Clone)]
pub struct Solver {
    substep_count: u32,
}

impl Solver {
    pub fn simulate(&mut self, state: &mut State, delta_time: Duration) {
        let substep_duration = delta_time / self.substep_count;
        let h = substep_duration.as_secs_f32();
        for _ in 0..self.substep_count {
            state.time += substep_duration;
            for d in state.linear_data.values_mut() {
                d.previous_position = d.position;
                d.velocity += h * d.force * d.inverse_mass;
                d.position += h * d.velocity;
            }

            // solve positions
            // ...

            for d in state.linear_data.values_mut() {
                d.velocity = (d.position - d.previous_position) / h;
            }

            // solve velocities
            // ...
        }
    }
}

impl Default for Solver {
    fn default() -> Self {
        Solver { substep_count: 10 }
    }
}
