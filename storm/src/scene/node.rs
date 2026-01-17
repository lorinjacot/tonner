use std::{collections::HashMap, fmt::Display};

use bytemuck::{Pod, Zeroable, bytes_of, cast_slice};
use glam::{Mat4, Quat, Vec3};
use thiserror::Error;
use uuid::Uuid;
use wgpu::util::DeviceExt;

use crate::scene::Scene;

/// A unique id for a node. A node can only have one id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(Uuid);

impl Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeId({})", self.0)
    }
}

/// A builder for scene graph nodes.
#[derive(Default)]
pub struct NodeBuilder {
    name: Option<String>,
    parent: Option<NodeId>,
    translation: Option<Vec3>,
    rotation: Option<Quat>,
    scale: Option<Vec3>,
}

impl NodeBuilder {
    /// Set the node name.
    pub fn name(self, name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..self
        }
    }

    /// Set the node parent. A node without any parent will be added as a root node.
    pub fn parent(self, parent: impl Into<NodeId>) -> Self {
        Self {
            parent: Some(parent.into()),
            ..self
        }
    }

    /// Set the translation component of the node local transform. Defaults to [`Vec3::ZERO`].
    pub fn translation(self, translation: impl Into<Vec3>) -> Self {
        Self {
            translation: Some(translation.into()),
            ..self
        }
    }

    /// Set the rotation component of the node local transform. Defaults to [`Quat::IDENTITY`].
    pub fn rotation(self, rotation: impl Into<Quat>) -> Self {
        Self {
            rotation: Some(rotation.into()),
            ..self
        }
    }

    /// Set the scale component of the node local transform. Defaults to [`Vec3::ONE`].
    pub fn scale(self, scale: impl Into<Vec3>) -> Self {
        Self {
            scale: Some(scale.into()),
            ..self
        }
    }

    /// Build the node and add it to the scene.
    pub fn build(self, scene: &mut Scene) -> Result<NodeId, NodeBuilderError> {
        let id = NodeId(Uuid::new_v4());
        let local_matrix = Mat4::from_scale_rotation_translation(
            self.scale.unwrap_or(Vec3::ONE),
            self.rotation.unwrap_or(Quat::IDENTITY),
            self.translation.unwrap_or(Vec3::ZERO),
        );
        let global_matrix = match self.parent {
            Some(parent) => {
                let parent_data = scene
                    .node_manager
                    .nodes
                    .get_mut(&parent)
                    .ok_or(NodeBuilderError::InvalidParentNode(parent))?;
                parent_data.children.push(id);
                parent_data.global_matrix * local_matrix
            }
            None => {
                scene.node_manager.root_nodes.push(id);
                local_matrix
            }
        };
        scene.node_manager.nodes.insert(
            id,
            NodeData {
                id,
                index: None,
                name: self.name,
                parent: self.parent,
                children: Vec::new(),
                local_matrix,
                global_matrix,
            },
        );

        Ok(id)
    }
}

#[derive(Debug, Error)]
pub enum NodeBuilderError {
    #[error("invalid parent node: {0}")]
    InvalidParentNode(NodeId),
}

pub(super) struct NodeManager {
    nodes: HashMap<NodeId, NodeData>,
    root_nodes: Vec<NodeId>,
    buffer: wgpu::Buffer,
}

