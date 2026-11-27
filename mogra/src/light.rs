use std::collections::HashMap;

use bytemuck::{Pod, Zeroable, bytes_of};
use glam::Vec3;

use crate::{
    Context, NodeId, Skybox,
    buffer::WritableBuffer,
    light::image_based_lightning::{
        PREFILTER_MAP_MIP_COUNT, create_brdf_lut_sampler, create_brdf_lut_texture,
        create_cube_index_buffer, create_cube_vertex_buffer, create_ibl_bind_group,
        create_ibl_bind_group_layout, create_irradiance_map, create_irradiance_map_sampler,
        create_irradiance_pipeline, create_prefilter_map, create_prefilter_map_pipeline,
        create_prefilter_map_sampler, create_prefilter_roughness_bind_group_layout,
        create_roughness_bind_groups, create_view_projection_bind_group_layout,
        create_view_projection_bind_groups,
    },
    scene_graph::SceneGraph,
};

mod image_based_lightning;

const DEFAULT_POINT_LIGHT_BUFFER_SIZE: wgpu::BufferAddress = point_lights_size(8);

#[derive(Debug, Clone)]
pub struct PointLight {
    color: Vec3,
}

#[derive(Debug, Clone)]
pub(super) struct LightManager {
    ibl_bind_group: wgpu::BindGroup,
    point_lights: HashMap<NodeId, PointLight>,
}

impl LightManager {
    /// Create a new light manager with no lights except those from the skybox.
    pub fn new(skybox: &Skybox, ctx: &Context, encoder: &mut wgpu::CommandEncoder) -> Self {
        let irradiance_map = create_irradiance_map(skybox.bind_group(), ctx, encoder);
        let prefilter_map = create_prefilter_map(skybox.bind_group(), ctx, encoder);
        let ibl_bind_group = create_ibl_bind_group(&irradiance_map, &prefilter_map, ctx);

        let point_lights = HashMap::new();

        LightManager {
            ibl_bind_group,
            point_lights,
        }
    }

    /// Changes the skybox used for lighting.
    ///
    /// This function should only be called if the skybox actually changed,
    /// as recompute the skybox lighting is expensive.
    pub fn set_skybox(
        &mut self,
        skybox: &Skybox,
        ctx: &Context,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let irradiance_map = create_irradiance_map(skybox.bind_group(), ctx, encoder);
        let prefilter_map = create_prefilter_map(skybox.bind_group(), ctx, encoder);
        self.ibl_bind_group = create_ibl_bind_group(&irradiance_map, &prefilter_map, ctx);
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LightContext {
    ibl_bind_group_layout: wgpu::BindGroupLayout,
    irradiance_pipeline: wgpu::RenderPipeline,
    irradiance_map_sampler: wgpu::Sampler,
    prefilter_map_pipeline: wgpu::RenderPipeline,
    prefilter_map_sampler: wgpu::Sampler,
    brdf_lut_texture: wgpu::TextureView,
    brdf_lut_sampler: wgpu::Sampler,
    cube_vertex_buffer: wgpu::Buffer,
    cube_index_buffer: wgpu::Buffer,
    view_projection_bind_groups: [wgpu::BindGroup; 6],
    roughness_bind_groups: [wgpu::BindGroup; PREFILTER_MAP_MIP_COUNT],
}

impl LightContext {
    pub fn new(
        skybox_bind_group_layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Self {
        let ibl_module =
            device.create_shader_module(wgpu::include_wgsl!("light/image_based_lightning.wgsl"));

        let view_projection_bind_group_layout = create_view_projection_bind_group_layout(device);
        let view_projection_bind_groups =
            create_view_projection_bind_groups(&view_projection_bind_group_layout, device);

        let roughness_bind_group_layout = create_prefilter_roughness_bind_group_layout(device);
        let roughness_bind_groups =
            create_roughness_bind_groups(&roughness_bind_group_layout, device);

        let ibl_bind_group_layout = create_ibl_bind_group_layout(device);
        let irradiance_pipeline = create_irradiance_pipeline(
            &view_projection_bind_group_layout,
            skybox_bind_group_layout,
            device,
            &ibl_module,
        );

        let prefilter_map_pipeline = create_prefilter_map_pipeline(
            &ibl_module,
            &view_projection_bind_group_layout,
            skybox_bind_group_layout,
            &roughness_bind_group_layout,
            device,
        );

        let irradiance_map_sampler = create_irradiance_map_sampler(device);
        let prefilter_map_sampler = create_prefilter_map_sampler(device);
        let brdf_lut_texture = create_brdf_lut_texture(device, &ibl_module, encoder);
        let brdf_lut_sampler = create_brdf_lut_sampler(device);

        let cube_vertex_buffer = create_cube_vertex_buffer(device);
        let cube_index_buffer = create_cube_index_buffer(device);

        LightContext {
            ibl_bind_group_layout,
            irradiance_pipeline,
            irradiance_map_sampler,
            prefilter_map_pipeline,
            prefilter_map_sampler,
            brdf_lut_sampler,
            brdf_lut_texture,
            cube_index_buffer,
            cube_vertex_buffer,
            view_projection_bind_groups,
            roughness_bind_groups,
        }
    }
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
