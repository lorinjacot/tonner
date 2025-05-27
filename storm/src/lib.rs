// pub use asset::open_gltf;
// pub use environment::Environment;
// pub use math::Transform;
// pub use scene::camera;
// pub use storage::{DenseEntry, Id};

// mod asset;
// mod environment;
// pub mod geometry;
// pub mod math;
// pub mod mesh;
// pub mod resources;
// pub mod scene;
// mod storage;
// mod texture;

pub trait Storm: Sized {
    type Resources: Resources<Self>;
    type Scene: Scene<Self>;
    type Node: Node<Self>;
    type Geometry: Geometry<Self>;
}

pub trait Resources<S: Storm<Resources = Self>> {}

pub trait Scene<S: Storm<Scene = Self>> {}

pub trait Node<S: Storm<Node = Self>> {
    type Builder: for<'s> NodeBuilder<'s, S>;
}

pub trait NodeBuilder<'s, S: Storm>
where
    S::Node: Node<S, Builder = Self>,
{
    fn build(self) -> &'s mut S::Node;
}

pub trait Geometry<S: Storm<Geometry = Self>> {
    type Builder;
}

pub trait GeometryBuilder<'s, S: Storm>
where
    S::Geometry: Geometry<S, Builder = Self>,
{
    fn build(self) -> &'s mut S::Geometry;
}
