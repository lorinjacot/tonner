use std::time::Duration;

use glam::{DMat3, DQuat, DVec3};
use log::info;
#[cfg(feature = "pyo3")]
use log::warn;
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
pub mod collision;
pub mod constraint;
pub mod joint;
mod particle;
mod rigid_body;
pub mod shape;
mod transform;

#[derive(Debug, Clone)]
struct PositionalData {
    position: DVec3,
    previous_position: DVec3,
    velocity: DVec3,
    previous_velocity: DVec3,
    inverse_mass: f64,
    force: DVec3,
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

#[derive(Debug, Clone)]
#[cfg_attr(feature = "pyo3", pyclass(skip_from_py_object))]
pub struct State {
    bodies: KeyRegistry,
    particles: SecondaryMap<()>,
    rigid_bodies: RigidBodies,
    positional_data: SecondaryMap<PositionalData>,
    angular_data: SecondaryMap<AngularData>,
    particle_constraints: ParticleConstraintManager,
    joints: JointManager,
}

impl State {
    pub fn new() -> Self {
        State {
            bodies: KeyRegistry::new(),
            particles: SecondaryMap::new(),
            rigid_bodies: RigidBodies::new(),
            positional_data: SecondaryMap::new(),
            angular_data: SecondaryMap::new(),
            particle_constraints: ParticleConstraintManager::new(),
            joints: JointManager::new(),
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
}

#[cfg(feature = "pyo3")]
#[pymethods]
impl State {
    #[new]
    fn py_new() -> Self {
        State::new()
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
}

impl Solver {
    pub fn simulate(&mut self, state: &mut State, delta_time: Duration) {
        assert!(self.substep_count > 0, "substep_count must be > 0");
        if delta_time.is_zero() {
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
            for d in state.positional_data.values_mut() {
                d.previous_position = d.position;
                d.previous_velocity = d.velocity;
                d.velocity += h * d.force * d.inverse_mass;
                d.position += h * d.velocity;
            }

            for d in state.angular_data.values_mut() {
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

            state.rigid_bodies.solve_positions(
                inverse_h_squared,
                &mut state.positional_data,
                &mut state.angular_data,
            );

            state
                .particle_constraints
                .solve_positions(&mut state.positional_data, inverse_h_squared);

            state.joints.solve_positions(
                &mut state.positional_data,
                &mut state.angular_data,
                inverse_h_squared,
            );

            for d in state.positional_data.values_mut() {
                d.velocity = (d.position - d.previous_position) / h;
            }

            for d in state.angular_data.values_mut() {
                let delta_orientation = d.orientation * d.previous_orientation.conjugate();
                d.velocity = 2.0 * delta_orientation.xyz() / h;
                if delta_orientation.w < 0.0 {
                    d.velocity = -d.velocity;
                }
            }

            // solve velocities
            state.rigid_bodies.solve_velocities(
                &mut state.positional_data,
                &mut state.angular_data,
                h,
            );
        }
    }
}

impl Default for Solver {
    fn default() -> Self {
        Solver { substep_count: 10 }
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
    fn set_substep_count(&mut self, mut value: u32) {
        if value < 1 {
            warn!("substep_count must be > 0, got {}. Setting to 1.", value);
            value = 1;
        }
        self.substep_count = value;
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
    use super::{BodyId, Solver, State, joint::AttachJointId};
}

#[cfg(test)]
mod tests {
    use glam::dvec3;

    use super::*;

    #[test]
    fn test_implicit_euler() {
        const ITERATOR_COUNT: usize = 10;
        const DELTA_TIME: Duration = Duration::from_millis(1);
        const DT: f64 = DELTA_TIME.as_secs_f64();

        let p0 = dvec3(1.0, 2.0, 3.0);
        let v0 = dvec3(10.0, 20.0, 30.0);
        let f = dvec3(100.0, 200.0, 300.0);
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
                previous_velocity: v0,
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

    mod pendulum {
        pub const L1: f64 = 1.0;
        pub const L2: f64 = 1.0;
        pub const M0: f64 = f64::INFINITY;
        pub const M1: f64 = 1.0;
        pub const M2: f64 = 1.0;
        pub const G: f64 = 9.81;

        pub fn theta1_ddot(theta1: f64, theta1_dot: f64, theta2: f64, theta2_dot: f64) -> f64 {
            let num = -G * (2.0 * M1 + M2) * theta1.sin()
                - M2 * G * (theta1 - 2.0 * theta2).sin()
                - 2.0
                    * (theta1 - theta2).sin()
                    * M2
                    * (theta2_dot.powi(2) * L2 + theta1_dot.powi(2) * L1 * (theta1 - theta2).cos());
            let den = L1 * (2.0 * M1 + M2 - M2 * (2.0 * theta1 - 2.0 * theta2).cos());
            num / den
        }

        pub fn theta2_ddot(theta1: f64, theta1_dot: f64, theta2: f64, theta2_dot: f64) -> f64 {
            let num = 2.0
                * (theta1 - theta2).sin()
                * (theta1_dot.powi(2) * L1 * (M1 + M2)
                    + G * (M1 + M2) * theta1.cos()
                    + theta2_dot.powi(2) * L2 * M2 * (theta1 - theta2).cos());
            let den = L2 * (2.0 * M1 + M2 - M2 * (2.0 * theta1 - 2.0 * theta2).cos());
            num / den
        }
    }

    #[test]
    fn test_double_pendulum() {
        let mut state = State::new();
        let a = ParticleBuilder::default()
            .mass(pendulum::M0)
            .position([0.0, 0.0, 0.0])
            .build(&mut state);
        let b = ParticleBuilder::default()
            .mass(pendulum::M1)
            .position([pendulum::L1, 0.0, 0.0])
            .build(&mut state);
        let c = ParticleBuilder::default()
            .mass(pendulum::M2)
            .position([pendulum::L1 + pendulum::L2, 0.0, 0.0])
            .build(&mut state);

        state.add_particle_distance_constraint(ParticleDistanceConstraint {
            particles: [a, b],
            distance: pendulum::L1,
            compliance: 0.0,
        });
        state.add_particle_distance_constraint(ParticleDistanceConstraint {
            particles: [b, c],
            distance: pendulum::L2,
            compliance: 0.0,
        });

        state.force_mut(b).unwrap().y -= pendulum::M1 * pendulum::G;
        state.force_mut(c).unwrap().y -= pendulum::M2 * pendulum::G;

        let time_step = Duration::from_millis(10);
        let mut solver = Solver::default();

        let mut theta1 = std::f64::consts::FRAC_PI_2;
        let mut theta1_dot = 0.0;
        let mut theta2 = std::f64::consts::FRAC_PI_2;
        let mut theta2_dot = 0.0;

        for iteration in 0..100 {
            solver.simulate(&mut state, time_step);

            theta1_dot += pendulum::theta1_ddot(theta1, theta1_dot, theta2, theta2_dot)
                * time_step.as_secs_f64();
            theta2_dot += pendulum::theta2_ddot(theta1, theta1_dot, theta2, theta2_dot)
                * time_step.as_secs_f64();

            theta1 += theta1_dot * time_step.as_secs_f64();
            theta2 += theta2_dot * time_step.as_secs_f64();

            let expected_b_pos = DVec3::new(
                pendulum::L1 * theta1.sin(),
                -pendulum::L1 * theta1.cos(),
                0.0,
            );
            let expected_c_pos = expected_b_pos
                + DVec3::new(
                    pendulum::L2 * theta2.sin(),
                    -pendulum::L2 * theta2.cos(),
                    0.0,
                );
            let actual_b_pos = state.position(b).unwrap();
            let actual_c_pos = state.position(c).unwrap();

            let max_abs_diff = 1e-2 + iteration as f64 * 1e-3;
            assert!(
                actual_b_pos.abs_diff_eq(expected_b_pos, max_abs_diff),
                "particle b: expected b at {:?}, got {:?} at iteration {}",
                expected_b_pos,
                actual_b_pos,
                iteration
            );
            assert!(
                actual_c_pos.abs_diff_eq(expected_c_pos, max_abs_diff),
                "particle c: expected c at {:?}, got {:?} at iteration {}",
                expected_c_pos,
                actual_c_pos,
                iteration
            );
        }
    }

    #[test]
    fn test_rotation() {
        let mut state = State::new();
        let body = RigidBodyBuilder::default()
            .mass(1.0)
            .inertia(DMat3::IDENTITY)
            .angular_velocity([0.0, 1.0, 0.0])
            .box3d(shape::Box3D::from_dimensions(1.0, 1.0, 1.0))
            .build(&mut state);

        let time_step = Duration::from_millis(100);
        let mut solver = Solver::default();

        for iteration in 0..100 {
            solver.simulate(&mut state, time_step);
            let orientation = state.orientation(body).unwrap();
            let expected_angle = (iteration + 1) as f64 * time_step.as_secs_f64();
            let expected_orientation = DQuat::from_axis_angle(DVec3::Y, expected_angle);
            let max_abs_diff = 1e-2 + iteration as f64 * 1e-3;
            assert!(
                orientation.abs_diff_eq(expected_orientation, max_abs_diff),
                "expected orientation {:?}, got {:?} at iteration {}",
                expected_orientation,
                orientation,
                iteration
            );
        }
    }
}
