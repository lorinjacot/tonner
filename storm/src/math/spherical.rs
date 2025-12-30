use std::f32::consts::PI;

use glam::{Vec3, vec3};

/// This struct can be used to represent points in 3D space as
/// [Spherical coordinates](https://en.wikipedia.org/wiki/Spherical_coordinate_system).
///
/// Based on [threejs Spherical](https://threejs.org/docs/#Spherical).
#[derive(Debug, Clone, Copy)]
pub struct Spherical {
    /// The radius, or the Euclidean distance (straight-line distance) from the point to the origin.
    pub radius: f32,

    /// The polar angle in radians from the y (up) axis.
    pub phi: f32,

    /// The equator/azimuthal angle in radians around the y (up) axis.
    pub theta: f32,
}

impl Spherical {
    /// All zeroes.
    pub const ZERO: Spherical = Spherical {
        radius: 0.0,
        phi: 0.0,
        theta: 0.0,
    };

    /// Restricts the polar angle to be between `0.000001` and `PI - 0.000001`.
    pub fn safe(&self) -> Self {
        const EPS: f32 = 0.000001;

        Self {
            phi: self.phi.clamp(EPS, PI - EPS),
            ..*self
        }
    }

    /// Sets the spherical components from the given vector which is assumed to hold
    /// Cartesian coordinates.
    pub fn from_vec3(v: Vec3) -> Self {
        Self::from_cartesian_coordinates(v.x, v.y, v.z)
    }

    /// Sets the spherical components from the given Cartesian coordinates.
    pub fn from_cartesian_coordinates(x: f32, y: f32, z: f32) -> Self {
        let radius = (x * x + y * y + z * z).sqrt();

        if radius == 0.0 {
            Self {
                radius,
                phi: 0.0,
                theta: 0.0,
            }
        } else {
            Self {
                radius,
                theta: z.atan2(x),
                phi: (y / radius).clamp(-1.0, 1.0).acos(),
            }
        }
    }

    /// Transform the spherical coordinates into a vector of cartesian coordinates.
    pub fn to_vec3(&self) -> Vec3 {
        let sin_phi_radius = self.phi.sin() * self.radius;
        vec3(
            sin_phi_radius * self.theta.sin(),
            self.phi.cos() * self.radius,
            sin_phi_radius * self.theta.cos(),
        )
    }
}

impl From<Vec3> for Spherical {
    fn from(value: Vec3) -> Self {
        Self::from_vec3(value.into())
    }
}

impl From<Spherical> for Vec3 {
    fn from(value: Spherical) -> Self {
        value.to_vec3()
    }
}
