use std::ops::{Index, IndexMut};

use wgpu::util::DeviceExt;

use crate::storage::{Id, Storage};

pub struct MaterialManager {
    materials: Storage<Material>,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl MaterialManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Material bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 10,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        Self {
            materials: Storage::new(),
            bind_group_layout,
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn create(&mut self, material: MaterialDescriptor, device: &wgpu::Device) -> MaterialId {
        let material_uniform = MaterialUniform {
            base_color_factor: material.base_color_factor,
            base_color_tex_coord: material.base_color_tex_coord,
            metallic_factor: material.metallic_factor,
            roughness_factor: material.roughness_factor,
            metallic_roughness_tex_coord: material.metallic_roughness_tex_coord,
            normal_texture_scale: material.normal_texture_scale,
            normal_tex_coord: material.normal_tex_coord,
            occlusion_strength: material.occlusion_strength,
            occlusion_tex_coord: material.occlusion_tex_coord,
            emissive_factor: material.emissive_factor,
            emissive_tex_coord: material.emissive_tex_coord,
        };
        let material_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Material uniform buffer"),
            contents: bytemuck::cast_slice(&[material_uniform]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&material.base_color_texture),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&material.base_color_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &material.metallic_roughness_texture,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&material.metallic_roughness_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&material.normal_texture),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&material.normal_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&material.occlusion_texture),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&material.occlusion_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&material.emissive_texture),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&material.emissive_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: material_buffer.as_entire_binding(),
                },
            ],
        });

        self.materials.add(Material { bind_group })
    }
}

impl Index<MaterialId> for MaterialManager {
    type Output = Material;

    fn index(&self, index: MaterialId) -> &Self::Output {
        &self.materials[index]
    }
}

impl IndexMut<MaterialId> for MaterialManager {
    fn index_mut(&mut self, index: MaterialId) -> &mut Self::Output {
        &mut self.materials[index]
    }
}

pub type MaterialId = Id<Material>;

pub struct Material {
    bind_group: wgpu::BindGroup,
}

impl Material {
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialUniform {
    base_color_factor: [f32; 4],
    base_color_tex_coord: u32,
    metallic_factor: f32,
    roughness_factor: f32,
    metallic_roughness_tex_coord: u32,
    normal_texture_scale: f32,
    normal_tex_coord: u32,
    occlusion_strength: f32,
    occlusion_tex_coord: u32,
    emissive_factor: [f32; 3],
    emissive_tex_coord: u32,
}

pub struct MaterialDescriptor {
    pub base_color_factor: [f32; 4],
    pub base_color_tex_coord: u32,
    pub base_color_texture: wgpu::TextureView,
    pub base_color_sampler: wgpu::Sampler,
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub metallic_roughness_tex_coord: u32,
    pub metallic_roughness_texture: wgpu::TextureView,
    pub metallic_roughness_sampler: wgpu::Sampler,
    pub normal_texture_scale: f32,
    pub normal_tex_coord: u32,
    pub normal_texture: wgpu::TextureView,
    pub normal_sampler: wgpu::Sampler,
    pub occlusion_strength: f32,
    pub occlusion_tex_coord: u32,
    pub occlusion_texture: wgpu::TextureView,
    pub occlusion_sampler: wgpu::Sampler,
    pub emissive_tex_coord: u32,
    pub emissive_texture: wgpu::TextureView,
    pub emissive_sampler: wgpu::Sampler,
    pub emissive_factor: [f32; 3],
}
