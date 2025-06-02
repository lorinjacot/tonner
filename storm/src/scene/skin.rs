use std::iter::{once, repeat};

use bytemuck::{Pod, Zeroable, bytes_of, cast_slice};
use glam::Mat4;

use crate::{DenseEntry, Id};

use super::{Node, Scene};

pub struct Skin {
    id: Id<Self>,
    joints: Vec<Joint>,
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
        self.scene.skins.insert(Skin { id, joints })
    }
}

struct Joint {
    node: Id<Node>,
    inverse_bind_matrix: Mat4,
}

impl Scene {
    pub fn joint_matrices(&self, skin: Id<Skin>) -> impl Iterator<Item = Mat4> {
        self.skins[skin].joints.iter().map(
            |Joint {
                 node,
                 inverse_bind_matrix,
             }| {
                let node = &self.nodes[*node];
                node.world_matrix() * *inverse_bind_matrix
            },
        )
    }

    pub(super) fn update_skins_buffer(&mut self) {
        let joint_matrices = Vec::from_iter(
            once(Mat4::IDENTITY).chain(
                self.skins
                    .iter()
                    .flat_map(|skin| self.joint_matrices(skin.id())),
            ),
        );
        let header = SkinStorageHeader {
            joint_count: joint_matrices.len() as u32,
            _pad: [0; 3],
        };

        let header_size = size_of::<SkinStorageHeader>();
        let size = (header_size + joint_matrices.len() * size_of::<Mat4>()) as u64;

        let header = bytes_of(&header);
        let joint_matrices = cast_slice(&joint_matrices);

        match &self.skins_buffer {
            Some(buffer) if buffer.size() >= size => {
                self.queue.write_buffer(buffer, 0, header);
                self.queue
                    .write_buffer(buffer, header_size as u64, joint_matrices);
            }
            _ => {
                self.render_bind_group = None;

                let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Skins buffer"),
                    size,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: true,
                });
                {
                    let mut view = buffer.slice(..).get_mapped_range_mut();
                    view[..header_size].copy_from_slice(header);
                    view[header_size..].copy_from_slice(joint_matrices);
                }
                buffer.unmap();
                self.skins_buffer = Some(buffer);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct SkinStorageHeader {
    joint_count: u32,
    _pad: [u32; 3],
}
