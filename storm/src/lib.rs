use std::ops::IndexMut;

// pub use asset::open_gltf;
// pub use environment::Environment;
// pub use math::Transform;
// pub use scene::camera;
pub use storage::{DenseEntry, Id};

// mod asset;
// mod environment;
pub mod geometry;
pub mod math;
// pub mod mesh;
// pub mod resources;
// pub mod scene;
pub mod storage;
// mod texture;

pub trait Storm: Sized {
    type Resources: Resources<Self>;

    type Geometry: Geometry<Self>;
    type GeometryManager: GeometryManager<Self>;
    type GeometryBuilder<'a, 'r>: GeometryBuilder<'a, 'r, Self>;

    // type Scene: Scene<Self>;
    // type Node: Node<Self>;
}

pub trait Manager<T: 'static>: IndexMut<Id<T>, Output = T> + IntoIterator<Item = T> {
    type Iter<'a>: Iterator<Item = &'a T>
    where
        Self: 'a;
    type IterMut<'a>: Iterator<Item = &'a mut T>
    where
        Self: 'a;

    fn get(&self, id: Id<T>) -> Option<&T>;
    fn get_mut(&mut self, id: Id<T>) -> Option<&mut T>;

    fn iter(&self) -> Self::Iter<'_>;
    fn iter_mut(&mut self) -> Self::IterMut<'_>;
}

pub trait Resources<S: Storm<Resources = Self>>: 'static {
    fn device(&self) -> &wgpu::Device;
    fn queue(&self) -> &wgpu::Queue;

    fn geometries(&self) -> &S::GeometryManager;
    fn geometries_mut(&mut self) -> &mut S::GeometryManager;
    fn geometry_builder<'a, 'r>(
        &'r mut self,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> S::GeometryBuilder<'a, 'r> {
        S::GeometryBuilder::new(self, encoder)
    }
}

// pub trait Scene<S: Storm<Scene = Self>>: 'static {}

// pub trait Node<S: Storm<Node = Self>> {}

pub trait Geometry<S: Storm<Geometry = Self>>: DenseEntry<Key = Self> + 'static {}

pub trait GeometryManager<S: Storm<GeometryManager = Self>>: Manager<S::Geometry> {
    fn new(device: &wgpu::Device) -> Self;
}

pub trait GeometryBuilder<'a, 'r, S>
where
    S: Storm<GeometryBuilder<'a, 'r> = Self>,
{
    fn new(resources: &'r mut S::Resources, encoder: &'a mut wgpu::CommandEncoder) -> Self;

    fn build(self) -> &'r mut S::Geometry;
}
