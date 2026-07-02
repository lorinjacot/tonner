use std::time::Duration;

use glam::{DMat3, DQuat, DVec3};
use log::{info, warn};
#[cfg(feature = "pyo3")]
use pyo3::{exceptions::PyValueError, prelude::*};
use sparse_keyed::{Key, KeyRegistry, SecondaryMap};

use crate::constraint::particle::{ParticleDistanceConstraint, ParticleDistanceConstraintId};
use crate::joint::JointManager;
use crate::{constraint::particle::ParticleConstraintManager, rigid_body::RigidBodies};
pub use aabb::AABB;
pub use particle::ParticleBuilder;
pub use rigid_body::RigidBodyBuilder;
pub use transform::Transform;

mod aabb;
mod collision;
pub mod constraint;
pub mod joint;
mod particle;
mod rigid_body;
pub mod shape;
mod transform;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "pyo3", pyclass(frozen, from_py_object))]
pub struct BodyId(Key);

#[derive(Debug, Clone)]
struct PositionalData {
    position: DVec3,
    previous_position: DVec3,
    velocity: DVec3,
    previous_velocity: DVec3,
    inverse_mass: f64,
    force: DVec3,
}

impl Default for PositionalData {
    fn default() -> Self {
        Self {
            position: DVec3::ZERO,
            previous_position: DVec3::ZERO,
            velocity: DVec3::ZERO,
            previous_velocity: DVec3::ZERO,
            inverse_mass: 0.0,
            force: DVec3::ZERO,
        }
    }
}

#[derive(Debug, Clone)]
struct AngularData {
    orientation: DQuat,
    previous_orientation: DQuat,
    velocity: DVec3,
    previous_velocity: DVec3,
    inertia: DMat3,
    inverse_inertia: DMat3,
    torque: DVec3,
}

