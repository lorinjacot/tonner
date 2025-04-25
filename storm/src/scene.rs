use std::ops::{Index, IndexMut};

use bytemuck::{Pod, Zeroable, cast_slice};
use glam::{Mat4, usize};
use wgpu::util::DeviceExt;

use crate::{
    math::Transform,
    mesh::{Mesh, Primitive},
    storage::{DenseEntry, Id, SetEntry, SparseMap, SparseSet},
};

pub struct Scene {
    id: Id<Self>,
    pub name: String,
    nodes: SparseSet<Node>,
    nodes_buffer: Option<wgpu::Buffer>,
    root_nodes: Vec<Id<Node>>,
    meshes: SparseMap<MeshInstances>,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: Option<wgpu::BindGroup>,
}

impl Scene {
    pub fn node_builder(&mut self) -> NodeBuilder {
        NodeBuilder::new(self)
    }

    pub fn root_nodes(&self) -> &[Id<Node>] {
        &self.root_nodes
    }

    pub fn update(&mut self, _aspect_ration: f32, device: &wgpu::Device, queue: &wgpu::Queue) {
        let nodes_buffer = {
            let data: Vec<_> = self
                .nodes
                .iter()
                .map(|node| NodeUniform {
                    model: node.global_transform,
                })
                .collect();

            match &self.nodes_buffer {
                Some(buffer) => {
                    queue.write_buffer(buffer, 0, cast_slice(&data));
                    buffer
                }
                None => {
                    self.bind_group = None;
                    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("{}'s nodes buffer", self.name)),
                        contents: cast_slice(&data),
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    });
                    self.nodes_buffer.insert(buffer)
                }
            }
        };

        for mesh_instances in self.meshes.iter_mut() {
            const INDEX_SIZE: usize = size_of::<u32>();
            let data: Vec<_> = mesh_instances
                .nodes
                .iter()
                .map(|id| self.nodes.dense_index(*id).unwrap() as u32)
                .collect();
            let buffer = &mut mesh_instances.vertex_buffer;
            if buffer.size() >= (mesh_instances.nodes.len() * INDEX_SIZE) as wgpu::BufferAddress {
                queue.write_buffer(&buffer, 0, cast_slice(&data));
            } else {
                *buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Mesh instances vertex buffer"),
                    contents: cast_slice(&data),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });
            }
        }

        if self.bind_group.is_none() {
            self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("{} scene bind group", self.name)),
                layout: &self.bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: nodes_buffer.as_entire_binding(),
                }],
            }))
        }
    }

    pub fn render(&self, _render_pass: &mut wgpu::RenderPass) {}
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
    pub(super) bind_group_layout: wgpu::BindGroupLayout,
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

        Scene {
            id,
            name,
            nodes,
            nodes_buffer: None,
            root_nodes,
            meshes,
            bind_group_layout: desc.bind_group_layout,
            bind_group: None,
        }
    }
}

pub struct Node {
    id: Id<Node>,
    pub name: String,
    parent: Option<Id<Node>>,
    children: Vec<Id<Node>>,
    local_transform: Transform,
    global_transform: Mat4,
}

impl Node {
    pub fn children(&self) -> &[Id<Node>] {
        &self.children
    }

    pub fn local_transform(&self) -> &Transform {
        &self.local_transform
    }
}

pub struct NodeBuilder<'a, 's> {
    scene: &'s mut Scene,
    desc: NodeDescriptor,
    mesh: Option<&'a Mesh>,
}

impl<'a, 's> NodeBuilder<'a, 's> {
    pub fn new(scene: &'s mut Scene) -> Self {
        Self {
            scene,
            desc: NodeDescriptor {
                parent: None,
                name: None,
                children: Vec::new(),
                local_transform: Transform::IDENTITY,
                global_transform: Mat4::IDENTITY,
            },
            mesh: None,
        }
    }

    pub fn name(mut self, name: Option<String>) -> Self {
        self.desc.name = name;
        self
    }

    pub fn parent(mut self, parent: Option<Id<Node>>) -> Self {
        self.desc.parent = parent;
        self
    }

    pub fn mesh(mut self, mesh: Option<&'a Mesh>) -> Self {
        self.mesh = mesh;
        self
    }

    pub fn build(mut self, device: &wgpu::Device) -> &'s mut Node {
        let id = self.scene.nodes.next_id();
        match self.desc.parent {
            Some(parent) => {
                let parent = &mut self.scene.nodes[parent];
                parent.children.push(id);
                self.desc.global_transform =
                    parent.global_transform * self.desc.local_transform.matrix();
            }
            None => self.scene.root_nodes.push(id),
        }
        if let Some(mesh) = self.mesh {
            self.scene
                .meshes
                .entry(mesh.id())
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
                .push(id);
        }
        self.scene.nodes_buffer = None;
        self.scene.nodes.push(self.desc)
    }
}

pub struct NodeDescriptor {
    name: Option<String>,
    parent: Option<Id<Node>>,
    children: Vec<Id<Node>>,
    local_transform: Transform,
    global_transform: Mat4,
}

impl DenseEntry for Node {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

impl SetEntry for Node {
    type Descriptor = NodeDescriptor;

    fn new(id: Id<Self::Key>, desc: Self::Descriptor) -> Self {
        let name = desc.name.unwrap_or_else(|| id.to_string());
        Self {
            id,
            name,
            parent: desc.parent,
            children: desc.children,
            local_transform: desc.local_transform,
            global_transform: desc.global_transform,
        }
    }
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct NodeUniform {
    model: Mat4,
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
