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
        // r x n in the paper
        let local_rotation_axis = [0, 1].map(|i| {
            let local_dir = orientations[i].conjugate() * self.direction;
            self.application_points[i].cross(local_dir)
        });

        // I^-1 * (r x n) in the paper
        let local_angular_corrections =
            [0, 1].map(|i| inverse_inertias[i] * local_rotation_axis[i]);

        // w = 1/m + (r x n)^T * I^-1 * (r x n) in the paper
        let w = [0, 1]
            .map(|i| inverse_masses[i] + local_rotation_axis[i].dot(local_angular_corrections[i]));

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
        [0, 1].map(|i| self.value * self.correction.direction * self.inverse_masses[i])
    }

    pub fn angular_correction(&self) -> [DQuat; 2] {
        // 1/2 * [I^-1 * (r x p), 0] * q = 1/2 * [delta_lambda * I^-1 * (r x r), 0] * q in the paper
        [0, 1].map(|i| {
            let q = self.orientations[i];
            let angle = q * self.local_angular_corrections[i] * self.value;
            DQuat::from_xyzw(angle.x, angle.y, angle.z, 0.0) * q * 0.5
        })
    }
}
