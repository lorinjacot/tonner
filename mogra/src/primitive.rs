use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use sparse_keyed::PrimaryMap;

use crate::{
    Context,
    geometry::{GeometryIndices, MAX_MORPH_TARGET_COUNT},
    mesh::MeshInstance,
    scene_graph::SceneGraph,
};

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct PrimitiveInstance {
    pub model_matrix: Mat4,
    pub weights: [f32; MAX_MORPH_TARGET_COUNT],
    pub joint_offset: u32,
    pub _pad: [u32; 3],
}

/// All primitives using the same geometry and the same mesh. They will be rendered by a single draw call.
struct PrimitiveBatch {
    instance_count: u32,
    instance_bound: Range<wgpu::BufferAddress>,
    vertex_count: u32,
    indices: Option<GeometryIndices>,
    primitive_bind_group: wgpu::BindGroup,
}

impl PrimitiveBatch {
    /// Renders the primitive batch using the provided render pass.
    ///
    /// The correct render pipeline must be set prior to calling this function
    /// and the `vertex_buffer` should contained the instance data in the [`PrimitiveBatch::instance_bound`] slice.
    fn render(&self, instance_buffer: &wgpu::Buffer, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_vertex_buffer(0, instance_buffer.slice(self.instance_bound.clone()));
        render_pass.set_bind_group(1, &self.primitive_bind_group, &[]);
        let instances = 0..self.instance_count;
        match &self.indices {
            Some(indices) => {
                render_pass.set_index_buffer(indices.buffer().slice(..), indices.format());
                render_pass.draw_indexed(0..indices.count() as u32, 0, instances);
            }
            None => {
                render_pass.draw(0..self.vertex_count, instances);
            }
        }
    }
}

struct PipelinePrimitiveBatches {
    pipeline: wgpu::RenderPipeline,
    batches: PrimaryMap<PrimitiveBatch>,
}

impl PipelinePrimitiveBatches {
    /// Renders all primitives using this pipeline to the provided render pass.
    /// The `instance_buffer` must already contain the instance data.
    fn render(&self, instance_buffer: &wgpu::Buffer, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_pipeline(&self.pipeline);
        self.batches
            .values()
            .for_each(|batch| batch.render(instance_buffer, render_pass));
    }
}

pub struct PreparedPrimitives {
    instance_buffer: wgpu::Buffer,
    opaque_batches: PipelinePrimitiveBatches,
    transparent_batches: PipelinePrimitiveBatches,
}

impl PreparedPrimitives {
    /// Prepares the primitives for rendering.
    ///
    /// This methods splits the primitives into opaque and transparent batches
    /// and writes the instance data to the `vertex_buffer` provided.
    /// If `vertex_buffer` is not large enough, this function allocates a larger using the provided `ctx`.
    ///
    /// # Panics
    /// This function will panic if any of the [`MeshInstance::node()`] are not found in the `scene_graph`.
    pub fn from_mesh_instances<'a, M>(
        mesh_instances: M,
        scene_graph: &SceneGraph,
        vertex_buffer: &mut wgpu::Buffer,
        ctx: &Context,
    ) -> Self
    where
        M: IntoIterator<Item = &'a MeshInstance>,
    {
        let _ = mesh_instances.into_iter();
        let _ = scene_graph;
        let _ = vertex_buffer;
        let _ = ctx;
        todo!()
    }

    /// Renders all opaque primitives to the provided render pass.
    pub fn render_opaque(&self, render_pass: &mut wgpu::RenderPass) {
        self.opaque_batches
            .render(&self.instance_buffer, render_pass);
    }

    /// Renders all transparent primitives to the provided render pass.
    pub fn render_transparent(&self, render_pass: &mut wgpu::RenderPass) {
        self.transparent_batches
            .render(&self.instance_buffer, render_pass);
    }
}
