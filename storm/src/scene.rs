use std::ops::{Index, IndexMut};

use bytemuck::{Pod, Zeroable, cast_slice};
pub use camera::Camera;
use glam::{Mat4, Vec3, usize};
pub use node::{Node, NodeBuilder, NodeHandle};
use wgpu::util::DeviceExt;

use crate::{
    mesh::{Mesh, Primitive},
    storage::{DenseEntry, Id, SetEntry, SparseMap, SparseSet},
};

pub mod camera;
mod node;

pub struct Scene {
    id: Id<Self>,
    pub name: String,
    device: wgpu::Device,
    queue: wgpu::Queue,
    nodes: SparseSet<Node>,
    nodes_buffer: Option<wgpu::Buffer>,
    root_nodes: Vec<Id<Node>>,
    meshes: SparseMap<MeshInstances>,
    cameras: SparseMap<Camera>,
    active_camera: Option<Id<Node>>,
    camera_buffer: wgpu::Buffer,
    render_bind_group_layout: wgpu::BindGroupLayout,
    render_bind_group: Option<wgpu::BindGroup>,
}

impl Scene {
    pub fn node_handle(&mut self, id: Id<Node>) -> NodeHandle {
        NodeHandle { id, scene: self }
    }

    pub fn node_builder(&mut self) -> NodeBuilder {
        NodeBuilder::new(self)
    }

    pub fn root_nodes(&self) -> &[Id<Node>] {
        &self.root_nodes
    }

    pub fn camera(&self, id: Id<Node>) -> Option<&Camera> {
        self.cameras.get(id)
    }

    pub fn camera_mut(&mut self, id: Id<Node>) -> Option<&mut Camera> {
        self.cameras.get_mut(id)
    }

    pub fn active_camera(&self) -> Option<Id<Node>> {
        self.active_camera
    }

    pub fn set_active_camera(&mut self, camera: Option<Id<Node>>) {
        if let Some(camera) = camera {
            assert!(
                self.cameras.contains(camera),
                "no camera associated with node {camera}"
            )
        }
        self.active_camera = camera;
    }

    pub fn cameras(&self) -> std::slice::Iter<'_, Camera> {
        self.cameras.iter()
    }

    pub fn aspect_ratio(&self) -> Option<f32> {
        match self.cameras[self.active_camera?].projection {
            camera::Projection::Perspective { aspect_ratio, .. } => aspect_ratio,
            camera::Projection::Orthographic { x_mag, y_mag, .. } => Some(x_mag / y_mag),
        }
    }

    pub fn update(&mut self, viewport_aspect_ration: f32) {
        let nodes_buffer = {
            let data: Vec<_> = self
                .nodes
                .iter()
                .map(|node| NodeUniform {
                    model: node.world_matrix(),
                })
                .collect();

            match &self.nodes_buffer {
                Some(buffer) => {
                    self.queue.write_buffer(buffer, 0, cast_slice(&data));
                    buffer
                }
                None => {
                    self.render_bind_group = None;
                    let buffer =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some(&format!("{}'s nodes buffer", self.name)),
                                contents: cast_slice(&data),
                                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                            });
                    self.nodes_buffer.insert(buffer)
                }
            }
        };

        if let Some(camera) = self.active_camera {
            let projection = self.cameras[camera]
                .projection
                .matrix(viewport_aspect_ration);
            let camera = &self.nodes[camera];
            let view = Mat4::look_to_rh(
                camera.world_position(),
                camera.world_matrix().transform_vector3(-Vec3::Z),
                camera.world_matrix().transform_vector3(Vec3::Y),
            );
            let camera_uniform = CameraUniform {
                view_projection: projection * view,
            };
            self.queue
                .write_buffer(&self.camera_buffer, 0, cast_slice(&[camera_uniform]));

            for mesh_instances in self.meshes.iter_mut() {
                const INDEX_SIZE: usize = size_of::<u32>();
                let data: Vec<_> = mesh_instances
                    .nodes
                    .iter()
                    .map(|id| self.nodes.dense_index(*id).unwrap() as u32)
                    .collect();
                let buffer = &mut mesh_instances.vertex_buffer;
                if buffer.size() >= (mesh_instances.nodes.len() * INDEX_SIZE) as wgpu::BufferAddress
                {
                    self.queue.write_buffer(&buffer, 0, cast_slice(&data));
                } else {
                    *buffer = self
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Mesh instances vertex buffer"),
                            contents: cast_slice(&data),
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        });
                }
            }

            if self.render_bind_group.is_none() {
                self.render_bind_group =
                    Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some(&format!("{} scene bind group", self.name)),
                        layout: &self.render_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: nodes_buffer.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: self.camera_buffer.as_entire_binding(),
                            },
                        ],
                    }))
            }
        } else {
            self.render_bind_group = None;
        }
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass) {
        if let Some(render_bind_group) = self.render_bind_group.as_ref() {
            render_pass.set_bind_group(0, render_bind_group, &[]);
            for mesh_instances in self.meshes.iter() {
                let instances_count = mesh_instances.nodes.len() as u32;
                render_pass.set_vertex_buffer(0, mesh_instances.vertex_buffer.slice(..));
                for primitive in mesh_instances.primitives.iter() {
                    render_pass.set_pipeline(&primitive.pipeline);
                    for (slot, vertex_buffer) in primitive.vertex_buffers.iter().enumerate() {
                        render_pass.set_vertex_buffer(slot as u32 + 1, vertex_buffer.slice(..));
                    }
                    match &primitive.index_buffer {
                        Some(index_buffer) => {
                            render_pass.set_index_buffer(
                                index_buffer.buffer.slice(index_buffer.offset..),
                                index_buffer.format,
                            );
                            render_pass.draw_indexed(
                                0..primitive.vertex_count,
                                0,
                                0..instances_count,
                            );
                        }
                        None => render_pass.draw(0..primitive.vertex_count, 0..instances_count),
                    }
                }
            }
        }
    }
}

