use std::{
    iter::{once, repeat_n},
    ops::{Index, IndexMut, Range},
};

use bytemuck::{cast_slice, Pod, Zeroable};
use glam::{Mat3, Mat4, Vec3};

use super::{
    buffer::BufferManager,
    material::MaterialManager,
    mesh::{Mesh, MeshManager, PrimitivePipeline},
    storage::{Entry, Id, SparseMap, SparseSet},
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

    pub fn get(&self, scene: Id<Scene>) -> Option<&Scene> {
        self.scenes.get(scene)
    }

    pub fn get_mut(&mut self, scene: Id<Scene>) -> Option<&mut Scene> {
        self.scenes.get_mut(scene)
    }

    pub fn len(&self) -> usize {
        self.scenes.len()
    }

    pub fn load_scene(
        &mut self,
        asset: Id<Asset>,
        scene: gltf::Scene,
        viewport_aspect_ratio: f32,
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
                asset,
                scene,
                viewport_aspect_ratio,
                buffers,
                textures,
                materials,
                meshes,
                device,
                queue,
            ),
        }
    }

    fn create_scene(
        &mut self,
        asset: Id<Asset>,
        gltf_scene: gltf::Scene,
        viewport_aspect_ratio: f32,
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
            nodes: SparseSet::with_capacity(gltf_scene.nodes().count()),
            root_nodes: Vec::with_capacity(gltf_scene.nodes().count()),
            primitives: SparseMap::new(),
            cameras: SparseMap::new(),
            active_camera: None,
            viewport_aspect_ratio,
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

    pub fn iter(&self) -> std::slice::Iter<'_, (Id<Scene>, Scene)> {
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
    node: gltf::Node,
    parent: Option<Id<Node>>,
    parent_transform: Mat4,
    scene: &mut Scene,
    buffers: &mut BufferManager,
    textures: &mut TextureManager,
    materials: &mut MaterialManager,
    meshes: &mut MeshManager,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Id<Node> {
    let label = node
        .name()
        .map_or_else(|| node.index().to_string(), str::to_string);
    let local_transform = Mat4::from_cols_array_2d(&node.transform().matrix());
    let global_transform = parent_transform * local_transform;

    let node_id = scene.nodes.push(Node {
        label,
        local_transform,
        global_transform,
        children: Vec::new(),
        parent,
    });

    if parent.is_none() {
        scene.root_nodes.push(node_id);
    }

    if let Some(mesh) = node.mesh() {
        let mesh_id = meshes.load_mesh(asset, mesh, buffers, textures, materials, device, queue);
        let mesh = &meshes[mesh_id];
        for (pipeline, primitives) in &mesh.primitives {
            match scene
                .primitives
                .entry(*pipeline)
                .or_insert_with(|| (meshes[*pipeline].clone(), SparseMap::new()))
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

    if let Some(camera) = node.camera() {
        let label = camera
            .name()
            .map_or_else(|| camera.index().to_string(), str::to_string);
        let projection = Projection::from(camera.projection());
        let matrix = projection.matrix(scene.viewport_aspect_ratio);

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label} camera buffer")),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{label} camera bind group")),
            layout: meshes.camera_bind_group_layout(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        let camera = Camera {
            label,
            projection,
            matrix,
            buffer,
            bind_group,
        };
        scene.cameras.insert(node_id, camera);
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
    viewport_aspect_ratio: f32,
}

impl Scene {
    pub fn root_nodes(&self) -> &Vec<Id<Node>> {
        &self.root_nodes
    }

    pub fn camera(&self, id: Id<Node>) -> Option<&Camera> {
        self.cameras.get(id)
    }

    pub fn update(&self, queue: &wgpu::Queue) {
        for (_, mesh_map) in self.primitives.values() {
            for (_, nodes, buffer) in mesh_map.values() {
                let transforms: Vec<_> = nodes
                    .iter()
                    .map(|node| {
                        let transform = self.nodes[*node].global_transform;
                        Transform {
                            point: transform.to_cols_array(),
                            vector: Mat3::from_mat4(transform.inverse())
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

            let position = node.global_transform.transform_point3(Vec3::ZERO);
            let view = Mat4::look_to_rh(
                position,
                node.global_transform.transform_vector3(Vec3::Z),
                node.global_transform.transform_vector3(Vec3::Y),
            );

            let data = CameraUniform {
                view_projection: (camera.matrix * view).to_cols_array(),
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
    pub label: String,
    local_transform: Mat4,
    global_transform: Mat4,
    parent: Option<Id<Node>>,
    children: Vec<Id<Node>>,
}

impl Node {
    pub fn children(&self) -> &Vec<Id<Node>> {
        &self.children
    }

    pub fn local_transform(&self) -> Mat4 {
        self.local_transform
    }

    pub fn global_transform(&self) -> Mat4 {
        self.global_transform
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
    pub label: String,
    projection: Projection,
    matrix: Mat4,
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

enum Projection {
    Orthographic {
        xmag: f32,
        ymag: f32,
        zfar: f32,
        znear: f32,
    },
    Perspective {
        aspect_ration: Option<f32>,
        yfov: f32,
        zfar: Option<f32>,
        znear: f32,
    },
}

impl Projection {
    fn matrix(&self, viewport_aspect_ratio: f32) -> Mat4 {
        match self {
            Projection::Orthographic {
                xmag,
                ymag,
                zfar,
                znear,
            } => Mat4::from_cols_array_2d(&[
                [1.0 / xmag, 0.0, 0.0, 0.0],
                [0.0, 1.0 / ymag, 0.0, 0.0],
                [0.0, 0.0, 2.0 / (znear - zfar), 0.0],
                [0.0, 0.0, (zfar + znear) / (znear - zfar), 0.0],
            ]),
            Projection::Perspective {
                aspect_ration,
                yfov,
                zfar,
                znear,
            } => {
                let a = aspect_ration.unwrap_or(viewport_aspect_ratio);
                let tan_y = (0.5 * yfov).tan();
                let (zz, zw) = match zfar {
                    Some(zfar) => (
                        (zfar + znear) / (znear - zfar),
                        (2.0 * zfar * znear) / (znear - zfar),
                    ),
                    None => (-1.0, -2.0 * znear),
                };
                Mat4::from_cols_array_2d(&[
                    [1.0 / (a * tan_y), 0.0, 0.0, 0.0],
                    [0.0, 1.0 / a, 0.0, 0.0],
                    [0.0, 0.0, zz, -1.0],
                    [0.0, 0.0, zw, 0.0],
                ])
            }
        }
    }
}

impl<'a> From<gltf::camera::Projection<'a>> for Projection {
    fn from(value: gltf::camera::Projection) -> Self {
        match value {
            gltf::camera::Projection::Orthographic(ortho) => Self::Orthographic {
                xmag: ortho.xmag(),
                ymag: ortho.ymag(),
                zfar: ortho.zfar(),
                znear: ortho.znear(),
            },
            gltf::camera::Projection::Perspective(pers) => Self::Perspective {
                aspect_ration: pers.aspect_ratio(),
                yfov: pers.yfov(),
                zfar: pers.zfar(),
                znear: pers.znear(),
            },
        }
    }
}
