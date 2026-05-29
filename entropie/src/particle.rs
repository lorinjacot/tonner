use glam::Vec3;

use crate::{BodyId, LinearData, State};

/// A particle is a point mass with no orientation. It is defined by its position, velocity and mass. Infinite mass particles are supported, and cannot be influenced by any force or constraint. Particles cannot collide with each other.
///
/// Particles are the simplest type of body in the physics engine, and can be used to represent small objects or to create more complex bodies by connecting multiple particles together with constraints.
///
/// Note that particles do not have an orientation, and therefore cannot be affected by torques or angular constraints. Only particles with a finite mass are affected by forces and positional constraints.
///
/// # Examples
/// ```
/// # use glam::Vec3;
/// # use entropie::{State, ParticleBuilder};
/// let mut state = State::new();
///
/// let pos = Vec3::new(1.0, 2.0, 3.0);
/// let a = ParticleBuilder::default().position(pos).build(&mut state);
/// assert!(state.is_particle(a));
/// assert_eq!(state.position(a).unwrap(), pos);
/// ```
#[derive(Debug, Clone)]
#[must_use]
pub struct ParticleBuilder {
    position: Vec3,
    velocity: Vec3,
    inverse_mass: f32,
}

impl ParticleBuilder {
    /// Sets the initial position of the particle. The default position is `Vec3::ZERO`.
    ///
    /// # Examples
    /// ```
    /// # use glam::Vec3;
    /// # use entropie::{State, ParticleBuilder};
    /// let mut state = State::new();
    ///
    /// let a = ParticleBuilder::default().build(&mut state);
    /// assert_eq!(state.position(a).unwrap(), Vec3::ZERO);
    ///
    /// let pos = Vec3::new(1.0, 2.0, 3.0);
    /// let b = ParticleBuilder::default().position(pos).build(&mut state);
    /// assert_eq!(state.position(b).unwrap(), pos);
    /// ```
    pub fn position(mut self, position: impl Into<Vec3>) -> Self {
        self.position = position.into();
        self
    }

    /// Sets the initial velocity of the particle. The default velocity is `Vec3::ZERO`.
    ///
    /// # Examples
    /// ```
    /// # use glam::Vec3;
    /// # use entropie::{State, ParticleBuilder};
    /// let mut state = State::new();
    ///
    /// let a = ParticleBuilder::default().build(&mut state);
    /// assert_eq!(state.velocity(a).unwrap(), Vec3::ZERO);
    ///
    /// let vel = Vec3::new(1.0, 2.0, 3.0);
    /// let b = ParticleBuilder::default().velocity(vel).build(&mut state);
    /// assert_eq!(state.velocity(b).unwrap(), vel);
    /// ```
    pub fn velocity(mut self, velocity: impl Into<Vec3>) -> Self {
        self.velocity = velocity.into();
        self
    }

    /// Sets the mass of the particle. The default mass is `f32::INFINITY`, which means that the particle is immovable. The mass must be positive strictly positive.
    ///
    /// # Examples
    /// ```
    /// # use glam::Vec3;
    /// # use entropie::{State, ParticleBuilder};
    /// let mut state = State::new();
    ///
    /// let a = ParticleBuilder::default().build(&mut state);
    /// assert_eq!(state.mass(a).unwrap(), f32::INFINITY);
    /// assert_eq!(state.inverse_mass(a).unwrap(), 0.0);
    ///
    /// let b = ParticleBuilder::default().mass(2.0).build(&mut state);
    /// assert_eq!(state.mass(b).unwrap(), 2.0);
    /// assert_eq!(state.inverse_mass(b).unwrap(), 0.5);
    /// ```
    pub fn mass(mut self, mass: f32) -> Self {
        self.inverse_mass = mass.recip();
        self
    }

    /// Sets the inverse mass of the particle. The default inverse mass is `0.0`, which means that the particle is immovable. The inverse mass must be non-negative.
    ///
    /// # Examples
    /// ```
    /// # use glam::Vec3;
    /// # use entropie::{State, ParticleBuilder};
    /// let mut state = State::new();
    ///
    /// let a = ParticleBuilder::default().build(&mut state);
    /// assert_eq!(state.mass(a).unwrap(), f32::INFINITY);
    /// assert_eq!(state.inverse_mass(a).unwrap(), 0.0);
    ///
    /// let b = ParticleBuilder::default().inverse_mass(0.5).build(&mut state);
    /// assert_eq!(state.mass(b).unwrap(), 2.0);
    /// assert_eq!(state.inverse_mass(b).unwrap(), 0.5);
    /// ```
    pub fn inverse_mass(mut self, inverse_mass: f32) -> Self {
        self.inverse_mass = inverse_mass;
        self
    }

    /// Builds the particle and adds it to the state, returning the `BodyId` of the newly created particle.
    ///
    /// # Examples
    /// ```
    /// # use glam::Vec3;
    /// # use entropie::{State, ParticleBuilder};
    /// let mut state = State::new();
    ///
    /// let pos = Vec3::new(1.0, 2.0, 3.0);
    /// let a = ParticleBuilder::default().position(pos).build(&mut state);
    /// assert!(state.is_particle(a));
    /// assert_eq!(state.position(a).unwrap(), pos);
    /// ```
    pub fn build(self, state: &mut State) -> BodyId {
        let id = state.bodies.create();

        state.particles.insert(id, ());
        state.linear_data.insert(
            id,
            LinearData {
                position: self.position,
                previous_position: self.position,
                velocity: self.velocity,
                inverse_mass: self.inverse_mass,
                force: Vec3::ZERO,
            },
        );

        BodyId(id)
    }
}

impl Default for ParticleBuilder {
    fn default() -> Self {
        ParticleBuilder {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            inverse_mass: 0.0,
        }
    }
}
