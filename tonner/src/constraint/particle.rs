use std::{any::TypeId, collections::HashMap, fmt::Debug, marker::PhantomData};

use glam::DVec3;
use log::{error, warn};
#[cfg(feature = "pyo3")]
use pyo3::prelude::*;
use sparse_keyed::{Key, PrimaryMap, SecondaryMap};
use thiserror::Error;

use crate::{BodyId, PositionalData};

/// A particle constraint is an equation involving the positions of a set of particles that must be satisfied.
pub trait ParticleConstraint {
    /// Returns the particles involved in the constraint. This should always return the same set of particles in the same order for the same constraint.
    fn particles(&self) -> &[BodyId];

    /// Evaluates the constraint for the given `positions` of the particles and returns the value of the constraint violation. The value is `0.0` iff the constraint is satisfied. The gradient of the constraint violation with respect to the positions of the particles should be written to `gradient`.
    fn value(&self, positions: &[DVec3], gradient: &mut [DVec3]) -> f64;

    /// Compliance (inverse of stiffness) of the constraint. Expressed in meters per Newton. Should always be non-negative.
    fn compliance(&self) -> f64;
}

#[derive(Debug, Clone, Error)]
enum SolverError {
    #[error("Particle {0:?} not found")]
    ParticleNotFound(BodyId),
    #[error(
        "Constraint is unsolveable. This is likely due to a zero gradient and/or infinite masses."
    )]
    UnsolveableConstraint,
}

trait Solver<C: ParticleConstraint> {
    fn solve_positions(
        &mut self,
        constraint: &C,
        positional_data: &mut SecondaryMap<PositionalData>,
        inverse_h_squared: f64,
    ) -> Result<(), SolverError>;
}

#[derive(Debug, Clone)]
pub struct GenericSolver<C: ParticleConstraint> {
    positions: Vec<DVec3>,
    gradient: Vec<DVec3>,
    inverse_masses: Vec<f64>,
    constraint: PhantomData<C>,
}

impl<C: ParticleConstraint> GenericSolver<C> {
    pub fn new() -> Self {
        GenericSolver {
            positions: Vec::new(),
            gradient: Vec::new(),
            inverse_masses: Vec::new(),
            constraint: PhantomData,
        }
    }
}

impl<C: ParticleConstraint> Solver<C> for GenericSolver<C> {
    fn solve_positions(
        &mut self,
        constraint: &C,
        positional_data: &mut SecondaryMap<PositionalData>,
        inverse_h_squared: f64,
    ) -> Result<(), SolverError> {
        let particles = constraint.particles();
        let n = particles.len();

        self.positions.clear();
        self.gradient.clear();
        self.inverse_masses.clear();
        self.gradient.resize(n, DVec3::ZERO);

        for particle in particles {
            let Some(d) = positional_data.get(particle.0) else {
                return Err(SolverError::ParticleNotFound(*particle));
            };
            self.positions.push(d.position);
            self.inverse_masses.push(d.inverse_mass);
        }

        let value = constraint.value(&self.positions, &mut self.gradient);
        let compliance = constraint.compliance();

        let weighted_inverse_mass: f64 = self
            .inverse_masses
            .iter()
            .zip(&self.gradient)
            .map(|(inverse_mass, grad)| inverse_mass * grad.length_squared())
            .sum();

        let denominator = weighted_inverse_mass + compliance * inverse_h_squared;
        if denominator == 0.0 {
            return Err(SolverError::UnsolveableConstraint);
        }

        let langrange_multiplier = -value / denominator;
        for ((particle, inverse_mass), grad) in particles
            .iter()
            .zip(&self.inverse_masses)
            .zip(&self.gradient)
        {
            positional_data[particle.0].position += inverse_mass * grad * langrange_multiplier;
        }

        Ok(())
    }
}

trait Container: Debug + Sync + Send {
    fn solve_positions(
        &mut self,
        positional_data: &mut SecondaryMap<PositionalData>,
        inverse_h_squared: f64,
    );

    fn clone_box(&self) -> Box<dyn Container>;
}

