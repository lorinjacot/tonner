use std::{collections::HashMap, fmt::Display};

use bytemuck::{Pod, Zeroable, bytes_of, checked::cast_slice};
use glam::Vec3;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    Context,
    entity_component::{ComponentsView, EntityId},
    scene_graph::SceneGraph,
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
pub struct PointLightBuilder {
    entity: EntityId,
    name: Option<String>,
    color: Option<Vec3>,
}

impl PointLightBuilder {
    /// Create a new point light builder for the given entity.
    ///
    /// If `entity` does not have a node component, it will be created. The
    /// node global transform will determine the position of the light.
    pub fn new(entity: EntityId) -> Self {
        PointLightBuilder {
            entity,
            name: None,
            color: None,
        }
    }

    /// Gives a name to the light. Usefull for GUI and debugging.
    pub fn name(self, name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
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
    pub fn build(
        self,
        scene_graph: &mut SceneGraph,
        light_manager: &mut LightManager,
    ) -> Result<PointLightId, PointLightBuilderError> {
        let name = self.name.unwrap_or_default();
        if !scene_graph.has(self.entity) {
            scene_graph.add(self.entity, None);
        }

        let color = self.color.unwrap_or(Vec3::ONE);
        let id = PointLightId(Uuid::new_v4());
        let data = PointLightData {
            id,
            index: None,
            _name: name,
            entity: self.entity,
            color,
        };
        light_manager.point_lights.insert(id, data);
        Ok(id)
    }
}

/// Error when [`PointLightBuilder::build`] fails.
#[derive(Debug, Error)]
pub enum PointLightBuilderError {
    #[error("invalid node {0}")]
    InvalidNode(EntityId),
}

/// Manages all point lights
#[derive(Debug)]
pub struct LightManager {
    point_lights: HashMap<PointLightId, PointLightData>,
    point_light_buffer: wgpu::Buffer,
}

impl LightManager {
    /// Creates an empty light manager.
    pub fn new(ctx: &Context) -> Self {
        let point_light_buffer = Self::create_point_light_buffer(&[], ctx.device());

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

        let header_size = header_size as wgpu::BufferAddress;
        let size = size as wgpu::BufferAddress;
        buffer
            .get_mapped_range_mut(0..header_size)
            .copy_from_slice(bytes_of(&header));
        if header_size < size {
            buffer
                .get_mapped_range_mut(header_size..size)
                .copy_from_slice(cast_slice(data));
        }
        buffer.unmap();

        buffer
    }

    /// Buffer containing the point light data. This is used when a gpu shader need point light access. The return
    /// buffer should not be keep as this method could return another buffer on another call.
    pub(crate) fn point_light_buffer(&self) -> &wgpu::Buffer {
        &self.point_light_buffer
    }

    /// Update the point light buffer with the current state of the nodes.
    pub(crate) fn update_point_light_buffer(
        &mut self,
        scene_graph: &SceneGraph,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), LightError> {
        let mut uniforms = Vec::with_capacity(self.point_lights.len());
        for (i, data) in self.point_lights.values_mut().enumerate() {
            data.index = Some(i as u32);
            uniforms.push(PointLightUniform {
                position: scene_graph
                    .get(data.entity)
                    .ok_or(LightError::InvalidNode(data.id, data.entity))?
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

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum LightError {
    #[error("invalid node {0}")]
    InvalidNode(PointLightId, EntityId),
}

#[derive(Debug)]
struct PointLightData {
    id: PointLightId,
    index: Option<u32>,
    _name: String,
    entity: EntityId,
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
