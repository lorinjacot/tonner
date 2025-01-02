use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

use crate::storage::{Id, Storage};

pub struct NodeManager {
    nodes: Storage<Node>,
    local_transform_buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl NodeManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let local_transform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
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
            local_transform_buffer,
            bind_group,
            bind_group_layout,
        }
    }

    pub fn create(
        &mut self,
        nodes: impl IntoIterator<Item = Builder>,
        device: &wgpu::Device,
    ) -> Result<Vec<NodeId>, ()> {
        let nodes_id = self.create_recursive(nodes)?;

        self.create_buffer(device);

        Ok(nodes_id)
    }

    fn create_recursive(
        &mut self,
        nodes: impl IntoIterator<Item = Builder>,
    ) -> Result<Vec<NodeId>, ()> {
        let nodes = nodes.into_iter();
        let mut nodes_id = Vec::with_capacity(nodes.size_hint().0);
        for node in nodes {
            let local_matrix = match node.local_transform {
                Transform::Matrix(matrix) => matrix,
                Transform::TRS {
                    translation,
                    rotation,
                    scale,
                } => Mat4::from_scale_rotation_translation(scale, rotation, translation),
            };

            let global_transform = match node.parent {
                Some(parent_id) => {
                    self.nodes.get(parent_id).ok_or(())?.global_transform * local_matrix
                }
                None => local_matrix,
            };

            let node_id = self.nodes.add(Node {
                local_transform: node.local_transform,
                global_transform,
                parent: node.parent,
                children: Vec::new(),
            });
            nodes_id.push(node_id);

            let children: Vec<_> = node
                .children
                .into_iter()
                .map(|node| node.set_parent(node_id))
                .collect();
            let children = self.create_recursive(children)?;
            self.nodes[node_id].children = children;
        }

        Ok(nodes_id)
    }

    fn create_buffer(&mut self, device: &wgpu::Device) {
        let global_transforms = self
            .nodes
            .values()
            .map(|node| node.global_transform)
            .collect::<Vec<_>>();
        let contents = bytemuck::cast_slice(&global_transforms);
        self.local_transform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Nodes local transform buffer"),
                contents,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
            });

        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Nodes bind group"),
            layout: &self.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.local_transform_buffer.as_entire_binding(),
            }],
        }));
    }
}

pub struct Node {
    local_transform: Transform,
    global_transform: Mat4,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
}

pub type NodeId = Id<Node>;

pub struct Builder {
    local_transform: Transform,
    parent: Option<NodeId>,
    children: Vec<Builder>,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            local_transform: Transform::Matrix(Mat4::IDENTITY),
            parent: None,
            children: Vec::new(),
        }
    }

    pub fn set_transform(mut self, transform: Transform) -> Self {
        self.local_transform = transform;
        self
    }

    pub fn set_parent(mut self, parent_id: NodeId) -> Self {
        self.parent = Some(parent_id);
        self
    }

    pub fn set_children(mut self, children: Vec<Builder>) -> Self {
        self.children = children;
        self
    }
}

#[derive(Clone, Copy)]
pub enum Transform {
    Matrix(Mat4),
    TRS {
        translation: Vec3,
        rotation: Quat,
        scale: Vec3,
    },
}
