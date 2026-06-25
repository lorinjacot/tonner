//! Collision detection algorithms and data structures.
//!
//! A collision happens when distinct objects overlap in space. Collision detection is the problem of determining whether two objects collide and, if so, how they collide.
//! This process is subdivided into two phases:
//! 1. **Broad phase**: This phase quickly identifies pairs of objects that might collide. It typically uses simple bounding volumes (like axis-aligned bounding boxes)
//! to quickly rule out pairs of objects that are far apart and cannot collide. For large scenes with many objects, this phase also uses spatial partitioning techniques
//! like grids, BVH (bounding volume hierarchies) or KD-trees to further reduce the number of pairs that need to be checked in the narrow phase.
//! 2. **Narrow phase**: This phase performs detailed collision checks on the pairs of objects identified in the broad phase. It uses more precise algorithms that take into account
//! the actual geometry of the objects to determine if they collide and, if so, how they collide (e.g., contact points, penetration depth, etc.).

use glam::DVec3;

/// Information about a collision between two objects. This is returned by the narrow phase of the collision detection process.
///
/// This struct contains the minimal translation that can be applied to the second object to separate the two objects, as well as the contact points on both objects.
/// Contact points are points on the surface of the objects that are in contact with each other.
/// The force applied to the objects during the collision response is applied at these contact points.
#[derive(Debug, Clone)]
pub struct CollisionInfo {
    /// The minimal translation that can be applied to the second object to separate the two objects. If the two objects are exactly touching, this vector is `DVec3::ZERO`.
    ///
    /// This vector is always equal to the difference between the contact points on the two objects, i.e. `contact_point[0] - contact_point[1]`.
    pub separating_vector: DVec3,

    /// The contact points on the two objects. These are points on the surface of the objects that are in contact with each other. They are expressed in the local frame of each object (i.e., before applying the object's transform).
    ///
    /// If one applies the `separating_vector` to the second object, the two objects will be exactly touching at these points.
    pub local_contact_points: [DVec3; 2],
}
