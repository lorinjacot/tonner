use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    hash::Hash,
};

use bytemuck::cast_slice;
use glam::Mat4;
use thiserror::Error;
use uuid::{NonNilUuid, Uuid};

use crate::{
    Context,
    scene_graph::{NodeId, SceneGraph},
};

/// A unique id for a skin. A skin can only have one id.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkinId(NonNilUuid);

impl Display for SkinId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SkinId({})", self.0)
    }
}

/// A skin is used for *vertex skinning*. Vertex skinning allows a geometry (vertices)
/// of a mesh to be deformed based on the pose of a skeleton. This is essential in order
/// to give animated geometry, for example of virtual characters, a realistic appearance.
#[derive(Debug)]
pub struct Skin {
    id: SkinId,
    pub name: String,
    joints: Vec<SkinJoint>,
}

impl Skin {
    /// Unique id of the skin. This will always return the same value.
    pub fn id(&self) -> SkinId {
        self.id
    }

    /// All joints making up the skin. See [SkinJoint] for more informations.
    pub fn joints(&self) -> &[SkinJoint] {
        &self.joints
    }
}

impl PartialEq for Skin {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Skin {}

impl Hash for Skin {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

/// Skin joints are defining the "bones" of a skin's skeleton.
#[derive(Debug)]
pub struct SkinJoint {
    /// Node defining the position of the bone.
    pub node: NodeId,
    /// Transform the geometry into the space of the joint. This is the inverse of the
    /// global transform of the joint in its initial configuration.
    pub inverse_bind_matrix: Mat4,
}

/// A build for skins.
#[must_use]
#[derive(Default)]
pub struct SkinBuilder {
    name: String,
    joints: Vec<SkinJoint>,
    nodes: Vec<NodeId>,
    inverse_bind_matrices: Vec<Mat4>,
}

impl SkinBuilder {
    /// Gives a name to the skin. Defaults to an empty string.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Add all given `joints` to the skin. If called multiple times, all joints
    /// will be used.
    pub fn joints(mut self, joints: impl IntoIterator<Item = SkinJoint>) -> Self {
        self.joints.extend(joints);
        self
    }

    /// Add new nodes to the skin. If called multiple time,
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

    /// Build the skin and add it to the scene. Be sure to call at least [SkinBuilder::joints()]
    /// or [SkinBuilder::nodes()] once.
    pub fn build(mut self) -> Result<Skin, SkinBuilderError> {
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

        self.joints
            .extend(self.nodes.into_iter().zip(inverse_bind_matrices).map(
                |(node, inverse_bind_matrix)| SkinJoint {
                    node,
                    inverse_bind_matrix,
                },
            ));

        let id = SkinId(NonNilUuid::new(Uuid::new_v4()).unwrap());

        Ok(Skin {
            id,
            name: self.name,
            joints: self.joints,
        })
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

/// Manages multiple skins. See [Skin] for more informations.
#[derive(Debug)]
pub struct SkinManager {
    pub skins: HashSet<Skin>,
    joint_matrices: Vec<Mat4>,
    offsets: HashMap<SkinId, u32>,
    buffer: wgpu::Buffer,
}

impl SkinManager {
    /// Create a new empty skin manager.
    pub fn new(ctx: &Context) -> Self {
        let buffer = Self::create_buffer(
            wgpu::util::align_to(
                size_of::<Mat4>() as wgpu::BufferAddress,
                wgpu::COPY_BUFFER_ALIGNMENT,
            ),
            false,
            ctx.device(),
        );
        Self {
            skins: HashSet::new(),
            joint_matrices: Vec::new(),
            offsets: HashMap::new(),
            buffer,
        }
    }

    // pub fn remove(&mut self, skin: SkinId) {
    //     self.skins.remove(&skin.0)
    // }

    fn create_buffer(size: u64, mapped_at_creation: bool, device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Joint matrices buffer"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation,
        })
    }

    pub(crate) fn prepare<'a>(
        &'a mut self,
        scene_graph: &SceneGraph,
        ctx: &Context,
    ) -> Result<PreparedSkins<'a>, SkinError> {
        self.joint_matrices.clear();

        let mut offset = 1;
        for skin in self.skins.iter() {
            self.offsets.insert(skin.id, offset);
            offset += skin.joints.len() as u32;
            for &SkinJoint {
                node,
                inverse_bind_matrix,
            } in &skin.joints
            {
                self.joint_matrices.push(
                    scene_graph
                        .get(node)
                        .ok_or(SkinError::InvalidNode(skin.id, node))?
                        .global_transformation()
                        * inverse_bind_matrix,
                );
            }
        }

        let offset = size_of::<Mat4>();
        let size = (1 + self.joint_matrices.len()) * size_of::<Mat4>();
        let wgpu_size = size as wgpu::BufferAddress;
        let joint_matrices = cast_slice(&self.joint_matrices);

        if self.buffer.size() >= wgpu_size {
            ctx.queue()
                .write_buffer(&self.buffer, offset as wgpu::BufferAddress, joint_matrices);
        } else {
            self.buffer = Self::create_buffer(
                wgpu::util::align_to(wgpu_size, wgpu::COPY_BUFFER_ALIGNMENT),
                true,
                ctx.device(),
            );
            let mut buffer_view = self.buffer.slice(..).get_mapped_range_mut();
            buffer_view[offset..size].copy_from_slice(joint_matrices);
            drop(buffer_view);
            self.buffer.unmap();
        }

        Ok(PreparedSkins {
            buffer: &self.buffer,
            offsets: &self.offsets,
        })
    }
}

#[derive(Debug, Error)]
pub enum SkinError {
    #[error("invalid node: {0}")]
    InvalidNode(SkinId, NodeId),
}

pub(crate) struct PreparedSkins<'a> {
    buffer: &'a wgpu::Buffer,
    offsets: &'a HashMap<SkinId, u32>,
}

impl<'a> PreparedSkins<'a> {
    pub(crate) fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    pub(crate) fn offset(&self, skin: SkinId) -> Option<u32> {
        self.offsets.get(&skin).cloned()
    }
}
