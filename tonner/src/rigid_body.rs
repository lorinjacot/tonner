use glam::{DMat3, DQuat, DVec3};
use sparse_keyed::SecondaryMap;

use crate::{
    AngularData, BodyId, Engine, PositionalData, Transform,
    collision::narrow::{
        collides_ball_ball, collides_ball_box, collides_box_box,
        contact::{Contact, SolvedContact},
    },
    shape::{Ball, Box3D},
};

#[derive(Debug, Clone)]
#[must_use]
pub struct RigidBodyBuilder {
    positional_data: PositionalData,
    angular_data: AngularData,
    shape: Shape,
}

impl RigidBodyBuilder {
    /// Sets the initial position of the center of mass of the rigid body. The default position is `DVec3::ZERO`.
    ///
    /// # Examples
    /// ```
    /// # use glam::DVec3;
    /// # use tonner::{Engine, RigidBodyBuilder};
    /// let mut engine = Engine::new();
    ///
    /// let pos = DVec3::new(1.0, 2.0, 3.0);
    /// let a = RigidBodyBuilder::default().position(pos).build(&mut engine);
    /// assert_eq!(engine.position(a).unwrap(), pos);
    ///
    /// let b = RigidBodyBuilder::default().build(&mut engine);
    /// assert_eq!(engine.position(b).unwrap(), DVec3::ZERO);
    /// ```
    pub fn position(mut self, position: impl Into<DVec3>) -> Self {
        self.positional_data.position = position.into();
        self
    }

