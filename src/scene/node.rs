use std::{
    collections::HashMap,
    ops::{Index, IndexMut},
};

use glam::{Mat4, Quat, Vec3};
use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::storage::{Id, Storage};

use super::mesh::{MeshId, MeshManager};

pub struct NodeManager {
    nodes: Storage<Node>,
    global_transform_buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl NodeManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let global_transform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Nodes local transform buffer"),
                contents: &[],
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
            });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Nodes bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = None;

        Self {
            nodes: Storage::new(),
            global_transform_buffer,
            bind_group,
            bind_group_layout,
        }
    }

    pub fn bind_group(&self) -> &Option<wgpu::BindGroup> {
        &self.bind_group
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn create(
        &mut self,
        nodes: impl IntoIterator<Item = NodeDescriptor>,
        meshes: &mut MeshManager,
        device: &wgpu::Device,
    ) -> Result<Vec<NodeId>, NodeCreationError> {
        let nodes = nodes.into_iter();
        let mut nodes_id = Vec::with_capacity(nodes.size_hint().0);
        let mut meshes_nodes = HashMap::new();

        for node in nodes {
            nodes_id.push(self.create_as_child(node, None, &mut meshes_nodes)?);
        }

        self.create_buffer(device);

        for (mesh, nodes) in meshes_nodes {
            let mesh = meshes
                .get_mut(mesh)
                .ok_or(NodeCreationError::InvalidMesh(mesh))?;
            mesh.nodes.extend(&nodes);
            mesh.update_nodes_buffer(self.nodes.dense_indices_u32(nodes), device);
        }

        Ok(nodes_id)
    }

    fn create_as_child(
        &mut self,
        node: NodeDescriptor,
        parent: Option<NodeId>,
        meshes_nodes: &mut HashMap<MeshId, Vec<NodeId>>,
    ) -> Result<NodeId, NodeCreationError> {
        let local_matrix = match node.local_transform {
            Transform::Matrix(matrix) => matrix,
            Transform::TRS {
                translation,
                rotation,
                scale,
            } => Mat4::from_scale_rotation_translation(scale, rotation, translation),
        };

        let node_id = self.nodes.add(Node {
            local_transform: node.local_transform,
            global_transform: local_matrix,
            parent,
            children: Vec::new(),
            mesh: node.mesh,
        });

        if let Some(parent_id) = parent {
            let parent = self
                .nodes
                .get_mut(parent_id)
                .ok_or(NodeCreationError::InvalidParent(parent_id))?;
            parent.children.push(node_id);
            self.nodes[node_id].global_transform = parent.global_transform * local_matrix;
        }

        if node.children.len() > 0 {
            let mut children = Vec::with_capacity(node.children.len());
            for child in node.children {
                children.push(self.create_as_child(child, Some(node_id), meshes_nodes)?);
            }
            self.nodes[node_id].children = children;
        }

        if let Some(mesh_id) = node.mesh {
            meshes_nodes.entry(mesh_id).or_default().push(node_id);
        }

        Ok(node_id)
    }

    fn create_buffer(&mut self, device: &wgpu::Device) {
        let global_transforms = self
            .nodes
            .values()
            .map(|node| node.global_transform)
            .collect::<Vec<_>>();
        let contents = bytemuck::cast_slice(&global_transforms);
        self.global_transform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Nodes local transform buffer"),
                contents,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
            });

        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Nodes bind group"),
            layout: &self.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.global_transform_buffer.as_entire_binding(),
            }],
        }));
    }

    pub fn dense_indices_u32(&self, ids: impl IntoIterator<Item = NodeId>) -> Vec<u32> {
        self.nodes.dense_indices_u32(ids)
    }
}

impl Index<NodeId> for NodeManager {
    type Output = Node;

    fn index(&self, index: NodeId) -> &Self::Output {
        &self.nodes[index]
    }
}

impl IndexMut<NodeId> for NodeManager {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        &mut self.nodes[index]
    }
}

pub struct Node {
    local_transform: Transform,
    global_transform: Mat4,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    pub(super) mesh: Option<MeshId>,
}

pub type NodeId = Id<Node>;

#[derive(Debug, Default)]
#[non_exhaustive]
pub struct NodeDescriptor {
    pub local_transform: Transform,
    pub children: Vec<NodeDescriptor>,
    pub mesh: Option<MeshId>,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum NodeCreationError {
    #[error("invalid parent node {0:?}")]
    InvalidParent(NodeId),
    #[error("invalid mesh {0:?}")]
    InvalidMesh(MeshId),
}

#[derive(Debug, Clone, Copy)]
pub enum Transform {
    Matrix(Mat4),
    TRS {
        translation: Vec3,
        rotation: Quat,
        scale: Vec3,
    },
}

impl Default for Transform {
    fn default() -> Self {
        Transform::Matrix(Mat4::IDENTITY)
    }
}