impl Index<Id<Node>> for Scene {
    type Output = Node;

    fn index(&self, index: Id<Node>) -> &Self::Output {
        &self.nodes[index]
    }
}

impl IndexMut<Id<Node>> for Scene {
    fn index_mut(&mut self, index: Id<Node>) -> &mut Self::Output {
        &mut self.nodes[index]
    }
}

pub struct SceneDescriptor {
    pub(super) name: Option<String>,
    pub(super) render_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
}

impl DenseEntry for Scene {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

impl SetEntry for Scene {
    type Descriptor = SceneDescriptor;

    fn new(id: Id<Self::Key>, desc: Self::Descriptor) -> Self {
        let name = desc.name.unwrap_or_else(|| id.to_string());
        let nodes = SparseSet::new();
        let root_nodes = Vec::new();
        let meshes = SparseMap::new();
        let cameras = SparseMap::new();

        let camera_buffer = desc.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera buffer"),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Scene {
            id,
            name,
            device: desc.device,
            queue: desc.queue,
            nodes,
            nodes_buffer: None,
            root_nodes,
            meshes,
            cameras,
            active_camera: None,
            camera_buffer,
            render_bind_group_layout: desc.render_bind_group_layout,
            render_bind_group: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct NodeUniform {
    model: Mat4,
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct CameraUniform {
    view_projection: Mat4,
}

struct MeshInstances {
    mesh: Id<Mesh>,
    primitives: Vec<Primitive>,
    nodes: Vec<Id<Node>>,
    vertex_buffer: wgpu::Buffer,
}

impl DenseEntry for MeshInstances {
    type Key = Mesh;

    fn id(&self) -> Id<Self::Key> {
        self.mesh
    }
}

impl SparseMap<MeshInstances> {
    fn instanciate_unchecked(&mut self, mesh: &Mesh, node: Id<Node>, device: &wgpu::Device) {
        self.entry(mesh.id())
            .or_insert_with(|| {
                let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Mesh instance vertex buffer"),
                    size: size_of::<u32>() as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                MeshInstances {
                    mesh: mesh.id(),
                    primitives: mesh.primitives.clone(),
                    nodes: Vec::with_capacity(1),
                    vertex_buffer,
                }
            })
            .nodes
            .push(node);
    }
}
