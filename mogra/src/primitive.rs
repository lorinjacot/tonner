use std::{
    collections::HashMap,
    ops::Range,
    sync::mpsc::{Receiver, Sender, channel},
};

use bytemuck::{Pod, Zeroable, cast_slice};
use glam::Mat4;
use wgpu::BufferViewMut;

use crate::{
    Context, geometry::MAX_MORPH_TARGET_COUNT, mesh::MeshInstance, scene_graph::SceneGraph,
};

const DEFAULT_INSTANCE_BUFFER_SIZE: wgpu::BufferAddress =
    8 * size_of::<PrimitiveInstance>() as wgpu::BufferAddress;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AlphaMode {
    /// The rendered material is fully opaque and any `alpha` value is ignored.
    Opaque = 0,
    /// The rendered material is either fully opaque or fully transparent depending
    /// on the alpha value and the specified `alpha_cutoff` value.
    Mask = 1,
    /// The rendered material is combined with the background using the specified
    /// `alpha` value as transparency
    Blend = 2,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Primitive {
    alpha_mode: AlphaMode,

    /// - vertex count for unindexed geometries
    /// - indices buffer, format and count for indexed geometries
    indices: Indices,

    /// Bind group containing all geometry and material data.
    bind_group: wgpu::BindGroup,

    /// Pipeline for this primitive. The first should be used when the model matrix
    /// has a positive determinant, and the second one is for negative determinant.
    pipelines: [wgpu::RenderPipeline; 2],
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct PrimitiveInstance {
    pub model_matrix: Mat4,
    pub weights: [f32; MAX_MORPH_TARGET_COUNT],
    pub joint_offset: u32,
    pub _pad: [u32; 3],
}

/// All instances of the same [Primitive]. They will be rendered by a single draw call.
#[derive(Debug)]
struct PrimitiveBatch {
    /// Number of Primitive instances in the batch.
    instance_count: u32,

    /// Location of the [PrimitiveInstance]s in the vertex buffer.
    instance_bound: Range<wgpu::BufferAddress>,

    /// - vertex count for unindexed geometries
    /// - indices buffer, format and count for indexed geometries
    indices: Indices,

    /// Bind group containing all geometry and material data.
    bind_group: wgpu::BindGroup,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Indices {
    None {
        vertex_count: u32,
    },
    Some {
        buffer: wgpu::Buffer,
        format: wgpu::IndexFormat,
        index_count: u32,
    },
}

impl PrimitiveBatch {
    /// Renders the primitive batch using the provided render pass.
    ///
    /// The correct render pipeline must be set prior to calling this function
    /// and the `instance_buffer` should contained the instance data in the [`PrimitiveBatch::instance_bound`] slice.
    fn render(&self, instance_buffer: &wgpu::Buffer, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_vertex_buffer(0, instance_buffer.slice(self.instance_bound.clone()));
        render_pass.set_bind_group(1, &self.bind_group, &[]);
        let instances = 0..self.instance_count;
        match &self.indices {
            Indices::None { vertex_count } => {
                render_pass.draw(0..*vertex_count, instances);
            }
            Indices::Some {
                buffer,
                format,
                index_count,
            } => {
                render_pass.set_index_buffer(buffer.slice(..), *format);
                render_pass.draw_indexed(0..*index_count, 0, instances);
            }
        }
    }
}

struct PrimitiveBatchBuilder {
    instances: Vec<PrimitiveInstance>,

    /// - vertex count for unindexed geometries
    /// - indices buffer, format and count for indexed geometries
    indices: Indices,

    /// Bind group containing all geometry and material data.
    bind_group: wgpu::BindGroup,
}

impl PrimitiveBatchBuilder {
    fn new(primitive: &Primitive) -> Self {
        let indices = primitive.indices.clone();
        let bind_group = primitive.bind_group.clone();

        PrimitiveBatchBuilder {
            instances: Vec::new(),
            indices,
            bind_group,
        }
    }

    fn add(&mut self, instance: PrimitiveInstance) {
        self.instances.push(instance);
    }

    fn build(self, view: &mut BufferViewMut, offset: wgpu::BufferAddress) -> PrimitiveBatch {
        let start = offset as usize;
        let end = start + self.instances.len() * size_of::<PrimitiveInstance>();
        view.slice(start..end)
            .copy_from_slice(cast_slice(&self.instances));
        PrimitiveBatch {
            instance_count: self.instances.len() as u32,
            instance_bound: offset..end as wgpu::BufferAddress,
            indices: self.indices,
            bind_group: self.bind_group,
        }
    }
}
struct PipelineBatches {
    inner: HashMap<wgpu::RenderPipeline, Vec<PrimitiveBatch>>,
}

impl PipelineBatches {
    fn new() -> Self {
        PipelineBatches {
            inner: HashMap::new(),
        }
    }

    fn add_batch(&mut self, pipeline: &wgpu::RenderPipeline, batch: PrimitiveBatch) {
        match self.inner.get_mut(pipeline) {
            Some(batches) => {
                batches.push(batch);
            }
            None => {
                self.inner.insert(pipeline.clone(), vec![batch]);
            }
        }
    }

    fn render(&self, instance_buffer: &wgpu::Buffer, render_pass: &mut wgpu::RenderPass) {
        for (pipeline, batches) in &self.inner {
            render_pass.set_pipeline(pipeline);
            batches
                .iter()
                .for_each(|batch| batch.render(instance_buffer, render_pass));
        }
    }
}

pub struct PreparedPrimitives<'a> {
    instance_buffer: &'a wgpu::Buffer,
    opaque: PipelineBatches,
    transparent: PipelineBatches,
}

impl<'a> PreparedPrimitives<'a> {
    /// Prepares the primitives for rendering.
    ///
    /// This methods splits the primitives into opaque and transparent batches
    /// and writes the instance data to the `vertex_buffer` provided.
    /// If `vertex_buffer` is not large enough, this function allocates a larger using the provided `ctx`.
    ///
    /// # Panics
    /// This function will panic if any of the [`MeshInstance::node()`] are not found in the `scene_graph`.
    pub fn from_mesh_instances<M>(
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
        self.opaque.render(&self.instance_buffer, render_pass);
    }

    /// Renders all transparent primitives to the provided render pass.
    pub fn render_transparent(&self, render_pass: &mut wgpu::RenderPass) {
        self.transparent.render(&self.instance_buffer, render_pass);
    }
}

#[derive(Debug)]
pub(super) struct PrimitiveRenderer {
    staging_buffer_receiver: Receiver<wgpu::Buffer>,
    staging_buffer_sender: Sender<wgpu::Buffer>,
    instance_buffer: wgpu::Buffer,
}

impl PrimitiveRenderer {
    pub fn new(ctx: &Context) -> Self {
        let instance_buffer = create_instance_buffer(DEFAULT_INSTANCE_BUFFER_SIZE, &ctx.device);
        let (sender, receiver) = channel();

        PrimitiveRenderer {
            staging_buffer_receiver: receiver,
            staging_buffer_sender: sender,
            instance_buffer,
        }
    }

    pub fn prepare<'a, 'b>(
        &'a mut self,
        mesh_instances: impl IntoIterator<Item = &'b MeshInstance>,
        ctx: &Context,
        encoder: &mut wgpu::CommandEncoder,
    ) -> PreparedPrimitives<'a> {
        let mut opaque_primitives: HashMap<
            (&wgpu::RenderPipeline, &Primitive),
            PrimitiveBatchBuilder,
        > = HashMap::new();
        let mut transparent_primitives: HashMap<
            (&wgpu::RenderPipeline, &Primitive),
            PrimitiveBatchBuilder,
        > = HashMap::new();

        let mut primitive_count: usize = 0;
        for mesh_instance in mesh_instances {
            let pipeline_idx = mesh_instance
                .model_matrix()
                .determinant()
                .is_sign_negative() as usize;

            for primitive in mesh_instance.primitives() {
                primitive_count += 1;
                let pipeline = &primitive.pipelines[pipeline_idx];

                let map = match primitive.alpha_mode {
                    AlphaMode::Opaque | AlphaMode::Mask => &mut opaque_primitives,
                    AlphaMode::Blend => &mut transparent_primitives,
                };
                map.entry((pipeline, primitive))
                    .or_insert_with(|| PrimitiveBatchBuilder::new(primitive))
                    .add(*mesh_instance.vertex_data());
            }
        }

        let required_sized = (primitive_count * size_of::<PrimitiveInstance>()) as u64;
        if self.instance_buffer.size() < required_sized {
            log::debug!("Creating larger instance buffer and deleting staging buffers");
            let size = wgpu::util::align_to(required_sized, wgpu::COPY_BUFFER_ALIGNMENT)
                .max(2 * self.instance_buffer.size());
            self.instance_buffer = create_instance_buffer(size, &ctx.device);
            (self.staging_buffer_sender, self.staging_buffer_receiver) = channel();
        }
        let staging_buffer = self.staging_buffer_receiver.try_recv().unwrap_or_else(|_| {
            log::debug!("Creating new staging buffer");
            create_staging_buffer(self.instance_buffer.size(), &ctx.device)
        });

        let mut prepared = PreparedPrimitives {
            instance_buffer: &self.instance_buffer,
            opaque: PipelineBatches::new(),
            transparent: PipelineBatches::new(),
        };
        let mut offset: wgpu::BufferAddress = 0;
        let mut buffer_view = staging_buffer.get_mapped_range_mut(..);
        for ((pipeline, _), batch_builder) in opaque_primitives {
            let batch = batch_builder.build(&mut buffer_view, offset);
            offset = batch.instance_bound.end;
            prepared.opaque.add_batch(pipeline, batch);
        }
        for ((pipeline, _), batch_builder) in transparent_primitives {
            let batch = batch_builder.build(&mut buffer_view, offset);
            offset = batch.instance_bound.end;
            prepared.transparent.add_batch(pipeline, batch);
        }
        drop(buffer_view);

        staging_buffer.unmap();

        encoder.copy_buffer_to_buffer(&staging_buffer, 0, &self.instance_buffer, 0, required_sized);

        let sender = self.staging_buffer_sender.clone();
        encoder.map_buffer_on_submit(
            &staging_buffer.clone(),
            wgpu::MapMode::Write,
            ..,
            move |result| match result {
                Ok(()) => {
                    let _ = sender.send(staging_buffer);
                }
                Err(err) => {
                    log::error!("{err}");
                }
            },
        );
        prepared
    }
}

fn create_staging_buffer(size: wgpu::BufferAddress, device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mogra primitive renderer staging buffer"),
        size,
        usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::MAP_WRITE,
        mapped_at_creation: true,
    })
}

fn create_instance_buffer(size: wgpu::BufferAddress, device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mogra primitive renderer staging buffer"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
        mapped_at_creation: false,
    })
}

#[derive(Debug, Clone)]
pub struct PrimitiveContext {}
