# Tonner

Entropie is a 3D physics engine written in Rust. It is designed to be fast, flexible, and easy to use. It is currently in early development.

## Goals/focus

- Real-time physics and convincing (as opposed to physically accurate) results.
- A focus on 3D physics, but support for 2D physics is also planned.
- User-defined constraints and forces
- Rigid body dynamics:
    - Support for various shapes convex shapes, including boxes, balls, cylinders, capsules and polyhedrons.
    - Support for position and orientation-based constraints.
- (Planned) Fluid dynamics
- (Planned) Soft body dynamics
- A simple and intuitive API with Python and JavaScript/TypeScript bindings.

## Architecture

Modules: 
```
├── shape                   Traits and structs for defining the geometry of objects.
├── force                   Traits to define forces and algorithms to apply them to objects.
├── constraint              Traits to define constraints and algorithms to solve them.
└── collision               Algorithms to detect collisions and create constraints from them.
    ├── broad_phase         Broad phase collision detection algorithms and data structures.
    └── narrow_phase        Narrow phase collision detection algorithms and data structures.
```