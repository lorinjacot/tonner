use std::{
    ops::{Index, IndexMut},
    time::Duration,
};

use bytemuck::{Pod, Zeroable, bytes_of, cast_slice};
pub use camera::Camera;
use glam::{Mat3, Mat4, Vec3, Vec4, usize};
pub use node::{Node, NodeBuilder, NodeHandle};
use wgpu::util::DeviceExt;

use crate::{
    Environment, Resources,
    mesh::{Mesh, Primitive},
    storage::{DenseEntry, Id, SparseMap, SparseSet},
};

pub mod animation;
pub mod camera;
mod node;

pub struct Scene {
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
    animations: SparseSet<animation::Animation>,
    playing_animations: SparseMap<Id<animation::Animation>>,
    point_lights: SparseMap<PointLight>,
    lights_buffer: Option<wgpu::Buffer>,
    irradiance_map_view: wgpu::TextureView,
    irradiance_map_sampler: wgpu::Sampler,
    prefilter_map_view: wgpu::TextureView,
    prefilter_map_sampler: wgpu::Sampler,
    brdf_lut_view: wgpu::TextureView,
    brdf_lut_sampler: wgpu::Sampler,
    render_bind_group_layout: wgpu::BindGroupLayout,
    render_bind_group: Option<wgpu::BindGroup>,
    skybox_bind_group: Option<wgpu::BindGroup>,
    skybox_pipeline: wgpu::RenderPipeline,
}

impl Scene {
    pub fn new(
        name: String,
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Self {
        let camera_buffer = resources.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera buffer"),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let environment = match resources.default_environmnent {
            Some(id) => id,
            None => {
                let id = resources
                    .environment_builder()
                    .name("Default environment".to_string())
                    .build(encoder)
                    .id();
                resources.default_environmnent = Some(id);
                id
            }
        };
        let environment = &resources.environments[environment];

        let animations = SparseSet::new();
        let playing_animations = SparseMap::new();

        Self {
            name,
            device: resources.device.clone(),
            queue: resources.queue.clone(),
            nodes: SparseSet::new(),
            nodes_buffer: None,
            root_nodes: Vec::new(),
            meshes: SparseMap::new(),
            cameras: SparseMap::new(),
            active_camera: None,
            camera_buffer,
            point_lights: SparseMap::new(),
            lights_buffer: None,
            animations,
            playing_animations,
            irradiance_map_view: environment.irradiance_map_view().clone(),
            irradiance_map_sampler: environment.irradiance_map_sampler().clone(),
            prefilter_map_view: environment.prefilter_map_view().clone(),
            prefilter_map_sampler: environment.prefilter_map_sampler().clone(),
            brdf_lut_view: environment.brdf_lut_view().clone(),
            brdf_lut_sampler: environment.brdf_lut_sampler().clone(),
            render_bind_group_layout: resources.render_bind_group_layout.clone(),
            render_bind_group: None,
            skybox_bind_group: None,
            skybox_pipeline: resources.skybox_pipeline.clone(),
        }
    }

    pub fn node_handle(&mut self, id: Id<Node>) -> NodeHandle {
        NodeHandle { id, scene: self }
    }

    pub fn node_builder(&mut self) -> NodeBuilder {
        NodeBuilder::new(self)
    }

