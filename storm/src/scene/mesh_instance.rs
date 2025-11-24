use std::collections::HashMap;

use thiserror::Error;
use uuid::Uuid;

use crate::{
    asset::mesh::{Mesh, MeshPrimitive},
    geometry::{GeometryIndices, MAX_MORPH_TARGET_COUNT},
    scene::{Scene, node::NodeId, skin::SkinId},
};

/// A unique id for a mesh attached to a node. A mesh instance will always have the same id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshInstanceId(Uuid);

/// A mesh instance builder. This is used to add a mesh to a scene.
#[must_use]
pub struct MeshInstanceBuilder {
    mesh: Mesh,
    node: NodeId,
    weights: Vec<f32>,
    skin: Option<SkinId>,
}

impl MeshInstanceBuilder {
    /// Create a new mesh instance builder. This is used to add `mesh` to a scene.
    /// The global transform of `node` will be used as the mesh model transform.
    pub fn new(mesh: Mesh, node: NodeId) -> Self {
        Self {
            mesh,
            node,
            weights: Vec::new(),
            skin: None,
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
        if !scene.node_manager.contains(self.node) {
            return Err(MeshInstanceBuilderError::InvalidNode(self.node));
        }
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
            node: self.node,
            mesh: self.mesh,
            weights,
            skin: self.skin,
        };

        let manager = &mut scene.mesh_instance_manager;
        manager.meshes.insert(id, data);

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
    meshes: HashMap<MeshInstanceId, MeshInstanceData>,
    opaque_primitives: PrimitivesByPipeline,
    transparent_primitives: PrimitivesByPipeline,
}

impl MeshInstanceManager {
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

            for (primitive, instances) in primitives {
                render_pass.set_vertex_buffer(0, instances.nodes_indices_buffer.slice(..));
                render_pass.set_bind_group(1, primitive.bind_group(), &[]);
                let instances = 0..instances.nodes_indices.len() as u32;
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
    mesh: Mesh,
    weights: [f32; MAX_MORPH_TARGET_COUNT],
    skin: Option<SkinId>,
}

struct PrimitiveInstances {
    nodes_indices: Vec<u32>,
    nodes_indices_buffer: wgpu::Buffer,
}

impl PrimitiveInstances {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            nodes_indices: Vec::new(),
            nodes_indices_buffer: device.create_buffer(&wgpu::wgt::BufferDescriptor {
                label: Some("Primitive instance node indices buffer"),
                size: size_of::<u32>() as u64,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            }),
        }
    }
}

struct PrimitivesByPipeline(
    HashMap<wgpu::RenderPipeline, Vec<(MeshPrimitive, PrimitiveInstances)>>,
);
