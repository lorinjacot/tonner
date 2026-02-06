pub use asset::{environment, geometry};
pub use scene::{Scene, SceneBuilder};
pub use scene::{camera, light, renderer, scene_graph, skin};

use crate::asset::environment::EnvironmentContext;
use crate::mesh::MeshContext;
use crate::mesh::material::MaterialContext;
use crate::scene::renderer::RendererContext;
use crate::texture::TextureContex;

pub mod mesh;
mod asset {
    pub mod environment;
    pub mod geometry;
}
pub mod math;
mod scene;
pub mod texture;

/// Contains everything long-lived and shared by the engine.
/// This is the first thing you need when using storm.
///
/// [Context] is cheap to clone, and any clone refers to the same data.
/// In general, two objects created with different contexts cannot be used together.
///
/// Contains:
/// - bind group layouts
/// - pipeline layouts
/// - shader modules
/// - pipelines
/// - default buffers, textures, samplers
/// - ...
#[derive(Debug, Clone)]
pub struct Context {
    device: wgpu::Device,
    queue: wgpu::Queue,

    texture_ctx: TextureContex,
    material_ctx: MaterialContext,
    mesh_ctx: MeshContext,
    environment_ctx: EnvironmentContext,
    renderer_ctx: RendererContext,
}

impl Context {
    /// Create the context using the provided [wgpu::Device] and [wgpu::Queue].
    pub fn from_device(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let mut encoder = device.create_command_encoder(&wgpu::wgt::CommandEncoderDescriptor {
            label: Some("storm::Context::from_device command encoder"),
        });

        let texture_ctx = TextureContex::new(&device);
        let material_ctx = MaterialContext::new(&device);
        let renderer_ctx = RendererContext::new(&device);
        let mesh_ctx = MeshContext::new(&renderer_ctx.render_bind_group_layout, &device);
        let environment_ctx = EnvironmentContext::new(&device, &mut encoder);

        queue.submit([encoder.finish()]);

        Self {
            device,
            queue,
            texture_ctx,
            material_ctx,
            mesh_ctx,
            environment_ctx,
            renderer_ctx,
        }
    }

    /// The GPU used by the engine.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }
}

pub trait GpuContext {
    fn device(&self) -> &wgpu::Device;
}

pub trait GpuAllocator: GpuContext {
    /// Allocate a slice with the given `size` and `alignment` and return it.
    ///
    /// `size` must be a multiple of [`wgpu::COPY_BUFFER_ALIGNMENT`]
    /// (as is required by the underlying buffer operations).
    ///
    /// To use this slice, call [`wgpu::BufferSlice::get_mapped_range_mut()`] and write your data into
    /// that [`wgpu::BufferViewMut`].
    ///
    /// You can then record your own GPU commands to perform with the slice,
    /// such as copying it to a texture or executing a compute shader that reads it (whereas
    /// [`GpuBufferWriter::write_buffer()`] can only write to other buffers).
    ///
    /// The chosen slice will be positioned within the buffer at a multiple of `alignment`,
    /// which may be used to meet alignment requirements for the operation you wish to perform
    /// with the slice. This does not necessarily affect the alignment of the [`wgpu::BufferViewMut`].
    fn allocate(
        &mut self,
        size: wgpu::BufferSize,
        alignment: wgpu::BufferSize,
    ) -> wgpu::BufferSlice<'_>;
}

