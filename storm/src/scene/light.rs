use std::{collections::HashMap, fmt::Display};

use bytemuck::{Pod, Zeroable, bytes_of, checked::cast_slice};
use glam::Vec3;
use thiserror::Error;
use uuid::Uuid;

use crate::scene::{
    Scene,
    scene_graph::{NodeBuilder, NodeId, SceneGraph},
};

/// A unique id for a point light. A point light has one and only one id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PointLightId(Uuid);

impl Display for PointLightId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PointLightId({})", self.0)
    }
}

/// A builder for point light.
#[must_use]
#[derive(Default)]
pub struct PointLightBuilder {
    name: Option<String>,
    node: Option<NodeId>,
    color: Option<Vec3>,
}

impl PointLightBuilder {
    /// Gives a name to the light. Usefull for GUI and debugging.
    pub fn name(self, name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..self
        }
    }

    /// Attaches the light to an existing node. If not set, a new node will be created. The
    /// node global transform will determine the position of the light.
    pub fn node(self, node: impl Into<NodeId>) -> Self {
        Self {
            node: Some(node.into()),
            ..self
        }
    }

    /// Sets the color of the light. By default, the light is white.
    pub fn color(self, color: impl Into<Vec3>) -> Self {
        Self {
            color: Some(color.into()),
            ..self
        }
    }

    /// Create the light.
    pub fn build(self, scene: &mut Scene) -> Result<PointLightId, PointLightBuilderError> {
        let name = self.name.unwrap_or_default();
        let node = match self.node {
            Some(node) => {
                if !scene.scene_graph.contains(node) {
                    return Err(PointLightBuilderError::InvalidNode(node));
                }
                node
            }
            None => NodeBuilder::default()
                .name(name.clone())
                .build(&mut scene.scene_graph)
                .unwrap(),
        };
        let color = self.color.unwrap_or(Vec3::ONE);
        let id = PointLightId(Uuid::new_v4());
        let data = PointLightData {
            id,
            index: None,
            name,
            node,
            color,
        };
        scene.light_manager.point_lights.insert(id, data);
        Ok(id)
    }
}

/// Error when [`PointLightBuilder::build`] fails.
#[derive(Debug, Error)]
pub enum PointLightBuilderError {
    #[error("invalid node {0}")]
    InvalidNode(NodeId),
}

/// Manages all point lights
#[derive(Debug)]
pub(super) struct LightManager {
    point_lights: HashMap<PointLightId, PointLightData>,
    point_light_buffer: wgpu::Buffer,
}

impl LightManager {
    /// Creates an empty light manager.
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let point_light_buffer = Self::create_point_light_buffer(&[], device);

        Self {
            point_lights: HashMap::new(),
            point_light_buffer,
        }
    }

    fn point_light_buffer_size(data: &[PointLightUniform]) -> (usize, usize) {
        let header_size = size_of::<PointLightStorageHeader>();
        let size = header_size + data.len() * size_of::<PointLightUniform>();
        (header_size, size)
    }

    fn create_point_light_buffer(
        data: &[PointLightUniform],
        device: &wgpu::Device,
    ) -> wgpu::Buffer {
        let count = data.len() as u32;
        let header = PointLightStorageHeader {
            count,
            _pad: [0; 3],
        };

        let (header_size, size) = Self::point_light_buffer_size(data);

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Point light buffer"),
            size: wgpu::util::align_to(
                size.max(header_size + size_of::<PointLightUniform>()) as u64,
                wgpu::COPY_BUFFER_ALIGNMENT,
            ),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });

        let mut view = buffer.get_mapped_range_mut(..);
        view[0..header_size].copy_from_slice(bytes_of(&header));
        view[header_size..size].copy_from_slice(cast_slice(data));
        drop(view);
        buffer.unmap();

        buffer
    }

    /// Buffer containing the point light data. This is used when a gpu shader need point light access. The return
    /// buffer should not be keep as this method could return another buffer on another call.
    pub(super) fn point_light_buffer(&self) -> &wgpu::Buffer {
        &self.point_light_buffer
    }

    /// Update the point light buffer with the current state of the nodes.
    pub(super) fn update_point_light_buffer(
        &mut self,
        scene_graph: &SceneGraph,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), UpdatePointLightBufferError> {
        let mut uniforms = Vec::with_capacity(self.point_lights.len());
        for (i, data) in self.point_lights.values_mut().enumerate() {
            data.index = Some(i as u32);
            uniforms.push(PointLightUniform {
                position: scene_graph
                    .get(data.node)
                    .ok_or(UpdatePointLightBufferError::InvalidNode(data.node))?
                    .global_transformation()
                    .transform_point3(Vec3::ZERO)
                    .to_array(),
                _pad0: 0,
                color: data.color.to_array(),
                _pad1: 0,
            });
        }

        let (header_size, size) = Self::point_light_buffer_size(&uniforms);

        if self.point_light_buffer.size() < size as u64 {
            self.point_light_buffer = Self::create_point_light_buffer(&uniforms, device);
        } else {
            let header = PointLightStorageHeader {
                count: self.point_lights.len() as u32,
                _pad: [0; 3],
            };
            queue.write_buffer(&self.point_light_buffer, 0, bytes_of(&header));
            queue.write_buffer(
                &self.point_light_buffer,
                header_size as u64,
                cast_slice(&uniforms),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub(super) enum UpdatePointLightBufferError {
    #[error("invalid node {0}")]
    InvalidNode(NodeId),
}

#[derive(Debug)]
struct PointLightData {
    id: PointLightId,
    index: Option<u32>,
    name: String,
    node: NodeId,
    color: Vec3,
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct PointLightStorageHeader {
    count: u32,
    _pad: [u32; 3],
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct PointLightUniform {
    position: [f32; 3],
    _pad0: u32,
    color: [f32; 3],
    _pad1: u32,
}
