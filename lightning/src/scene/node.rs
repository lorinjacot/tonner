use std::ops::Deref;

use glam::{Mat4, Quat, Vec3};

use storm::{DenseEntry, Id, math::Transform};
use storm_renderer::mesh::Mesh;

use super::{Camera, PointLight, Scene, camera::CameraDescriptor, instanciate_mesh_unchecked};

pub struct Node {
    id: Id<Node>,
    pub name: String,
    parent: Option<Id<Node>>,
    children: Vec<Id<Node>>,
    pub(super) local_transform: Transform,
    world_matrix: Mat4,
    mesh: Option<Id<Mesh>>,
}

impl Node {
    pub fn parent(&self) -> Option<Id<Node>> {
        self.parent
    }

    pub fn children(&self) -> &[Id<Node>] {
        &self.children
    }

    pub fn local_transform(&self) -> &Transform {
        &self.local_transform
    }

    pub fn local_position(&self) -> Vec3 {
        self.local_transform.translation()
    }

    pub fn world_matrix(&self) -> Mat4 {
        self.world_matrix
    }

    pub fn world_position(&self) -> Vec3 {
        self.world_matrix.project_point3(Vec3::ZERO)
    }
}

pub struct NodeHandle<'a> {
    pub(super) id: Id<Node>,
    pub(super) scene: &'a mut Scene,
}

impl<'a> NodeHandle<'a> {
    pub(super) fn update_world_matrices_parent(&mut self, parent_matrix: Mat4) {
        let node = &mut self.scene[self.id];
        let world_matrix = parent_matrix * node.local_transform.matrix();
        node.world_matrix = world_matrix;
        let children = node.children.clone();
        for child in children {
            self.scene
                .node_handle(child)
                .update_world_matrices_parent(world_matrix);
        }
    }

    pub(super) fn update_world_matrices(&mut self) {
        let node = &self.scene[self.id];
        let world_matrix = match self.parent {
            Some(parent) => self.scene[parent].world_matrix * node.local_transform().matrix(),
            None => node.local_transform.matrix(),
        };
        let node = &mut self.scene[self.id];
        node.world_matrix = world_matrix;
        let children = node.children.clone();
        for child in children {
            self.scene
                .node_handle(child)
                .update_world_matrices_parent(world_matrix);
        }
    }

    pub fn set_local_transform(&mut self, transform: Transform) {
        let node = &mut self.scene[self.id];
        node.local_transform = transform;
        self.update_world_matrices();
    }

    pub fn set_mesh(&mut self, mesh: Option<&Mesh>) {
        let node = &mut self.scene[self.id];
        if node.mesh == mesh.map(|mesh| mesh.id()) {
            return;
        }

        // remove old mesh
        self.scene[self.id].mesh.take().map(|old_mesh| {
            let nodes = &mut self.scene.meshes[old_mesh].nodes;
            let index = nodes.iter().position(|node| *node == self.id).unwrap();
            nodes.swap_remove(index);
        });

        if let Some(mesh) = mesh {
            instanciate_mesh_unchecked(&mut self.scene.meshes, mesh, self.id, &self.scene.device);
        }
    }

    pub fn set_camera(&mut self, camera: Option<CameraDescriptor>) {
        match camera {
            Some(desc) => {
                self.scene.cameras.insert(Camera::new(self.id, desc));
            }
            None => {
                self.scene.cameras.remove(self.id);
            }
        }
    }
}

impl<'a> Deref for NodeHandle<'a> {
    type Target = Node;

    fn deref(&self) -> &Self::Target {
        &self.scene[self.id]
    }
}

pub struct NodeBuilder<'a, 's> {
    scene: &'s mut Scene,
    name: Option<String>,
    parent: Option<Id<Node>>,
    local_transform: Transform,
    world_matrix: Mat4,
    mesh: Option<&'a Mesh>,
    camera: Option<CameraDescriptor>,
    point_light: Option<Vec3>,
}

impl<'a, 's> NodeBuilder<'a, 's> {
    pub fn new(scene: &'s mut Scene) -> Self {
        Self {
            scene,
            parent: None,
            name: None,
            local_transform: Transform::IDENTITY,
            world_matrix: Mat4::IDENTITY,
            mesh: None,
            camera: None,
            point_light: None,
        }
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn parent(mut self, parent: Option<Id<Node>>) -> Self {
        self.parent = parent;
        self
    }

    pub fn translation_rotation_scale(
        mut self,
        translation: Vec3,
        rotation: Quat,
        scale: Vec3,
    ) -> Self {
        self.local_transform
            .translation_rotation_scale(translation, rotation, scale);
        self
    }

    pub fn local_position(mut self, position: Vec3) -> Self {
        self.local_transform.set_translation(position);
        self
    }

    pub fn local_matrix(mut self, matrix: Mat4) -> Self {
        self.local_transform.set_matrix(matrix);
        self
    }

    pub fn mesh(mut self, mesh: &'a Mesh) -> Self {
        self.mesh = Some(mesh);
        self
    }

    pub fn camera(mut self, camera: Option<CameraDescriptor>) -> Self {
        self.camera = camera;
        self
    }

    pub fn point_light(mut self, color: Vec3) -> Self {
        self.point_light = Some(color);
        self
    }

    pub fn build(mut self) -> &'s mut Node {
        let id = self.scene.nodes.next_id();
        match self.parent {
            Some(parent) => {
                let parent = &mut self.scene.nodes[parent];
                parent.children.push(id);
                self.world_matrix = parent.world_matrix * self.local_transform.matrix();
            }
            None => {
                self.scene.root_nodes.push(id);
                self.world_matrix = self.local_transform.matrix()
            }
        }
        self.scene.nodes_buffer = None;
        let id = self.scene.nodes.next_id();
        let node = Node {
            id,
            name: self.name.unwrap_or_else(|| format!("Node {id}")),
            parent: self.parent,
            children: Vec::new(),
            local_transform: self.local_transform,
            world_matrix: self.world_matrix,
            mesh: self.mesh.map(|mesh| mesh.id()),
        };
        let node = self.scene.nodes.insert(node);

        if let Some(mesh) = self.mesh {
            instanciate_mesh_unchecked(&mut self.scene.meshes, mesh, node.id(), &self.scene.device);
        }

        if let Some(camera) = self.camera {
            self.scene.cameras.insert(Camera::new(node.id(), camera));
        }

        if let Some(color) = self.point_light {
            self.scene.point_lights.insert(PointLight {
                node: node.id(),
                color,
            });
        }

        node
    }
}

impl DenseEntry for Node {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}
