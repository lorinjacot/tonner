use glam::{DQuat, DVec3};
use log::error;
use sparse_keyed::SecondaryMap;

use crate::{
    AngularData, BodyId, PositionalData,
    rigid_body::{generalized_inverse_mass, positional_correction::PositionalCorrection},
};

#[derive(Debug, Clone)]
pub(crate) struct Contact {
    pub bodies: [BodyId; 2],
    pub world_normal: DVec3,
    pub local_contact_points: [DVec3; 2],
    pub static_friction_coefficient: f64,
}

impl Contact {
    fn contact_positions(&self, positions: [DVec3; 2], orientations: [DQuat; 2]) -> [DVec3; 2] {
        [
            positions[0] + orientations[0] * self.local_contact_points[0],
            positions[1] + orientations[1] * self.local_contact_points[1],
        ]
    }

    fn normal_correction(&self, contact_positions: [DVec3; 2]) -> Option<PositionalCorrection> {
        let penetration = (contact_positions[0] - contact_positions[1]).dot(self.world_normal);
        if penetration <= 0.0 {
            None
        } else {
            Some(PositionalCorrection {
                direction: self.world_normal,
                magnitude: penetration,
                application_points: self.local_contact_points,
                compliance: 0.0,
            })
        }
    }

    fn tangential_correction(
        &self,
        contact_positions: [DVec3; 2],
        previous_contact_positions: [DVec3; 2],
    ) -> PositionalCorrection {
        let delta_p = (contact_positions[0] - previous_contact_positions[0])
            - (contact_positions[1] - previous_contact_positions[1]);
        let delta_tangential = delta_p - delta_p.dot(self.world_normal) * self.world_normal;
        let (direction, magnitude) = delta_tangential.normalize_and_length();
        PositionalCorrection {
            direction,
            magnitude,
            application_points: self.local_contact_points,
            compliance: 0.0,
        }
    }

    pub fn solve_positions(
        &self,
        positional_data: &mut SecondaryMap<PositionalData>,
        angular_data: &mut SecondaryMap<AngularData>,
        inverse_h_squared: f64,
    ) {
        let d_pos = self.bodies.map(|b| &positional_data[b.0]);
        let d_ang = self.bodies.map(|b| &angular_data[b.0]);

        let generalized_inverse_masses = [
            generalized_inverse_mass(
                d_pos[0].inverse_mass,
                d_ang[0].inverse_inertia,
                self.local_contact_points[0],
                d_ang[0].orientation.conjugate() * self.world_normal,
            ),
            generalized_inverse_mass(
                d_pos[1].inverse_mass,
                d_ang[1].inverse_inertia,
                self.local_contact_points[1],
                d_ang[1].orientation.conjugate() * self.world_normal,
            ),
        ];

        let contact_positions = self.contact_positions(
            [d_pos[0].position, d_pos[1].position],
            [d_ang[0].orientation, d_ang[1].orientation],
        );

        let Some(normal_correction) = self.normal_correction(contact_positions) else {
            return;
        };

        let Ok(normal_multiplier) =
            normal_correction.lagrange_multiplier(generalized_inverse_masses, inverse_h_squared)
        else {
            error!(
                "Contact between {:?} and {:?} is unsolveable. This is likely due to infinite masses. Skipping.",
                self.bodies[0], self.bodies[1]
            );
            return;
        };

        let previous_contact_positions = self.contact_positions(
            [d_pos[0].previous_position, d_pos[1].previous_position],
            [d_ang[0].previous_orientation, d_ang[1].previous_orientation],
        );

        let tangential_correction =
            self.tangential_correction(contact_positions, previous_contact_positions);

        let Ok(tangential_multiplier) = tangential_correction
            .lagrange_multiplier(generalized_inverse_masses, inverse_h_squared)
        else {
            error!(
                "Contact between {:?} and {:?} is unsolveable. This is likely due to infinite masses. Skipping.",
                self.bodies[0], self.bodies[1]
            );
            return;
        };

        let inverse_masses = [d_pos[0].inverse_mass, d_pos[1].inverse_mass];
        let inverse_inertias = [d_ang[0].inverse_inertia, d_ang[1].inverse_inertia];
        let orientations = [d_ang[0].orientation, d_ang[1].orientation];

        normal_correction.apply(
            normal_multiplier,
            inverse_masses,
            inverse_inertias,
            orientations,
            self.bodies,
            positional_data,
            angular_data,
        );

        if tangential_multiplier < self.static_friction_coefficient * normal_multiplier {
            tangential_correction.apply(
                tangential_multiplier,
                inverse_masses,
                inverse_inertias,
                orientations,
                self.bodies,
                positional_data,
                angular_data,
            );
        }
    }
}