impl NodeManager {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Node manager buffer"),
            contents: bytes_of(&NodeStorageHeader {
                count: 0,
                _pad: [0; 3],
            }),
            usage: wgpu::BufferUsages::STORAGE,
        });

        Self {
            nodes: HashMap::new(),
            root_nodes: Vec::new(),
            buffer,
        }
    }

    /// Index of the node in [`NodeManager::buffer`]. [`None`] if invalid id or if the node
    /// is not in the buffer yet.
    pub(super) fn index(&self, node: NodeId) -> Option<u32> {
        self.nodes.get(&node).and_then(|data| data.index)
    }

    /// `true` if `node` is a valid id. `false` otherwise.
    pub(super) fn contains(&self, node: NodeId) -> bool {
        self.nodes.contains_key(&node)
    }

    /// Returns the node local matrix, or `None` if the node does not exists.
    ///
    /// The local matrix describe the transform between the node local coordinate system
    /// to the parent one.
    /// If the node has no parent, the local transform is equal to the global transform.
    pub(super) fn local_matrix(&self, node: NodeId) -> Option<Mat4> {
        self.nodes.get(&node).map(|data| data.local_matrix)
    }

    /// Returns the node local position, or `None` if the node does not exists.
    ///
    /// The local position describe the translation between the node coordinate system origin
    /// and the parent one.
    /// If the node has no parent, the local position is equal to the global position.
    pub(super) fn local_position(&self, node: NodeId) -> Option<Vec3> {
        self.local_matrix(node)
            .map(|m| m.transform_point3(Vec3::ZERO))
    }

    /// Modifies the scale part of the local transform. The local transform is expected
    /// to be a 3D affine transformation matrix otherwise the resulting transform will be invalid.
    pub(super) fn set_local_scale(&mut self, node: NodeId, scale: Vec3) -> Result<(), ()> {
        let node_data = self.nodes.get_mut(&node).ok_or(())?;
        let (_, rotation, translation) = node_data.local_matrix.to_scale_rotation_translation();
        node_data.local_matrix =
            Mat4::from_scale_rotation_translation(scale, rotation, translation);
        self.update_global_matrix(node)
    }

    /// Modifies the rotation part of the local transform. The local transform is expected
    /// to be a 3D affine transformation matrix otherwise the resulting transform will be invalid.
    pub(super) fn set_local_rotation(&mut self, node: NodeId, rotation: Quat) -> Result<(), ()> {
        let node_data = self.nodes.get_mut(&node).ok_or(())?;
        let (scale, _, translation) = node_data.local_matrix.to_scale_rotation_translation();
        node_data.local_matrix =
            Mat4::from_scale_rotation_translation(scale, rotation, translation);
        self.update_global_matrix(node)
    }

    /// Modifies the translation part of the local transform. The local transform is expected
    /// to be a 3D affine transformation matrix otherwise the resulting transform will be invalid.
    pub(super) fn set_local_translation(
        &mut self,
        node: NodeId,
        translation: Vec3,
    ) -> Result<(), ()> {
        let node_data = self.nodes.get_mut(&node).ok_or(())?;
        let (scale, rotation, _) = node_data.local_matrix.to_scale_rotation_translation();
        node_data.local_matrix =
            Mat4::from_scale_rotation_translation(scale, rotation, translation);
        self.update_global_matrix(node)
    }

    fn update_global_matrix(&mut self, node: NodeId) -> Result<(), ()> {
        let parent = self.nodes.get(&node).ok_or(())?.parent;
        let parent_global_matrix = match parent {
            Some(parent) => self.nodes.get(&parent).ok_or(())?.global_matrix,
            None => Mat4::IDENTITY,
        };
        self.update_global_matrix_from_parent_matrix(node, parent_global_matrix)
    }

    fn update_global_matrix_from_parent_matrix(
        &mut self,
        node: NodeId,
        parent_global_matrix: Mat4,
    ) -> Result<(), ()> {
        let node = self.nodes.get_mut(&node).ok_or(())?;
        let global_matrix = parent_global_matrix * node.local_matrix;
        node.global_matrix = global_matrix;
        for node in node.children.clone() {
            self.update_global_matrix_from_parent_matrix(node, global_matrix)?;
        }
        Ok(())
    }

    /// Returns the node global matrix, or `None` if the node does not exists.
    ///
    /// The global matrix describe the transform between the node local coordinate system
    /// to the world coordinate system.
    /// If the node has no parent, the global transform is equal to the local transform.
    pub(super) fn global_matrix(&self, node: NodeId) -> Option<Mat4> {
        self.nodes.get(&node).map(|data| data.global_matrix)
    }

    /// Buffer containing the node data. This is used when a gpu shader need node access. The return
    /// buffer should not be keep as this method could return another buffer on another call.
    pub(super) fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Update the node buffer with the current state of the nodes.
    pub(super) fn update_buffer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let data: Vec<_> = self
            .nodes
            .values_mut()
            .enumerate()
            .map(|(i, data)| {
                data.index = Some(i as u32);
                NodeUniform::from(&*data)
            })
            .collect();

        let header = NodeStorageHeader {
            count: self.nodes.len() as u32,
            _pad: [0; 3],
        };

        let header_size = size_of::<NodeStorageHeader>();
        let size = header_size + self.nodes.len() * size_of::<NodeUniform>();
        let header = bytes_of(&header);
        let data = cast_slice(&data);

        if self.buffer.size() >= size as u64 {
            queue.write_buffer(&self.buffer, 0, header);
            queue.write_buffer(&self.buffer, header_size as u64, data);
        } else {
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Node storage buffer"),
                size: wgpu::util::align_to(size as u64, wgpu::COPY_BUFFER_ALIGNMENT),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: true,
            });
            let mut buffer_view = self.buffer.slice(..).get_mapped_range_mut();
            buffer_view[..header_size].copy_from_slice(header);
            buffer_view[header_size..size].copy_from_slice(data);
            drop(buffer_view);
            self.buffer.unmap();
        }
    }
}

