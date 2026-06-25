use glam::{DMat3, DQuat, DVec3};
use log::error;
use sparse_keyed::SecondaryMap;

use crate::{
    AngularData, BodyId, PositionalData, State, Transform,
    constraint::rigid_body::{PositionalCorrection, PositionalLagrangeMultiplier},
    shape::{Ball, Box3D, collides_2balls, collision_info_2balls},
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
pub(crate) struct RigidBodies {
    rigid_bodies: SecondaryMap<()>,
    boxes: SecondaryMap<Box3D>,
    balls: SecondaryMap<Ball>,
    contacts: Vec<Contact>,
}

impl RigidBodies {
    #[must_use]
    pub fn new() -> Self {
        RigidBodies {
            rigid_bodies: SecondaryMap::new(),
            boxes: SecondaryMap::new(),
            balls: SecondaryMap::new(),
            contacts: Vec::new(),
        }
    }

    pub fn is_rigid_body(&self, id: BodyId) -> bool {
        self.rigid_bodies.contains(id.0)
    }

    pub fn solve_contacts(
        &mut self,
        inverse_timestep_squared: f64,
        positional_data: &mut SecondaryMap<PositionalData>,
        angular_data: &mut SecondaryMap<AngularData>,
    ) {
        self.detect_contacts(positional_data, angular_data);

        for contact in self.contacts.drain(..) {
            if let Some(solved_contact) =
                contact.solve(positional_data, angular_data, inverse_timestep_squared)
            {
                solved_contact.solve_positions(positional_data, angular_data);
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

                    if collides_2balls((ball0, &transform0), (ball1, &transform1)) {
                        let info =
                            collision_info_2balls((ball0, &transform0), (ball1, &transform1));
                        let contact = Contact {
                            bodies: [BodyId(body0), BodyId(body1)],
                            world_normal: info.separating_vector.normalize_or(DVec3::X),
                            local_contact_points: info.local_contact_points,
                            static_friction_coefficient: 0.5,
                        };
                        self.contacts.push(contact);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Contact {
    bodies: [BodyId; 2],
    world_normal: DVec3,
    local_contact_points: [DVec3; 2],
    static_friction_coefficient: f64,
}

impl Contact {
    fn solve(
        self,
        positional_data: &mut SecondaryMap<PositionalData>,
        angular_data: &mut SecondaryMap<AngularData>,
        inverse_h_squared: f64,
    ) -> Option<SolvedContact> {
        let d_pos0 = &positional_data[self.bodies[0].0];
        let d_pos1 = &positional_data[self.bodies[1].0];
        let inverse_masses = [d_pos0.inverse_mass, d_pos1.inverse_mass];

        let d_ang0 = &angular_data[self.bodies[0].0];
        let d_ang1 = &angular_data[self.bodies[1].0];
        let inverse_inertias = [d_ang0.inverse_inertia, d_ang1.inverse_inertia];
        let orientations = [d_ang0.orientation, d_ang1.orientation];

        // let positions = [d_pos0.position, d_pos1.position];

        let pos = [
            d_pos0.position + d_ang0.orientation * self.local_contact_points[0],
            d_pos1.position + d_ang1.orientation * self.local_contact_points[1],
        ];
        let old_pos = [
            d_pos0.previous_position + d_ang0.previous_orientation * self.local_contact_points[0],
            d_pos1.previous_position + d_ang1.previous_orientation * self.local_contact_points[1],
        ];

        let penetration_depth = (pos[0] - pos[1]).dot(self.world_normal);
        if penetration_depth <= 0.0 {
            return None;
        }

        let normal_correction = PositionalCorrection {
            direction: self.world_normal,
            magnitude: penetration_depth,
            application_points: self.local_contact_points,
            compliance: 0.0,
        };

        let Ok(normal_multiplier) = normal_correction.lagrange_multiplier(
            inverse_masses,
            inverse_inertias,
            orientations,
            inverse_h_squared,
        ) else {
            error!(
                "Contact between {:?} and {:?} is unsolveable. This is likely due to infinite masses. Skipping.",
                self.bodies[0], self.bodies[1]
            );
            return None;
        };

        let delta_position = (pos[0] - old_pos[0]) - (pos[1] - old_pos[1]);
        let delta_tangial =
            delta_position - delta_position.dot(self.world_normal) * self.world_normal;

        let (direction, magnitude) = delta_tangial.normalize_and_length();
        let tangential_correction = PositionalCorrection {
            direction,
            magnitude,
            application_points: self.local_contact_points,
            compliance: 0.0,
        };

        let Ok(tangential_multiplier) = tangential_correction.lagrange_multiplier(
            inverse_masses,
            inverse_inertias,
            orientations,
            inverse_h_squared,
        ) else {
            error!(
                "Contact between {:?} and {:?} is unsolveable. This is likely due to infinite masses. Skipping.",
                self.bodies[0], self.bodies[1]
            );
            return None;
        };

        Some(SolvedContact {
            contact: self,
            normal_multiplier,
            tangential_multiplier,
        })
    }
}

#[derive(Debug, Clone)]
struct SolvedContact {
    contact: Contact,
    normal_multiplier: PositionalLagrangeMultiplier,
    tangential_multiplier: PositionalLagrangeMultiplier,
}

impl SolvedContact {
    fn solve_positions(
        &self,
        positional_data: &mut SecondaryMap<PositionalData>,
        angular_data: &mut SecondaryMap<AngularData>,
    ) {
        let static_friction = self.tangential_multiplier.value()
            < self.contact.static_friction_coefficient * self.normal_multiplier.value();

        let linear_corrections = self.normal_multiplier.linear_corrections();
        positional_data[self.contact.bodies[0].0].position += linear_corrections[0];
        positional_data[self.contact.bodies[1].0].position += linear_corrections[1];
        if static_friction {
            let linear_corrections = self.tangential_multiplier.linear_corrections();
            positional_data[self.contact.bodies[0].0].position += linear_corrections[0];
            positional_data[self.contact.bodies[1].0].position += linear_corrections[1];
        }

        let angular_corrections = self.normal_multiplier.angular_corrections();
        let q0 = &mut angular_data[self.contact.bodies[0].0].orientation;
        *q0 = (*q0 + angular_corrections[0]).normalize();
        let q1 = &mut angular_data[self.contact.bodies[1].0].orientation;
        *q1 = (*q1 + angular_corrections[1]).normalize();
    }
}
