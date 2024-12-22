use std::{collections::HashMap, time::Duration};

use animation::AnimationManager;
use glam::{Mat4, Quat, Vec3};
use mesh::MeshManager;
use wgpu::util::DeviceExt;

use crate::{asset::Asset, camera::Camera};

mod animation;
mod mesh;

pub struct Scene {
    nodes: Vec<Node>,
    nodes_bind_group: wgpu::BindGroup,
    meshes: MeshManager,
    animations: AnimationManager,
    pub camera: Camera,
}

impl Scene {
    pub fn load(
        gltf_scene: &gltf::Scene,
        asset: &Asset,
        device: &wgpu::Device,
        targets: &[Option<wgpu::ColorTargetState>],
        camera: Camera,
    ) -> Self {
        let mut nodes_mapping = HashMap::new();
        let mut mesh_node_mapping = HashMap::new();
        let mut nodes = Vec::new();

        for gltf_node in gltf_scene.nodes() {
            load_node(
                &gltf_node,
                &Mat4::IDENTITY,
                &mut nodes,
                &mut nodes_mapping,
                &mut mesh_node_mapping,
            );
        }

        let mut meshes = MeshManager::new(device, targets);

        for gltf_mesh in asset.document.meshes() {
            if let Some(nodes_id) = mesh_node_mapping.remove(&gltf_mesh.index()) {
                meshes.add_mesh_to_nodes(&gltf_mesh, &nodes_id, &mut nodes, device, asset);
            }
        }

        let nodes_values = nodes
            .iter()
            .map(|node| node.global_transform)
            .collect::<Vec<_>>();

        let nodes_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Nodes buffer"),
            contents: bytemuck::cast_slice(&nodes_values),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let nodes_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Nodes bind group"),
            layout: meshes.nodes_bind_group_layout(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: nodes_buffer.as_entire_binding(),
            }],
        });

        let animations = AnimationManager::load(asset, nodes_mapping);

        Self {
            nodes,
            nodes_bind_group,
            meshes,
            animations,
            camera,
        }
    }

    pub fn update(&mut self, delta_time: Duration, queue: &wgpu::Queue) {
        self.animations.update(delta_time, queue);
    }
}

fn load_node(
    gltf_node: &gltf::Node,
    parent_transform: &Mat4,
    nodes: &mut Vec<Node>,
    nodes_mapping: &mut HashMap<usize, usize>,
    mesh_node_mapping: &mut HashMap<usize, Vec<usize>>,
) -> usize {
    let node_id = nodes.len();

    let local_transform = match gltf_node.transform() {
        gltf::scene::Transform::Decomposed {
            translation,
            rotation,
            scale,
        } => {
            let translation = Vec3::from_array(translation);
            let rotation = Quat::from_array(rotation);
            let scale = Vec3::from_array(scale);
            Mat4::from_scale_rotation_translation(scale, rotation, translation)
        }
        gltf::scene::Transform::Matrix { matrix } => Mat4::from_cols_array_2d(&matrix),
    };

    let global_transform = *parent_transform * local_transform;

    let children = gltf_node
        .children()
        .map(|child| {
            load_node(
                &child,
                &global_transform,
                nodes,
                nodes_mapping,
                mesh_node_mapping,
            )
        })
        .collect();

    nodes.push(Node {
        id: node_id,
        children,
        local_transform,
        global_transform,
        mesh: None,
    });
    nodes_mapping.insert(gltf_node.index(), node_id);

    if let Some(gltf_mesh) = gltf_node.mesh() {
        mesh_node_mapping
            .entry(gltf_mesh.index())
            .or_default()
            .push(node_id);
    }

    node_id
}

struct Node {
    id: usize,
    children: Vec<usize>,
    local_transform: Mat4,
    global_transform: Mat4,
    mesh: Option<usize>,
}

pub trait DrawScene {
    fn draw_scene(&mut self, scene: &Scene);
}
