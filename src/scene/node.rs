use std::ops::{Index, IndexMut};

use glam::{Mat3, Mat4, Quat, Vec3, Vec4};
use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::storage::{Id, Storage};

use super::mesh::{MeshId, MeshManager};

pub struct NodeManager {
    nodes: Storage<Node>,
    transform_buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl NodeManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let transform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
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
            transform_buffer,
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
        node: &NodeDescriptor,
        meshes: &mut MeshManager,
        device: &wgpu::Device,
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
            parent: node.parent,
            children: Vec::new(),
            mesh: node.mesh,
        });

        if let Some(parent_id) = node.parent {
            let parent = self
                .nodes
                .get_mut(parent_id)
                .ok_or(NodeCreationError::InvalidParent(parent_id))?;
            parent.children.push(node_id);
            self.nodes[node_id].global_transform = parent.global_transform * local_matrix;
        }

        if let Some(mesh) = node.mesh {
            let mesh = meshes
                .get_mut(mesh)
                .ok_or(NodeCreationError::InvalidMesh(mesh))?;
            mesh.nodes.insert(node_id);
            mesh.update_nodes_buffer(
                self.nodes.dense_indices_u32(mesh.nodes.iter().copied()),
                device,
            );
        }

        self.create_buffer(device);

        Ok(node_id)
    }

    fn create_buffer(&mut self, device: &wgpu::Device) {
        let transforms: Vec<_> = self
            .nodes
            .values()
            .map(|node| {
                let normal = Mat3::from_mat4(node.global_transform).inverse().transpose();
                TransformStorage {
                    model: node.global_transform,
                    normal: [
                        normal.x_axis.extend(0.0),
                        normal.y_axis.extend(0.0),
                        normal.z_axis.extend(0.0),
                    ],
                }
            })
            .collect();
        // let global_transform: Vec<_> = self
        //     .nodes
        //     .values()
        //     .map(|node| node.global_transform.to_cols_array())
        //     .collect();
        // dbg!(&transforms, &global_transform);

        let contents = bytemuck::cast_slice(&transforms);
        self.transform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Nodes local transform buffer"),
            contents,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
        });

        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Nodes bind group"),
            layout: &self.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.transform_buffer.as_entire_binding(),
            }],
        }));
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
    #[allow(dead_code)]
    local_transform: Transform,
    global_transform: Mat4,
    #[allow(dead_code)]
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    #[allow(dead_code)]
    mesh: Option<MeshId>,
}

pub type NodeId = Id<Node>;

#[derive(Debug, Default)]
#[non_exhaustive]
pub struct NodeDescriptor {
    pub local_transform: Transform,
    pub parent: Option<NodeId>,
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

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TransformStorage {
    model: Mat4,
    normal: [Vec4; 3],
}
