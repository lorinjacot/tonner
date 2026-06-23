use glam::{DMat3, DQuat, DVec3};

use crate::BodyId;

pub(crate) trait PositionalConstraint {
    fn bodies(&self) -> &[BodyId; 2];

    fn correction(&self, positions: &[DVec3; 2], orientations: &[DQuat; 2])
    -> PositionalCorrection;
}

#[derive(Debug, Clone)]
pub(crate) struct PositionalCorrection {
    pub direction: DVec3,
    pub magnitude: f64,
    /// Expressed in local frame (body space)
    pub application_points: [DVec3; 2],
    pub compliance: f64,
}

impl PositionalCorrection {
    pub fn lagrange_multiplier(
        self,
        inverse_masses: [f64; 2],
        inverse_inertias: [DMat3; 2],
        orientations: [DQuat; 2],
        inverse_timestep_squared: f64,
    ) -> Result<PositionalLagrangeMultiplier, ()> {
        let local_directions = orientations.map(|o| o.conjugate() * self.direction);

        // r x n in the paper
        let local_rotation_axis = [
            self.application_points[0].cross(local_directions[0]),
            self.application_points[1].cross(local_directions[1]),
        ];

        // I^-1 * (r x n) in the paper
        let local_angular_corrections = [
            inverse_inertias[0] * local_rotation_axis[0],
            inverse_inertias[1] * local_rotation_axis[1],
        ];

        // w = 1/m + (r x n)^T * I^-1 * (r x n) in the paper
        let w = [
            inverse_masses[0] + local_rotation_axis[0].dot(local_angular_corrections[0]),
            inverse_masses[1] + local_rotation_axis[1].dot(local_angular_corrections[1]),
        ];

        // w1 + w2 + alpha_hat = w1 + w2 + compliance / (h^2) in the paper
        let denominator = w[0] + w[1] + self.compliance * inverse_timestep_squared;
        if denominator <= 0.0 {
            return Err(());
        }

        Ok(PositionalLagrangeMultiplier {
            value: -self.magnitude / denominator,
            local_angular_corrections,
            inverse_masses,
            orientations,
            correction: self,
        })
    }
}

pub(crate) struct PositionalLagrangeMultiplier {
    correction: PositionalCorrection,
    value: f64,
    inverse_masses: [f64; 2],
    orientations: [DQuat; 2],
    local_angular_corrections: [DVec3; 2],
}

impl PositionalLagrangeMultiplier {
    pub fn linear_correction(&self) -> [DVec3; 2] {
        // p / m = delta_lambda * n / m in the paper
        [
            self.value * self.correction.direction * self.inverse_masses[0],
            -self.value * self.correction.direction * self.inverse_masses[1],
        ]
    }

    pub fn angular_correction(&self) -> [DQuat; 2] {
        // 1/2 * [I^-1 * (r x p), 0] * q = 1/2 * [delta_lambda * I^-1 * (r x r), 0] * q in the pape
        let local_correction_angle = self.local_angular_corrections.map(|a| a * self.value);
        let local_correction_quat =
            local_correction_angle.map(|a| DQuat::from_xyzw(a.x, a.y, a.z, 0.0));
        [
            self.orientations[0] * local_correction_quat[0] * 0.5,
            -self.orientations[1] * local_correction_quat[1] * 0.5,
        ]
    }
}