/// Nodes operations
impl Scene {
    /// Returns the node local position, or `None` if the node does not exists.
    ///
    /// The local position describe the translation between the node coordinate system origin
    /// and the parent one.
    /// If the node has no parent, the local position is equal to the global position.
    pub fn local_position(&self, node: NodeId) -> Option<Vec3> {
        self.node_manager.local_position(node)
    }

    /// Returns the node local rotation, or `None` if the node does not exists.
    pub fn local_rotation(&self, node: NodeId) -> Option<Quat> {
        self.node_manager
            .nodes
            .get(&node)
            .map(|node| node.local_matrix.to_scale_rotation_translation().1)
    }

    /// Returns the node local matrix, or `None` if the node does not exists.
    ///
    /// The local matrix describe the transform between the node local coordinate system
    /// to the parent one.
    /// If the node has no parent, the local transform is equal to the global transform.
    pub fn local_matrix(&self, node: NodeId) -> Option<Mat4> {
        self.node_manager.local_matrix(node)
    }

    /// Modifies the translation part of the local transform. The local transform is expected
    /// to be a 3D affine transformation matrix otherwise the resulting transform will be invalid.
    pub fn set_local_position(&mut self, node: NodeId, position: Vec3) -> Result<(), ()> {
        self.node_manager.set_local_translation(node, position)
    }

    /// Rotate `node` such that it is facing `target`. Returns `Err(())` if the node does not
    /// exist.
    pub fn look_at(&mut self, node: NodeId, target: Vec3) -> Result<(), ()> {
        let node_data = self.node_manager.nodes.get(&node).ok_or(())?;
        let eye = node_data.global_matrix.transform_point3(Vec3::ZERO);
        let matrix = Mat4::look_at_rh(eye, target, Vec3::Y);
        let mut rotation = Quat::from_mat4(&matrix.inverse());
        if let Some(parent) = node_data.parent {
            let parent_rotation = self
                .node_manager
                .global_matrix(parent)
                .unwrap()
                .to_scale_rotation_translation()
                .1;
            rotation = parent_rotation.inverse() * rotation;
        }
        self.node_manager
            .set_local_rotation(node, rotation)
            .unwrap();
        Ok(())
    }
}

struct NodeData {
    id: NodeId,
    /// Index of the node in [`NodeManager::buffer`] or [`None`] if not in there.
    index: Option<u32>,
    name: Option<String>,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    local_matrix: Mat4,
    global_matrix: Mat4,
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct NodeStorageHeader {
    count: u32,
    _pad: [u32; 3],
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct NodeUniform {
    matrix: Mat4,
}

impl From<&NodeData> for NodeUniform {
    fn from(value: &NodeData) -> Self {
        Self {
            matrix: value.global_matrix,
        }
    }
}
