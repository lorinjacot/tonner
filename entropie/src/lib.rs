use std::{fmt::Debug, sync::Arc, time::Duration};

use glam::{Quat, Vec3};
use log::error;
#[cfg(feature = "pyo3")]
use pyo3::{exceptions::PyValueError, prelude::*};
use sparse_keyed::{Key, KeyRegistry, PrimaryMap, SecondaryMap};

pub use aabb::AABB;
pub use particle::ParticleBuilder;
pub use transform::Transform;

use crate::constraint::PositionalConstraint;
#[cfg(feature = "pyo3")]
use crate::constraint::{DistanceConstraint, PositionalConstraintId};

mod aabb;
pub mod collision;
pub mod constraint;
mod particle;
pub mod shape;
mod transform;

#[derive(Debug, Clone)]
struct PositionalData {
    position: Vec3,
    previous_position: Vec3,
    velocity: Vec3,
    inverse_mass: f32,
    force: Vec3,
}

#[derive(Clone)]
#[cfg_attr(feature = "pyo3", pyclass(skip_from_py_object))]
pub struct State {
    bodies: KeyRegistry,
    particles: SecondaryMap<()>,
    positional_data: SecondaryMap<PositionalData>,
    positional_constraints: PrimaryMap<Arc<dyn PositionalConstraint + Sync + Send>>,
}

impl Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("bodies", &self.bodies)
            .field("particles", &self.particles.keys().collect::<Vec<_>>())
            .field("positional_data", &self.positional_data)
            .field(
                "positional_constraints",
                &self.positional_constraints.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl State {
    pub fn new() -> Self {
        State {
            bodies: KeyRegistry::new(),
            particles: SecondaryMap::new(),
            positional_data: SecondaryMap::new(),
            positional_constraints: PrimaryMap::new(),
        }
    }

    pub fn is_particle(&self, body: BodyId) -> bool {
        self.particles.contains(body.0)
    }

    pub fn position(&self, body: BodyId) -> Option<Vec3> {
        self.positional_data.get(body.0).map(|data| data.position)
    }

    pub fn position_mut(&mut self, body: BodyId) -> Option<&mut Vec3> {
        self.positional_data
            .get_mut(body.0)
            .map(|data| &mut data.position)
    }

    pub fn velocity(&self, body: BodyId) -> Option<Vec3> {
        self.positional_data.get(body.0).map(|data| data.velocity)
    }

    pub fn velocity_mut(&mut self, body: BodyId) -> Option<&mut Vec3> {
        self.positional_data
            .get_mut(body.0)
            .map(|data| &mut data.velocity)
    }

    pub fn mass(&self, body: BodyId) -> Option<f32> {
        self.positional_data
            .get(body.0)
            .map(|data| data.inverse_mass.recip())
    }

    pub fn inverse_mass(&self, body: BodyId) -> Option<f32> {
        self.positional_data
            .get(body.0)
            .map(|data| data.inverse_mass)
    }

    pub fn inverse_mass_mut(&mut self, body: BodyId) -> Option<&mut f32> {
        self.positional_data
            .get_mut(body.0)
            .map(|data| &mut data.inverse_mass)
    }

    pub fn force(&mut self, body: BodyId) -> Option<Vec3> {
        self.positional_data.get(body.0).map(|data| data.force)
    }

    pub fn force_mut(&mut self, body: BodyId) -> Option<&mut Vec3> {
        self.positional_data
            .get_mut(body.0)
            .map(|data| &mut data.force)
    }
}

#[cfg(feature = "pyo3")]
#[pymethods]
impl State {
    #[new]
    fn py_new() -> Self {
        State::new()
    }

    #[pyo3(signature = (position=[0.0; 3], velocity=[0.0; 3], mass=f32::INFINITY))]
    fn add_particle(&mut self, position: [f32; 3], velocity: [f32; 3], mass: f32) -> BodyId {
        ParticleBuilder::default()
            .position(position)
            .velocity(velocity)
            .mass(mass)
            .build(self)
    }

    fn add_force(&mut self, body: BodyId, force: [f32; 3]) -> PyResult<()> {
        let f = self
            .force_mut(body)
            .ok_or_else(|| PyValueError::new_err("Invalid body ID"))?;
        *f += Vec3::from_array(force);
        Ok(())
    }

    #[pyo3(signature = (bodies, distance, compliance = 0.0, application_points = [[0.0; 3]; 2]))]
    fn add_distance_constraint(
        &mut self,
        bodies: [BodyId; 2],
        distance: f32,
        compliance: f32,
        application_points: [[f32; 3]; 2],
    ) -> PositionalConstraintId {
        let c = DistanceConstraint {
            bodies,
            distance,
            compliance,
            application_points: application_points.map(|v| Vec3::from_array(v)),
        };
        let key = self.positional_constraints.add(Arc::new(c));
        PositionalConstraintId(key)
    }

    #[pyo3(name = "position")]
    fn py_position(&self, body: BodyId) -> PyResult<[f32; 3]> {
        self.position(body)
            .map(|p| p.to_array())
            .ok_or_else(|| PyValueError::new_err("Invalid body ID"))
    }
}

impl Default for State {
    fn default() -> Self {
        State::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "pyo3", pyclass(frozen, from_py_object))]
pub struct BodyId(Key);

#[derive(Debug, Clone)]
#[cfg_attr(feature = "pyo3", pyclass(skip_from_py_object))]
pub struct Solver {
    substep_count: u32,
    inverse_masses: Vec<f32>,
    positions: Vec<Vec3>,
    orientations: Vec<Quat>,
    position_gradient: Vec<Vec3>,
    application_points: Vec<Vec3>,
}

impl Solver {
    pub fn simulate(&mut self, state: &mut State, delta_time: Duration) {
        let substep_duration = delta_time / self.substep_count;
        let h = substep_duration.as_secs_f32();
        let h_squared = h * h;
        for _ in 0..self.substep_count {
            for d in state.positional_data.values_mut() {
                d.previous_position = d.position;
                d.velocity += h * d.force * d.inverse_mass;
                d.position += h * d.velocity;
            }

            // solve positions
            self.solve_constraints(
                &mut state.positional_data,
                &state.positional_constraints,
                h_squared,
            );

            for d in state.positional_data.values_mut() {
                d.velocity = (d.position - d.previous_position) / h;
            }

            // solve velocities
            // ...
        }
    }

    fn solve_constraints(
        &mut self,
        positional_data: &mut SecondaryMap<PositionalData>,
        positional_constraints: &PrimaryMap<Arc<dyn PositionalConstraint + Sync + Send>>,
        h_squared: f32,
    ) {
        'outer: for (key, c) in positional_constraints.iter() {
            let bodies = c.bodies();
            let n = bodies.len();

            self.inverse_masses.reserve(n);
            self.positions.reserve(n);
            self.orientations.reserve(n);
            for &body in bodies {
                let Some(data) = positional_data.get(body.0) else {
                    error!(
                        "Body {:?} involved in constraint {:?} does not exist",
                        body, key
                    );
                    continue 'outer;
                };
                self.inverse_masses.push(data.inverse_mass);
                self.positions.push(data.position);
                self.orientations.push(Quat::IDENTITY);
            }

            self.position_gradient.resize(n, Vec3::ZERO);
            self.application_points.resize(n, Vec3::ZERO);

            let value = c.value(
                &self.positions,
                &self.orientations,
                &mut self.position_gradient,
                &mut self.application_points,
            );
            let alpha_tilde = c.compliance() / h_squared;

            let w_tot: f32 = self
                .inverse_masses
                .iter()
                .zip(self.position_gradient.iter())
                .map(|(inverse_mass, grad)| inverse_mass * grad.length_squared())
                .sum();
            let delta_lambda = -value / (w_tot + alpha_tilde);

            for ((body, inverse_mass), grad) in bodies
                .iter()
                .zip(&self.inverse_masses)
                .zip(&self.position_gradient)
            {
                positional_data[body.0].position += inverse_mass * grad * delta_lambda;
            }

            self.inverse_masses.clear();
            self.positions.clear();
            self.orientations.clear();
            self.position_gradient.clear();
            self.application_points.clear();
        }
    }
}

