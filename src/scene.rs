use light::{DrawLights, LightManager};
use mesh::{DrawMeshes, MeshManager};
use node::NodeManager;

use crate::camera::Camera;

mod light;
mod mesh;
mod node;

pub use mesh::{MeshBuilder, MeshId, PrimitiveBuilder};
pub use node::{NodeCreationError, NodeDescriptor, NodeId, Transform as NodeTransform};

pub struct Scene {
    nodes: NodeManager,
    meshes: MeshManager,
    lights: LightManager,
    pub camera: Camera,
}

impl Scene {
    pub fn new(device: &wgpu::Device, camera: Camera) -> Self {
        let nodes = NodeManager::new(device);
        let meshes = MeshManager::new(
            device,
            nodes.bind_group_layout(),
            camera.bind_group_layout(),
        );
        let lights = LightManager::new(device, camera.bind_group_layout());
        Self {
            nodes,
            meshes,
            lights,
            camera,
        }
    }

    pub fn create_node(
        &mut self,
        nodes: impl IntoIterator<Item = NodeDescriptor>,
        device: &wgpu::Device,
    ) -> Result<Vec<NodeId>, NodeCreationError> {
        self.nodes.create(nodes, &mut self.meshes, device)
    }

    pub fn create_mesh(&mut self, mesh: MeshBuilder, device: &wgpu::Device) -> Result<MeshId, ()> {
        self.meshes.create(mesh, device)
    }
}

pub trait DrawScene {
    fn draw_scene(&mut self, scene: &Scene);
}

impl<'a> DrawScene for wgpu::RenderPass<'a> {
    fn draw_scene(&mut self, scene: &Scene) {
        self.draw_lights(&scene.lights, scene.camera.bind_group());

        if let Some(nodes_bind_group) = scene.nodes.bind_group() {
            self.draw_meshes(&scene.meshes, nodes_bind_group, scene.camera.bind_group());
        }
    }
}
