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

    /// Build the node and add it to the scene.
    pub fn build(self, scene: &mut Scene) -> Result<NodeId, NodeBuilderError> {
        let id = NodeId(Uuid::new_v4());
        let local_matrix = Mat4::from_translation(self.translation.unwrap_or(Vec3::ZERO));
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

    /// Modifies the scale part of the local transform. The local transform is expected
    /// to be a 3D affine transformation matrix otherwise the resulting transform will be invalid.
    pub(super) fn set_local_scale(&mut self, node: NodeId, scale: Vec3) -> Result<(), ()> {
        let node = self.nodes.get_mut(&node).ok_or(())?;
        let (_, rotation, translation) = node.local_matrix.to_scale_rotation_translation();
        node.local_matrix = Mat4::from_scale_rotation_translation(scale, rotation, translation);
        Ok(())
    }

    /// Modifies the rotation part of the local transform. The local transform is expected
    /// to be a 3D affine transformation matrix otherwise the resulting transform will be invalid.
    pub(super) fn set_local_rotation(&mut self, node: NodeId, rotation: Quat) -> Result<(), ()> {
        let node = self.nodes.get_mut(&node).ok_or(())?;
        let (scale, _, translation) = node.local_matrix.to_scale_rotation_translation();
        node.local_matrix = Mat4::from_scale_rotation_translation(scale, rotation, translation);
        Ok(())
    }

    /// Modifies the translation part of the local transform. The local transform is expected
    /// to be a 3D affine transformation matrix otherwise the resulting transform will be invalid.
    pub(super) fn set_local_translation(
        &mut self,
        node: NodeId,
        translation: Vec3,
    ) -> Result<(), ()> {
        let node = self.nodes.get_mut(&node).ok_or(())?;
        let (scale, rotation, _) = node.local_matrix.to_scale_rotation_translation();
        node.local_matrix = Mat4::from_scale_rotation_translation(scale, rotation, translation);
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
                usage: wgpu::BufferUsages::STORAGE,
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
