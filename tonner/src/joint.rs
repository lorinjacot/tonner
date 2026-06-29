#[cfg(feature = "pyo3")]
use pyo3::prelude::*;
use std::fmt::Debug;

use glam::DVec3;
use log::error;
use sparse_keyed::{Key, PrimaryMap, SecondaryMap, primary_map::Values};

use crate::{
    AngularData, BodyId, PositionalData, State,
    rigid_body::{PositionalConstraint, PositionalCorrection, generalized_inverse_mass},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "pyo3", pyclass(frozen, from_py_object))]
pub struct AttachJointId(Key);

#[derive(Debug, Clone)]
pub struct AttachJoint {
    bodies: [BodyId; 2],
    rest_distance: f64,
    /// Expressed in local frame (body space)
    attachment_points: [DVec3; 2],
    compliance: f64,
}

impl PositionalConstraint for AttachJoint {
    fn bodies(&self) -> &[BodyId; 2] {
        &self.bodies
    }

    fn correction(
        &self,
        positions: &[DVec3; 2],
        orientations: &[glam::DQuat; 2],
    ) -> PositionalCorrection {
        let r0 = positions[0] + orientations[0] * self.attachment_points[0];
        let r1 = positions[1] + orientations[1] * self.attachment_points[1];
        let delta_pos = r0 - r1;
        let (direction, distance) = delta_pos.normalize_and_length();
        PositionalCorrection {
            direction,
            magnitude: distance - self.rest_distance,
            application_points: self.attachment_points,
            compliance: self.compliance,
        }
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct AttachJointBuilder(AttachJoint);

impl AttachJointBuilder {
    pub fn new(body_a: BodyId, body_b: BodyId) -> Self {
        Self(AttachJoint {
            bodies: [body_a, body_b],
            rest_distance: 0.0,
            attachment_points: [DVec3::ZERO, DVec3::ZERO],
            compliance: 0.0,
        })
    }

    pub fn rest_distance(mut self, distance: f64) -> Self {
        self.0.rest_distance = distance;
        self
    }

    pub fn attachment_points(mut self, points: [DVec3; 2]) -> Self {
        self.0.attachment_points = points;
        self
    }

    pub fn compliance(mut self, compliance: f64) -> Self {
        self.0.compliance = compliance;
        self
    }

    pub fn build(self) -> AttachJoint {
        self.0
    }

    pub fn build_and_add(self, state: &mut State) -> AttachJointId {
        state.add_attach_joint(self.0)
    }
}

#[derive(Debug, Clone)]
pub(super) struct JointManager {
    attaches: PrimaryMap<AttachJoint>,
}

impl JointManager {
    pub fn new() -> Self {
        JointManager {
            attaches: PrimaryMap::new(),
        }
    }

    pub fn solve_positions(
        &self,
        positional_data: &mut SecondaryMap<PositionalData>,
        angular_data: &mut SecondaryMap<AngularData>,
        inverse_h_squared: f64,
    ) {
        solve_position(
            self.attaches.values(),
            positional_data,
            angular_data,
            inverse_h_squared,
        );
    }
}

fn solve_position<C: PositionalConstraint + Debug>(
    joints: Values<'_, C>,
    positional_data: &mut SecondaryMap<PositionalData>,
    angular_data: &mut SecondaryMap<AngularData>,
    inverse_h_squared: f64,
) {
    'outer: for joint in joints {
        let bodies = joint.bodies();

        let p0 = match positional_data.get(bodies[0].0) {
            Some(data) => data,
            None => {
                invalid_joint_error(joint, bodies[0]);
                continue 'outer;
            }
        };
        let p1 = match positional_data.get(bodies[1].0) {
            Some(data) => data,
            None => {
                invalid_joint_error(joint, bodies[1]);
                continue 'outer;
            }
        };

        let inverse_masses = [p0.inverse_mass, p1.inverse_mass];
        let positions = [p0.position, p1.position];
        let a0 = &angular_data[bodies[0].0];
        let a1 = &angular_data[bodies[1].0];

        let orientations = [a0.orientation, a1.orientation];
        let inverse_inertias = [a0.inverse_inertia, a1.inverse_inertia];

        let correction = joint.correction(&positions, &orientations);

        let generalized_inverse_masses = [
            generalized_inverse_mass(
                inverse_masses[0],
                inverse_inertias[0],
                correction.application_points[0],
                orientations[0].conjugate() * correction.direction,
            ),
            generalized_inverse_mass(
                inverse_masses[1],
                inverse_inertias[1],
                correction.application_points[1],
                orientations[1].conjugate() * correction.direction,
            ),
        ];

        match correction.lagrange_multiplier(generalized_inverse_masses, inverse_h_squared) {
            Ok(lagrange_multiplier) => {
                correction.apply(
                    lagrange_multiplier,
                    inverse_masses,
                    inverse_inertias,
                    orientations,
                    *bodies,
                    positional_data,
                    angular_data,
                );
            }
            Err(_) => {
                unsolveable_joint_error(joint);
            }
        }
    }
}

impl State {
    fn add_attach_joint(&mut self, joint: AttachJoint) -> AttachJointId {
        let id = self.joints.attaches.add(joint);
        AttachJointId(id)
    }
}

fn invalid_joint_error(joint: &impl Debug, body_id: BodyId) {
    error!("Body {body_id:?} from joint {joint:?} not found. Skipping constraint.");
}

fn unsolveable_joint_error(joint: &impl Debug) {
    error!(
        "Constraint {joint:?} is unsolveable. This is likely due to a zero gradient or infinite masses. Skipping constraint."
    );
}
