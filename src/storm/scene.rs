use glam::Mat4;

use crate::storage::{Id, SecondaryStorage, Storage};

use super::{camera::Camera, mesh::MeshId};

pub struct Scene {
    nodes: Storage<Node>,
    cameras: SecondaryStorage<Node, Box<dyn Camera>>,
    meshes: SecondaryStorage<MeshId, Vec<NodeId>>,
}

impl Scene {
    pub fn new() -> Self {
        let nodes = Storage::new();
        let cameras = SecondaryStorage::new();
        let meshes = SecondaryStorage::new();

        Scene {
            nodes,
            cameras,
            meshes,
        }
    }

    pub fn create_node(&mut self, parent: Option<NodeId>, local_transform: Mat4) -> NodeId {
        let children = Vec::new();
        match parent {
            Some(parent_id) => {
                let global_transform = self.nodes[parent_id].global_transform * local_transform;
                let id = self.nodes.add(Node {
                    local_transform,
                    global_transform,
                    parent,
                    children,
                });
                self.nodes[parent_id].children.push(id);
                id
            }
            None => self.nodes.add(Node {
                local_transform,
                global_transform: local_transform,
                parent,
                children,
            }),
        }
    }

    pub fn add_camera(&mut self, camera: impl Camera + 'static, node: NodeId) {
        assert!(self.nodes.contains(node));
        self.cameras.add(Box::new(camera), node);
    }

    pub fn camera_mut(&mut self, id: NodeId) -> Option<(&mut dyn Camera, &mut Node)> {
        Some((self.cameras.get_mut(id)?.as_mut(), self.nodes.get_mut(id)?))
    }

    pub fn update(&mut self) {}

    pub fn render(&self, camera: NodeId, render_pass: &mut wgpu::RenderPass) {
        // render_pass.set_pipeline(&self.hdr_pipeline);
        // render_pass.set_bind_group(0, Some(&self.hdr_bind_group), &[]);
        // render_pass.draw(0..3, 0..1);
    }
}

pub type NodeId = Id<Node>;
pub struct Node {
    local_transform: Mat4,
    global_transform: Mat4,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
}
