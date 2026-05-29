use std::time::Duration;

use glam::Vec3;

use crate::BodyId;

/// An external force applied to bodies in the physics engine. `Force`s will always be applied at the center of mass of the body, and will not cause any rotation of the body.
///
/// `Force`s can be used to simulate various effects, such as gravity, wind, or user input. They can be applied to particles, rigid bodies or soft bodies.
pub trait Force {
    /// Returns the bodies that the force is applied to.
    fn bodies(&self) -> &[BodyId];

    /// Returns the value of the force at a given time. The value is expressed in world space.
    fn value(&self, time: Duration) -> Vec3;
}

/// An external torque applied to bodies in the physics engine. `Torque`s will always be applied at the center of mass of the body and will not cause any movement of the center of mass.
///
/// `Torque`s can be used to simulate various effects, such as wind, or user input. They can be applied to rigid bodies or soft bodies. Applying a `Torque` to a particle will have no effect.
pub trait Torque {
    /// Returns the bodies that the torque is applied to.
    fn bodies(&self) -> &[BodyId];

    /// Returns the value of the torque at a given time. The value is expressed in world space.
    fn value(&self, time: Duration) -> Vec3;
}
