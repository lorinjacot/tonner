use std::{
    iter::repeat_with,
    ops::{Index, IndexMut},
    time::Duration,
    u32,
};

use bytemuck::{Pod, Zeroable, bytes_of, cast_slice};
pub use camera::Camera;
use glam::{Mat4, Vec3, usize};
pub use node::{Node, NodeBuilder, NodeHandle};
use skin::init_skins_buffer;
use wgpu::util::DeviceExt;

use crate::{
    Engine, Environment, Resources,
    geometry::Indices,
    material::AlphaMode,
    mesh::{Mesh, PrimitivePipeline},
    storage::{DenseEntry, Id, SparseMap, SparseSet},
    texture::TextureBuilder,
};

pub mod animation;
pub mod camera;
mod node;
mod renderer;
pub mod skin;

const NODE_INDEX_SIZE: usize = size_of::<u32>();

/// A scene describes a world. A scene can be evolve over time and can be rendered
/// to a screen or a texture.
///
/// A scene is made up of [Node]s. Nodes are organized in a parent-child hierachy, known as the
/// node-hierarchy or the scene graph. A node is called a root node when it doesn't have a parent.
/// Each node defines a local space. The local transform is used to get from the parent node local
/// space (parent space for short) to the local space. The global transform is used to get from the
/// scene space (or global space) to the local space. Both transforms are equal for root nodes.
///
/// To add an object to the scene, attach it to a node. For example, each node can have a mesh. During
/// rendering, the attached mesh will be rendered at the local space origin.
pub struct Scene {
    pub name: String,
    device: wgpu::Device,
    queue: wgpu::Queue,
    nodes: SparseSet<Node>,
    nodes_buffer: Option<wgpu::Buffer>,
    root_nodes: Vec<Id<Node>>,
    skins: SparseSet<skin::Skin>,
    skins_buffer: wgpu::Buffer,
    meshes: SparseMap<MeshInstances>,
    opaque_pipeline_primitives: SparseMap<PipelinePrimitives>,
    transparent_pipeline_primitives: SparseMap<PipelinePrimitives>,
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
    render_width: u32,
    render_height: u32,
    opaque_attachment: wgpu::TextureView,
    accumulation_attachment: wgpu::TextureView,
    revealage_attachment: wgpu::TextureView,
    depth_attachment: wgpu::TextureView,
    compose_bind_group: wgpu::BindGroup,
    compose_pipeline: wgpu::RenderPipeline,
    brightness_bind_group: wgpu::BindGroup,
    brightness_pipeline: wgpu::RenderPipeline,
    bloom_textures: [(wgpu::TextureView, wgpu::BindGroup); 2],
    gaussian_blur_pipeline: wgpu::RenderPipeline,
    tone_mapping_pipeline: wgpu::RenderPipeline,
    tone_mapping_bind_group: wgpu::BindGroup,
    bloom_amount: usize,
}

impl Scene {
    pub fn new(
        name: String,
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
        render_width: u32,
        render_height: u32,
    ) -> Self {
        let nodes = SparseSet::new();

        let mut skins = SparseSet::new();
        let skins_buffer = init_skins_buffer(&mut skins, &nodes, &resources.device);

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

        let animations = SparseSet::new();
        let playing_animations = SparseMap::new();

        let bloom_amount = 10;
        let (
            [
                opaque_attachment,
                accumulation_attachment,
                revealage_attachment,
                depth_attachment,
            ],
            bloom_textures,
            [
                compose_bind_group,
                brightness_bind_group,
                tone_mapping_bind_group,
            ],
        ) = create_render_attachments(
            render_width,
            render_height,
            bloom_amount,
            resources,
            encoder,
        );

        let environment = &resources.environments[environment];

        Self {
            name,
            device: resources.device.clone(),
            queue: resources.queue.clone(),
            nodes,
            nodes_buffer: None,
            root_nodes: Vec::new(),
            skins,
            skins_buffer,
            meshes: SparseMap::new(),
            opaque_pipeline_primitives: SparseMap::new(),
            transparent_pipeline_primitives: SparseMap::new(),
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
            render_width,
            render_height,
            opaque_attachment,
            accumulation_attachment,
            revealage_attachment,
            depth_attachment,
            compose_bind_group,
            compose_pipeline: resources.compose_pipeline.clone(),
            brightness_bind_group,
            brightness_pipeline: resources.brightness_pipeline.clone(),
            bloom_textures,
            bloom_amount,
            gaussian_blur_pipeline: resources.gaussian_blur_pipeline.clone(),
            tone_mapping_pipeline: resources.tone_mapping_pipeline.clone(),
            tone_mapping_bind_group,
        }
    }

