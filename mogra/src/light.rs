use std::collections::HashMap;

use bytemuck::{Pod, Zeroable, bytes_of};
use glam::Vec3;

use crate::{Context, NodeId, buffer::WritableBuffer, scene_graph::SceneGraph};

const DEFAULT_POINT_LIGHT_BUFFER_SIZE: wgpu::BufferAddress = point_lights_size(8);

#[derive(Debug, Clone)]
pub struct PointLight {
    color: Vec3,
}

#[derive(Debug, Clone)]
pub(super) struct LightManager {
    point_lights: HashMap<NodeId, PointLight>,
}

impl LightManager {
    /// Create a new light manager with no lights.
    pub fn new() -> Self {
        let point_lights = HashMap::new();
        LightManager { point_lights }
    }

    /// Write the `PointLightStorage` data to the given buffer.
    ///
    /// # Panics
    /// This function will panic if one of the point light node is not part of the `scene_graph`.
    pub fn write_point_light(
        &self,
        scene_graph: &SceneGraph,
        buffer: &mut WritableBuffer,
        ctx: &Context,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let size = point_lights_size(self.point_lights.len());
        buffer.write_with(0..size, &ctx.device, encoder, |view| {
            let header = PointLightStorageHeader {
                count: self.point_lights.len() as u32,
                _pad: [0; 3],
            };
            let mut start = size_of::<PointLightStorageHeader>();
            view.slice(0..start).copy_from_slice(bytes_of(&header));

            for (&node, light) in &self.point_lights {
                let uniform = PointLightUniform {
                    position: scene_graph[node].global_position(),
                    _pad0: 0,
                    color: light.color,
                    _pad1: 0,
                };
                let end = start + size_of::<PointLightUniform>();
                view.slice(start..end).copy_from_slice(bytes_of(&uniform));
                start = end;
            }
        });
    }
}

pub fn create_point_light_buffer(device: &wgpu::Device) -> WritableBuffer {
    WritableBuffer::new(
        Some("mogra point light buffer"),
        point_lights_size(10),
        wgpu::BufferUsages::STORAGE,
        device,
    )
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct PointLightStorageHeader {
    count: u32,
    _pad: [u32; 3],
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct PointLightUniform {
    position: Vec3,
    _pad0: u32,
    color: Vec3,
    _pad1: u32,
}

const fn point_lights_size(count: usize) -> wgpu::BufferAddress {
    let size = size_of::<PointLightStorageHeader>() + count * size_of::<PointLightUniform>();
    size as wgpu::BufferAddress
}
