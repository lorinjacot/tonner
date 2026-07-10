use glam::{DQuat, DVec3};
use log::error;
use sparse_keyed::SecondaryMap;

use crate::{
    AngularData, BodyId, PositionalData,
    constraint::{
        positional::{PositionalCorrection, PreparedPositionalCorrection},
        velocity::VelocityCorrection,
    },
};

#[derive(Debug, Clone)]
pub struct Contact {
    pub bodies: [BodyId; 2],
    pub world_normal: DVec3,
    pub local_contact_points: [DVec3; 2],
    pub static_friction_coefficient: f64,
    pub dynamic_friction_coefficient: f64,
    pub restitution_coefficient: f64,
}

impl Contact {
    pub fn solve_positions(
        &self,
        positional_data: &mut SecondaryMap<PositionalData>,
        angular_data: &mut SecondaryMap<AngularData>,
        inverse_timestep_squared: f64,
    ) -> Option<SolvedContact> {
        let d_pos = self.bodies.map(|b| &positional_data[b.0]);
        let d_ang = self.bodies.map(|b| &angular_data[b.0]);

        let positions = [d_pos[0].position, d_pos[1].position];
        let inverse_masses = [d_pos[0].inverse_mass, d_pos[1].inverse_mass];
        let orientations = [d_ang[0].orientation, d_ang[1].orientation];
        let inverse_inertias = [d_ang[0].inverse_inertia, d_ang[1].inverse_inertia];

        let Some([normal_correction, tangential_correction]) = self.prepare(
            positions,
            inverse_masses,
            orientations,
            inverse_inertias,
            inverse_timestep_squared,
        ) else {
            return None;
        };

        normal_correction.solve_positions(self.bodies, positional_data, angular_data);

        let lambda_n = normal_correction.lagrange_multiplier();
        let lambda_t = tangential_correction.lagrange_multiplier();
        if lambda_t.abs() <= self.static_friction_coefficient * lambda_n.abs() {
            tangential_correction.solve_positions(self.bodies, positional_data, angular_data);
        }

        Some(SolvedContact {
            bodies: self.bodies,
            world_normal: self.world_normal,
            local_contact_points: self.local_contact_points,
            normal_force: lambda_n / inverse_timestep_squared,
            dynamic_friction_coefficient: self.dynamic_friction_coefficient,
            restitution_coefficient: self.restitution_coefficient,
        })
    }

    fn prepare(
        &self,
        positions: [DVec3; 2],
        inverse_masses: [f64; 2],
        orientations: [DQuat; 2],
        inverse_inertias: [glam::DMat3; 2],
        inverse_timestep_squared: f64,
    ) -> Option<[PreparedPositionalCorrection; 2]> {
        let contact_positions = self.contact_positions(positions, orientations);

        let Some(normal_correction) = self.normal_correction(contact_positions) else {
            return None;
        };
        let Ok(normal_correction) = normal_correction.prepare(
            inverse_masses,
            orientations,
            inverse_inertias,
            inverse_timestep_squared,
        ) else {
            self.unsolveable_error();
            return None;
        };

        let tangential_correction =
            self.tangential_correction(contact_positions, contact_positions);
        let Ok(tangential_correction) = tangential_correction.prepare(
            inverse_masses,
            orientations,
            inverse_inertias,
            inverse_timestep_squared,
        ) else {
            self.unsolveable_error();
            return None;
        };

        Some([normal_correction, tangential_correction])
    }

    fn unsolveable_error(&self) {
        error!(
            "Contact between {:?} and {:?} is unsolveable. This is likely due to infinite masses. Skipping.",
            self.bodies[0], self.bodies[1]
        );
    }

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
}

#[derive(Debug, Clone)]
pub(crate) struct SolvedContact {
    bodies: [BodyId; 2],
    world_normal: DVec3,
    local_contact_points: [DVec3; 2],
    normal_force: f64,
    dynamic_friction_coefficient: f64,
    restitution_coefficient: f64,
}

impl SolvedContact {
    fn contact_velocities(
        &self,
        velocities: [DVec3; 2],
        orientations: [DQuat; 2],
        angular_velocities: [DVec3; 2],
    ) -> [DVec3; 2] {
        [
            velocities[0]
                + angular_velocities[0].cross(orientations[0] * self.local_contact_points[0]),
            velocities[1]
                + angular_velocities[1].cross(orientations[1] * self.local_contact_points[1]),
        ]
    }

    fn relative_velocity(
        &self,
        velocities: [DVec3; 2],
        orientations: [DQuat; 2],
        angular_velocities: [DVec3; 2],
    ) -> DVec3 {
        let contact_velocities =
            self.contact_velocities(velocities, orientations, angular_velocities);
        contact_velocities[0] - contact_velocities[1]
    }

    fn relative_normal_velocity(&self, relative_velocity: DVec3) -> f64 {
        self.world_normal.dot(relative_velocity)
    }

    fn relative_tangential_velocity(
        &self,
        relative_velocity: DVec3,
        relative_normal_velocity: f64,
    ) -> DVec3 {
        relative_velocity - self.world_normal * relative_normal_velocity
    }

    pub fn solve_velocities(
        &self,
        positional_data: &mut SecondaryMap<PositionalData>,
        angular_data: &mut SecondaryMap<AngularData>,
        timestep: f64,
    ) {
        let d_pos = self.bodies.map(|b| &positional_data[b.0]);
        let d_ang = self.bodies.map(|b| &angular_data[b.0]);

        let velocities = [d_pos[0].velocity, d_pos[1].velocity];
        let orientations = [d_ang[0].orientation, d_ang[1].orientation];
        let angular_velocities = [d_ang[0].velocity, d_ang[1].velocity];

        let relative_velocity =
            self.relative_velocity(velocities, orientations, angular_velocities);

        let relative_normal_velocity = self.relative_normal_velocity(relative_velocity);
        let relative_tangential_velocity =
            self.relative_tangential_velocity(relative_velocity, relative_normal_velocity);

        let (direction, magnitude) = relative_tangential_velocity.normalize_and_length();
        let friction_correction = VelocityCorrection {
            direction,
            magnitude: magnitude
                .min(timestep * self.dynamic_friction_coefficient * self.normal_force.abs()),
            application_points: self.local_contact_points,
        };

        let previous_velocities = [d_pos[0].previous_velocity, d_pos[1].previous_velocity];
        let previous_angular_velocities = [d_ang[0].previous_velocity, d_ang[1].previous_velocity];
        let previous_relative_velocity = self.relative_normal_velocity(self.relative_velocity(
            previous_velocities,
            orientations,
            previous_angular_velocities,
        ));
        let restitution_correction = VelocityCorrection {
            direction: self.world_normal,
            magnitude: -relative_normal_velocity
                + 0.0f64.min(-self.restitution_coefficient * previous_relative_velocity),
            application_points: self.local_contact_points,
        };

        friction_correction
            .prepare(
                self.bodies.map(|b| positional_data[b.0].inverse_mass),
                orientations,
                self.bodies.map(|b| angular_data[b.0].inverse_inertia),
            )
            .unwrap()
            .solve_velocities(self.bodies, positional_data, angular_data);
        restitution_correction
            .prepare(
                self.bodies.map(|b| positional_data[b.0].inverse_mass),
                orientations,
                self.bodies.map(|b| angular_data[b.0].inverse_inertia),
            )
            .unwrap()
            .solve_velocities(self.bodies, positional_data, angular_data);
    }
}
