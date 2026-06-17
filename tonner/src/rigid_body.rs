use glam::{DMat3, DQuat, DVec3};
use sparse_keyed::SecondaryMap;

use crate::{
    AngularData, BodyId, PositionalData, State,
    shape::{Ball, Box3D},
};

#[derive(Debug, Clone)]
#[must_use]
pub struct RigidBodyBuilder {
    position: DVec3,
    velocity: DVec3,
    inverse_mass: f64,
    orientation: DQuat,
    angular_velocity: DVec3,
    inertia: DMat3,
    inverse_inertia: DMat3,
    shape: Shape,
}

impl RigidBodyBuilder {
    /// Sets the initial position of the center of mass of the rigid body. The default position is `DVec3::ZERO`.
    ///
    /// # Examples
    /// ```
    /// # use glam::DVec3;
    /// # use tonner::{State, RigidBodyBuilder};
    /// let mut state = State::new();
    ///
    /// let pos = DVec3::new(1.0, 2.0, 3.0);
    /// let a = RigidBodyBuilder::default().position(pos).build(&mut state);
    /// assert_eq!(state.position(a).unwrap(), pos);
    ///
    /// let b = RigidBodyBuilder::default().build(&mut state);
    /// assert_eq!(state.position(b).unwrap(), DVec3::ZERO);
    /// ```
    pub fn position(mut self, position: impl Into<DVec3>) -> Self {
        self.position = position.into();
        self
    }

    /// Sets the initial velocity of the center of mass of the rigid body. The default velocity is `DVec3::ZERO`.
    ///
    /// # Examples
    /// ```
    /// # use glam::DVec3;
    /// # use tonner::{State, RigidBodyBuilder};
    /// let mut state = State::new();
    ///
    /// let vel = DVec3::new(1.0, 2.0, 3.0);
    /// let a = RigidBodyBuilder::default().velocity(vel).build(&mut state);
    /// assert_eq!(state.velocity(a).unwrap(), vel);
    ///
    /// let b = RigidBodyBuilder::default().build(&mut state);
    /// assert_eq!(state.velocity(b).unwrap(), DVec3::ZERO);
    /// ```
    pub fn velocity(mut self, velocity: impl Into<DVec3>) -> Self {
        self.velocity = velocity.into();
        self
    }

    /// Sets the mass of the rigid body. The default mass is `f64::INFINITY`, which means the rigid body is immovable. The mass must be strictly positive.
    ///
    /// # Panics
    /// Panics if the mass is not strictly positive.
    ///
    /// # Examples
    /// ```
    /// # use glam::DVec3;
    /// # use tonner::{State, RigidBodyBuilder};
    /// let mut state = State::new();
    ///
    /// let a = RigidBodyBuilder::default().mass(2.0).build(&mut state);
    /// assert_eq!(state.mass(a).unwrap(), 2.0);
    /// assert_eq!(state.inverse_mass(a).unwrap(), 0.5);
    ///
    /// let b = RigidBodyBuilder::default().build(&mut state);
    /// assert_eq!(state.mass(b).unwrap(), f64::INFINITY);
    /// assert_eq!(state.inverse_mass(b).unwrap(), 0.0);
    /// ```
    pub fn mass(mut self, mass: f64) -> Self {
        assert!(mass > 0.0, "Mass must be positive");
        self.inverse_mass = 1.0 / mass;
        self
    }

