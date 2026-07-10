use glam::{DMat3, DQuat, DVec3};
use sparse_keyed::SecondaryMap;

use crate::{AngularData, BodyId, PositionalData, rigid_body::generalized_inverse_mass};

pub trait PositionalConstraint {
    fn bodies(&self) -> &[BodyId; 2];

    fn correction(&self, positions: &[DVec3; 2], orientations: &[DQuat; 2])
    -> PositionalCorrection;
}

#[derive(Debug, Clone)]
pub struct PositionalCorrection {
    /// Expressed in world frame. Unit vector pointing from the first body to the second body.
    pub direction: DVec3,
    pub magnitude: f64,
    /// Expressed in local frame (body space)
    pub application_points: [DVec3; 2],
    pub compliance: f64,
}

impl PositionalCorrection {
    pub fn prepare(
        &self,
        inverse_masses: [f64; 2],
        orientations: [DQuat; 2],
        inverse_inertias: [DMat3; 2],
        inverse_timestep_squared: f64,
    ) -> Result<PreparedPositionalCorrection, ()> {
        let local_directions = orientations.map(|o| o.conjugate() * self.direction);

        let w = [
            generalized_inverse_mass(
                inverse_masses[0],
                inverse_inertias[0],
                self.application_points[0],
                local_directions[0],
            ),
            generalized_inverse_mass(
                inverse_masses[1],
                inverse_inertias[1],
                self.application_points[1],
                local_directions[1],
            ),
        ];

        self.lagrange_multiplier(w, inverse_timestep_squared)
            .map(|lagrange_multiplier| PreparedPositionalCorrection {
                lagrange_multiplier,
                world_direction: self.direction,
                local_directions,
                local_application_points: self.application_points,
                inverse_masses,
                inverse_inertias,
                orientations,
            })
    }

    /// Delta_lambda = -c / (w1 + w2 + alpha_hat) in the paper
    fn lagrange_multiplier(
        &self,
        generalized_inverse_masses: [f64; 2],
        inverse_timestep_squared: f64,
    ) -> Result<f64, ()> {
        // w1 + w2 + alpha_hat = w1 + w2 + compliance / (h^2) in the paper
        let denominator = generalized_inverse_masses[0]
            + generalized_inverse_masses[1]
            + self.compliance * inverse_timestep_squared;
        if denominator <= 0.0 {
            return Err(());
        }

        Ok(-self.magnitude / denominator)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedPositionalCorrection {
    /// Delta_lambda = -c / (w1 + w2 + alpha_hat) in the paper
    lagrange_multiplier: f64,
    world_direction: DVec3,
    local_directions: [DVec3; 2],
    local_application_points: [DVec3; 2],
    inverse_masses: [f64; 2],
    inverse_inertias: [DMat3; 2],
    orientations: [DQuat; 2],
}

impl PreparedPositionalCorrection {
    /// Delta_lambda = -c / (w1 + w2 + alpha_hat) in the paper
    pub fn lagrange_multiplier(&self) -> f64 {
        self.lagrange_multiplier
    }

    /// Delta_x = p / m in the paper
    fn linear_corrections(&self) -> [DVec3; 2] {
        let p = self.lagrange_multiplier * self.world_direction;
        [p * self.inverse_masses[0], -p * self.inverse_masses[1]]
    }

    /// Delta_q = 1/2 * [I^-1 * (r x p), 0] * q in the paper
    fn angular_corrections(&self) -> [DQuat; 2] {
        let local_p = self.local_directions.map(|d| self.lagrange_multiplier * d);
        let r_cross_p = [
            self.local_application_points[0].cross(local_p[0]),
            self.local_application_points[1].cross(local_p[1]),
        ];
        let angles = [
            self.inverse_inertias[0] * r_cross_p[0],
            self.inverse_inertias[1] * r_cross_p[1],
        ];
        let corrections = angles.map(|a| DQuat::from_xyzw(a.x, a.y, a.z, 0.0) * 0.5);
        [
            self.orientations[0] * corrections[0],
            -self.orientations[1] * corrections[1],
        ]
    }

    pub fn solve_positions(
        &self,
        bodies: [BodyId; 2],
        positional_data: &mut SecondaryMap<PositionalData>,
        angular_data: &mut SecondaryMap<AngularData>,
    ) {
        let linear_corrections = self.linear_corrections();
        positional_data[bodies[0].0].position += linear_corrections[0];
        positional_data[bodies[1].0].position += linear_corrections[1];

        let angular_corrections = self.angular_corrections();
        let q0 = &mut angular_data[bodies[0].0].orientation;
        *q0 = (*q0 + angular_corrections[0]).normalize();
        let q1 = &mut angular_data[bodies[1].0].orientation;
        *q1 = (*q1 + angular_corrections[1]).normalize();
    }
}
