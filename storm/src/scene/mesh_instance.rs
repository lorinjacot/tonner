use std::{collections::HashMap, ops::Range};

use bytemuck::{Pod, Zeroable, cast_slice};
use glam::{Mat4, Vec4};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    asset::mesh::{Mesh, MeshPrimitive},
    geometry::{GeometryIndices, MAX_MORPH_TARGET_COUNT},
    material::AlphaMode,
    mesh::MeshPrimitiveId,
    scene::{Scene, skin::SkinId},
    scene_graph::{NodeBuilder, NodeId, SceneGraph},
    skin::SkinManager,
};

/// A unique id for a mesh attached to a node. A mesh instance will always have the same id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshInstanceId(Uuid);

/// A mesh instance builder. This is used to add a mesh to a scene.
#[must_use]
pub struct MeshInstanceBuilder {
    mesh: Mesh,
    node: Option<NodeId>,
    name: String,
    weights: Vec<f32>,
    skin: Option<SkinId>,
}

impl MeshInstanceBuilder {
    /// Create a new mesh instance builder. This is used to add `mesh` to a scene.
    pub fn new(mesh: impl Into<Mesh>) -> Self {
        Self {
            mesh: mesh.into(),
            node: None,
            name: String::new(),
            weights: Vec::new(),
            skin: None,
        }
    }

    /// The global transform of `node` will be used as the mesh model transform.
    /// By default, a new root node with identity transform is created.
    pub fn node(self, node: impl Into<NodeId>) -> Self {
        Self {
            node: Some(node.into()),
            ..self
        }
    }

    /// Give a name to the mesh instance. Useful for GUI and debugging.
    pub fn name(self, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..self
        }
    }

    /// Sets the morph targets initial weights. `weights` len must match the number of morph target.
    pub fn weights(mut self, weights: impl IntoIterator<Item = f32>) -> Self {
        self.weights.extend(weights);
        self
    }

    /// Enables skinning on the mesh. `skin` will be used for the skeleton.
    pub fn skin(self, skin: SkinId) -> Self {
        Self {
            skin: Some(skin),
            ..self
        }
    }

    /// Add the mesh to the scene.
    pub fn build(self, scene: &mut Scene) -> Result<MeshInstanceId, MeshInstanceBuilderError> {
        let node = match self.node {
            Some(node) if scene.scene_graph.contains(node) => node,
            Some(node) => return Err(MeshInstanceBuilderError::InvalidNode(node)),
            None => NodeBuilder::default()
                .name(self.name.clone())
                .build(&mut scene.scene_graph)
                .unwrap(),
        };

        let morph_target_count = self.mesh.morph_target_count();
        if self.weights.len() != morph_target_count {
            return Err(MeshInstanceBuilderError::InvalidWeightsCount {
                expected: morph_target_count,
                actual: self.weights.len(),
            });
        }
        let weights = std::array::from_fn(|i| self.weights.get(i).copied().unwrap_or(0.0));
        let id = MeshInstanceId(Uuid::new_v4());
        let data = MeshInstanceData {
            id,
            node,
            mesh: self.mesh,
            name: self.name,
            weights,
            skin: self.skin,
        };

        let manager = &mut scene.mesh_instance_manager;
        manager.mesh_instances.insert(id, data);

        Ok(id)
    }
}

/// Error when [`MeshInstanceBuilder::build`] fails.
#[derive(Debug, Error)]
pub enum MeshInstanceBuilderError {
    #[error("invalid node {0}")]
    InvalidNode(NodeId),
    #[error("invalid weights count: exptected {expected} but found {actual}")]
    InvalidWeightsCount { expected: usize, actual: usize },
}

pub(super) struct MeshInstanceManager {
    mesh_instances: HashMap<MeshInstanceId, MeshInstanceData>,
    vertex_buffer: wgpu::Buffer,
    opaque_primitives: PrimitivesByPipeline,
    transparent_primitives: PrimitivesByPipeline,
}

impl MeshInstanceManager {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let vertex_buffer = Self::create_vertex_buffer(0, false, device);

