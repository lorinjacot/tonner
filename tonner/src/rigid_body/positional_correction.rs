use glam::{DMat3, DQuat, DVec3};
use sparse_keyed::SecondaryMap;

use crate::{BodyId, PositionalData};

#[derive(Debug, Clone)]
pub(crate) struct PositionalCorrection {
    pub direction: DVec3,
    pub magnitude: f64,
    /// Expressed in local frame (body space)
    pub application_points: [DVec3; 2],
    pub compliance: f64,
}

impl PositionalCorrection {
    /// Delta_lambda = -c / (w1 + w2 + alpha_hat) in the paper
    pub fn lagrange_multiplier(
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

    /// Delta_x = p / m in the paper
    fn linear_corrections(&self, lagrange_multiplier: f64, inverse_masses: [f64; 2]) -> [DVec3; 2] {
        let p = lagrange_multiplier * self.direction;
        [p * inverse_masses[0], -p * inverse_masses[1]]
    }

    /// Delta_q = 1/2 * [I^-1 * (r x p), 0] * q in the paper
    fn angular_corrections(
        &self,
        lagrange_multiplier: f64,
        inverse_inertias: [DMat3; 2],
        orientations: [DQuat; 2],
    ) -> [DQuat; 2] {
        let p = lagrange_multiplier * self.direction;
        let local_p = orientations.map(|o| o.conjugate() * p);
        let r_cross_p = [
            self.application_points[0].cross(local_p[0]),
            self.application_points[1].cross(local_p[1]),
        ];
        let angles = [
            inverse_inertias[0] * r_cross_p[0],
            inverse_inertias[1] * r_cross_p[1],
        ];
        let corrections = angles.map(|a| DQuat::from_xyzw(a.x, a.y, a.z, 0.0) * 0.5);
        [
            orientations[0] * corrections[0],
            -orientations[1] * corrections[1],
        ]
    }

    fn apply_linear(
        &self,
        lagrange_multiplier: f64,
        inverse_masses: [f64; 2],
        bodies: [BodyId; 2],
        positional_data: &mut SecondaryMap<PositionalData>,
    ) {
        let linear_corrections = self.linear_corrections(lagrange_multiplier, inverse_masses);
        for i in 0..2 {
            positional_data[bodies[i].0].position += linear_corrections[i];
        }
    }

    fn apply_angular(
        &self,
        lagrange_multiplier: f64,
        inverse_inertias: [DMat3; 2],
        orientations: [DQuat; 2],
        bodies: [BodyId; 2],
        angular_data: &mut SecondaryMap<crate::AngularData>,
    ) {
        let angular_corrections =
            self.angular_corrections(lagrange_multiplier, inverse_inertias, orientations);
        for i in 0..2 {
            let q = &mut angular_data[bodies[i].0].orientation;
            *q = (*q + angular_corrections[i]).normalize();
        }
    }

    pub fn apply(
        &self,
        lagrange_multiplier: f64,
        inverse_masses: [f64; 2],
        inverse_inertias: [DMat3; 2],
        orientations: [DQuat; 2],
        bodies: [BodyId; 2],
        positional_data: &mut SecondaryMap<PositionalData>,
        angular_data: &mut SecondaryMap<crate::AngularData>,
    ) {
        self.apply_linear(lagrange_multiplier, inverse_masses, bodies, positional_data);
        self.apply_angular(
            lagrange_multiplier,
            inverse_inertias,
            orientations,
            bodies,
            angular_data,
        );
    }
}
