use glam::Vec3;
use sparse_keyed::{Key, KeyRegistry, SecondaryMap};

pub use aabb::AABB;
pub use particle::ParticleBuilder;
pub use transform::Transform;

mod aabb;
pub mod collision;
pub mod constraint;
pub mod force;
mod particle;
pub mod shape;
mod transform;

#[derive(Debug, Clone)]
pub struct State {
    bodies: KeyRegistry,
    particles: SecondaryMap<()>,
    positions: SecondaryMap<Vec3>,
    velocities: SecondaryMap<Vec3>,
    inverse_masses: SecondaryMap<f32>,
}

impl State {
    pub fn new() -> Self {
        State {
            bodies: KeyRegistry::new(),
            particles: SecondaryMap::new(),
            positions: SecondaryMap::new(),
            velocities: SecondaryMap::new(),
            inverse_masses: SecondaryMap::new(),
        }
    }

    pub fn is_particle(&self, body: BodyId) -> bool {
        self.particles.contains(body.0)
    }

    pub fn position(&self, body: BodyId) -> Option<Vec3> {
        self.positions.get(body.0).copied()
    }

    pub fn position_mut(&mut self, body: BodyId) -> Option<&mut Vec3> {
        self.positions.get_mut(body.0)
    }

    pub fn velocity(&self, body: BodyId) -> Option<Vec3> {
        self.velocities.get(body.0).copied()
    }

    pub fn velocity_mut(&mut self, body: BodyId) -> Option<&mut Vec3> {
        self.velocities.get_mut(body.0)
    }

    pub fn mass(&self, body: BodyId) -> Option<f32> {
        self.inverse_masses
            .get(body.0)
            .map(|inv_mass| inv_mass.recip())
    }

    pub fn inverse_mass(&self, body: BodyId) -> Option<f32> {
        self.inverse_masses.get(body.0).copied()
    }

    pub fn inverse_mass_mut(&mut self, body: BodyId) -> Option<&mut f32> {
        self.inverse_masses.get_mut(body.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyId(Key);
