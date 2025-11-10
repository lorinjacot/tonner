use std::{collections::HashMap, fmt::Display, iter::once};

use bytemuck::{Pod, Zeroable, bytes_of, cast_slice};
use glam::Mat4;
use thiserror::Error;
use uuid::Uuid;
use wgpu::util::DeviceExt;

use crate::{
    scene::{NodeManager, node::NodeId},
    storage::{DenseEntry, Id, SparseSet},
};

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
        let data = SkinData { id, joints };

        scene.skin_manager.skins.insert(id, data);

        Ok(id)
    }
}

/// Error when [`SkinBuilder.build`] fails.
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
        let buffer = Self::create_buffer(&[], device);
        Self {
            skins: HashMap::new(),
            buffer,
        }
    }

    fn create_buffer(contents: &[u8], device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skin manager buffer"),
            contents,
            usage: wgpu::BufferUsages::STORAGE,
        })
    }

    /// Buffer inddx of the first joint matrix part of the skin,
    /// or `None` if the skin is not in the buffer yet.
    pub(super) fn index(&self, skin: SkinId) -> Option<u32> {
        self.skins.get(&skin).and_then(|data| data.index)
    }

    pub(super) fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    pub(super) fn update_skin_buffer(
        &mut self,
        node_manager: &NodeManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        todo!()
    }
}

pub struct SkinData {
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

impl Scene {
    pub(super) fn update_skins_buffer(&mut self) {
        let (header, joint_matrices) = skins_buffer_data(&mut self.skins, &self.nodes);
        let (header, header_size, joint_matrices, size) =
            skins_buffer_bytes(&header, &joint_matrices);

        if self.skins_buffer.size() >= size {
            self.queue.write_buffer(&self.skins_buffer, 0, header);
            self.queue
                .write_buffer(&self.skins_buffer, header_size as u64, joint_matrices);
        } else {
            self.render_bind_group = None;

            self.skins_buffer =
                create_skins_buffer(header, header_size, joint_matrices, size, &self.device);
        }
    }
}

fn skins_buffer_data(
    skins: &mut SparseSet<Skin>,
    nodes: &SparseSet<Node>,
) -> (SkinStorageHeader, Vec<Mat4>) {
    let joint_matrices =
        Vec::from_iter(once(Mat4::IDENTITY).chain(skins.iter().flat_map(|skin| {
            skin.joints.iter().map(
                |Joint {
                     node,
                     inverse_bind_matrix,
                 }| {
                    let node = &nodes[*node];
                    node.world_matrix() * *inverse_bind_matrix
                },
            )
        })));

    let mut offset = 1;
    skins.iter_mut().for_each(|skin| {
        skin.joint_offset = offset;
        offset += skin.joints.len() as u32;
    });
    let header = SkinStorageHeader {
        joint_count: joint_matrices.len() as u32,
        _pad: [0; 3],
    };

    (header, joint_matrices)
}

fn skins_buffer_bytes<'a>(
    header: &'a SkinStorageHeader,
    joint_matrices: &'a [Mat4],
) -> (&'a [u8], usize, &'a [u8], u64) {
    let header_size = size_of::<SkinStorageHeader>();
    let size = header_size + joint_matrices.len() * size_of::<Mat4>();

    let header = bytes_of(header);
    let joint_matrices = cast_slice(&joint_matrices);

    (header, header_size, joint_matrices, size as u64)
}

fn create_skins_buffer(
    header: &[u8],
    header_size: usize,
    joint_matrices: &[u8],
    size: u64,
    device: &wgpu::Device,
) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Skins buffer"),
        size: size as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: true,
    });
    {
        let mut view = buffer.slice(..).get_mapped_range_mut();
        view[..header_size].copy_from_slice(header);
        view[header_size..].copy_from_slice(joint_matrices);
    }
    buffer.unmap();
    buffer
}

pub(super) fn init_skins_buffer(
    skins: &mut SparseSet<Skin>,
    nodes: &SparseSet<Node>,
    device: &wgpu::Device,
) -> wgpu::Buffer {
    let (header, joint_matrices) = skins_buffer_data(skins, nodes);
    let (header, header_size, joint_matrices, size) = skins_buffer_bytes(&header, &joint_matrices);
    create_skins_buffer(header, header_size, joint_matrices, size, device)
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct SkinStorageHeader {
    joint_count: u32,
    _pad: [u32; 3],
}
