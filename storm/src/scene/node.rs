use std::ops::Deref;

use glam::{Mat4, Quat, Vec3};

use crate::{DenseEntry, Id, Transform, mesh::Mesh, storage::SetEntry};

use super::{Camera, Scene, camera::CameraDescriptor};

pub struct Node {
    id: Id<Node>,
    pub name: String,
    parent: Option<Id<Node>>,
    children: Vec<Id<Node>>,
    local_transform: Transform,
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
    fn update_matrices(&mut self, parent_matrix: Mat4) {
        let node = &mut self.scene[self.id];
        let world_matrix = parent_matrix * node.local_transform.matrix();
        node.world_matrix = world_matrix;
        let children = node.children.clone();
        for child in children {
            self.scene.node_handle(child).update_matrices(world_matrix);
        }
    }

    pub fn set_local_transform(&mut self, transform: Transform) {
        let node = &self.scene[self.id];
        let world_matrix = match node.parent {
            Some(parent) => self.scene[parent].world_matrix * transform.matrix(),
            None => transform.matrix(),
        };
        let node = &mut self.scene[self.id];
        node.local_transform = transform;
        node.world_matrix = world_matrix;
        let children = node.children.clone();
        for child in children {
            self.scene.node_handle(child).update_matrices(world_matrix);
        }
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
            self.scene
                .meshes
                .instanciate_unchecked(mesh, self.id, &self.scene.device);
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
    desc: NodeDescriptor,
    mesh: Option<&'a Mesh>,
    camera: Option<CameraDescriptor>,
}

impl<'a, 's> NodeBuilder<'a, 's> {
    pub fn new(scene: &'s mut Scene) -> Self {
        Self {
            scene,
            desc: NodeDescriptor {
                parent: None,
                name: None,
                children: Vec::new(),
                local_transform: Transform::IDENTITY,
                world_matrix: Mat4::IDENTITY,
                mesh: None,
            },
            mesh: None,
            camera: None,
        }
    }

    pub fn name(mut self, name: String) -> Self {
        self.desc.name = Some(name);
        self
    }

    pub fn parent(mut self, parent: Option<Id<Node>>) -> Self {
        self.desc.parent = parent;
        self
    }

    pub fn translation_rotation_scale(
        mut self,
        translation: Vec3,
        rotation: Quat,
        scale: Vec3,
    ) -> Self {
        self.desc
            .local_transform
            .translation_rotation_scale(translation, rotation, scale);
        self
    }

    pub fn local_position(mut self, position: Vec3) -> Self {
        self.desc.local_transform.set_translation(position);
        self
    }

    pub fn local_matrix(mut self, matrix: Mat4) -> Self {
        self.desc.local_transform.set_matrix(matrix);
        self
    }

    pub fn mesh(mut self, mesh: &'a Mesh) -> Self {
        self.mesh = Some(mesh);
        self.desc.mesh = Some(mesh.id());
        self
    }

    pub fn camera(mut self, camera: Option<CameraDescriptor>) -> Self {
        self.camera = camera;
        self
    }

    pub fn build(mut self) -> &'s mut Node {
        let id = self.scene.nodes.next_id();
        match self.desc.parent {
            Some(parent) => {
                let parent = &mut self.scene.nodes[parent];
                parent.children.push(id);
                self.desc.world_matrix = parent.world_matrix * self.desc.local_transform.matrix();
            }
            None => {
                self.scene.root_nodes.push(id);
                self.desc.world_matrix = self.desc.local_transform.matrix()
            }
        }
        self.scene.nodes_buffer = None;
        let node = self.scene.nodes.push(self.desc);

        if let Some(mesh) = self.mesh {
            self.scene
                .meshes
                .instanciate_unchecked(mesh, node.id(), &self.scene.device);
        }

        if let Some(camera) = self.camera {
            self.scene.cameras.insert(Camera::new(node.id(), camera));
        }

        node
    }
}

pub struct NodeDescriptor {
    name: Option<String>,
    parent: Option<Id<Node>>,
    children: Vec<Id<Node>>,
    local_transform: Transform,
    world_matrix: Mat4,
    mesh: Option<Id<Mesh>>,
}

impl DenseEntry for Node {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

impl SetEntry for Node {
    type Descriptor = NodeDescriptor;

    fn new(id: Id<Self::Key>, desc: Self::Descriptor) -> Self {
        let name = desc.name.unwrap_or_else(|| id.to_string());
        Self {
            id,
            name,
            parent: desc.parent,
            children: desc.children,
            local_transform: desc.local_transform,
            world_matrix: desc.world_matrix,
            mesh: desc.mesh,
        }
    }
}