impl Default for AngularData {
    fn default() -> Self {
        Self {
            orientation: DQuat::IDENTITY,
            previous_orientation: DQuat::IDENTITY,
            velocity: DVec3::ZERO,
            previous_velocity: DVec3::ZERO,
            inertia: DMat3::IDENTITY,
            inverse_inertia: DMat3::IDENTITY,
            torque: DVec3::ZERO,
        }
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "pyo3", pyclass)]
pub struct Engine {
    bodies: KeyRegistry,
    particles: SecondaryMap<()>,
    rigid_bodies: RigidBodies,
    positional_data: SecondaryMap<PositionalData>,
    angular_data: SecondaryMap<AngularData>,
    particle_constraints: ParticleConstraintManager,
    joints: JointManager,
    substep_count: u32,
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            bodies: KeyRegistry::new(),
            particles: SecondaryMap::new(),
            rigid_bodies: RigidBodies::new(),
            positional_data: SecondaryMap::new(),
            angular_data: SecondaryMap::new(),
            particle_constraints: ParticleConstraintManager::new(),
            joints: JointManager::new(),
            substep_count: 10,
        }
    }

    pub fn is_particle(&self, body: BodyId) -> bool {
        self.particles.contains(body.0)
    }

    pub fn is_rigid_body(&self, body: BodyId) -> bool {
        self.rigid_bodies.is_rigid_body(body)
    }

    pub fn position(&self, body: BodyId) -> Option<DVec3> {
        self.positional_data.get(body.0).map(|data| data.position)
    }

    pub fn position_mut(&mut self, body: BodyId) -> Option<&mut DVec3> {
        self.positional_data
            .get_mut(body.0)
            .map(|data| &mut data.position)
    }

    pub fn velocity(&self, body: BodyId) -> Option<DVec3> {
        self.positional_data.get(body.0).map(|data| data.velocity)
    }

    pub fn velocity_mut(&mut self, body: BodyId) -> Option<&mut DVec3> {
        self.positional_data
            .get_mut(body.0)
            .map(|data| &mut data.velocity)
    }

    pub fn mass(&self, body: BodyId) -> Option<f64> {
        self.positional_data
            .get(body.0)
            .map(|data| data.inverse_mass.recip())
    }

    pub fn inverse_mass(&self, body: BodyId) -> Option<f64> {
        self.positional_data
            .get(body.0)
            .map(|data| data.inverse_mass)
    }

    pub fn inverse_mass_mut(&mut self, body: BodyId) -> Option<&mut f64> {
        self.positional_data
            .get_mut(body.0)
            .map(|data| &mut data.inverse_mass)
    }

    pub fn force(&self, body: BodyId) -> Option<DVec3> {
        self.positional_data.get(body.0).map(|data| data.force)
    }

    pub fn force_mut(&mut self, body: BodyId) -> Option<&mut DVec3> {
        self.positional_data
            .get_mut(body.0)
            .map(|data| &mut data.force)
    }

    pub fn orientation(&self, body: BodyId) -> Option<DQuat> {
        self.angular_data.get(body.0).map(|data| data.orientation)
    }

    pub fn orientation_mut(&mut self, body: BodyId) -> Option<&mut DQuat> {
        self.angular_data
            .get_mut(body.0)
            .map(|data| &mut data.orientation)
    }

    pub fn angular_velocity(&self, body: BodyId) -> Option<DVec3> {
        self.angular_data.get(body.0).map(|data| data.velocity)
    }

    pub fn angular_velocity_mut(&mut self, body: BodyId) -> Option<&mut DVec3> {
        self.angular_data
            .get_mut(body.0)
            .map(|data| &mut data.velocity)
    }

    pub fn inertia(&self, body: BodyId) -> Option<DMat3> {
        self.angular_data.get(body.0).map(|data| data.inertia)
    }

    pub fn inverse_inertia(&self, body: BodyId) -> Option<DMat3> {
        self.angular_data
            .get(body.0)
            .map(|data| data.inverse_inertia)
    }

    pub fn torque(&self, body: BodyId) -> Option<DVec3> {
        self.angular_data.get(body.0).map(|data| data.torque)
    }

    pub fn torque_mut(&mut self, body: BodyId) -> Option<&mut DVec3> {
        self.angular_data
            .get_mut(body.0)
            .map(|data| &mut data.torque)
    }

    pub fn add_particle_distance_constraint(
        &mut self,
        constraint: ParticleDistanceConstraint,
    ) -> ParticleDistanceConstraintId {
        self.particle_constraints
            .add_distance_constraint(constraint)
    }

    pub fn substep_count(&self) -> u32 {
        self.substep_count
    }

    pub fn set_substep_count(&mut self, value: u32) {
        if value < 1 {
            warn!("substep_count must be > 0, got {}. Setting to 1.", value);
            self.substep_count = 1;
        } else {
            self.substep_count = value;
        }
    }

    pub fn simulate(&mut self, delta_time: Duration) {
        assert!(self.substep_count > 0, "substep_count must be > 0");
        if delta_time.is_zero() {
            warn!("delta_time is zero, nothing to simulate.");
            return;
        }
        info!(
            "Simulating for {:?} with {} substeps",
            delta_time, self.substep_count
        );
        let substep_duration = delta_time / self.substep_count;
        let h = substep_duration.as_secs_f64();
        let h_squared = h * h;
        for _ in 0..self.substep_count {
            for d in self.positional_data.values_mut() {
                d.previous_position = d.position;
                d.previous_velocity = d.velocity;
                d.velocity += h * d.force * d.inverse_mass;
                d.position += h * d.velocity;
            }

            for d in self.angular_data.values_mut() {
                d.previous_orientation = d.orientation;
                d.previous_velocity = d.velocity;
                d.velocity +=
                    h * d.inverse_inertia * (d.torque - d.velocity.cross(d.inertia * d.velocity));
                d.orientation = d.orientation
                    + DQuat::from_xyzw(d.velocity.x, d.velocity.y, d.velocity.z, 0.0)
                        * d.orientation
                        * h
                        * 0.5;
                d.orientation = d.orientation.normalize();
            }

            let inverse_h_squared = 1.0 / h_squared;

            self.rigid_bodies.solve_positions(
                inverse_h_squared,
                &mut self.positional_data,
                &mut self.angular_data,
            );

            self.particle_constraints
                .solve_positions(&mut self.positional_data, inverse_h_squared);

            self.joints.solve_positions(
                &mut self.positional_data,
                &mut self.angular_data,
                inverse_h_squared,
            );

            for d in self.positional_data.values_mut() {
                d.velocity = (d.position - d.previous_position) / h;
            }

            for d in self.angular_data.values_mut() {
                let delta_orientation = d.orientation * d.previous_orientation.conjugate();
                d.velocity = 2.0 * delta_orientation.xyz() / h;
                if delta_orientation.w < 0.0 {
                    d.velocity = -d.velocity;
                }
            }

            // solve velocities
            self.rigid_bodies.solve_velocities(
                &mut self.positional_data,
                &mut self.angular_data,
                h,
            );
        }
    }
}

