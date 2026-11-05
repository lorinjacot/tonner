use std::collections::HashMap;

use sparse_keyed::PrimaryMap;
use thiserror::Error;

use crate::{Context, NodeId, geometry::GeometryIndices, primitive::PrimitiveInstance};

/// A mesh represents a 3D object instance in a scene.
///
/// A mesh is composed by
/// - a transform describing the mesh position, rotation and scale in the scene;
/// - a collection of geometry-material pairs called primitives;
/// - (optionally) weights to deform the mesh geometry (for example to animate a character);
/// - (optionally) a skeleton used for skinning the mesh.
#[derive(Debug, Clone)]
pub struct MeshInstance {
    node: NodeId,
    vertex_data: PrimitiveInstance,
    morph_target_count: usize,
}

impl MeshInstance {
    /// Returns the node ID of the mesh instance.
    pub fn node(&self) -> NodeId {
        self.node
    }

    pub fn weights(&self) -> &[f32] {
        &self.vertex_data.weights[..self.morph_target_count]
    }
}

/// Error when building a mesh with [`MeshBuilder::build()`].
#[derive(Debug, Clone, Error)]
pub enum MeshBuilderError {}

/// All primitives using the same geometry and the same mesh. They will be rendered by a single draw call.
#[derive(Debug, Clone)]
struct PrimitiveBatch {
    /// `true` if `vertex_buffer` needs to be recreated from `instances`.
    dirty: bool,
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
    indices: Option<GeometryIndices>,
    instances: PrimaryMap<PrimitiveInstance>,
    primitive_bind_group: wgpu::BindGroup,
}

impl PrimitiveBatch {
    /// Prepares the primitives for rendering.
    ///
    /// This methods synchronizes the GPU buffers with the CPU states if required.
    fn prepare<'a>(&'a mut self, ctx: &Context) -> PreparedPrimitiveBatch<'a> {
        if self.dirty {
            self.dirty = false;
            _ = ctx;
            todo!()
        }

        PreparedPrimitiveBatch {
            primitive_batch: self,
        }
    }
}

/// All primitives using the same geometry and the same mesh. They will be rendered by a single draw call.
///
/// This struct is returned by [PrimitiveBatch::prepare] and is used to render the primitives.
struct PreparedPrimitiveBatch<'a> {
    primitive_batch: &'a PrimitiveBatch,
}

impl<'a> PreparedPrimitiveBatch<'a> {
    /// Renders the primitive batch using the provided render pass. The correct render pipeline must be set prior to calling this function.
    fn render(&self, render_pass: &mut wgpu::RenderPass) {
        let batch = self.primitive_batch;
        render_pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
        render_pass.set_bind_group(1, &batch.primitive_bind_group, &[]);
        let instances = 0..batch.instances.len() as u32;
        match &batch.indices {
            Some(indices) => {
                render_pass.set_index_buffer(indices.buffer().slice(..), indices.format());
                render_pass.draw_indexed(0..indices.count() as u32, 0, instances);
            }
            None => {
                render_pass.draw(0..batch.vertex_count, instances);
            }
        }
    }
}

/// All primitives using the same render pipeline. Primitives are grouped by batches, see [PrimitiveBatch].
#[derive(Debug, Clone)]
struct PipelinePrimitiveBatches {
    pipeline: wgpu::RenderPipeline,
    batches: PrimaryMap<PrimitiveBatch>,
}

impl PipelinePrimitiveBatches {
    /// Prepares the primitive for rendering.
    ///
    /// This methods synchronizes the GPU buffer with the CPU states if required.
    fn prepare<'a>(&'a mut self, ctx: &Context) -> PreparedPipelinePrimitiveBatches<'a> {
        let batches = self
            .batches
            .values_mut()
            .map(|batch| batch.prepare(ctx))
            .collect();
        PreparedPipelinePrimitiveBatches {
            pipeline: &self.pipeline,
            batches,
        }
    }
}

/// All primitives using the same render pipeline. Primitives are grouped by batches, see [PrimitiveBatch].
///
/// This struct is returned by [PipelinePrimitiveBatches::prepare] and is used to render the primitives.
struct PreparedPipelinePrimitiveBatches<'a> {
    pipeline: &'a wgpu::RenderPipeline,
    batches: Vec<PreparedPrimitiveBatch<'a>>,
}

impl<'a> PreparedPipelinePrimitiveBatches<'a> {
    /// Renders all primitives using this pipeline to the provided render pass.
    fn render(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_pipeline(&self.pipeline);
        self.batches
            .iter()
            .for_each(|batch| batch.render(render_pass));
    }
}

#[derive(Debug, Clone)]
pub(super) struct MeshManager {
    instances: HashMap<NodeId, MeshInstance>,
}

impl MeshManager {
    pub fn new(ctx: &Context) -> Self {
        let _ = ctx;
        MeshManager {
            instances: HashMap::new(),
        }
    }

    pub fn get(&self, node: NodeId) -> Option<&MeshInstance> {
        self.instances.get(&node)
    }

    pub fn set_weights(&mut self, node: NodeId, weights: &[f32]) {
        let mesh = self.instances.get_mut(&node).expect("no such mesh");
        mesh.vertex_data.weights[..mesh.morph_target_count].copy_from_slice(weights);
    }

    pub fn remove(&mut self, node: NodeId) -> Option<MeshInstance> {
        self.instances.remove(&node)
    }
}
