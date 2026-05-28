//! Constraints for the physics engine.
//!
//! This module defines different types of constraints that can be applied to objects
//! in the physics engine. Constraints are used to enforce certain conditions on the
//! motion of objects, such as keeping them at a fixed distance from each other (e.g.,
//! for a rope or a rod) or preventing them from penetrating each other (e.g., for
//! collision response). Constraints can be either positional (enforcing conditions
//! on the positions of objects) or angular (enforcing conditions on the orientations
//! of objects).

use glam::Vec3;

use crate::Transform;

pub trait PositionalConstraint {
    fn delta(&self, transform0: &Transform, transform1: &Transform) -> PositionalCorrection;

    /// Compliance (inverse of stiffness) of the constraint. Expressed in meters per Newton. Should always be non-negative.
    /// 
    /// A compliance of 0 means that the constraint is perfectly rigid, while a higher compliance means that the constraint is more flexible.
    /// Constraints with a strictly positive compliance will act like a physical spring, applying a force proportional to the violation of the constraint.
    /// The higher the compliance, the weaker the spring.
    fn compliance(&self) -> f32;
}

/// Information about how to correct the positions of two objects to satisfy a positional constraint.
pub struct PositionalCorrection {
    /// The direction in which to apply the correction. This is a unit vector pointing from the first object to the second object.
    /// Expressed in world space.
    pub direction: Vec3,
    /// The magnitude of the correction to apply.
    pub magnitude: f32,
    /// The positions (in local space) where the correction should be applied on the first and second object, respectively.
    /// Expressed in the local space of each object, i.e. without the object's transform applied.
    pub positions: [Vec3; 2],
}

pub trait AngularConstraint {
    fn delta(&self, transform0: &Transform, transform1: &Transform) -> (Vec3, f32);

    fn compliance(&self) -> f32;
}
