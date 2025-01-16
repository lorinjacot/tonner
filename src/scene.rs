use light::{DrawLights, LightManager};
use material::MaterialManager;
use mesh::{DrawMeshes, MeshManager};
use node::NodeManager;

use crate::camera::Camera;

mod light;
mod material;
mod mesh;
mod node;

pub use material::{MaterialDescriptor, MaterialId};
pub use mesh::{
    MeshCreationError, MeshDescriptor, MeshId, PrimitiveAttributes, PrimitiveDescriptor,
    PrimitiveIndices,
};
pub use node::{NodeCreationError, NodeDescriptor, NodeId, Transform as NodeTransform};

pub struct Scene {
    nodes: NodeManager,
    meshes: MeshManager,
    materials: MaterialManager,
    lights: LightManager,
    pub camera: Camera,
}

impl Scene {
    pub fn new(device: &wgpu::Device, camera: Camera) -> Self {
        let nodes = NodeManager::new(device);
        
        let lights = LightManager::new(device, camera.bind_group_layout());
        
        let materials = MaterialManager::new(device);
        
        log::debug!("Creating meshes manager...");
        let meshes = MeshManager::new(
            device,
            nodes.bind_group_layout(),
            camera.bind_group_layout(),
            lights.bind_group_layout(),
            materials.bind_group_layout(),
        );
        log::debug!("Meshes manager created");

        Self {
            nodes,
            meshes,
            materials,
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

    pub fn create_mesh(
        &mut self,
        mesh: MeshDescriptor,
        device: &wgpu::Device,
    ) -> Result<MeshId, MeshCreationError> {
        self.meshes.create(mesh, device)
    }

    pub fn create_material(
        &mut self,
        material: MaterialDescriptor,
        device: &wgpu::Device,
    ) -> MaterialId {
        self.materials.create(material, device)
    }
}

pub trait DrawScene {
    fn draw_scene(&mut self, scene: &Scene);
}

impl<'a> DrawScene for wgpu::RenderPass<'a> {
    fn draw_scene(&mut self, scene: &Scene) {
        self.draw_lights(&scene.lights, scene.camera.bind_group());

        if let Some(nodes_bind_group) = scene.nodes.bind_group() {
            self.draw_meshes(
                &scene.meshes,
                &scene.materials,
                nodes_bind_group,
                scene.camera.bind_group(),
                scene.lights.bind_group(),
            );
        }
    }
}