    /// Sets the inverse mass of the rigid body. The default inverse mass is `0.0`, which means the rigid body is immovable. The inverse mass must be non-negative.
    ///
    /// # Panics
    ///
    /// Panics if the inverse mass is negative
    ///
    /// # Examples
    ///
    /// ```
    /// # use glam::DVec3;
    /// # use tonner::{State, RigidBodyBuilder};
    /// let mut state = State::new();
    ///
    /// let a = RigidBodyBuilder::default().inverse_mass(0.5).build(&mut state);
    /// assert_eq!(state.mass(a).unwrap(), 2.0);
    /// assert_eq!(state.inverse_mass(a).unwrap(), 0.5);
    ///
    /// let b = RigidBodyBuilder::default().build(&mut state);
    /// assert_eq!(state.mass(b).unwrap(), f64::INFINITY);
    /// assert_eq!(state.inverse_mass(b).unwrap(), 0.0);
    /// ```
    pub fn inverse_mass(mut self, inverse_mass: f64) -> Self {
        assert!(inverse_mass >= 0.0, "Inverse mass must be non-negative");
        self.inverse_mass = inverse_mass;
        self
    }

    /// Sets the initial orientation of the rigid body. The default orientation is `DQuat::IDENTITY`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use glam::DQuat;
    /// # use tonner::{State, RigidBodyBuilder};
    /// let mut state = State::new();
    ///
    /// let orientation = DQuat::from_rotation_y(1.0);
    /// let a = RigidBodyBuilder::default().orientation(orientation).build(&mut state);
    /// assert_eq!(state.orientation(a).unwrap(), orientation);
    ///
    /// let b = RigidBodyBuilder::default().build(&mut state);
    /// assert_eq!(state.orientation(b).unwrap(), DQuat::IDENTITY);
    /// ```
    pub fn orientation(mut self, orientation: impl Into<DQuat>) -> Self {
        self.orientation = orientation.into();
        self
    }

    /// Sets the initial angular velocity of the rigid body. The default angular velocity is `DVec3::ZERO`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use glam::DVec3;
    /// # use tonner::{State, RigidBodyBuilder};
    /// let mut state = State::new();
    ///
    /// let angular_velocity = DVec3::new(1.0, 2.0, 3.0);
    /// let a = RigidBodyBuilder::default().angular_velocity(angular_velocity).build(&mut state);
    /// assert_eq!(state.angular_velocity(a).unwrap(), angular_velocity);
    ///
    /// let b = RigidBodyBuilder::default().build(&mut state);
    /// assert_eq!(state.angular_velocity(b).unwrap(), DVec3::ZERO);
    /// ```
    pub fn angular_velocity(mut self, angular_velocity: impl Into<DVec3>) -> Self {
        self.angular_velocity = angular_velocity.into();
        self
    }

    /// Sets the inertia tensor of the rigid body. The default inertia tensor is `DMat3::IDENTITY`. The inertia tensor must be positive definite.
    ///
    /// # Panics
    ///
    /// Panics if the inertia tensor is not positive definite.
    ///
    /// # Examples
    ///
    /// ```
    /// # use glam::{DMat3, DVec3};
    /// # use tonner::{State, RigidBodyBuilder};
    /// let mut state = State::new();
    ///
    /// let inertia = DMat3::from_diagonal(DVec3::new(2.0, 3.0, 4.0));
    /// let a = RigidBodyBuilder::default().inertia(inertia).build(&mut state);
    /// assert_eq!(state.inertia(a).unwrap(), inertia);
    /// assert_eq!(state.inverse_inertia(a).unwrap(), inertia.inverse());
    ///
    /// let b = RigidBodyBuilder::default().build(&mut state);
    /// assert_eq!(state.inertia(b).unwrap(), DMat3::IDENTITY);
    /// assert_eq!(state.inverse_inertia(b).unwrap(), DMat3::IDENTITY);
    /// ```
    pub fn inertia(mut self, inertia: impl Into<DMat3>) -> Self {
        let inertia: DMat3 = inertia.into();
        assert!(
            inertia.determinant() > 0.0,
            "Inertia must be positive definite"
        );
        self.inertia = inertia;
        self.inverse_inertia = inertia.inverse();
        self
    }

