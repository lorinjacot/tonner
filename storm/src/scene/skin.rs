use std::{collections::HashMap, fmt::Display};

use bytemuck::cast_slice;
use glam::Mat4;
use thiserror::Error;
use uuid::Uuid;

use crate::scene::scene_graph::{NodeId, SceneGraph};

use super::Scene;

/// A unique id for a skin. A skin can only have one id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkinId(Uuid);

impl Display for SkinId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SkinId({})", self.0)
    }
}

/// A build for skins.
#[must_use]
#[derive(Default)]
pub struct SkinBuilder {
    nodes: Vec<NodeId>,
    inverse_bind_matrices: Vec<Mat4>,
}

impl SkinBuilder {
    /// Add new nodes to the skin. This function should be called at least once. If called multiple time,
    /// nodes from all calls will be used.
    pub fn nodes(mut self, nodes: impl IntoIterator<Item = NodeId>) -> Self {
        self.nodes.extend(nodes);
        self
    }

    /// Add inverse bind matrices to the skin. If never called, identity matrices will be used. If called one
    /// or more times, the order and number of provided matrices must match the number of nodes.
    pub fn inverse_bind_matrices(
        mut self,
        inverse_bind_matrices: impl IntoIterator<Item = Mat4>,
    ) -> Self {
        self.inverse_bind_matrices.extend(inverse_bind_matrices);
        self
    }

    /// Build the skin and add it to the scene.
    pub fn build(self, scene: &mut Scene) -> Result<SkinId, SkinBuilderError> {
        let inverse_bind_matrices = if self.inverse_bind_matrices.is_empty() {
            vec![Mat4::IDENTITY; self.nodes.len()]
        } else {
            if self.inverse_bind_matrices.len() == self.nodes.len() {
                self.inverse_bind_matrices
            } else {
                return Err(SkinBuilderError::InvalidInverseBindMatrixCount {
                    expected: self.nodes.len(),
                    actual: self.inverse_bind_matrices.len(),
                });
            }
        };

        let joints = self
            .nodes
            .into_iter()
            .zip(inverse_bind_matrices)
            .map(|(node, inverse_bind_matrix)| Joint {
                node,
                inverse_bind_matrix,
            })
            .collect();

        let id = SkinId(Uuid::new_v4());
        let data = SkinData {
            id,
            index: None,
            joints,
        };

        scene.skin_manager.skins.insert(id, data);

        Ok(id)
    }
}

/// Error when [`SkinBuilder::build`] fails.
#[derive(Debug, Error)]
pub enum SkinBuilderError {
    #[error("invalid node: {0}")]
    InvalidNode(NodeId),
    #[error("invalid inverse bind matrix count: expected {expected}, got {actual}")]
    InvalidInverseBindMatrixCount { expected: usize, actual: usize },
}

/// Manages all skins in a scene.
pub(super) struct SkinManager {
    skins: HashMap<SkinId, SkinData>,
    buffer: wgpu::Buffer,
}

impl SkinManager {
    /// Create a new empty skin manager.
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let buffer = Self::create_buffer(
            wgpu::util::align_to(size_of::<Mat4>() as u64, wgpu::COPY_BUFFER_ALIGNMENT),
            false,
            device,
        );
        Self {
            skins: HashMap::new(),
            buffer,
        }
    }

    fn create_buffer(size: u64, mapped_at_creation: bool, device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Joint matrices buffer"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation,
        })
    }

    /// Buffer index of the first joint matrix part of the skin,
    /// or `None` if the skin is not in the buffer yet.
    pub(super) fn index(&self, skin: SkinId) -> Option<u32> {
        self.skins.get(&skin).and_then(|data| data.index)
    }

    /// Buffer containing the skin joint. This is used when a gpu shader need the skin joint matrices. The returned
    /// buffer should not be keep as this method could return another buffer on another call.
    pub(super) fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Update the skin buffer with the current state of the skins.
    pub(super) fn update_buffer(
        &mut self,
        scene_graph: &SceneGraph,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), UpdateSkinBufferError> {
        let mut joint_matrices =
            Vec::with_capacity(self.skins.values_mut().map(|data| data.joints.len()).sum());
        for (index, skin) in self.skins.values_mut().enumerate() {
            skin.index = Some(index as u32);
            for &Joint {
                node,
                inverse_bind_matrix,
            } in &skin.joints
            {
                joint_matrices.push(
                    scene_graph
                        .get(node)
                        .ok_or(UpdateSkinBufferError::InvalidNode(node))?
                        .global_transformation()
                        * inverse_bind_matrix,
                );
            }
        }

        let size = joint_matrices.len() * size_of::<Mat4>();
        let joint_matrices = cast_slice(&joint_matrices);

        if self.buffer.size() >= size as u64 {
            queue.write_buffer(&self.buffer, 0, joint_matrices);
        } else {
            self.buffer = Self::create_buffer(
                wgpu::util::align_to(size as u64, wgpu::COPY_BUFFER_ALIGNMENT),
                true,
                device,
            );
            let mut buffer_view = self.buffer.slice(..).get_mapped_range_mut();
            buffer_view[..size].copy_from_slice(joint_matrices);
            drop(buffer_view);
            self.buffer.unmap();
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum UpdateSkinBufferError {
    #[error("invalid node: {0}")]
    InvalidNode(NodeId),
}

struct SkinData {
    id: SkinId,
    /// Buffer inddx of the first joint matrix part of the skin,
    /// or `None` if the skin is not in the buffer yet.
    index: Option<u32>,
    joints: Vec<Joint>,
}

#[derive(Debug)]
struct Joint {
    node: NodeId,
    inverse_bind_matrix: Mat4,
}
