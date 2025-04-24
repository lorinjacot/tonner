mod controls;

use std::{
    iter::{once, repeat_n},
    ops::{Index, IndexMut, Range},
};

use bytemuck::{cast_slice, Pod, Zeroable};
use controls::{Controls, OrbitControls};
use glam::{Mat3, Mat4, Quat, Vec3};

use super::{
    buffer::BufferManager,
    material::MaterialManager,
    math,
    mesh::{Mesh, MeshManager, PrimitivePipeline},
    storage::{Entry, Id, Iter, SparseMap, SparseSet},
    texture::{EnvironmentMap, TextureManager},
    Asset, Name,
};

pub struct SceneManager {
    scenes: SparseSet<Scene>,
    camera_bind_group_layout: wgpu::BindGroupLayout,
    assets: SparseMap<Asset, Vec<Option<Id<Scene>>>>,
}

impl SceneManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let scenes = SparseSet::new();
        let assets = SparseMap::new();

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        SceneManager {
            scenes,
            camera_bind_group_layout,
            assets,
        }
    }

    pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.camera_bind_group_layout
    }

    pub fn get(&self, scene: Id<Scene>) -> Option<&Scene> {
        self.scenes.get(scene)
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
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<Scene> {
        match self.assets.entry(asset).or_default().get(scene.index()) {
            Some(Some(id)) => *id,
            _ => self.create_scene(
                asset, scene, buffers, textures, materials, meshes, device, queue,
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
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<Scene> {
        let label = gltf_scene
            .name()
            .map_or_else(|| gltf_scene.index().to_string(), str::to_string);
        let mut scene = Scene {
            label,
            nodes: SparseSet::with_capacity(gltf_scene.nodes().count() + 2),
            root_nodes: Vec::with_capacity(gltf_scene.nodes().count() + 2),
            primitives: SparseMap::new(),
            cameras: SparseMap::new(),
            active_camera: None,
            environment_map: None,
            controls: SparseSet::with_capacity(1),
            camera_bind_group_layout: self.camera_bind_group_layout.clone(),
        };

        for node in gltf_scene.nodes() {
            create_node(
                asset, node, None, &mut scene, buffers, textures, materials, meshes, device, queue,
            );
        }

        let target = scene.node_builder().name("Orbit camera target").build();
        let cursor = scene.node_builder().name("Orbit camera cursor").build();
        let camera = scene
            .node_builder()
            .name("Orbit camera node")
            .translation(1.5 * Vec3::Z)
            .build();
        scene.create_camera(
            Some("Orbit camera"),
            Projection::Perspective {
                aspect_ratio: None,
                y_fov: f32::to_radians(90.0),
                z_far: Some(100.0),
                z_near: 0.01,
            },
            // Projection::Orthographic {
            //     x_mag: 0.5,
            //     y_mag: 0.5,
            //     z_far: 0.01,
            //     z_near: 10.0,
            // },
            camera,
            device,
        );
        scene.active_camera = Some(camera);
        scene
            .controls
            .push(OrbitControls::new(target, cursor, camera, &scene.nodes).into());

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

    pub fn iter(&self) -> Iter<'_, Scene, Scene> {
        self.scenes.iter()
    }
}

impl Index<Id<Scene>> for SceneManager {
    type Output = Scene;

    fn index(&self, index: Id<Scene>) -> &Self::Output {
        &self.scenes[index]
    }
}

impl IndexMut<Id<Scene>> for SceneManager {
    fn index_mut(&mut self, index: Id<Scene>) -> &mut Self::Output {
        &mut self.scenes[index]
    }
}

fn create_node(
    asset: Id<Asset>,
    gltf_node: gltf::Node,
    parent: Option<Id<Node>>,
    scene: &mut Scene,
    buffers: &mut BufferManager,
    textures: &mut TextureManager,
    materials: &mut MaterialManager,
    meshes: &mut MeshManager,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Id<Node> {
    let mut builder = scene.node_builder();
    builder.name = gltf_node.name();
    builder.parent = parent;
    match gltf_node.transform() {
        gltf::scene::Transform::Decomposed {
            translation,
            rotation,
            scale,
        } => {
            builder.translation(translation);
            builder.rotation(Quat::from_array(rotation));
            builder.scale(scale);
        }
        gltf::scene::Transform::Matrix { matrix } => {
            builder.local_matrix(Mat4::from_cols_array_2d(&matrix));
        }
    }
    let node_id = builder.build();

    if let Some(mesh) = gltf_node.mesh() {
        let mesh_id = meshes.load_mesh(asset, mesh, buffers, textures, materials, device, queue);
        let mesh = &meshes[mesh_id];
        for (pipeline, primitives) in &mesh.primitives {
            match scene
                .primitives
                .entry(pipeline)
                .or_insert_with(|| (meshes[pipeline].clone(), SparseMap::new()))
                .1
                .entry(mesh_id)
            {
                Entry::Occupied(entry) => {
                    let (_, nodes, buffer) = entry.into_mut();
                    nodes.push(node_id);
                    *buffer = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("Node transforms buffer"),
                        size: buffer.size() + size_of::<Transform>() as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                }
                Entry::Vacant(entry) => {
                    entry.insert((
                        primitives
                            .iter()
                            .map(|primitive| Primitive::new(primitive, buffers, materials))
                            .collect(),
                        vec![node_id],
                        device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("Node transforms buffer"),
                            size: size_of::<Transform>() as u64,
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        }),
                    ));
                }
            }
        }
    }

    if let Some(camera) = gltf_node.camera() {
        let projection = Projection::from(camera.projection());
        scene.create_camera(camera.name(), projection, node_id, device);
    }

    let children: Vec<_> = gltf_node
        .children()
        .map(|child| {
            create_node(
                asset,
                child,
                Some(node_id),
                scene,
                buffers,
                textures,
                materials,
                meshes,
                device,
                queue,
            )
        })
        .collect();

    let node = &mut scene.nodes[node_id];
    node.children = children;

    node_id
}

pub struct Scene {
    pub label: String,
    nodes: SparseSet<Node>,
    root_nodes: Vec<Id<Node>>,
    primitives: SparseMap<
        PrimitivePipeline,
        (
            wgpu::RenderPipeline,
            SparseMap<Mesh, (Vec<Primitive>, Vec<Id<Node>>, wgpu::Buffer)>,
        ),
    >,
    pub cameras: SparseMap<Node, Camera>,
    pub active_camera: Option<Id<Node>>,
    pub environment_map: Option<Id<EnvironmentMap>>,
    controls: SparseSet<Controls>,
    camera_bind_group_layout: wgpu::BindGroupLayout,
}

impl Scene {
    pub fn root_nodes(&self) -> &Vec<Id<Node>> {
        &self.root_nodes
    }

    pub fn camera(&self, id: Id<Node>) -> Option<&Camera> {
        self.cameras.get(id)
    }

    pub fn aspect_ratio(&self) -> Option<f32> {
        match self.cameras[self.active_camera?].projection {
            Projection::Perspective { aspect_ratio, .. } => aspect_ratio,
            Projection::Orthographic { x_mag, y_mag, .. } => Some(x_mag / y_mag),
        }
    }

    pub fn node_builder(&mut self) -> NodeBuilder {
        NodeBuilder {
            scene: self,
            name: None,
            parent: None,
            transform: math::Transform::IDENTITY,
        }
    }

    pub fn create_camera(
        &mut self,
        name: Option<&str>,
        projection: Projection,
        node: Id<Node>,
        device: &wgpu::Device,
    ) {
        let name = Name::from_name_or_else(|| &self.nodes[node].name, name);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{name} camera buffer")),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{name} camera bind group")),
            layout: &self.camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        self.cameras.insert(
            node,
            Camera {
                name,
                projection,
                buffer,
                bind_group,
            },
        );
    }

    pub fn take_input(&mut self, inputs: &mut egui::InputState, viewport_size: egui::Vec2) {
        for controls in self.controls.values_mut() {
            controls
                .0
                .take_input(inputs, viewport_size, &self.nodes, &self.cameras);
        }
    }

    pub fn update(&mut self, viewport_aspect_ratio: f32, queue: &wgpu::Queue) {
        for controls in self.controls.values_mut() {
            controls
                .0
                .update(viewport_aspect_ratio, &mut self.nodes, &mut self.cameras);
        }

        for (_, mesh_map) in self.primitives.values() {
            for (_, nodes, buffer) in mesh_map.values() {
                let transforms: Vec<_> = nodes
                    .iter()
                    .map(|node| {
                        let world_matrix = self.nodes[*node].world_matrix;
                        Transform {
                            point: world_matrix.to_cols_array(),
                            vector: Mat3::from_mat4(world_matrix.inverse())
                                .transpose()
                                .to_cols_array(),
                        }
                    })
                    .collect();
                queue.write_buffer(buffer, 0, cast_slice(&transforms));
            }
        }

        if let Some(id) = self.active_camera {
            let node = &self.nodes[id];
            let camera = &self.cameras[id];

            let position = node.world_matrix.transform_point3(Vec3::ZERO);
            let view = Mat4::look_to_rh(
                position,
                node.world_matrix.transform_vector3(-Vec3::Z),
                node.world_matrix.transform_vector3(Vec3::Y),
            );
            let projection = camera.projection_matrix(viewport_aspect_ratio);

            let data = CameraUniform {
                view_projection: (projection * view).to_cols_array(),
                world_position: position.to_array(),
                _padding: [0.0; 1],
            };

            queue.write_buffer(&camera.buffer, 0, cast_slice(&[data]));
        }
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass) {
        let camera = match self
            .active_camera
            .map(|camera| self.cameras.get(camera))
            .flatten()
        {
            Some(camera) => &camera.bind_group,
            None => return,
        };

        for (pipeline, by_mesh) in self.primitives.values() {
            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, camera, &[]);
            for (primitives, nodes, transforms) in by_mesh.values() {
                let node_count = nodes.len() as u32;
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

pub struct Node {
    pub name: Name,
    local_transform: math::Transform,
    world_matrix: Mat4,
    parent: Option<Id<Node>>,
    children: Vec<Id<Node>>,
}

impl Node {
    pub fn children(&self) -> &Vec<Id<Node>> {
        &self.children
    }

    pub fn local_matrix(&self) -> Mat4 {
        self.local_transform.matrix()
    }

    pub fn world_matrix(&self) -> Mat4 {
        self.world_matrix
    }

    pub fn local_position(&self) -> Vec3 {
        self.local_transform.position()
    }

    pub fn local_transform(&self) -> math::Transform {
        self.local_transform
    }
}

impl SparseSet<Node> {
    fn update_global_matrix(&mut self, node: Id<Node>, parent: Mat4) {
        let node = &mut self[node];
        let world_matrix = parent * node.local_matrix();
        node.world_matrix = world_matrix;
        for child in node.children.to_vec() {
            self.update_global_matrix(child, world_matrix);
        }
    }

    fn set_local_transform(&mut self, node: Id<Node>, transform: math::Transform) {
        let parent = match self[node].parent {
            Some(parent) => self[parent].world_matrix,
            None => Mat4::IDENTITY,
        };
        let node = &mut self[node];
        node.local_transform = transform;
        let world_matrix = parent * node.local_matrix();
        node.world_matrix = world_matrix;
        for child in node.children.to_vec() {
            self.update_global_matrix(child, world_matrix);
        }
    }
}

pub struct NodeBuilder<'a> {
    scene: &'a mut Scene,
    name: Option<&'a str>,
    parent: Option<Id<Node>>,
    transform: math::Transform,
}

impl<'a> NodeBuilder<'a> {
    pub fn name(&mut self, name: &'a str) -> &mut Self {
        self.name = Some(name);
        self
    }

    pub fn scale(&mut self, scale: impl Into<Vec3>) -> &mut Self {
        self.transform.set_scale(scale);
        self
    }

    pub fn rotation(&mut self, rotation: impl Into<Quat>) -> &mut Self {
        self.transform.set_rotation(rotation);
        self
    }

    pub fn translation(&mut self, translation: impl Into<Vec3>) -> &mut Self {
        self.transform.set_position(translation);
        self
    }

    pub fn local_matrix(&mut self, local_matrix: impl Into<Mat4>) -> &mut Self {
        self.transform.set_matrix(local_matrix);
        self
    }

    pub fn build(&mut self) -> Id<Node> {
        let world_matrix = match self.parent {
            Some(parent_id) => self.scene.nodes[parent_id].world_matrix * self.transform.matrix(),
            None => self.transform.matrix(),
        };
        let name = Name::from_name_or_else(|| self.scene.nodes.next_id(), self.name);
        let id = self.scene.nodes.push(Node {
            name,
            local_transform: self.transform,
            world_matrix,
            parent: self.parent,
            children: Vec::new(),
        });
        match self.parent {
            Some(parent) => self.scene[parent].children.push(id),
            None => self.scene.root_nodes.push(id),
        }
        id
    }
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Transform {
    point: [f32; 16],
    vector: [f32; 9],
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct CameraUniform {
    view_projection: [f32; 16],
    world_position: [f32; 3],
    _padding: [f32; 1],
}

struct Primitive {
    indices: Option<(wgpu::Buffer, Range<u64>, wgpu::IndexFormat)>,
    vertex_buffers: Vec<wgpu::Buffer>,
    vertex_count: u32,
    material: wgpu::BindGroup,
}

impl Primitive {
    fn new(
        primitive: &super::mesh::Primitive,
        buffers: &BufferManager,
        materials: &MaterialManager,
    ) -> Self {
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
    }
}

pub struct Camera {
    pub name: Name,
    projection: Projection,
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl Camera {
    fn projection_matrix(&self, viewport_aspect_ratio: f32) -> Mat4 {
        match self.projection {
            Projection::Orthographic {
                x_mag,
                y_mag,
                z_far,
                z_near,
                zoom,
            } => Mat4::orthographic_rh(
                -x_mag / zoom,
                x_mag / zoom,
                -y_mag / zoom,
                y_mag / zoom,
                z_near,
                z_far,
            ),
            Projection::Perspective {
                aspect_ratio,
                y_fov,
                z_far: Some(z_far),
                z_near,
            } => Mat4::perspective_rh(
                y_fov,
                aspect_ratio.unwrap_or(viewport_aspect_ratio),
                z_near,
                z_far,
            ),
            Projection::Perspective {
                aspect_ratio,
                y_fov,
                z_far: None,
                z_near,
            } => Mat4::perspective_infinite_rh(
                y_fov,
                aspect_ratio.unwrap_or(viewport_aspect_ratio),
                z_near,
            ),
        }
    }
}

pub enum Projection {
    Orthographic {
        x_mag: f32,
        y_mag: f32,
        z_far: f32,
        z_near: f32,
        zoom: f32,
    },
    Perspective {
        aspect_ratio: Option<f32>,
        y_fov: f32,
        z_far: Option<f32>,
        z_near: f32,
    },
}

impl<'a> From<gltf::camera::Projection<'a>> for Projection {
    fn from(value: gltf::camera::Projection) -> Self {
        match value {
            gltf::camera::Projection::Orthographic(ortho) => Self::Orthographic {
                x_mag: ortho.xmag(),
                y_mag: ortho.ymag(),
                z_far: ortho.zfar(),
                z_near: ortho.znear(),
                zoom: 1.0,
            },
            gltf::camera::Projection::Perspective(pers) => Self::Perspective {
                aspect_ratio: pers.aspect_ratio(),
                y_fov: pers.yfov(),
                z_far: pers.zfar(),
                z_near: pers.znear(),
            },
        }
    }
}