impl Default for Solver {
    fn default() -> Self {
        Solver {
            substep_count: 10,
            inverse_masses: Vec::with_capacity(2),
            positions: Vec::with_capacity(2),
            orientations: Vec::with_capacity(2),
            position_gradient: Vec::with_capacity(2),
            application_points: Vec::with_capacity(2),
        }
    }
}

#[cfg(feature = "pyo3")]
#[pymethods]
impl Solver {
    #[new]
    fn py_new() -> Self {
        Solver::default()
    }

    #[pyo3(name = "simulate")]
    fn py_simulate(&mut self, state: &mut State, delta_time: Duration) {
        self.simulate(state, delta_time);
    }

    #[getter]
    fn substep_count(&self) -> u32 {
        self.substep_count
    }

    #[setter]
    fn set_substep_count(&mut self, value: u32) {
        self.substep_count = value;
    }
}

#[cfg(feature = "pyo3")]
#[pymodule(name = "entropie")]
mod py_entropie {
    #[pymodule_export]
    use super::{BodyId, Solver, State};
}

#[cfg(test)]
mod tests {
    use glam::vec3;

    use super::*;

    #[test]
    fn test_implicit_euler() {
        const ITERATOR_COUNT: usize = 10;
        const DELTA_TIME: Duration = Duration::from_millis(1);
        const DT: f32 = DELTA_TIME.as_secs_f32();

        let p0 = vec3(1.0, 2.0, 3.0);
        let v0 = vec3(10.0, 20.0, 30.0);
        let f = vec3(100.0, 200.0, 300.0);
        let expected: Vec<_> = (0..ITERATOR_COUNT)
            .scan((p0, v0), |(p, v), _| {
                let a = f; // mass = 1.0
                *v += a * DT;
                *p += *v * DT;
                Some(*p)
            })
            .collect();

        let mut state = State::new();
        let key = state.bodies.create();

        state.positional_data.insert(
            key,
            PositionalData {
                position: p0,
                previous_position: p0,
                velocity: v0,
                inverse_mass: 1.0,
                force: f,
            },
        );
        let mut solver = Solver::default();
        solver.substep_count = 1;
        for (i, expected_pos) in expected.into_iter().enumerate() {
            solver.simulate(&mut state, DELTA_TIME);
            let actual_pos = state.position(BodyId(key)).unwrap();
            assert!(
                actual_pos.abs_diff_eq(expected_pos, 1e-4),
                "Iteration {}: expected {:?}, got {:?}",
                i,
                expected_pos,
                actual_pos
            );
        }
    }
}
