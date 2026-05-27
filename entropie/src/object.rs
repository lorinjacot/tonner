use glam::Vec3;

/// A particle is a point mass object that can move in space. It has a position, a velocity and a mass.
pub struct Particle {
    position: Vec3,
    velocity: Vec3,
    inverse_mass: f32,
}

impl Particle {
    /// Creates a new particle with the given position, velocity and mass. The mass must be positive. A mass of `f32::INFINITY` can be used to create an immovable particle.
    ///
    /// # Examples
    /// ```
    /// # use glam::Vec3;
    /// # use entropie::Particle;
    /// let a = Particle::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 1.0);
    /// assert_eq!(a.position(), Vec3::new(0.0, 0.0, 0.0));
    /// assert_eq!(a.velocity(), Vec3::new(1.0, 0.0, 0.0));
    /// assert_eq!(a.mass(), 1.0);
    /// assert_eq!(a.inverse_mass(), 1.0);
    ///
    /// let b = Particle::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), f32::INFINITY);
    /// assert_eq!(b.position(), Vec3::new(0.0, 0.0, 0.0));
    /// assert_eq!(b.velocity(), Vec3::new(0.0, 1.0, 0.0));
    /// assert_eq!(b.mass(), f32::INFINITY);
    /// assert_eq!(b.inverse_mass(), 0.0);
    /// ```
    pub fn new(position: Vec3, velocity: Vec3, mass: f32) -> Particle {
        Particle {
            position,
            velocity,
            inverse_mass: mass.recip(),
        }
    }

    /// Creates a new immovable particle with the given position and velocity. The mass of the particle is set to `f32::INFINITY`.
    ///
    /// # Examples
    /// ```
    /// # use glam::Vec3;
    /// # use entropie::Particle;
    /// let a = Particle::new_immovable(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
    /// assert_eq!(a.position(), Vec3::new(0.0, 0.0, 0.0));
    /// assert_eq!(a.velocity(), Vec3::new(1.0, 0.0, 0.0));
    /// assert_eq!(a.mass(), f32::INFINITY);
    /// assert_eq!(a.inverse_mass(), 0.0);
    /// ```
    pub fn new_immovable(position: Vec3, velocity: Vec3) -> Particle {
        Particle {
            position,
            velocity,
            inverse_mass: 0.0,
        }
    }

    /// Returns the position of the particle.
    /// 
    /// # Examples
    /// ```
    /// # use glam::Vec3;
    /// # use entropie::Particle;
    /// let a = Particle::new(Vec3::new(1.0, 2.0, 3.0), Vec3::ZERO, 1.0);
    /// assert_eq!(a.position(), Vec3::new(1.0, 2.0, 3.0));
    /// ```
    pub fn position(&self) -> Vec3 {
        self.position
    }

    /// Returns the velocity of the particle.
    /// 
    /// # Examples
    /// ```
    /// # use glam::Vec3;
    /// # use entropie::Particle;
    /// let a = Particle::new(Vec3::ZERO, Vec3::new(1.0, 2.0, 3.0), 1.0);
    /// assert_eq!(a.velocity(), Vec3::new(1.0, 2.0, 3.0));
    /// ```
    pub fn velocity(&self) -> Vec3 {
        self.velocity
    }

    /// Returns the mass of the particle. Might return `f32::INFINITY` for an immovable particle.
    /// 
    /// # Examples
    /// ```
    /// # use glam::Vec3;
    /// # use entropie::Particle;
    /// let a = Particle::new(Vec3::ZERO, Vec3::ZERO, 1.0);
    /// assert_eq!(a.mass(), 1.0);
    /// 
    /// let b = Particle::new(Vec3::ZERO, Vec3::ZERO, f32::INFINITY);
    /// assert_eq!(b.mass(), f32::INFINITY);
    /// ```
    pub fn mass(&self) -> f32 {
        self.inverse_mass.recip()
    }

    /// Returns the inverse mass of the particle. Might return `0.0` for an immovable particle.
    /// 
    /// # Examples
    /// ```
    /// # use glam::Vec3;
    /// # use entropie::Particle;
    /// let a = Particle::new(Vec3::ZERO, Vec3::ZERO, 1.0);
    /// assert_eq!(a.inverse_mass(), 1.0);
    /// 
    /// let b = Particle::new(Vec3::ZERO, Vec3::ZERO, f32::INFINITY);
    /// assert_eq!(b.inverse_mass(), 0.0);
    /// ```
    pub fn inverse_mass(&self) -> f32 {
        self.inverse_mass
    }
}
