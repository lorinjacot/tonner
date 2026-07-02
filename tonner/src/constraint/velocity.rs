use glam::DVec3;
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
    pub fn solve_velocities(
        &self,
        bodies: [BodyId; 2],
        positional_data: &mut SecondaryMap<PositionalData>,
        angular_data: &mut SecondaryMap<crate::AngularData>,
    ) {
        let pd = bodies.map(|b| &positional_data[b.0]);
        let ad = bodies.map(|b| &angular_data[b.0]);

        let generalized_inverse_masses = [
            generalized_inverse_mass(
                pd[0].inverse_mass,
                ad[0].inverse_inertia,
                self.application_points[0],
                ad[0].orientation.conjugate() * self.direction,
            ),
            generalized_inverse_mass(
                pd[1].inverse_mass,
                ad[1].inverse_inertia,
                self.application_points[1],
                ad[1].orientation.conjugate() * self.direction,
            ),
        ];

        let p = self.magnitude * self.direction
            / (generalized_inverse_masses[0] + generalized_inverse_masses[1]);

        let delta_v = [p * pd[0].inverse_mass, -p * pd[1].inverse_mass];

        let delta_w = [
            ad[0].orientation * (ad[0].inverse_inertia * self.application_points[0].cross(p)),
            -ad[1].orientation * (ad[1].inverse_inertia * self.application_points[1].cross(p)),
        ];

        positional_data[bodies[0].0].velocity += delta_v[0];
        positional_data[bodies[1].0].velocity += delta_v[1];
        angular_data[bodies[0].0].velocity += delta_w[0];
        angular_data[bodies[1].0].velocity += delta_w[1];
    }
}