    /// Sets the initial velocity of the center of mass of the rigid body. The default velocity is `DVec3::ZERO`.
    ///
    /// # Examples
    /// ```
    /// # use glam::DVec3;
    /// # use tonner::{Engine, RigidBodyBuilder};
    /// let mut engine = Engine::new();
    ///
    /// let vel = DVec3::new(1.0, 2.0, 3.0);
    /// let a = RigidBodyBuilder::default().velocity(vel).build(&mut engine);
    /// assert_eq!(engine.velocity(a).unwrap(), vel);
    ///
    /// let b = RigidBodyBuilder::default().build(&mut engine);
    /// assert_eq!(engine.velocity(b).unwrap(), DVec3::ZERO);
    /// ```
    pub fn velocity(mut self, velocity: impl Into<DVec3>) -> Self {
        self.positional_data.velocity = velocity.into();
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
    /// # use tonner::{Engine, RigidBodyBuilder};
    /// let mut engine = Engine::new();
    ///
    /// let a = RigidBodyBuilder::default().mass(2.0).build(&mut engine);
    /// assert_eq!(engine.mass(a).unwrap(), 2.0);
    /// assert_eq!(engine.inverse_mass(a).unwrap(), 0.5);
    ///
    /// let b = RigidBodyBuilder::default().build(&mut engine);
    /// assert_eq!(engine.mass(b).unwrap(), f64::INFINITY);
    /// assert_eq!(engine.inverse_mass(b).unwrap(), 0.0);
    /// ```
    pub fn mass(mut self, mass: f64) -> Self {
        assert!(mass > 0.0, "Mass must be positive");
        self.positional_data.inverse_mass = 1.0 / mass;
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
    /// # use tonner::{Engine, RigidBodyBuilder};
    /// let mut engine = Engine::new();
    ///
    /// let a = RigidBodyBuilder::default().inverse_mass(0.5).build(&mut engine);
    /// assert_eq!(engine.mass(a).unwrap(), 2.0);
    /// assert_eq!(engine.inverse_mass(a).unwrap(), 0.5);
    ///
    /// let b = RigidBodyBuilder::default().build(&mut engine);
    /// assert_eq!(engine.mass(b).unwrap(), f64::INFINITY);
    /// assert_eq!(engine.inverse_mass(b).unwrap(), 0.0);
    /// ```
    pub fn inverse_mass(mut self, inverse_mass: f64) -> Self {
        assert!(inverse_mass >= 0.0, "Inverse mass must be non-negative");
        self.positional_data.inverse_mass = inverse_mass;
        self
    }

    /// Sets the initial orientation of the rigid body. The default orientation is `DQuat::IDENTITY`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use glam::DQuat;
    /// # use tonner::{Engine, RigidBodyBuilder};
    /// let mut engine = Engine::new();
    ///
    /// let orientation = DQuat::from_rotation_y(1.0);
    /// let a = RigidBodyBuilder::default().orientation(orientation).build(&mut engine);
    /// assert_eq!(engine.orientation(a).unwrap(), orientation);
    ///
    /// let b = RigidBodyBuilder::default().build(&mut engine);
    /// assert_eq!(engine.orientation(b).unwrap(), DQuat::IDENTITY);
    /// ```
    pub fn orientation(mut self, orientation: impl Into<DQuat>) -> Self {
        self.angular_data.orientation = orientation.into();
        self
    }

    /// Sets the initial angular velocity of the rigid body. The default angular velocity is `DVec3::ZERO`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use glam::DVec3;
    /// # use tonner::{Engine, RigidBodyBuilder};
    /// let mut engine = Engine::new();
    ///
    /// let angular_velocity = DVec3::new(1.0, 2.0, 3.0);
    /// let a = RigidBodyBuilder::default().angular_velocity(angular_velocity).build(&mut engine);
    /// assert_eq!(engine.angular_velocity(a).unwrap(), angular_velocity);
    ///
    /// let b = RigidBodyBuilder::default().build(&mut engine);
    /// assert_eq!(engine.angular_velocity(b).unwrap(), DVec3::ZERO);
    /// ```
    pub fn angular_velocity(mut self, angular_velocity: impl Into<DVec3>) -> Self {
        self.angular_data.velocity = angular_velocity.into();
        self
    }

    /// Sets the inertia tensor of the rigid body. The default inertia tensor is diagonal with all diagonal entries equal to `f64::INFINITY`, which prevents the rigid body from rotating. The inertia tensor must be positive definite.
    ///
    /// # Panics
    ///
    /// Panics if the inertia tensor is not positive definite.
    ///
    /// # Examples
    ///
    /// ```
    /// # use glam::{DMat3, DVec3};
    /// # use tonner::{Engine, RigidBodyBuilder};
    /// let mut engine = Engine::new();
    ///
    /// let inertia = DMat3::from_diagonal(DVec3::new(2.0, 3.0, 4.0));
    /// let a = RigidBodyBuilder::default().inertia(inertia).build(&mut engine);
    /// assert_eq!(engine.inertia(a).unwrap(), inertia);
    /// assert_eq!(engine.inverse_inertia(a).unwrap(), inertia.inverse());
    ///
    /// let b = RigidBodyBuilder::default().build(&mut engine);
    /// assert_eq!(engine.inertia(b).unwrap(), DMat3::from_diagonal(DVec3::INFINITY));
    /// assert_eq!(engine.inverse_inertia(b).unwrap(), DMat3::ZERO);
    /// ```
    pub fn inertia(mut self, inertia: impl Into<DMat3>) -> Self {
        let inertia: DMat3 = inertia.into();
        assert!(
            inertia.determinant() > 0.0,
            "Inertia must be positive definite"
        );
        self.angular_data.inertia = inertia;
        self.angular_data.inverse_inertia = inertia.inverse();
        self
    }

    /// Sets the inverse inertia tensor of the rigid body. The default inverse inertia tensor is `DMat3::ZERO`, which prevents the rigid body from rotating. The inverse inertia tensor passed to this method must be positive definite.
    ///
    /// # Panics
    ///
    /// Panics if the inverse inertia tensor is not positive definite.
    ///
    /// # Examples
    ///
    /// ```
    /// # use glam::{DMat3, DVec3};
    /// # use tonner::{Engine, RigidBodyBuilder};
    /// let mut engine = Engine::new();
    ///
    /// let inverse_inertia = DMat3::from_diagonal(DVec3::new(0.5, 0.3333333333333333, 0.25));
    /// let a = RigidBodyBuilder::default().inverse_inertia(inverse_inertia).build(&mut engine);
    /// assert_eq!(engine.inverse_inertia(a).unwrap(), inverse_inertia);
    /// assert_eq!(engine.inertia(a).unwrap(), inverse_inertia.inverse());
    ///
    /// let b = RigidBodyBuilder::default().build(&mut engine);
    /// assert_eq!(engine.inverse_inertia(b).unwrap(), DMat3::ZERO);
    /// assert_eq!(engine.inertia(b).unwrap(), DMat3::from_diagonal(DVec3::INFINITY));
    /// ```
    pub fn inverse_inertia(mut self, inverse_inertia: impl Into<DMat3>) -> Self {
        let inverse_inertia: DMat3 = inverse_inertia.into();
        assert!(
            inverse_inertia.determinant() > 0.0,
            "Inverse inertia must be positive definite"
        );
        self.angular_data.inverse_inertia = inverse_inertia;
        self.angular_data.inertia = inverse_inertia.inverse();
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

    pub fn build(self, engine: &mut Engine) -> BodyId {
        let id = engine.bodies.create();

        engine.rigid_bodies.rigid_bodies.insert(id, ());
        engine.positional_data.insert(id, self.positional_data);
        engine.angular_data.insert(id, self.angular_data);
        match self.shape {
            Shape::Box(box_) => {
                engine.rigid_bodies.boxes.insert(id, box_);
            }
            Shape::Ball(ball) => {
                engine.rigid_bodies.balls.insert(id, ball);
            }
        }

        BodyId(id)
    }
}

impl Default for RigidBodyBuilder {
    fn default() -> Self {
        RigidBodyBuilder {
            positional_data: PositionalData::default(),
            angular_data: AngularData::default(),
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
pub(crate) struct RigidBodies {
    rigid_bodies: SecondaryMap<()>,
    boxes: SecondaryMap<Box3D>,
    balls: SecondaryMap<Ball>,
    detected_contacts: Vec<Contact>,
    solved_contacts: Vec<SolvedContact>,
}

impl RigidBodies {
    #[must_use]
    pub fn new() -> Self {
        RigidBodies {
            rigid_bodies: SecondaryMap::new(),
            boxes: SecondaryMap::new(),
            balls: SecondaryMap::new(),
            detected_contacts: Vec::new(),
            solved_contacts: Vec::new(),
        }
    }

    pub fn is_rigid_body(&self, id: BodyId) -> bool {
        self.rigid_bodies.contains(id.0)
    }

    pub fn solve_positions(
        &mut self,
        inverse_timestep_squared: f64,
        positional_data: &mut SecondaryMap<PositionalData>,
        angular_data: &mut SecondaryMap<AngularData>,
    ) {
        self.detect_contacts(positional_data, angular_data);

        for contact in self.detected_contacts.drain(..) {
            if let Some(solved_contact) =
                contact.solve_positions(positional_data, angular_data, inverse_timestep_squared)
            {
                self.solved_contacts.push(solved_contact);
            }
        }
    }

    fn detect_contacts(
        &mut self,
        positional_data: &SecondaryMap<PositionalData>,
        angular_data: &SecondaryMap<AngularData>,
    ) {
        for (body0, ball0) in &self.balls {
            for (body1, ball1) in &self.balls {
                if body0 < body1 {
                    let transform0 = Transform {
                        translation: positional_data[body0].position,
                        rotation: angular_data[body0].orientation,
                    };
                    let transform1 = Transform {
                        translation: positional_data[body1].position,
                        rotation: angular_data[body1].orientation,
                    };

                    if let Some(info) =
                        collides_ball_ball((ball0, &transform0), (ball1, &transform1))
                    {
                        let contact = Contact {
                            bodies: [BodyId(body0), BodyId(body1)],
                            world_normal: info.world_normal,
                            local_contact_points: info.local_contact_points,
                            static_friction_coefficient: 0.5,
                            dynamic_friction_coefficient: 0.3,
                            restitution_coefficient: 0.5,
                        };
                        self.detected_contacts.push(contact);
                    }
                }
            }

            for (body1, box1) in &self.boxes {
                let transform0 = Transform {
                    translation: positional_data[body0].position,
                    rotation: angular_data[body0].orientation,
                };
                let transform1 = Transform {
                    translation: positional_data[body1].position,
                    rotation: angular_data[body1].orientation,
                };

                if let Some(info) = collides_ball_box((ball0, &transform0), (box1, &transform1)) {
                    let contact = Contact {
                        bodies: [BodyId(body0), BodyId(body1)],
                        world_normal: info.world_normal,
                        local_contact_points: info.local_contact_points,
                        static_friction_coefficient: 0.5,
                        dynamic_friction_coefficient: 0.3,
                        restitution_coefficient: 0.5,
                    };
                    self.detected_contacts.push(contact);
                }
            }
        }

        for (body0, box0) in &self.boxes {
            for (body1, box1) in &self.boxes {
                if body0 < body1 {
                    let transform0 = Transform {
                        translation: positional_data[body0].position,
                        rotation: angular_data[body0].orientation,
                    };
                    let transform1 = Transform {
                        translation: positional_data[body1].position,
                        rotation: angular_data[body1].orientation,
                    };

                    if let Some(info) = collides_box_box((box0, &transform0), (box1, &transform1)) {
                        let contact = Contact {
                            bodies: [BodyId(body0), BodyId(body1)],
                            world_normal: info.world_normal,
                            local_contact_points: info.local_contact_points,
                            static_friction_coefficient: 0.5,
                            dynamic_friction_coefficient: 0.3,
                            restitution_coefficient: 0.5,
                        };
                        self.detected_contacts.push(contact);
                    }
                }
            }
        }
    }

    pub fn solve_velocities(
        &mut self,
        positional_data: &mut SecondaryMap<PositionalData>,
        angular_data: &mut SecondaryMap<AngularData>,
        timestep: f64,
    ) {
        for contact in self.solved_contacts.drain(..) {
            contact.solve_velocities(positional_data, angular_data, timestep);
        }
    }
}

pub(crate) fn generalized_inverse_mass(
    inverse_mass: f64,
    inverse_inertia: DMat3,
    local_application_point: DVec3,
    local_direction: DVec3,
) -> f64 {
    let rotation = local_application_point.cross(local_direction);
    inverse_mass + rotation.dot(inverse_inertia * rotation)
}