#[cfg(feature = "pyo3")]
#[pymethods]
impl Engine {
    #[new]
    fn py_new() -> Self {
        Engine::new()
    }

    #[pyo3(signature = (
        position=[0.0; 3],
        velocity=[0.0; 3],
        mass=f64::INFINITY
    ))]
    fn add_particle(
        &mut self,
        position: [f64; 3],
        velocity: [f64; 3],
        mass: f64,
    ) -> PyResult<BodyId> {
        if mass <= 0.0 {
            return Err(PyValueError::new_err("Mass must be strictly positive."));
        }
        Ok(ParticleBuilder::default()
            .position(position)
            .velocity(velocity)
            .mass(mass)
            .build(self))
    }

    #[pyo3(signature = (
        position=[0.0; 3],
        velocity=[0.0; 3],
        mass=f64::INFINITY,
        orientation=[0.0, 0.0, 0.0, 1.0],
        angular_velocity=[0.0; 3],
        inertia=[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        radius=1.0
    ))]
    fn add_rigid_ball(
        &mut self,
        position: [f64; 3],
        velocity: [f64; 3],
        mass: f64,
        orientation: [f64; 4],
        angular_velocity: [f64; 3],
        inertia: [[f64; 3]; 3],
        radius: f64,
    ) -> PyResult<BodyId> {
        if mass <= 0.0 {
            return Err(PyValueError::new_err("Mass must be strictly positive."));
        }
        let inertia = DMat3::from_cols_array_2d(&inertia);
        if inertia.determinant() <= 0.0 {
            return Err(PyValueError::new_err("Inertia must be positive definite."));
        }
        let orientation = DQuat::from_array(orientation);
        let ball = shape::Ball::from_radius(radius);

        Ok(RigidBodyBuilder::default()
            .position(position)
            .velocity(velocity)
            .mass(mass)
            .orientation(orientation)
            .angular_velocity(angular_velocity)
            .inertia(inertia)
            .ball(ball)
            .build(self))
    }

    #[pyo3(signature = (
        position=[0.0; 3],
        velocity=[0.0; 3],
        mass=f64::INFINITY,
        orientation=[0.0, 0.0, 0.0, 1.0],
        angular_velocity=[0.0; 3],
        inertia=[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        dimensions=[1.0; 3]
    ))]
    fn add_rigid_box(
        &mut self,
        position: [f64; 3],
        velocity: [f64; 3],
        mass: f64,
        orientation: [f64; 4],
        angular_velocity: [f64; 3],
        inertia: [[f64; 3]; 3],
        dimensions: [f64; 3],
    ) -> PyResult<BodyId> {
        if mass <= 0.0 {
            return Err(PyValueError::new_err("Mass must be strictly positive."));
        }
        let inertia = DMat3::from_cols_array_2d(&inertia);
        if inertia.determinant() <= 0.0 {
            return Err(PyValueError::new_err("Inertia must be positive definite."));
        }
        let orientation = DQuat::from_array(orientation);
        let box_ = shape::Box3D::from_dimensions(dimensions[0], dimensions[1], dimensions[2]);

        Ok(RigidBodyBuilder::default()
            .position(position)
            .velocity(velocity)
            .mass(mass)
            .orientation(orientation)
            .angular_velocity(angular_velocity)
            .inertia(inertia)
            .box3d(box_)
            .build(self))
    }

    fn add_force(&mut self, body: BodyId, force: [f64; 3]) -> PyResult<()> {
        let f = self
            .force_mut(body)
            .ok_or_else(|| PyValueError::new_err("Invalid body ID"))?;
        *f += DVec3::from_array(force);
        Ok(())
    }

    #[pyo3(name = "add_particle_distance_constraint", signature = (
        particles,
        distance = 0.0,
        compliance = 0.0
    ))]
    fn py_add_particle_distance_constraint(
        &mut self,
        particles: [BodyId; 2],
        distance: f64,
        compliance: f64,
    ) -> PyResult<ParticleDistanceConstraintId> {
        for &body in &particles {
            if !self.is_particle(body) {
                return Err(PyValueError::new_err(format!(
                    "Body {:?} is not a particle",
                    body
                )));
            }
        }
        let constraint = ParticleDistanceConstraint {
            particles,
            distance,
            compliance,
        };
        Ok(self.add_particle_distance_constraint(constraint))
    }

    #[pyo3(name = "add_attach_joint", signature = (
        bodies,
        rest_distance = 0.0,
        attachment_points = [[0.0; 3]; 2],
        compliance = 0.0
    ))]
    fn py_add_attach_joint(
        &mut self,
        bodies: [BodyId; 2],
        rest_distance: f64,
        attachment_points: [[f64; 3]; 2],
        compliance: f64,
    ) -> PyResult<joint::AttachJointId> {
        for &body in &bodies {
            if !self.is_rigid_body(body) {
                return Err(PyValueError::new_err(format!(
                    "Body {:?} is not a rigid body",
                    body
                )));
            }
        }
        let attachment_points = [
            DVec3::from_array(attachment_points[0]),
            DVec3::from_array(attachment_points[1]),
        ];
        let joint = joint::AttachJointBuilder::new(bodies[0], bodies[1])
            .rest_distance(rest_distance)
            .attachment_points(attachment_points)
            .compliance(compliance)
            .build_and_add(self);
        Ok(joint)
    }

    #[pyo3(name = "position")]
    fn py_position(&self, body: BodyId) -> PyResult<[f64; 3]> {
        self.position(body)
            .map(|p| p.to_array())
            .ok_or_else(|| PyValueError::new_err("Invalid body ID"))
    }

    #[pyo3(name = "velocity")]
    fn py_velocity(&self, body: BodyId) -> PyResult<[f64; 3]> {
        self.velocity(body)
            .map(|v| v.to_array())
            .ok_or_else(|| PyValueError::new_err("Invalid body ID"))
    }

    #[pyo3(name = "mass")]
    fn py_mass(&self, body: BodyId) -> PyResult<f64> {
        self.mass(body)
            .ok_or_else(|| PyValueError::new_err("Invalid body ID"))
    }

    #[pyo3(name = "orientation")]
    fn py_orientation(&self, body: BodyId) -> PyResult<[f64; 4]> {
        self.orientation(body)
            .map(|o| o.to_array())
            .ok_or_else(|| PyValueError::new_err("Invalid body ID"))
    }

    #[pyo3(name = "angular_velocity")]
    fn py_angular_velocity(&self, body: BodyId) -> PyResult<[f64; 3]> {
        self.angular_velocity(body)
            .map(|v| v.to_array())
            .ok_or_else(|| PyValueError::new_err("Invalid body ID"))
    }

    #[pyo3(name = "inertia")]
    fn py_inertia(&self, body: BodyId) -> PyResult<[[f64; 3]; 3]> {
        self.inertia(body)
            .map(|i| i.to_cols_array_2d())
            .ok_or_else(|| PyValueError::new_err("Invalid body ID"))
    }

    #[getter(substep_count)]
    fn py_substep_count(&self) -> u32 {
        self.substep_count()
    }

    #[setter(substep_count)]
    fn py_set_substep_count(&mut self, value: u32) {
        self.set_substep_count(value);
    }

    #[pyo3(name = "simulate")]
    fn py_simulate(&mut self, delta_time: Duration) {
        self.simulate(delta_time);
    }
}

#[cfg(feature = "pyo3")]
#[pymodule(name = "tonner")]
mod py_tonner {
    use pyo3::prelude::*;

    #[pymodule_init]
    fn init(_: &Bound<'_, PyModule>) -> PyResult<()> {
        pyo3_log::init();
        Ok(())
    }

    #[pymodule_export]
    use super::{BodyId, Engine, joint::AttachJointId};
}
