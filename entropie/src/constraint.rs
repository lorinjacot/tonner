use glam::{Quat, Vec3};
#[cfg(feature = "pyo3")]
use pyo3::prelude::*;
use sparse_keyed::Key;

use crate::BodyId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "pyo3", pyclass(frozen, from_py_object))]
pub struct PositionalConstraintId(pub(crate) Key);

/// A constraint is a condition that must be satisfied by the positions and orientations of a set of bodies in the physics engine. `PositionalConstraint`s enforce conditions by moving the bodies.
///
/// Constraints are used to enforce certain conditions on the motion of objects, such as keeping them at a fixed distance from each other (e.g., for a rope or a rod) or preventing them from penetrating each other (e.g., for collision response).
///
/// Note that despite the name, `PositionalConstraint`s can also result in changes to the orientations of the bodies if the application points are not located at the center of mass of the body.
pub trait PositionalConstraint {
    /// Returns the bodies involved in the constraint.
    fn bodies(&self) -> &[BodyId];

    /// Evaluates the constraint for the given `positions` and `orientations` of the bodies and returns the value of the constraint violation. The value is `0.0` iff the constraint is satisfied. The gradient of the constraint violation with respect to the positions of the bodies should be stored in `position_gradient`, and the positions (in local space) where the constraint forces should be applied should be stored in `application_points`.
    ///
    /// Note that for particles, the orientation will always be `Quat::IDENTITY` and `application_points` will not be used, as no torque can be applied to a particle.
    fn value(
        &self,
        positions: &[Vec3],
        orientations: &[Quat],
        position_gradient: &mut [Vec3],
        application_points: &mut [Vec3],
    ) -> f32;

    /// Compliance (inverse of stiffness) of the constraint. Expressed in meters per Newton. Should always be non-negative.
    ///
    /// A compliance of 0 means that the constraint is perfectly rigid, while a higher compliance means that the constraint is more flexible.
    /// Constraints with a strictly positive compliance will act like a physical spring, applying a force proportional to the violation of the constraint.
    /// The higher the compliance, the weaker the spring.
    fn compliance(&self) -> f32;
}

/// A constraint is a condition that must be satisfied by the positions and orientations of a set of bodies in the physics engine. `AngularConstraint`s enforce conditions by rotating the bodies. An `AngularConstraint` will never cause any movement of the center of mass of the bodies.
///
/// Constraints are used to enforce certain conditions on the motion of objects, such as keeping them at a fixed distance from each other (e.g., for a rope or a rod) or preventing them from penetrating each other (e.g., for collision response).
///
/// Note that `AngularConstraint`s will have no effect on particles, as they have no orientation.
pub trait AngularConstraint {
    /// Returns the bodies involved in the constraint.
    fn bodies(&self) -> &[BodyId];

    /// Evaluates the constraint for the given `positions` and `orientations` of the bodies and returns the value of the constraint violation. The value is `0.0` iff the constraint is satisfied. The gradient of the constraint violation with respect to the orientations of the bodies should be stored in `orientation_gradient`.
    fn value(
        &self,
        positions: &[Vec3],
        orientations: &[Quat],
        orientation_gradient: &mut [Vec3],
    ) -> f32;

    /// Compliance (inverse of stiffness) of the constraint. Expressed in meters per Newton. Should always be non-negative.
    ///
    /// A compliance of 0 means that the constraint is perfectly rigid, while a higher compliance means that the constraint is more flexible.
    /// Constraints with a strictly positive compliance will act like a physical spring, applying a force proportional to the violation of the constraint.
    /// The higher the compliance, the weaker the spring.
    fn compliance(&self) -> f32;
}

pub struct DistanceConstraint {
    pub bodies: [BodyId; 2],
    pub distance: f32,
    pub compliance: f32,
    pub application_points: [Vec3; 2],
}

impl PositionalConstraint for DistanceConstraint {
    fn bodies(&self) -> &[BodyId] {
        &self.bodies
    }

    fn value(
        &self,
        positions: &[Vec3],
        _orientations: &[Quat],
        position_gradient: &mut [Vec3],
        application_points: &mut [Vec3],
    ) -> f32 {
        let delta_pos = positions[0] - positions[1];
        let (dir, dist) = delta_pos.normalize_and_length();
        position_gradient[0] = dir;
        position_gradient[1] = -dir;

        application_points.copy_from_slice(&self.application_points);

        dist - self.distance
    }

    fn compliance(&self) -> f32 {
        self.compliance
    }
}