    pub fn node_handle(&mut self, id: Id<Node>) -> NodeHandle<'_> {
        NodeHandle { id, scene: self }
    }

    pub fn node_builder(&mut self) -> NodeBuilder<'_> {
        NodeBuilder::new(self)
    }

    pub fn contains_node(&self, node: Id<Node>) -> bool {
        self.nodes.contains(node)
    }

    pub fn root_nodes(&self) -> &[Id<Node>] {
        &self.root_nodes
    }

    pub fn skin_builder<'a, 's>(&'s mut self) -> skin::SkinBuilder<'a, 's> {
        skin::SkinBuilder::new(self)
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

    pub fn set_render_dimension(
        &mut self,
        width: u32,
        height: u32,
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if self.render_width != width || self.render_height != height {
            self.render_width = width;
            self.render_height = height;

            (
                [
                    self.opaque_attachment,
                    self.accumulation_attachment,
                    self.revealage_attachment,
                    self.depth_attachment,
                ],
                self.bloom_textures,
                [
                    self.compose_bind_group,
                    self.brightness_bind_group,
                    self.tone_mapping_bind_group,
                ],
            ) = create_render_attachments(width, height, self.bloom_amount, resources, encoder);
        }
    }

    pub fn bloom_amout(&self) -> usize {
        self.bloom_amount
    }

    pub fn set_bloom_amount(&mut self, bloom_amount: usize, resources: &mut Resources) {
        self.bloom_amount = bloom_amount;
        self.tone_mapping_bind_group = create_tone_mapping_bind_group(
            resources,
            &self.opaque_attachment,
            &self.bloom_textures,
            bloom_amount,
        );
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

        self.update_nodes_buffer();

        self.update_skins_buffer();

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
                let mut node_indices = Vec::with_capacity(mesh_instances.nodes.len());
                let mut mirror_node_indices = Vec::with_capacity(mesh_instances.nodes.len());
                for node in &mesh_instances.nodes {
                    if self.nodes[*node]
                        .world_matrix()
                        .determinant()
                        .is_sign_positive()
                    {
                        node_indices.push(self.nodes.dense_index(*node).unwrap() as u32);
                    } else {
                        mirror_node_indices.push(self.nodes.dense_index(*node).unwrap() as u32);
                    }
                }
                let node_indices_size = node_indices.len() * NODE_INDEX_SIZE;
                if mesh_instances.node_indices.size() >= node_indices_size as u64 {
                    self.queue.write_buffer(
                        &mesh_instances.node_indices,
                        0,
                        cast_slice(&node_indices),
                    );
                    if mesh_instances.node_count != node_indices.len() {
                        mesh_instances.node_count = node_indices.len();
                        for (ids, pipeline_primitives) in [
                            (
                                &mesh_instances.opaque_primitives,
                                &mut self.opaque_pipeline_primitives,
                            ),
                            (
                                &mesh_instances.transparent_primitives,
                                &mut self.transparent_pipeline_primitives,
                            ),
                        ] {
                            for (pipeline, primitive) in ids {
                                let primitive =
                                    &mut pipeline_primitives[*pipeline].primitives[*primitive];
                                primitive.instance_count = node_indices.len() as u32;
                            }
                        }
                    }
                } else {
                    mesh_instances.node_indices =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Mesh instances vertex buffer"),
                                contents: cast_slice(&node_indices),
                                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            });
                    for (ids, pipeline_primitives) in [
                        (
                            &mesh_instances.opaque_primitives,
                            &mut self.opaque_pipeline_primitives,
                        ),
                        (
                            &mesh_instances.transparent_primitives,
                            &mut self.transparent_pipeline_primitives,
                        ),
                    ] {
                        for (pipeline, primitive) in ids {
                            let primitive =
                                &mut pipeline_primitives[*pipeline].primitives[*primitive];
                            primitive.node_indices = mesh_instances.node_indices.clone();
                            primitive.instance_count = node_indices.len() as u32;
                        }
                    }
                }

                let mirror_node_indices_size = mirror_node_indices.len() * NODE_INDEX_SIZE;
                if mesh_instances.mirror_node_indices.size() >= mirror_node_indices_size as u64 {
                    self.queue.write_buffer(
                        &mesh_instances.mirror_node_indices,
                        0,
                        cast_slice(&mirror_node_indices),
                    );
                    if mesh_instances.mirror_node_count != mirror_node_indices.len() {
                        mesh_instances.mirror_node_count = mirror_node_indices.len();
                        for (ids, pipeline_primitives) in [
                            (
                                &mesh_instances.opaque_primitives,
                                &mut self.opaque_pipeline_primitives,
                            ),
                            (
                                &mesh_instances.transparent_primitives,
                                &mut self.transparent_pipeline_primitives,
                            ),
                        ] {
                            for (pipeline, primitive) in ids {
                                let primitive =
                                    &mut pipeline_primitives[*pipeline].primitives[*primitive];
                                primitive.mirror_instance_count = mirror_node_indices.len() as u32;
                            }
                        }
                    }
                } else {
                    mesh_instances.mirror_node_indices =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Mesh instances vertex buffer"),
                                contents: cast_slice(&mirror_node_indices),
                                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            });
                    for (ids, pipeline_primitives) in [
                        (
                            &mesh_instances.opaque_primitives,
                            &mut self.opaque_pipeline_primitives,
                        ),
                        (
                            &mesh_instances.transparent_primitives,
                            &mut self.transparent_pipeline_primitives,
                        ),
                    ] {
                        for (pipeline, primitive) in ids {
                            let primitive =
                                &mut pipeline_primitives[*pipeline].primitives[*primitive];
                            primitive.mirror_node_indices =
                                mesh_instances.mirror_node_indices.clone();
                            primitive.mirror_instance_count = mirror_node_indices.len() as u32;
                        }
                    }
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
                                resource: self.nodes_buffer.as_ref().unwrap().as_entire_binding(),
                            },
                            // skins
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: self.skins_buffer.as_entire_binding(),
                            },
                            // camera
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: self.camera_buffer.as_entire_binding(),
                            },
                            // lights
                            wgpu::BindGroupEntry {
                                binding: 3,
                                resource: lights_buffer.as_entire_binding(),
                            },
                            // irradiance map
                            wgpu::BindGroupEntry {
                                binding: 4,
                                resource: wgpu::BindingResource::TextureView(
                                    &self.irradiance_map_view,
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 5,
                                resource: wgpu::BindingResource::Sampler(
                                    &self.irradiance_map_sampler,
                                ),
                            },
                            // prefilter map
                            wgpu::BindGroupEntry {
                                binding: 6,
                                resource: wgpu::BindingResource::TextureView(
                                    &self.prefilter_map_view,
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 7,
                                resource: wgpu::BindingResource::Sampler(
                                    &self.prefilter_map_sampler,
                                ),
                            },
                            // BRDF LUT
                            wgpu::BindGroupEntry {
                                binding: 8,
                                resource: wgpu::BindingResource::TextureView(&self.brdf_lut_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 9,
                                resource: wgpu::BindingResource::Sampler(&self.brdf_lut_sampler),
                            },
                        ],
                    }))
            }
        } else {
            self.render_bind_group = None;
        }
    }

    pub fn render(&self, render_texture: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        if let Some(render_bind_group) = self.render_bind_group.as_ref() {
            {
                let mut opaque_render_pass =
                    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Opaque render pass"),
                        color_attachments: &[
                            Some(wgpu::RenderPassColorAttachment {
                                view: &self.opaque_attachment,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                    store: wgpu::StoreOp::Store,
                                },
                            }),
                            None,
                            None,
                        ],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &self.depth_attachment,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                opaque_render_pass.set_bind_group(0, render_bind_group, &[]);

                for PipelinePrimitives {
                    pipeline,
                    mirror_pipeline,
                    primitives,
                    ..
                } in &self.opaque_pipeline_primitives
                {
                    opaque_render_pass.set_pipeline(pipeline);

                    for primitive in primitives {
                        opaque_render_pass.set_vertex_buffer(0, primitive.node_indices.slice(..));
                        opaque_render_pass.set_bind_group(1, &primitive.geometry, &[]);
                        opaque_render_pass.set_bind_group(2, &primitive.material, &[]);
                        match &primitive.vertex_indices {
                            Some(Indices {
                                buffer,
                                format,
                                count,
                            }) => {
                                opaque_render_pass.set_index_buffer(buffer.slice(..), *format);
                                opaque_render_pass.draw_indexed(
                                    0..*count as u32,
                                    0,
                                    0..primitive.instance_count,
                                );
                            }
                            None => opaque_render_pass.draw(
                                0..primitive.vertex_count as u32,
                                0..primitive.instance_count,
                            ),
                        }
                    }

                    opaque_render_pass.set_pipeline(mirror_pipeline);

                    for primitive in primitives {
                        opaque_render_pass
                            .set_vertex_buffer(0, primitive.mirror_node_indices.slice(..));
                        opaque_render_pass.set_bind_group(1, &primitive.geometry, &[]);
                        opaque_render_pass.set_bind_group(2, &primitive.material, &[]);
                        match &primitive.vertex_indices {
                            Some(Indices {
                                buffer,
                                format,
                                count,
                            }) => {
                                opaque_render_pass.set_index_buffer(buffer.slice(..), *format);
                                opaque_render_pass.draw_indexed(
                                    0..*count as u32,
                                    0,
                                    0..primitive.mirror_instance_count,
                                );
                            }
                            None => opaque_render_pass.draw(
                                0..primitive.vertex_count as u32,
                                0..primitive.mirror_instance_count,
                            ),
                        }
                    }
                }

                if let Some(skybox_bind_group) = self.skybox_bind_group.as_ref() {
                    opaque_render_pass.set_pipeline(&self.skybox_pipeline);
                    opaque_render_pass.set_bind_group(1, skybox_bind_group, &[]);
                    opaque_render_pass.draw(0..3, 0..1);
                }
            }

            {
                let mut transparent_render_pass =
                    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Transparent render pass"),
                        color_attachments: &[
                            None,
                            Some(wgpu::RenderPassColorAttachment {
                                view: &self.accumulation_attachment,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                    store: wgpu::StoreOp::Store,
                                },
                            }),
                            Some(wgpu::RenderPassColorAttachment {
                                view: &self.revealage_attachment,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                                    store: wgpu::StoreOp::Store,
                                },
                            }),
                        ],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &self.depth_attachment,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                transparent_render_pass.set_bind_group(0, render_bind_group, &[]);

                for PipelinePrimitives {
                    pipeline,
                    mirror_pipeline,
                    primitives,
                    ..
                } in &self.transparent_pipeline_primitives
                {
                    transparent_render_pass.set_pipeline(pipeline);

                    for primitive in primitives {
                        transparent_render_pass
                            .set_vertex_buffer(0, primitive.node_indices.slice(..));
                        transparent_render_pass.set_bind_group(1, &primitive.geometry, &[]);
                        transparent_render_pass.set_bind_group(2, &primitive.material, &[]);
                        match &primitive.vertex_indices {
                            Some(Indices {
                                buffer,
                                format,
                                count,
                            }) => {
                                transparent_render_pass.set_index_buffer(buffer.slice(..), *format);
                                transparent_render_pass.draw_indexed(
                                    0..*count as u32,
                                    0,
                                    0..primitive.instance_count,
                                );
                            }
                            None => transparent_render_pass.draw(
                                0..primitive.vertex_count as u32,
                                0..primitive.instance_count,
                            ),
                        }
                    }

                    transparent_render_pass.set_pipeline(mirror_pipeline);

                    for primitive in primitives {
                        transparent_render_pass
                            .set_vertex_buffer(0, primitive.mirror_node_indices.slice(..));
                        transparent_render_pass.set_bind_group(1, &primitive.geometry, &[]);
                        transparent_render_pass.set_bind_group(2, &primitive.material, &[]);
                        match &primitive.vertex_indices {
                            Some(Indices {
                                buffer,
                                format,
                                count,
                            }) => {
                                transparent_render_pass.set_index_buffer(buffer.slice(..), *format);
                                transparent_render_pass.draw_indexed(
                                    0..*count as u32,
                                    0,
                                    0..primitive.mirror_instance_count,
                                );
                            }
                            None => transparent_render_pass.draw(
                                0..primitive.vertex_count as u32,
                                0..primitive.mirror_instance_count,
                            ),
                        }
                    }
                }
            }

            {
                let mut compose_render_pass =
                    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Compose render pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &self.opaque_attachment,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                compose_render_pass.set_pipeline(&self.compose_pipeline);
                compose_render_pass.set_bind_group(0, &self.compose_bind_group, &[]);
                compose_render_pass.draw(0..3, 0..1);
            }

            {
                let mut brightness_render_pass =
                    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Brightness render pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &self.bloom_textures[0].0,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                brightness_render_pass.set_pipeline(&self.brightness_pipeline);
                brightness_render_pass.set_bind_group(0, &self.brightness_bind_group, &[]);
                brightness_render_pass.draw(0..3, 0..1);
            }

            let mut horizontal = false;
            for _ in 0..self.bloom_amount {
                let source = &self.bloom_textures[horizontal as usize].1;
                horizontal = !horizontal;
                let target = &self.bloom_textures[horizontal as usize].0;
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Gaussian blur render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                render_pass.set_pipeline(&self.gaussian_blur_pipeline);
                render_pass.set_bind_group(0, source, &[]);
                render_pass.draw(0..3, 0..1);
            }

            {
                let mut tone_mapping_render_pass =
                    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Tone mapping render pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &render_texture,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                tone_mapping_render_pass.set_pipeline(&self.tone_mapping_pipeline);
                tone_mapping_render_pass.set_bind_group(0, &self.tone_mapping_bind_group, &[]);
                tone_mapping_render_pass.draw(0..3, 0..1);
            }
        }
    }

    /// this method should only be used with node associated with no mesh
    fn add_mesh_to_node_unchecked(
        &mut self,
        mesh: Id<Mesh>,
        node: Id<Node>,
        resources: &Resources,
    ) {
        let mesh_instance = self.meshes.entry(mesh).or_insert_with(|| {
            let mesh = &resources.meshes[mesh];

            let node_indices = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Mesh instances vertex buffer"),
                size: NODE_INDEX_SIZE as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let mirror_node_indices = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Mesh mirror instances vertex buffer"),
                size: NODE_INDEX_SIZE as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let mut opaque_primitives = Vec::new();
            let mut transparent_primitives = Vec::new();

            for (pipeline, geometry, material) in mesh.primitives() {
                let pipeline = &resources.meshes.primitive_pipeline(*pipeline).unwrap();
                let (pipeline_primitives, primitive_ids) = match pipeline.alpha_mode() {
                    AlphaMode::Opaque | AlphaMode::Mask => {
                        (&mut self.opaque_pipeline_primitives, &mut opaque_primitives)
                    }

                    AlphaMode::Blend => (
                        &mut self.transparent_pipeline_primitives,
                        &mut transparent_primitives,
                    ),
                };

                let primitives = &mut pipeline_primitives
                    .entry(pipeline.id())
                    .or_insert_with(|| PipelinePrimitives {
                        id: pipeline.id(),
                        pipeline: pipeline.pipeline().clone(),
                        mirror_pipeline: pipeline.mirror_pipeline().clone(),
                        primitives: SparseSet::with_capacity(1),
                    })
                    .primitives;
                let id = primitives.next_id();
                primitive_ids.push((pipeline.id(), id));

                let geometry = &resources.geometries[*geometry];
                let vertex_indices = geometry.indices().clone();
                let vertex_count = geometry.vertex_count();
                let geometry = geometry.bind_group().clone();
                let material = resources.materials[*material].bind_group().clone();

                primitives.insert(Primitive {
                    id,
                    node_indices: node_indices.clone(),
                    instance_count: 0,
                    mirror_node_indices: mirror_node_indices.clone(),
                    mirror_instance_count: 0,
                    geometry,
                    vertex_indices,
                    vertex_count,
                    material,
                });
            }

            MeshInstances {
                mesh: mesh.id(),
                opaque_primitives,
                transparent_primitives,
                nodes: Vec::with_capacity(1),
                node_count: 0,
                node_indices,
                mirror_node_count: 0,
                mirror_node_indices,
            }
        });
        mesh_instance.nodes.push(node);
    }
}

