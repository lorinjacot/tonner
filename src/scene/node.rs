use std::{
    collections::HashMap,
    ops::{Index, IndexMut},
};

use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

use crate::storage::{Id, Storage};

use super::mesh::MeshId;

pub struct NodeManager {
    nodes: Storage<Node>,
    local_transform_buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl NodeManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let local_transform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
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
            local_transform_buffer,
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
        nodes: impl IntoIterator<Item = NodeBuilder>,
        device: &wgpu::Device,
    ) -> Result<(Vec<NodeId>, HashMap<MeshId, Vec<NodeId>>), ()> {
        let mut mesh_nodes_mapping = HashMap::new();
        let nodes_id = self.create_recursive(nodes, &mut mesh_nodes_mapping)?;

        self.create_buffer(device);

        Ok((nodes_id, mesh_nodes_mapping))
    }

    fn create_recursive(
        &mut self,
        nodes: impl IntoIterator<Item = NodeBuilder>,
        mesh_nodes_mapping: &mut HashMap<MeshId, Vec<NodeId>>,
    ) -> Result<Vec<NodeId>, ()> {
        let nodes = nodes.into_iter();
        let mut nodes_id = Vec::with_capacity(nodes.size_hint().0);
        for node in nodes {
            let local_matrix = match node.local_transform {
                Transform::Matrix(matrix) => matrix,
                Transform::TRS {
                    translation,
                    rotation,
                    scale,
                } => Mat4::from_scale_rotation_translation(scale, rotation, translation),
            };

            let global_transform = match node.parent {
                Some(parent_id) => {
                    self.nodes.get(parent_id).ok_or(())?.global_transform * local_matrix
                }
                None => local_matrix,
            };

            let node_id = self.nodes.add(Node {
                local_transform: node.local_transform,
                global_transform,
                parent: node.parent,
                children: Vec::new(),
                mesh: None,
            });
            nodes_id.push(node_id);

            if let Some(mesh) = node.mesh {
                mesh_nodes_mapping.entry(mesh).or_default().push(node_id);
            }

            let children: Vec<_> = node
                .children
                .into_iter()
                .map(|node| node.set_parent(node_id))
                .collect();
            let children = self.create_recursive(children, mesh_nodes_mapping)?;
            self.nodes[node_id].children = children;
        }

        Ok(nodes_id)
    }

    fn create_buffer(&mut self, device: &wgpu::Device) {
        let global_transforms = self
            .nodes
            .values()
            .map(|node| node.global_transform)
            .collect::<Vec<_>>();
        let contents = bytemuck::cast_slice(&global_transforms);
        self.local_transform_buffer =
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
                resource: self.local_transform_buffer.as_entire_binding(),
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

pub struct NodeBuilder {
    local_transform: Transform,
    parent: Option<NodeId>,
    children: Vec<NodeBuilder>,
    mesh: Option<MeshId>,
}

impl NodeBuilder {
    pub fn new() -> Self {
        Self {
            local_transform: Transform::Matrix(Mat4::IDENTITY),
            parent: None,
            children: Vec::new(),
            mesh: None,
        }
    }

    pub fn set_transform(mut self, transform: Transform) -> Self {
        self.local_transform = transform;
        self
    }

    pub fn set_parent(mut self, parent_id: NodeId) -> Self {
        self.parent = Some(parent_id);
        self
    }

    pub fn set_children(mut self, children: Vec<NodeBuilder>) -> Self {
        self.children = children;
        self
    }

    pub fn set_mesh(mut self, mesh: Option<MeshId>) -> Self {
        self.mesh = mesh;
        self
    }
}

#[derive(Clone, Copy)]
pub enum Transform {
    Matrix(Mat4),
    TRS {
        translation: Vec3,
        rotation: Quat,
        scale: Vec3,
    },
}
