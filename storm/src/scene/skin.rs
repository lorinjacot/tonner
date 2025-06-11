use std::iter::{once, repeat};

use bytemuck::{Pod, Zeroable, bytes_of, cast_slice};
use glam::Mat4;

use crate::{DenseEntry, Id, storage::SparseSet};

use super::{Node, Scene};

pub struct Skin {
    id: Id<Self>,
    joints: Vec<Joint>,
    joint_offset: u32,
}

impl Skin {
    pub(super) fn joint_offset(&self) -> u32 {
        self.joint_offset
    }
}

impl DenseEntry for Skin {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

#[must_use]
pub struct SkinBuilder<'a, 's> {
    scene: &'s mut Scene,
    nodes: Option<Box<dyn Iterator<Item = Id<Node>> + 'a>>,
    inverse_bind_matrices: Option<Box<dyn Iterator<Item = Mat4> + 'a>>,
}

impl<'a, 's> SkinBuilder<'a, 's> {
    pub fn new(scene: &'s mut Scene) -> Self {
        Self {
            scene,
            nodes: None,
            inverse_bind_matrices: None,
        }
    }

    pub fn nodes(mut self, nodes: impl IntoIterator<Item = Id<Node>> + 'a) -> Self {
        self.nodes = Some(Box::new(nodes.into_iter()));
        self
    }

    pub fn inverse_bind_matrices(
        mut self,
        inverse_bind_matrices: impl IntoIterator<Item = Mat4> + 'a,
    ) -> Self {
        self.inverse_bind_matrices = Some(Box::new(inverse_bind_matrices.into_iter()));
        self
    }

    pub fn build(self) -> &'s Skin {
        let nodes = self.nodes.expect("nodes should be set");
        let inverse_bind_matrices = self
            .inverse_bind_matrices
            .unwrap_or_else(|| Box::new(repeat(Mat4::IDENTITY)));
        let joints = nodes
            .zip(inverse_bind_matrices)
            .map(|(node, inverse_bind_matrix)| Joint {
                node,
                inverse_bind_matrix,
            })
            .collect();
        let id = self.scene.skins.next_id();
        self.scene.skins.insert(Skin {
            id,
            joints,
            joint_offset: 0,
        })
    }
}

struct Joint {
    node: Id<Node>,
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