        Self {
            mesh_instances: HashMap::new(),
            vertex_buffer,
            opaque_primitives: PrimitivesByPipeline(HashMap::new()),
            transparent_primitives: PrimitivesByPipeline(HashMap::new()),
        }
    }

    pub(super) fn update_buffer(
        &mut self,
        scene_graph: &SceneGraph,
        skin_manager: &SkinManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), UpdateMeshInstanceBufferError> {
        self.opaque_primitives.0.clear();
        self.transparent_primitives.0.clear();

        let mut primitive_count = 0;
        for mesh_instance in self.mesh_instances.values() {
            let model_matrix = scene_graph
                .get(mesh_instance.node)
                .ok_or(UpdateMeshInstanceBufferError::InvalidNode(
                    mesh_instance.node,
                ))?
                .global_transformation();
            let pipeline_index = model_matrix.determinant().is_sign_negative() as usize;
            let joint_offset = match mesh_instance.skin {
                Some(skin) => skin_manager
                    .index(skin)
                    .ok_or(UpdateMeshInstanceBufferError::InvalidSkin(skin))?,
                None => 0,
            };
            let data = PrimitiveInstanceVertex {
                model_matrix,
                weights_0: Vec4::from_slice(&mesh_instance.weights[0..4]),
                weights_1: Vec4::from_slice(&mesh_instance.weights[4..8]),
                joint_offset,
                _pad: [0; 3],
            };

            for primitive in mesh_instance.mesh.primitives() {
                primitive_count += 1;
                let primitves = match primitive.alpha_mode() {
                    AlphaMode::Opaque | AlphaMode::Mask => &mut self.opaque_primitives,
                    AlphaMode::Blend => &mut self.transparent_primitives,
                };
                let pipeline = primitive.render_pipelines()[pipeline_index].clone();
                primitves
                    .0
                    .entry(pipeline)
                    .or_default()
                    .entry(primitive.id())
                    .or_insert_with(|| {
                        (
                            primitive.clone(),
                            PrimitiveInstances {
                                count: 0,
                                bounds: 0..0,
                                data: Vec::with_capacity(1),
                            },
                        )
                    })
                    .1
                    .data
                    .push(data);
            }
        }

        let mut data = Vec::with_capacity(primitive_count);
        let size = size_of::<PrimitiveInstanceVertex>();
        self.opaque_primitives
            .0
            .values_mut()
            .chain(self.transparent_primitives.0.values_mut())
            .for_each(|primitives| {
                for (_, instances) in primitives.values_mut() {
                    instances.count = instances.data.len() as u32;
                    let start = data.len() * size;
                    data.append(&mut instances.data);
                    let end = data.len() * size;
                    instances.bounds = start as u64..end as u64;
                }
            });

        let size = data.len() * size_of::<PrimitiveInstanceVertex>();
        let aligned_size = wgpu::util::align_to(size as u64, wgpu::COPY_BUFFER_ALIGNMENT);
        if self.vertex_buffer.size() < size as u64 {
            self.vertex_buffer = Self::create_vertex_buffer(aligned_size, true, device);
            let mut view = self.vertex_buffer.get_mapped_range_mut(..);
            view[..size].copy_from_slice(cast_slice(&data));
            drop(view);
            self.vertex_buffer.unmap();
        } else {
            queue.write_buffer(&self.vertex_buffer, 0, cast_slice(&data));
        }

        Ok(())
    }

    fn create_vertex_buffer(
        size: u64,
        mapped_at_creation: bool,
        device: &wgpu::Device,
    ) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Mesh instance vertex buffer"),
            size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation,
        })
    }

    /// Render all opaque primitives using the provided render pass. The render pass must have a 3 colors attachments:
    /// - `vec4<f32>`-compatible;
    /// - not used;
    /// - not used.
    pub(super) fn render_opaque_primitives(&self, opaque_render_pass: &mut wgpu::RenderPass) {
        self.render_primitives(&self.opaque_primitives, opaque_render_pass);
    }

    /// Render all transparent primitives using the provided render pass. The render pass must have a 3 colors attachments:
    /// - not used;
    /// - `vec4<f32>`-compatible;
    /// - `f32`-compatible.
    pub(super) fn render_transparent_primitives(
        &self,
        transparent_render_pass: &mut wgpu::RenderPass,
    ) {
        self.render_primitives(&self.transparent_primitives, transparent_render_pass);
    }

    /// Primitive rendering logic. Used by [`Self::render_opaque_primitives`] and
    /// [`Self::render_transparent_primitives`].
    fn render_primitives(
        &self,
        primitives: &PrimitivesByPipeline,
        render_pass: &mut wgpu::RenderPass,
    ) {
        for (pipeline, primitives) in &primitives.0 {
            render_pass.set_pipeline(pipeline);

            for (primitive, instances) in primitives.values() {
                render_pass
                    .set_vertex_buffer(0, self.vertex_buffer.slice(instances.bounds.clone()));
                render_pass.set_bind_group(1, primitive.bind_group(), &[]);
                let instances = 0..instances.count;
                match primitive.indices() {
                    Some(GeometryIndices {
                        buffer,
                        format,
                        count,
                    }) => {
                        render_pass.set_index_buffer(buffer.slice(..), *format);
                        render_pass.draw_indexed(0..*count as u32, 0, instances);
                    }
                    None => {
                        render_pass.draw(0..primitive.vertex_count() as u32, instances);
                    }
                }
            }
        }
    }
}

struct MeshInstanceData {
    id: MeshInstanceId,
    node: NodeId,
    name: String,
    mesh: Mesh,
    weights: [f32; MAX_MORPH_TARGET_COUNT],
    skin: Option<SkinId>,
}

struct PrimitiveInstances {
    count: u32,
    data: Vec<PrimitiveInstanceVertex>,
    bounds: Range<u64>,
}

#[derive(Debug, Error)]
pub(super) enum UpdateMeshInstanceBufferError {
    #[error("invalid node: {0}")]
    InvalidNode(NodeId),
    #[error("invalid skin: {0}")]
    InvalidSkin(SkinId),
}

struct PrimitivesByPipeline(
    HashMap<wgpu::RenderPipeline, HashMap<MeshPrimitiveId, (MeshPrimitive, PrimitiveInstances)>>,
);

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub(crate) struct PrimitiveInstanceVertex {
    model_matrix: Mat4,
    weights_0: Vec4,
    weights_1: Vec4,
    joint_offset: u32,
    _pad: [u32; 3],
}
