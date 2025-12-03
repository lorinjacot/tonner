use std::{collections::HashMap, ops::Range, sync::mpsc::sync_channel};

use bytemuck::{Pod, Zeroable, cast_slice};
use glam::{Mat4, Vec4};
use thiserror::Error;
use uuid::Uuid;
use wgpu::BufferViewMut;

use crate::{
    asset::mesh::{Mesh, MeshPrimitive},
    geometry::{GeometryIndices, MAX_MORPH_TARGET_COUNT},
    material::AlphaMode,
    mesh::MeshPrimitiveId,
    node::{NodeBuilder, NodeManager},
    scene::{Scene, node::NodeId, skin::SkinId},
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
    pub fn new(mesh: Mesh) -> Self {
        Self {
            mesh,
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
            Some(node) if scene.node_manager.contains(node) => node,
            Some(node) => return Err(MeshInstanceBuilderError::InvalidNode(node)),
            None => NodeBuilder::default()
                .name(self.name.clone())
                .build(scene)
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
    staging_buffer: wgpu::Buffer,
    opaque_primitives: PrimitivesByPipeline,
    transparent_primitives: PrimitivesByPipeline,
}

impl MeshInstanceManager {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let (vertex_buffer, staging_buffer) = Self::create_buffers(0, device);

        Self {
            mesh_instances: HashMap::new(),
            vertex_buffer,
            staging_buffer,
            opaque_primitives: PrimitivesByPipeline(HashMap::new()),
            transparent_primitives: PrimitivesByPipeline(HashMap::new()),
        }
    }

    pub(super) fn update_buffer(
        &mut self,
        node_manager: &NodeManager,
        skin_manager: &SkinManager,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<(), UpdateMeshInstanceBufferError> {
        self.opaque_primitives.0.clear();
        self.transparent_primitives.0.clear();

        let mut primitive_count = 0;
        for mesh_instance in self.mesh_instances.values() {
            let model_matrix = node_manager.global_matrix(mesh_instance.node).ok_or(
                UpdateMeshInstanceBufferError::InvalidNode(mesh_instance.node),
            )?;
            let pipeline_index = model_matrix.determinant().is_sign_positive() as usize;
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

        let size = primitive_count * size_of::<PrimitiveInstanceVertex>();
        let aligned_size = wgpu::util::align_to(size as u64, wgpu::COPY_BUFFER_ALIGNMENT);
        if self.vertex_buffer.size() < size as u64 {
            (self.vertex_buffer, self.staging_buffer) = Self::create_buffers(aligned_size, device);
        } else {
            let (sender, receiver) = sync_channel(1);
            self.staging_buffer
                .map_async(wgpu::MapMode::Write, .., move |result| {
                    if let Ok(_) = result {
                        sender.send(()).unwrap();
                    }
                });
            if let Err(_) = receiver.recv() {
                self.staging_buffer = Self::create_staging_buffer(aligned_size, device);
            }
        }
        let mut view = self.staging_buffer.get_mapped_range_mut(0..size as u64);

        let mut start = 0;
        Self::update_buffer_and_bounds(&mut view, &mut start, &mut self.opaque_primitives);
        Self::update_buffer_and_bounds(&mut view, &mut start, &mut self.transparent_primitives);
        drop(view);
        self.staging_buffer.unmap();
        encoder.copy_buffer_to_buffer(&self.staging_buffer, 0, &self.vertex_buffer, 0, None);

        Ok(())
    }

    fn update_buffer_and_bounds(
        view: &mut BufferViewMut,
        start: &mut usize,
        primitives_by_pipeline: &mut PrimitivesByPipeline,
    ) {
        for pipeline in primitives_by_pipeline.0.values_mut() {
            for (_, instances) in pipeline.values_mut() {
                let end = *start + instances.data.len();
                let size = size_of::<PrimitiveInstanceVertex>();
                view[*start * size..end * size].copy_from_slice(cast_slice(&instances.data));
                instances.bounds = *start as u64..end as u64;
                *start = end;
            }
        }
    }

    fn create_buffers(size: u64, device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer) {
        (
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Mesh instance vertex buffer"),
                size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            Self::create_staging_buffer(size, device),
        )
    }

    fn create_staging_buffer(size: u64, device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Mesh instance staging buffer"),
            size,
            usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: true,
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
                let instances = 0..instances.data.len() as u32;
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

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub(crate) struct PrimitiveInstanceVertex {
    model_matrix: Mat4,
    weights_0: Vec4,
    weights_1: Vec4,
    joint_offset: u32,
    _pad: [u32; 3],
}