    /// Sets the inverse inertia tensor of the rigid body. The default inverse inertia tensor is `DMat3::IDENTITY`. The inverse inertia tensor must be positive definite.
    ///
    /// # Panics
    ///
    /// Panics if the inverse inertia tensor is not positive definite.
    ///
    /// # Examples
    ///
    /// ```
    /// # use glam::{DMat3, DVec3};
    /// # use tonner::{State, RigidBodyBuilder};
    /// let mut state = State::new();
    ///
    /// let inverse_inertia = DMat3::from_diagonal(DVec3::new(0.5, 0.3333333333333333, 0.25));
    /// let a = RigidBodyBuilder::default().inverse_inertia(inverse_inertia).build(&mut state);
    /// assert_eq!(state.inverse_inertia(a).unwrap(), inverse_inertia);
    /// assert_eq!(state.inertia(a).unwrap(), inverse_inertia.inverse());
    ///
    /// let b = RigidBodyBuilder::default().build(&mut state);
    /// assert_eq!(state.inverse_inertia(b).unwrap(), DMat3::IDENTITY);
    /// assert_eq!(state.inertia(b).unwrap(), DMat3::IDENTITY);
    /// ```
    pub fn inverse_inertia(mut self, inverse_inertia: impl Into<DMat3>) -> Self {
        let inverse_inertia: DMat3 = inverse_inertia.into();
        assert!(
            inverse_inertia.determinant() > 0.0,
            "Inverse inertia must be positive definite"
        );
        self.inverse_inertia = inverse_inertia;
        self.inertia = inverse_inertia.inverse();
        self
    }

    pub fn ball(mut self, ball: Ball) -> Self {
        self.shape = Shape::Ball(ball);
        self
    }

    pub fn box3d(mut self, box_: Box3D) -> Self {
        self.shape = Shape::Box(box_);
        self
    }

    pub fn build(self, state: &mut State) -> BodyId {
        let id = state.bodies.create();

        state.rigid_bodies.rigid_bodies.insert(id, ());
        state.positional_data.insert(
            id,
            PositionalData {
                position: self.position,
                previous_position: self.position,
                velocity: self.velocity,
                inverse_mass: self.inverse_mass,
                force: DVec3::ZERO,
            },
        );
        state.angular_data.insert(
            id,
            AngularData {
                orientation: self.orientation,
                previous_orientation: self.orientation,
                velocity: self.angular_velocity,
                inertia: self.inertia,
                inverse_inertia: self.inverse_inertia,
                torque: DVec3::ZERO,
            },
        );
        match self.shape {
            Shape::Box(box_) => {
                state.rigid_bodies.boxes.insert(id, box_);
            }
            Shape::Ball(ball) => {
                state.rigid_bodies.balls.insert(id, ball);
            }
        }

        BodyId(id)
    }
}

impl Default for RigidBodyBuilder {
    fn default() -> Self {
        RigidBodyBuilder {
            position: DVec3::ZERO,
            velocity: DVec3::ZERO,
            inverse_mass: 0.0,
            orientation: DQuat::IDENTITY,
            angular_velocity: DVec3::ZERO,
            inertia: DMat3::IDENTITY,
            inverse_inertia: DMat3::IDENTITY,
            shape: Shape::Ball(Ball::UNIT),
        }
    }
}

#[derive(Debug, Clone)]
enum Shape {
    Box(Box3D),
    Ball(Ball),
}

#[derive(Debug, Clone)]
pub struct RigidBodies {
    rigid_bodies: SecondaryMap<()>,
    boxes: SecondaryMap<Box3D>,
    balls: SecondaryMap<Ball>,
}

impl RigidBodies {
    #[must_use]
    pub fn new() -> Self {
        RigidBodies {
            rigid_bodies: SecondaryMap::new(),
            boxes: SecondaryMap::new(),
            balls: SecondaryMap::new(),
        }
    }

    pub fn is_rigid_body(&self, id: BodyId) -> bool {
        self.rigid_bodies.contains(id.0)
    }
}
