use std::collections::HashMap;

use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

use super::primitive::PrimitiveManager;

pub struct Scene {
    mapping: HashMap<usize, usize>,
    nodes: Vec<Node>,
    root_nodes: Vec<usize>,
    node_bind_group: Option<wgpu::BindGroup>,
    meshes: HashMap<usize, Mesh>,
}

impl Scene {
    pub fn from_nodes(
        gltf_nodes: gltf::iter::Nodes,
        device: &wgpu::Device,
        buffers: &Vec<gltf::buffer::Data>,
        primitive_manager: &mut PrimitiveManager,
    ) -> Self {
        let mut scene = Self {
            mapping: HashMap::new(),
            nodes: Vec::new(),
            root_nodes: Vec::new(),
            node_bind_group: None,
            meshes: HashMap::new(),
        };
        for node in gltf_nodes {
            let index =
                scene.init_node_recursive(node, Mat4::IDENTITY, device, buffers, primitive_manager);
            scene.root_nodes.push(index);
        }

        let nodes_data = scene
            .nodes
            .iter()
            .map(|node| node.global_transform)
            .collect::<Vec<_>>();
        let nodes_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Nodes buffer"),
            contents: bytemuck::cast_slice(&nodes_data),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        scene.node_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Node bind group"),
            layout: primitive_manager.node_bind_group_layout(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: nodes_buffer.as_entire_binding(),
            }],
        }));

        scene
    }

    pub fn nodes_bind_group(&self) -> &wgpu::BindGroup {
        self.node_bind_group.as_ref().unwrap()
    }

    fn init_node_recursive(
        &mut self,
        node: gltf::Node,
        parent_transform: Mat4,
        device: &wgpu::Device,
        buffers: &Vec<gltf::buffer::Data>,
        primitive_manager: &mut PrimitiveManager,
    ) -> usize {
        let gltf_index = node.index();
        if let Some(manager_index) = self.mapping.get(&gltf_index) {
            return *manager_index;
        }

        let local_transform = match node.transform() {
            gltf::scene::Transform::Matrix { matrix } => Mat4::from_cols_array_2d(&matrix),
            gltf::scene::Transform::Decomposed {
                translation,
                rotation,
                scale,
            } => {
                let scale = Vec3::from_array(scale);
                let rotation = Quat::from_array(rotation);
                let translation = Vec3::from_array(translation);
                Mat4::from_scale_rotation_translation(scale, rotation, translation)
            }
        };
        let global_transform = parent_transform * local_transform;

        let children = node
            .children()
            .map(|node| {
                self.init_node_recursive(node, global_transform, device, buffers, primitive_manager)
            })
            .collect();

        if let Some(mesh) = node.mesh() {
            primitive_manager.init_mesh(&mesh, device, buffers);
        }

        let manager_index = self.nodes.len();

        self.nodes.push(Node {
            local_transform,
            global_transform,
            children,
        });
        self.mapping.insert(gltf_index, manager_index);

        return manager_index;
    }
}

struct Node {
    local_transform: Mat4,
    global_transform: Mat4,
    children: Vec<usize>,
}

pub struct Mesh {
    nodes: Vec<usize>,
    vertex_buffer: Option<wgpu::Buffer>,
}

impl Mesh {
    pub fn new(nodes: Vec<usize>) -> Self {
        Self {
            nodes,
            vertex_buffer: None,
        }
    }

    pub fn add_node(&mut self, node: usize) {
        self.nodes.push(node);
        self.vertex_buffer = None;
    }
}
