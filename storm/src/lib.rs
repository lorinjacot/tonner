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

pub trait StormTrait: Sized + 'static {
    type Resources: ResourcesTrait<Self>;

    type Geometry: GeometryTrait<Self>;
    type GeometryManager: GeometryManagerTrait<Self>;
    type GeometryBuilder<'a, 'r>: GeometryBuilderTrait<'a, 'r, Self>;

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

pub trait ResourcesTrait<Storm: StormTrait<Resources = Self>>: 'static {
    fn device(&self) -> &wgpu::Device;
    fn queue(&self) -> &wgpu::Queue;

    fn geometries(&self) -> &Storm::GeometryManager;
    fn geometries_mut(&mut self) -> &mut Storm::GeometryManager;
    fn geometry_builder<'a, 'r>(
        &'r mut self,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> Storm::GeometryBuilder<'a, 'r> {
        Storm::GeometryBuilder::new(self, encoder)
    }
}

// pub trait Scene<S: Storm<Scene = Self>>: 'static {}

// pub trait Node<S: Storm<Node = Self>> {}

#[derive(Clone)]
pub struct IndexBuffer {
    pub buffer: wgpu::Buffer,
    pub format: wgpu::IndexFormat,
}

pub trait GeometryTrait<Storm: StormTrait<Geometry = Self>>:
    DenseEntry<Key = Self> + 'static
{
    fn indices(&self) -> &Option<IndexBuffer>;

    fn vertex_buffer(&self) -> &[wgpu::Buffer];

    fn vertex_buffer_layouts(
        &self,
    ) -> impl Iterator<Item = wgpu::VertexBufferLayout> + ExactSizeIterator;

    fn vertex_count(&self) -> u32;
}

pub trait GeometryManagerTrait<Storm: StormTrait<GeometryManager = Self>>:
    Manager<Storm::Geometry>
{
    fn new(device: &wgpu::Device) -> Self;
}

pub trait GeometryBuilderTrait<'a, 'r, Storm>
where
    Storm: StormTrait<GeometryBuilder<'a, 'r> = Self>,
{
    fn new(resources: &'r mut Storm::Resources, encoder: &'a mut wgpu::CommandEncoder) -> Self;

    fn build(self) -> &'r mut Storm::Geometry;
}