pub trait GpuBufferWriter: GpuContext {
    /// Copies the bytes of `data` into `buffer` starting at `offset`.
    ///
    /// The data must be written fully in-bounds, that is, `offset + data.len() <= buffer.len()`.
    ///
    /// # Performance considerations
    ///
    /// * Calls to `write_buffer()` do *not* submit the transfer to the GPU
    ///   immediately. They begin GPU execution only on the next call to
    ///   [`Queue::submit()`], just before the explicitly submitted commands.
    ///   To get a set of scheduled transfers started immediately,
    ///   it's fine to call `submit` with no command buffers at all:
    ///
    ///   ```no_run
    ///   # let queue: wgpu::Queue = todo!();
    ///   # let buffer: wgpu::Buffer = todo!();
    ///   # let data = [0u8];
    ///   queue.write_buffer(&buffer, 0, &data);
    ///   queue.submit([]);
    ///   ```
    ///
    ///   However, `data` will be immediately copied into staging memory, so the
    ///   caller may discard it any time after this call completes.
    ///
    /// * Consider using [`GpuBufferWriter::write_buffer_with()`] instead.
    ///   That method allows you to prepare your data directly within the staging
    ///   memory, rather than first placing it in a separate `[u8]` to be copied.
    ///   That is, `writer.write_buffer(b, offset, data)` is approximately equivalent
    ///   to `writer.write_buffer_with(b, offset, data.len()).copy_from_slice(data)`,
    ///   so use `write_buffer_with()` if you can do something smarter than that
    ///   `copy_from_slice()`. However, for small values
    ///   (e.g. a typical uniform buffer whose contents come from a `struct`),
    ///   there will likely be no difference, since the compiler will be able to
    ///   optimize out unnecessary copies regardless.
    fn write_buffer(&self, target: &wgpu::Buffer, offset: wgpu::BufferAddress, data: &[u8]);

    /// Allocate a staging belt slice of `size` to be copied into the `target` buffer
    /// at the specified offset.
    ///
    /// `offset` and `size` must be multiples of [`wgpu::COPY_BUFFER_ALIGNMENT`]
    /// (as is required by the underlying buffer operations).
    fn write_buffer_with(
        &mut self,
        target: &wgpu::Buffer,
        offset: wgpu::BufferAddress,
        size: wgpu::BufferSize,
    ) -> wgpu::BufferViewMut;
}

pub struct GpuCommandQueue {
    context: Context,
    stagin_belt: wgpu::util::StagingBelt,
    command_encoders: Vec<wgpu::CommandEncoder>,
}

impl GpuCommandQueue {
    pub fn new(context: Context, stagin_belt_chunck_size: wgpu::BufferAddress) -> Self {
        let command_encoders = vec![create_command_encoder(&context)];

        Self {
            context,
            stagin_belt: wgpu::util::StagingBelt::new(stagin_belt_chunck_size),
            command_encoders,
        }
    }

    fn create_command_encoder(&self, label: Option<&str>) -> wgpu::CommandEncoder {
        self.context
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label })
    }

    fn add_command_encoder(&mut self, command_encoder: wgpu::CommandEncoder) {
        self.command_encoders.push(command_encoder);
    }

    pub fn context(&self) -> &Context {
        &self.context
    }

    pub fn submit(&mut self) {
        self.stagin_belt.finish();

        self.context.queue.submit(
            self.command_encoders
                .drain(..)
                .map(|encoder| encoder.finish()),
        );
        self.command_encoders
            .push(create_command_encoder(&self.context));

        self.stagin_belt.recall();
    }
}

fn create_command_encoder(ctx: &Context) -> wgpu::CommandEncoder {
    ctx.device()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("storm::CommandQueue default command encoder"),
        })
}

impl GpuContext for GpuCommandQueue {
    fn device(&self) -> &wgpu::Device {
        self.context.device()
    }
}

impl GpuAllocator for GpuCommandQueue {
    fn allocate(
        &mut self,
        size: wgpu::BufferSize,
        alignment: wgpu::BufferSize,
    ) -> wgpu::BufferSlice<'_> {
        self.stagin_belt
            .allocate(size, alignment, self.context.device())
    }
}

impl GpuBufferWriter for GpuCommandQueue {
    fn write_buffer(&self, target: &wgpu::Buffer, offset: wgpu::BufferAddress, data: &[u8]) {
        self.context.queue.write_buffer(target, offset, data);
    }

    fn write_buffer_with(
        &mut self,
        target: &wgpu::Buffer,
        offset: wgpu::BufferAddress,
        size: wgpu::BufferSize,
    ) -> wgpu::BufferViewMut {
        self.stagin_belt.write_buffer(
            &mut self.command_encoders[0],
            target,
            offset,
            size,
            self.context.device(),
        )
    }
}