impl Scene {
    /// Create an scene builder with default values.
    pub fn builder() -> SceneBuilder {
        SceneBuilder::default()
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

/// A builder for [Scene].
#[must_use]
pub struct SceneBuilder {}

impl Default for SceneBuilder {
    fn default() -> Self {
        Self {}
    }
}

impl SceneBuilder {
    /// Build the scene using the provided engine.
    pub fn build(self, _engine: &mut Engine) -> Scene {
        todo!()
    }
}

fn create_render_attachments(
    width: u32,
    height: u32,
    bloom_amount: usize,
    resources: &mut Resources,
    encoder: &mut wgpu::CommandEncoder,
) -> (
    [wgpu::TextureView; 4],
    [(wgpu::TextureView, wgpu::BindGroup); 2],
    [wgpu::BindGroup; 3],
) {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let opaque_attachment = TextureBuilder::default()
        .name("Opaque render attachment")
        .empty(size, wgpu::TextureFormat::Rgba16Float)
        .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
        .build(resources, encoder)
        .create_view(&wgpu::TextureViewDescriptor {
            label: Some("Opaque render attachment"),
            ..Default::default()
        });

    let accumulation_attachment = TextureBuilder::default()
        .name("Accumulation render attachment")
        .empty(size, wgpu::TextureFormat::Rgba16Float)
        .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
        .build(resources, encoder)
        .create_view(&wgpu::TextureViewDescriptor {
            label: Some("Accumulation render attachment"),
            ..Default::default()
        });

    let revealage_attachment = TextureBuilder::default()
        .name("Revealage render attachment")
        .empty(size, wgpu::TextureFormat::R8Unorm)
        .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
        .build(resources, encoder)
        .create_view(&wgpu::TextureViewDescriptor {
            label: Some("Revealage render attachment"),
            ..Default::default()
        });

    let depth_attachment = TextureBuilder::default()
        .name("Depth render attachment")
        .empty(size, wgpu::TextureFormat::Depth24Plus)
        .usage(wgpu::TextureUsages::RENDER_ATTACHMENT)
        .build(resources, encoder)
        .create_view(&wgpu::TextureViewDescriptor {
            label: Some("Depth render attachment"),
            ..Default::default()
        });

    let compose_bind_group = resources
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compose bind group"),
            layout: &resources.compose_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&accumulation_attachment),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&revealage_attachment),
                },
            ],
        });

    let brightness_bind_group = resources
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Brightness bind group"),
            layout: &resources.brightness_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&opaque_attachment),
            }],
        });

    let mut horizontal = true;
    let bloom_textures: [(wgpu::TextureView, wgpu::BindGroup); 2] = repeat_with(|| {
        let texture = TextureBuilder::default()
            .name("Bloom texture")
            .empty(size, wgpu::TextureFormat::Rgba16Float)
            .usage(wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT)
            .build(resources, encoder)
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Bloom texture view"),
                ..Default::default()
            });
        let horizontal_buffer =
            resources
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Gaussian blur horizontal buffer"),
                    contents: bytes_of(&(horizontal as u32)),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
        horizontal = !horizontal;
        let bloom_bind_group = resources
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Bloom bind group"),
                layout: &resources.gaussian_blur_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&resources.bloom_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: horizontal_buffer.as_entire_binding(),
                    },
                ],
            });

        (texture, bloom_bind_group)
    })
    .take(2)
    .collect::<Vec<_>>()
    .try_into()
    .unwrap();

    let tone_mapping_bind_group = create_tone_mapping_bind_group(
        resources,
        &opaque_attachment,
        &bloom_textures,
        bloom_amount,
    );

    (
        [
            opaque_attachment,
            accumulation_attachment,
            revealage_attachment,
            depth_attachment,
        ],
        bloom_textures,
        [
            compose_bind_group,
            brightness_bind_group,
            tone_mapping_bind_group,
        ],
    )
}

