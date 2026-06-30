//! Collision detection algorithms and data structures.
//!
//! A collision happens when distinct objects overlap in space. Collision detection is the problem of determining whether two objects collide and, if so, how they collide.
//! This process is subdivided into two phases:
//! 1. **Broad phase**: This phase quickly identifies pairs of objects that might collide. It typically uses simple bounding volumes (like axis-aligned bounding boxes)
//! to quickly rule out pairs of objects that are far apart and cannot collide. For large scenes with many objects, this phase also uses spatial partitioning techniques
//! like grids, BVH (bounding volume hierarchies) or KD-trees to further reduce the number of pairs that need to be checked in the narrow phase.
//! 2. **Narrow phase**: This phase performs detailed collision checks on the pairs of objects identified in the broad phase. It uses more precise algorithms that take into account
//! the actual geometry of the objects to determine if they collide and, if so, how they collide (e.g., contact points, penetration depth, etc.).

pub mod narrow;
