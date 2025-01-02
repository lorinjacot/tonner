use mesh::{DrawMeshes, MeshManager};
use node::NodeManager;

use crate::camera::Camera;

mod mesh;
mod node;

pub use mesh::{MeshBuilder, MeshId, PrimitiveBuilder};
pub use node::{NodeBuilder, NodeId, Transform as NodeTransform};

pub struct Scene {
    nodes: NodeManager,
    meshes: MeshManager,
    pub camera: Camera,
}

impl Scene {
    pub fn new(
        device: &wgpu::Device,
        camera: Camera,
        targets: &[Option<wgpu::ColorTargetState>],
    ) -> Self {
        let nodes = NodeManager::new(device);
        let meshes = MeshManager::new(
            device,
            nodes.bind_group_layout(),
            camera.bind_group_layout(),
            targets,
        );
        Self {
            nodes,
            meshes,
            camera,
        }
    }

    pub fn create_node(
        &mut self,
        nodes: impl IntoIterator<Item = NodeBuilder>,
        device: &wgpu::Device,
    ) -> Result<Vec<NodeId>, ()> {
        let (nodes_id, mesh_nodes_mapping) = self.nodes.create(nodes, device)?;

        for (mesh, nodes) in mesh_nodes_mapping {
            self.add_mesh_to_nodes(mesh, nodes, device);
        }

        Ok(nodes_id)
    }

    pub fn create_mesh(&mut self, mesh: MeshBuilder, device: &wgpu::Device) -> Result<MeshId, ()> {
        self.meshes.create(mesh, device)
    }

    pub fn add_mesh_to_nodes(
        &mut self,
        mesh: MeshId,
        nodes: impl IntoIterator<Item = NodeId>,
        device: &wgpu::Device,
    ) {
        for node in nodes {
            if let Some(current_mesh) = self.nodes[node].mesh {
                if current_mesh == mesh {
                    continue;
                } else {
                    let current_mesh = &mut self.meshes[current_mesh];
                    current_mesh.nodes.remove(&node);
                    current_mesh.update_nodes_buffer(
                        self.nodes
                            .dense_indices_u32(current_mesh.nodes.iter().copied()),
                        device,
                    );
                }
            }

            self.meshes[mesh].nodes.insert(node);
        }

        let mesh = &mut self.meshes[mesh];
        mesh.update_nodes_buffer(
            self.nodes.dense_indices_u32(mesh.nodes.iter().copied()),
            device,
        );
    }
}

pub trait DrawScene {
    fn draw_scene(&mut self, scene: &Scene);
}

impl<'a> DrawScene for wgpu::RenderPass<'a> {
    fn draw_scene(&mut self, scene: &Scene) {
        if let Some(nodes_bind_group) = scene.nodes.bind_group() {
            self.draw_meshes(&scene.meshes, nodes_bind_group, scene.camera.bind_group());
        }
    }
}