#[derive(Debug, Clone)]
struct GenericContainer<C: ParticleConstraint, S: Solver<C>> {
    constraints: PrimaryMap<C>,
    solver: S,
}

impl<C, S> GenericContainer<C, S>
where
    C: ParticleConstraint,
    S: Solver<C>,
{
    pub fn add(&mut self, constraint: C) -> Key {
        self.constraints.add(constraint)
    }
}

impl<C, S> Container for GenericContainer<C, S>
where
    C: ParticleConstraint + Debug + Clone + Sync + Send + 'static,
    S: Solver<C> + Debug + Clone + Sync + Send + 'static,
{
    fn solve_positions(
        &mut self,
        positional_data: &mut SecondaryMap<PositionalData>,
        inverse_h_squared: f64,
    ) {
        for constraint in self.constraints.values() {
            match self
                .solver
                .solve_positions(constraint, positional_data, inverse_h_squared)
            {
                Err(SolverError::ParticleNotFound(particle)) => {
                    error!(
                        "Particle {particle:?} from constraint {constraint:?} not found. Skipping constraint."
                    );
                }
                Err(SolverError::UnsolveableConstraint) => {
                    warn!(
                        "Constraint {constraint:?} is unsolveable. This is likely due to a zero gradient and/or infinite masses. Skipping constraint."
                    );
                }
                Ok(()) => {}
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Container> {
        Box::new(self.clone())
    }
}

#[derive(Debug)]
pub(crate) struct ParticleConstraintManager {
    distance:
        GenericContainer<ParticleDistanceConstraint, GenericSolver<ParticleDistanceConstraint>>,
    others: HashMap<TypeId, Box<dyn Container>>,
}

impl ParticleConstraintManager {
    pub fn new() -> Self {
        ParticleConstraintManager {
            distance: GenericContainer {
                constraints: PrimaryMap::new(),
                solver: GenericSolver::new(),
            },
            others: HashMap::new(),
        }
    }

    pub fn add_distance_constraint(
        &mut self,
        constraint: ParticleDistanceConstraint,
    ) -> ParticleDistanceConstraintId {
        let key = self.distance.add(constraint);
        ParticleDistanceConstraintId(key)
    }

    pub fn solve_positions(
        &mut self,
        positional_data: &mut SecondaryMap<PositionalData>,
        inverse_h_squared: f64,
    ) {
        self.distance
            .solve_positions(positional_data, inverse_h_squared);
    }
}

impl Clone for ParticleConstraintManager {
    fn clone(&self) -> Self {
        ParticleConstraintManager {
            distance: self.distance.clone(),
            others: self
                .others
                .iter()
                .map(|(k, v)| (*k, v.clone_box()))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "pyo3", pyclass(frozen, from_py_object))]
pub struct ParticleDistanceConstraintId(Key);

#[derive(Debug, Clone)]
pub struct ParticleDistanceConstraint {
    pub particles: [BodyId; 2],
    pub distance: f64,
    pub compliance: f64,
}

impl ParticleDistanceConstraint {
    pub fn new(particle_a: BodyId, particle_b: BodyId) -> Self {
        ParticleDistanceConstraint {
            particles: [particle_a, particle_b],
            distance: 0.0,
            compliance: 0.0,
        }
    }

    pub fn with_distance(mut self, distance: f64) -> Self {
        self.distance = distance;
        self
    }

    pub fn with_compliance(mut self, compliance: f64) -> Self {
        self.compliance = compliance;
        self
    }
}

impl ParticleConstraint for ParticleDistanceConstraint {
    fn particles(&self) -> &[BodyId] {
        &self.particles
    }

    fn value(&self, positions: &[DVec3], gradient: &mut [DVec3]) -> f64 {
        let delta_pos = positions[0] - positions[1];
        let (dir, dist) = delta_pos.normalize_and_length();
        gradient[0] = dir;
        gradient[1] = -dir;

        dist - self.distance
    }

    fn compliance(&self) -> f64 {
        self.compliance
    }
}
