use std::ops::Range;

use glam::Mat4;

use super::{
    buffer::BufferManager,
    material::MaterialManager,
    mesh::{Mesh, MeshManager, PrimitivePipeline},
    storage::{Id, SparseMap, SparseSet},
    texture::TextureManager,
    Asset,
};

pub struct SceneManager {
    scenes: SparseSet<Scene>,
}

impl SceneManager {
    pub fn new() -> Self {
        let scenes = SparseSet::new();

        SceneManager { scenes }
    }

    pub fn create_scene(
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
        let mut scene = Scene {
            nodes: SparseSet::new(),
            primitives: SparseMap::new(),
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

        self.scenes.push(scene)
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
}

// impl Scene {
//     pub fn render(&self, render_pass: &mut wgpu::RenderPass) {
//         for (pipeline, nodes_primitives) in &self.primitives {
//             render_pass.set_pipeline(pipeline);
//             for (_, primitive, nodes) in nodes_primitives {
//                 render_pass.set_bind_group(1, &primitive.material, &[]);
//                 for (slot, vertex_buffer) in primitive.vertex_buffers.iter().enumerate() {
//                     render_pass.set_vertex_buffer(slot as u32, vertex_buffer.slice(..));
//                 }

//                 let node_count = nodes.len() as u32;
//                 match &primitive.indices {
//                     Some(indices) => {
//                         render_pass.set_index_buffer(indices.0.slice(indices.1.clone()), indices.2);
//                         render_pass.draw_indexed(0..primitive.vertex_count, 0, 0..node_count);
//                     }
//                     None => render_pass.draw(0..primitive.vertex_count, 0..node_count),
//                 }
//             }
//         }
//     }
// }

pub struct Node {
    local_transform: Mat4,
    global_transform: Mat4,
    parent: Option<Id<Node>>,
    children: Vec<Id<Node>>,
}
