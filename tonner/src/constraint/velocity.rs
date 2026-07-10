use glam::{DMat3, DQuat, DVec3};
use sparse_keyed::SecondaryMap;

use crate::{BodyId, PositionalData, rigid_body::generalized_inverse_mass};

pub struct VelocityCorrection {
    /// Expressed in world frame
    pub direction: DVec3,
    pub magnitude: f64,
    /// Expressed in local frame (body space)
    pub application_points: [DVec3; 2],
}

impl VelocityCorrection {
    pub fn prepare(
        &self,
        inverse_masses: [f64; 2],
        orientations: [glam::DQuat; 2],
        inverse_inertias: [glam::DMat3; 2],
    ) -> Result<PreparedVelocityCorrection, ()> {
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

        self.lagrange_multiplier(w)
            .map(|lagrange_multiplier| PreparedVelocityCorrection {
                lagrange_multiplier,
                world_direction: self.direction,
                local_directions,
                local_application_points: self.application_points,
                inverse_masses,
                inverse_inertias,
                orientations,
            })
    }

    fn lagrange_multiplier(&self, generalized_inverse_masses: [f64; 2]) -> Result<f64, ()> {
        let denominator = generalized_inverse_masses[0] + generalized_inverse_masses[1];
        if denominator == 0.0 {
            return Err(());
        }
        Ok(self.magnitude / denominator)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedVelocityCorrection {
    lagrange_multiplier: f64,
    world_direction: DVec3,
    local_directions: [DVec3; 2],
    local_application_points: [DVec3; 2],
    inverse_masses: [f64; 2],
    inverse_inertias: [DMat3; 2],
    orientations: [DQuat; 2],
}

impl PreparedVelocityCorrection {
    fn linear_corrections(&self) -> [DVec3; 2] {
        let p = self.lagrange_multiplier * self.world_direction;
        [p * self.inverse_masses[0], -p * self.inverse_masses[1]]
    }

    fn angular_corrections(&self) -> [DVec3; 2] {
        let local_p = self.local_directions.map(|d| self.lagrange_multiplier * d);
        let r_cross_p = [
            self.local_application_points[0].cross(local_p[0]),
            self.local_application_points[1].cross(local_p[1]),
        ];
        let local_angles = [
            self.inverse_inertias[0] * r_cross_p[0],
            self.inverse_inertias[1] * r_cross_p[1],
        ];
        [
            self.orientations[0] * local_angles[0],
            self.orientations[1] * local_angles[1],
        ]
    }

    pub fn solve_velocities(
        &self,
        bodies: [BodyId; 2],
        positional_data: &mut SecondaryMap<PositionalData>,
        angular_data: &mut SecondaryMap<crate::AngularData>,
    ) {
        let linear_corrections = self.linear_corrections();
        positional_data[bodies[0].0].velocity += linear_corrections[0];
        positional_data[bodies[1].0].velocity += linear_corrections[1];

        let angular_corrections = self.angular_corrections();
        angular_data[bodies[0].0].velocity += angular_corrections[0];
        angular_data[bodies[1].0].velocity += angular_corrections[1];
    }
}
