use node::NodeManager;

use crate::camera::Camera;

mod node;

pub use node::{Builder as NodeBuilder, NodeId, Transform as NodeTransform};

pub struct Scene {
    nodes: NodeManager,
    pub camera: Camera,
}

impl Scene {
    pub fn new(device: &wgpu::Device, camera: Camera) -> Self {
        Self {
            nodes: NodeManager::new(device),
            camera,
        }
    }

    pub fn create_node(
        &mut self,
        nodes: impl IntoIterator<Item = NodeBuilder>,
        device: &wgpu::Device,
    ) -> Result<Vec<NodeId>, ()> {
        self.nodes.create(nodes, device)
    }
}

pub trait DrawScene {
    fn draw_scene(&mut self, scene: &Scene);
}

impl<'a> DrawScene for wgpu::RenderPass<'a> {
    fn draw_scene(&mut self, _scene: &Scene) {}
}