fn create_tone_mapping_bind_group(
    resources: &mut Resources,
    opaque_texture: &wgpu::TextureView,
    bloom_textures: &[(wgpu::TextureView, wgpu::BindGroup); 2],
    bloom_amount: usize,
) -> wgpu::BindGroup {
    let final_bloom_texture = (bloom_amount % 2) as usize;

    resources
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Tone mapping bind group"),
            layout: &resources.tone_mapping_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(opaque_texture),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&resources.bloom_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &bloom_textures[final_bloom_texture].0,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&resources.bloom_sampler),
                },
            ],
        })
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
    opaque_primitives: Vec<(Id<PrimitivePipeline>, Id<Primitive>)>,
    transparent_primitives: Vec<(Id<PrimitivePipeline>, Id<Primitive>)>,
    nodes: Vec<Id<Node>>,
    node_count: usize,
    node_indices: wgpu::Buffer,
    mirror_node_count: usize,
    mirror_node_indices: wgpu::Buffer,
}

impl DenseEntry for MeshInstances {
    type Key = Mesh;

    fn id(&self) -> Id<Self::Key> {
        self.mesh
    }
}

struct PipelinePrimitives {
    id: Id<PrimitivePipeline>,
    pipeline: wgpu::RenderPipeline,
    mirror_pipeline: wgpu::RenderPipeline,
    primitives: SparseSet<Primitive>,
}

impl DenseEntry for PipelinePrimitives {
    type Key = PrimitivePipeline;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

struct Primitive {
    id: Id<Self>,
    node_indices: wgpu::Buffer,
    instance_count: u32,
    mirror_node_indices: wgpu::Buffer,
    mirror_instance_count: u32,
    geometry: wgpu::BindGroup,
    material: wgpu::BindGroup,
    vertex_indices: Option<Indices>,
    vertex_count: usize,
}

impl DenseEntry for Primitive {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}
