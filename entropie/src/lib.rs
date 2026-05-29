use std::{fmt::Debug, time::Duration};

use glam::Vec3;
use log::error;
use sparse_keyed::{Key, KeyRegistry, SecondaryMap};

pub use aabb::AABB;
pub use particle::ParticleBuilder;
pub use transform::Transform;

use crate::force::{Force, ForceId};

mod aabb;
pub mod collision;
pub mod constraint;
pub mod force;
mod particle;
pub mod shape;
mod transform;

#[derive(Debug, Clone)]
struct LinearData {
    position: Vec3,
    velocity: Vec3,
    inverse_mass: f32,
}

#[derive(Clone)]
pub struct State<'a> {
    bodies: KeyRegistry,
    particles: SecondaryMap<()>,
    linear_data: SecondaryMap<LinearData>,
    time: Duration,
    force_registry: KeyRegistry,
    forces: SecondaryMap<&'a dyn Force>,
}

impl<'a> State<'a> {
    pub fn new() -> Self {
        State {
            bodies: KeyRegistry::new(),
            particles: SecondaryMap::new(),
            linear_data: SecondaryMap::new(),
            time: Duration::ZERO,
            force_registry: KeyRegistry::new(),
            forces: SecondaryMap::new(),
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

    pub fn add_force<F: Force>(&mut self, force: &'a F) -> ForceId {
        let id = self.force_registry.create();

        self.forces.insert(id, force);

        ForceId(id)
    }
}

impl<'a> Default for State<'a> {
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
            let previous_pvm = state.linear_data.clone();

            for (force_id, force) in &state.forces {
                let f_ext = force.value(state.time);
                for body in force.bodies() {
                    match state.linear_data.get_mut(body.0) {
                        Some(pvm) => {
                            pvm.velocity += h * f_ext * pvm.inverse_mass;
                        }
                        None => {
                            error!(
                                "Failed to apply force {force_id:?} to body {body:?}: body not found in state"
                            );
                        }
                    }
                }
            }

            for pvm in state.linear_data.values_mut() {
                pvm.position += h * pvm.velocity;
            }

            // solve positions
            // ...

            for (new, old) in state.linear_data.values_mut().zip(previous_pvm.values()) {
                new.velocity = (new.position - old.position) / h;
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
