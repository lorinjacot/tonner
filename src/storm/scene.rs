use std::{
    iter::{once, repeat_n},
    ops::{Index, Range},
};

use bitflags::serde;
use bytemuck::{cast_slice, Pod, Zeroable};
use glam::{Mat3, Mat4};
use wgpu::util::DeviceExt;

use super::{
    buffer::BufferManager,
    camera::{Camera, CameraManager},
    material::MaterialManager,
    mesh::{Mesh, MeshManager, PrimitivePipeline},
    storage::{Id, SparseMap, SparseSet},
    texture::TextureManager,
    Asset,
};

pub struct SceneManager {
    scenes: SparseSet<Scene>,
    assets: SparseMap<Asset, Vec<Option<Id<Scene>>>>,
}

impl SceneManager {
    pub fn new() -> Self {
        let scenes = SparseSet::new();
        let assets = SparseMap::new();

        SceneManager { scenes, assets }
    }

    pub fn get_mut(&mut self, scene: Id<Scene>) -> Option<&mut Scene> {
        self.scenes.get_mut(scene)
    }

    pub fn load_scene(
        &mut self,
        asset: Id<Asset>,
        scene: gltf::Scene,
        buffers: &mut BufferManager,
        textures: &mut TextureManager,
        materials: &mut MaterialManager,
        meshes: &mut MeshManager,
        cameras: &mut CameraManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<Scene> {
        match self.assets.entry(asset).or_default().get(scene.index()) {
            Some(Some(id)) => *id,
            _ => self.create_scene(
                asset, scene, buffers, textures, materials, meshes, cameras, device, queue,
            ),
        }
    }

    fn create_scene(
        &mut self,
        asset: Id<Asset>,
        gltf_scene: gltf::Scene,
        buffers: &mut BufferManager,
        textures: &mut TextureManager,
        materials: &mut MaterialManager,
        meshes: &mut MeshManager,
        cameras: &mut CameraManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<Scene> {
        let mut scene = Scene {
            nodes: SparseSet::new(),
            primitives: SparseMap::new(),
            cameras: SparseMap::new(),
            active_camera: None,
        };

        for node in gltf_scene.nodes() {
            create_node(
                asset,
                node,
                None,
                Mat4::IDENTITY,
                &mut scene,
                buffers,
                textures,
                materials,
                meshes,
                cameras,
                device,
                queue,
            );
        }

        let id = self.scenes.push(scene);

        let mapping = &mut self.assets[asset];
        match mapping.get_mut(gltf_scene.index()) {
            Some(entry) => *entry = Some(id),
            None => {
                let iter = repeat_n(None, gltf_scene.index() - mapping.len()).chain(once(Some(id)));
                mapping.extend(iter);
            }
        }

        id
    }
}

fn create_node(
    asset: Id<Asset>,
    node: gltf::Node,
    parent: Option<Id<Node>>,
    parent_transform: Mat4,
    scene: &mut Scene,
    buffers: &mut BufferManager,
    textures: &mut TextureManager,
    materials: &mut MaterialManager,
    meshes: &mut MeshManager,
    cameras: &mut CameraManager,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Id<Node> {
    let local_transform = Mat4::from_cols_array_2d(&node.transform().matrix());
    let global_transform = parent_transform * local_transform;

    let node_id = scene.nodes.push(Node {
        local_transform,
        global_transform,
        children: Vec::new(),
        parent,
    });

    if let Some(mesh) = node.mesh() {
        let mesh_id = meshes.load_mesh(asset, mesh, buffers, textures, materials, device, queue);
        let mesh = &meshes[mesh_id];
        for (pipeline, primitives) in mesh.primitives.iter() {
            scene
                .primitives
                .entry(pipeline)
                .or_insert_with(|| (meshes[pipeline].clone(), SparseMap::new()))
                .1
                .entry(mesh_id)
                .or_insert_with(|| {
                    let primitives = primitives
                        .iter()
                        .map(|primitive| {
                            let indices = primitive.indices.map(|(accessor, index_format)| {
                                let accessor = &buffers[accessor];
                                (
                                    buffers[accessor.buffer()].clone(),
                                    accessor.bounds(),
                                    index_format,
                                )
                            });
                            let vertex_buffers = primitive
                                .vertex_buffers
                                .iter()
                                .map(|buffer| buffers[*buffer].clone())
                                .collect();
                            Primitive {
                                indices,
                                vertex_buffers,
                                vertex_count: primitive.vertex_count,
                                material: materials[primitive.material].bind_group().clone(),
                            }
                        })
                        .collect();
                    (primitives, Vec::with_capacity(1))
                })
                .1
                .push(node_id);
        }
    }

    if let Some(camera) = node.camera() {
        let name = camera.name().unwrap_or("");
        let camera_id = cameras.create_camera(camera, device);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{name} camera buffer")),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{name} camera bind group")),
            layout: meshes.camera_bind_group_layout(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        scene
            .cameras
            .insert(camera_id, (buffer, bind_group, node_id));
    }

    let children: Vec<_> = node
        .children()
        .map(|child| {
            create_node(
                asset,
                child,
                Some(node_id),
                global_transform,
                scene,
                buffers,
                textures,
                materials,
                meshes,
                cameras,
                device,
                queue,
            )
        })
        .collect();

    let node = &mut scene.nodes[node_id];
    node.children = children;

    node_id
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct CameraUniform {
    view_projection: [f32; 16],
    world_position: [f32; 3],
}

struct Primitive {
    indices: Option<(wgpu::Buffer, Range<u64>, wgpu::IndexFormat)>,
    vertex_buffers: Vec<wgpu::Buffer>,
    vertex_count: u32,
    material: wgpu::BindGroup,
}

pub struct Scene {
    nodes: SparseSet<Node>,
    primitives: SparseMap<
        PrimitivePipeline,
        (
            wgpu::RenderPipeline,
            SparseMap<Mesh, (Vec<Primitive>, Vec<Id<Node>>)>,
        ),
    >,
    cameras: SparseMap<Camera, (wgpu::Buffer, wgpu::BindGroup, Id<Node>)>,
    pub active_camera: Option<Id<Camera>>,
}

impl Scene {
    pub fn cameras(&self) -> impl Iterator<Item = Id<Camera>> + use<'_> {
        self.cameras.iter().map(|(camera, _)| camera)
    }

    pub fn update(&mut self, cameras: &mut CameraManager, queue: &wgpu::Queue) {
        if let Some(camera_id) = self.active_camera {
            let projection = cameras[camera_id].projection_matrix();
            if let Some((buffer, _, node)) = self.cameras.get(camera_id) {
                if let Some(node) = self.nodes.get(*node) {
                    let view = node.global_transform;
                    queue.write_buffer(buffer, 0, cast_slice(&[projection * view]));
                }
            }
        }
    }

    pub fn render(&self, device: &wgpu::Device, render_pass: &mut wgpu::RenderPass) {
        let camera = match self
            .active_camera
            .map(|camera| self.cameras.get(camera))
            .flatten()
        {
            Some((_, bind_group, _)) => bind_group,
            None => return,
        };

        for (pipeline, by_mesh) in self.primitives.values() {
            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, camera, &[]);
            for (primitives, nodes) in by_mesh.values() {
                let node_count = nodes.len() as u32;
                let transforms: Vec<_> = nodes
                    .iter()
                    .map(|node| {
                        let transform = self.nodes[*node].global_transform;
                        Transform {
                            point: transform.to_cols_array(),
                            vector: Mat3::from_mat4(transform.inverse().transpose())
                                .to_cols_array(),
                        }
                    })
                    .collect();
                let transforms = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Node transforms vertex buffer"),
                    contents: cast_slice(&transforms),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                render_pass.set_vertex_buffer(0, transforms.slice(..));

                for primitive in primitives {
                    render_pass.set_bind_group(1, &primitive.material, &[]);
                    for (slot, vertex_buffer) in primitive.vertex_buffers.iter().enumerate() {
                        render_pass.set_vertex_buffer(slot as u32 + 1, vertex_buffer.slice(..));
                    }

                    match &primitive.indices {
                        Some(indices) => {
                            render_pass
                                .set_index_buffer(indices.0.slice(indices.1.clone()), indices.2);
                            render_pass.draw_indexed(0..primitive.vertex_count, 0, 0..node_count);
                        }
                        None => render_pass.draw(0..primitive.vertex_count, 0..node_count),
                    }
                }
            }
        }
    }
}

impl Index<Id<Scene>> for SceneManager {
    type Output = Scene;

    fn index(&self, index: Id<Scene>) -> &Self::Output {
        &self.scenes[index]
    }
}

pub struct Node {
    local_transform: Mat4,
    global_transform: Mat4,
    parent: Option<Id<Node>>,
    children: Vec<Id<Node>>,
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Transform {
    point: [f32; 16],
    vector: [f32; 9],
}
