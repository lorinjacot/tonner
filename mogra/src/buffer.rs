use std::{
    ops::{Bound, RangeBounds},
    sync::mpsc::{Receiver, Sender, channel},
};

const STAGING_BUFFER_COUNT: usize = 2;

/// A write-only [`wgpu::Buffer`] usable in shaders. Auto-grow if required.
///
/// Internally, multiple buffers are used:
/// - a GPU-only buffer. This buffer is the one actually used by the shaders;
/// - 2 or more staging buffers. Those buffers can be written to by the CPU.
#[derive(Debug)]
pub struct WritableBuffer {
    label: Option<&'static str>,
    buffer: wgpu::Buffer,
    sender: Sender<wgpu::Buffer>,
    receiver: Receiver<wgpu::Buffer>,
}

impl WritableBuffer {
    /// Create a new writable buffer of size at least `base_size`.
    pub fn new(
        label: Option<&'static str>,
        base_size: wgpu::BufferAddress,
        usage: wgpu::BufferUsages,
        device: &wgpu::Device,
    ) -> Self {
        let buffer = create_gpu_buffer(label, base_size, usage, &device);

        let (sender, receiver) = channel();
        for _ in 0..STAGING_BUFFER_COUNT {
            let _ = sender.send(create_staging_buffer(base_size, &device));
        }

        WritableBuffer {
            label,
            buffer,
            sender,
            receiver,
        }
    }

    /// Write to the buffer using the provided callback.
    pub fn write_with<S, R>(
        &mut self,
        bounds: S,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        callback: impl FnOnce(&mut wgpu::BufferViewMut) -> R,
    ) -> R
    where
        S: RangeBounds<wgpu::BufferAddress>,
    {
        let offset = match bounds.start_bound() {
            Bound::Included(&start) => start,
            Bound::Excluded(&start) => start + 1,
            Bound::Unbounded => 0,
        };
        let size = match bounds.end_bound() {
            Bound::Included(&end) => end + 1,
            Bound::Excluded(&end) => end,
            Bound::Unbounded => self.buffer.size() - offset,
        };

        let required_size = offset + size;
        let staging = if let Bound::Included(_) | Bound::Excluded(_) = bounds.end_bound()
            && self.buffer.size() < required_size
        {
            let new_size = wgpu::util::align_to(required_size, wgpu::COPY_BUFFER_ALIGNMENT)
                .max(2 * self.buffer.size());
            self.buffer = create_gpu_buffer(self.label, new_size, self.buffer.usage(), device);
            for _ in 0..STAGING_BUFFER_COUNT - 1 {
                let _ = self.sender.send(create_staging_buffer(new_size, device));
            }
            create_staging_buffer(new_size, device)
        } else {
            self.receiver.try_recv().unwrap_or_else(|_| {
                log::warn!("No available staging buffer. Creating a new one");
                create_staging_buffer(self.buffer.size(), device)
            })
        };

        let mut view = staging.get_mapped_range_mut(bounds);
        let result = callback(&mut view);
        drop(view);

        staging.unmap();
        encoder.copy_buffer_to_buffer(&staging, offset, &self.buffer, offset, size);

        let sender = self.sender.clone();
        encoder.map_buffer_on_submit(&staging.clone(), wgpu::MapMode::Write, .., move |res| {
            match res {
                Ok(()) => {
                    let _ = sender.send(staging);
                }
                Err(err) => {
                    log::error!("{err}");
                }
            }
        });

        result
    }

    /// The buffer to be used by the shaders.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }
}

fn create_gpu_buffer(
    label: Option<&str>,
    size: wgpu::BufferAddress,
    usage: wgpu::BufferUsages,
    device: &wgpu::Device,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label,
        size,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_staging_buffer(size: wgpu::BufferAddress, device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mogra staging buffer"),
        size,
        usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::MAP_WRITE,
        mapped_at_creation: true,
    })
}