    pub fn contains_node(&self, node: Id<Node>) -> bool {
        self.nodes.contains(node)
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

    pub fn set_environment(&mut self, environment: Id<Environment>, resources: &Resources) {
        let environment = &resources.environments[environment];
        self.skybox_bind_group = Some(environment.skybox_bind_group().clone());
        self.irradiance_map_view = environment.irradiance_map_view().clone();
        self.irradiance_map_sampler = environment.irradiance_map_sampler().clone();
        self.prefilter_map_view = environment.prefilter_map_view().clone();
        self.prefilter_map_sampler = environment.prefilter_map_sampler().clone();
        self.brdf_lut_view = environment.brdf_lut_view().clone();
        self.brdf_lut_sampler = environment.brdf_lut_sampler().clone();
    }

    pub fn update(&mut self, delta_time: Duration, viewport_aspect_ration: f32) {
        self.update_animations(delta_time);

        let root_nodes = self.root_nodes.clone();
        for node in root_nodes {
            self.node_handle(node)
                .update_world_matrices_parent(Mat4::IDENTITY);
        }

        let nodes_buffer = {
            let data: Vec<_> = self
                .nodes
                .iter()
                .map(|node| {
                    let matrix = node.world_matrix();
                    let normal_matrix = Mat3::from_mat4(matrix).inverse().transpose();
                    NodeUniform {
                        matrix,
                        normal_matrix: [
                            normal_matrix.x_axis.extend(0.0),
                            normal_matrix.y_axis.extend(0.0),
                            normal_matrix.z_axis.extend(0.0),
                        ],
                    }
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

        let lights_buffer = {
            let mut point_lights: Vec<_> = self
                .point_lights
                .iter()
                .map(|light| {
                    let position = self.nodes[light.node].world_position();
                    PointLightUniform {
                        position: position.to_array(),
                        _pad0: 0,
                        color: light.color.to_array(),
                        _pad1: 0,
                    }
                })
                .collect();
            let point_light_count = point_lights.len() as u32;
            if point_light_count == 0 {
                point_lights.push(PointLightUniform {
                    position: [0.0; 3],
                    _pad0: 0,
                    color: [0.0; 3],
                    _pad1: 0,
                });
            }

            let light_storage = LightStorage {
                point_light_count,
                _pad: [0; 3],
            };
            let light_storage_size =
                size_of::<LightStorage>() + point_lights.len() * size_of::<PointLightUniform>();

            let data = &mut Vec::with_capacity(light_storage_size);
            data.extend_from_slice(bytes_of(&light_storage));
            data.extend_from_slice(cast_slice(&point_lights));
            assert_eq!(data.len(), light_storage_size);

            match &self.lights_buffer {
                Some(buffer) if buffer.size() as usize >= light_storage_size => {
                    self.queue.write_buffer(&buffer, 0, data);
                    buffer
                }
                _ => {
                    self.render_bind_group = None;
                    let buffer =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Lights buffer"),
                                contents: data,
                                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                            });
                    self.lights_buffer.insert(buffer)
                }
            }
        };

        if let Some(camera) = self.active_camera {
            let projection = self.cameras[camera]
                .projection
                .matrix(viewport_aspect_ration);
            let camera = &self.nodes[camera];
            let position = camera.world_position();
            let view = Mat4::look_to_rh(
                camera.world_position(),
                camera.world_matrix().transform_vector3(-Vec3::Z),
                camera.world_matrix().transform_vector3(Vec3::Y),
            );
            let view_projection = projection * view;
            let camera_uniform = CameraUniform {
                view_projection,
                view,
                projection_inverse: projection.inverse(),
                position,
                _pad: 0,
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
                        label: Some(&format!("{} render bind group", self.name)),
                        layout: &self.render_bind_group_layout,
                        entries: &[
                            // nodes
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: nodes_buffer.as_entire_binding(),
                            },
                            // camera
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: self.camera_buffer.as_entire_binding(),
                            },
                            // lights
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: lights_buffer.as_entire_binding(),
                            },
                            // irradiance map
                            wgpu::BindGroupEntry {
                                binding: 3,
                                resource: wgpu::BindingResource::TextureView(
                                    &self.irradiance_map_view,
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 4,
                                resource: wgpu::BindingResource::Sampler(
                                    &self.irradiance_map_sampler,
                                ),
                            },
                            // prefilter map
                            wgpu::BindGroupEntry {
                                binding: 5,
                                resource: wgpu::BindingResource::TextureView(
                                    &self.prefilter_map_view,
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 6,
                                resource: wgpu::BindingResource::Sampler(
                                    &self.prefilter_map_sampler,
                                ),
                            },
                            // BRDF LUT
                            wgpu::BindGroupEntry {
                                binding: 7,
                                resource: wgpu::BindingResource::TextureView(&self.brdf_lut_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 8,
                                resource: wgpu::BindingResource::Sampler(&self.brdf_lut_sampler),
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
                    render_pass.set_bind_group(1, &primitive.material, &[]);
                    for (slot, vertex_buffer) in primitive.vertex_buffers.iter().enumerate() {
                        render_pass.set_vertex_buffer(slot as u32 + 1, vertex_buffer.slice(..));
                    }
                    match &primitive.index_buffer {
                        Some(index_buffer) => {
                            render_pass.set_index_buffer(
                                index_buffer.buffer.slice(..),
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

            if let Some(skybox_bind_group) = self.skybox_bind_group.as_ref() {
                render_pass.set_pipeline(&self.skybox_pipeline);
                render_pass.set_bind_group(1, skybox_bind_group, &[]);
                render_pass.draw(0..3, 0..1);
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

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct NodeUniform {
    matrix: Mat4,
    normal_matrix: [Vec4; 3],
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct CameraUniform {
    view_projection: Mat4,
    view: Mat4,
    projection_inverse: Mat4,
    position: Vec3,
    _pad: u32,
}

struct PointLight {
    node: Id<Node>,
    color: Vec3,
}

impl DenseEntry for PointLight {
    type Key = Node;

    fn id(&self) -> Id<Self::Key> {
        self.node
    }
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct LightStorage {
    point_light_count: u32,
    _pad: [u32; 3],
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct PointLightUniform {
    position: [f32; 3],
    _pad0: u32,
    color: [f32; 3],
    _pad1: u32,
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
