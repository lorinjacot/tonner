use std::{array::from_fn, ops::Deref};

use bytemuck::{Pod, Zeroable, cast_slice};
use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

use crate::{DenseEntry, Id, Resources, Transform, geometry::MAX_MORPH_TARGET_COUNT, mesh::Mesh};

use super::{Camera, PointLight, Scene, camera::CameraDescriptor, skin::Skin};

pub struct Node {
    id: Id<Node>,
    pub name: String,
    parent: Option<Id<Node>>,
    children: Vec<Id<Node>>,
    pub(super) local_transform: Transform,
    world_matrix: Mat4,
    mesh: Option<Id<Mesh>>,
    skin: Option<Id<Skin>>,
    pub(super) weights: Vec<f32>,
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

    pub fn weights(&self) -> &[f32] {
        &self.weights
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

    pub fn set_mesh(&mut self, mesh: Option<Id<Mesh>>, resources: &Resources) {
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
                .add_mesh_to_node_unchecked(mesh, self.id, resources);
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

pub struct NodeBuilder<'s> {
    scene: &'s mut Scene,
    name: Option<String>,
    parent: Option<Id<Node>>,
    local_transform: Transform,
    world_matrix: Mat4,
    mesh: Option<Id<Mesh>>,
    camera: Option<CameraDescriptor>,
    point_light: Option<Vec3>,
    weights: Option<Vec<f32>>,
}

impl<'s> NodeBuilder<'s> {
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
            weights: None,
        }
    }

    pub fn name(mut self, name: impl Into<Option<String>>) -> Self {
        self.name = name.into();
        self
    }

    pub fn parent(mut self, parent: impl Into<Option<Id<Node>>>) -> Self {
        self.parent = parent.into();
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

    pub fn local_matrix(mut self, matrix: impl Into<Option<Mat4>>) -> Self {
        self.local_transform
            .set_matrix(matrix.into().unwrap_or(Mat4::IDENTITY));
        self
    }

    pub fn mesh(mut self, mesh: impl Into<Option<Id<Mesh>>>) -> Self {
        self.mesh = mesh.into();
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

    pub fn weights(mut self, weight: impl Into<Option<Vec<f32>>>) -> Self {
        self.weights = weight.into();
        self
    }

    pub fn build(mut self, resources: &Resources) -> &'s mut Node {
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
            weights: self.weights.unwrap_or_default(),
            skin: None,
        };
        let node = self.scene.nodes.insert(node).id();

        if let Some(mesh) = self.mesh {
            self.scene.add_mesh_to_node_unchecked(mesh, node, resources);
        }

        if let Some(camera) = self.camera {
            self.scene.cameras.insert(Camera::new(node, camera));
        }

        if let Some(color) = self.point_light {
            self.scene.point_lights.insert(PointLight { node, color });
        }

        &mut self.scene.nodes[node]
    }
}

impl DenseEntry for Node {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

impl Scene {
    pub(super) fn update_nodes_buffer(&mut self) {
        let data: Vec<_> = self
            .nodes
            .iter()
            .map(|node| {
                let matrix = node.world_matrix();
                let mut weights_iter = node.weights().iter().copied();
                let weights = from_fn(|_| weights_iter.next().unwrap_or(0.0));
                let joint_offset = match node.skin {
                    Some(skin) => self.skins[skin].joint_offset(),
                    None => 0,
                };
                NodeUniform {
                    matrix,
                    weights,
                    joint_offset,
                    _pad: [0; 3],
                }
            })
            .collect();

        match &self.nodes_buffer {
            Some(buffer) => {
                self.queue.write_buffer(buffer, 0, cast_slice(&data));
            }
            None => {
                self.render_bind_group = None;
                let buffer = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("{}'s nodes buffer", self.name)),
                        contents: cast_slice(&data),
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    });
                self.nodes_buffer = Some(buffer);
            }
        }
    }

    pub fn add_skin_to_node(&mut self, skin: Id<Skin>, node: Id<Node>) {
        self.nodes[node].skin = Some(skin);
    }
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct NodeUniform {
    matrix: Mat4,
    weights: [f32; MAX_MORPH_TARGET_COUNT],
    joint_offset: u32,
    _pad: [u32; 3],
}
