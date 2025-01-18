use light::{DrawLights, LightManager};
use material::MaterialManager;
use mesh::{DrawMeshes, MeshManager};
use node::NodeManager;

use crate::camera::Camera;

mod light;
mod material;
mod mesh;
mod node;

pub use material::{MaterialDescriptor, MaterialId, TextureDescriptor};
pub use mesh::{
    MeshCreationError, MeshDescriptor, MeshId, PrimitiveAttributes, PrimitiveDescriptor,
    PrimitiveIndices, COLORS_LEN, TEX_COORDS_LEN,
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
    pub fn new(camera: Camera, device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let nodes = NodeManager::new(device);

        let lights = LightManager::new(device, camera.bind_group_layout());

        let materials = MaterialManager::new(device, queue);

        let meshes = MeshManager::new(
            device,
            nodes.bind_group_layout(),
            camera.bind_group_layout(),
            lights.bind_group_layout(),
            materials.bind_group_layout(),
        );

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
        node: &NodeDescriptor,
        device: &wgpu::Device,
    ) -> Result<NodeId, NodeCreationError> {
        self.nodes.create(node, &mut self.meshes, device)
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
        material: &MaterialDescriptor,
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
