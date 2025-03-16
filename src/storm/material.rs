use bytemuck::cast_slice;
use wgpu::util::DeviceExt;

use crate::storage::{Id, Storage};

use super::texture_old::{TextureManager, TextureMip};

pub const TEX_COORD_COUNT: u32 = 2;

pub struct MaterialManager {
    materials: Storage<Material>,
    bind_group_layout: wgpu::BindGroupLayout,
    default_texture: wgpu::TextureView,
    default_sampler: wgpu::Sampler,
}

impl MaterialManager {
    pub fn new(
        device: &wgpu::Device,
    ) -> Self {
        let materials = Storage::new();

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Material bind group layout"),
            entries: &[
                // base_color_texture
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
                // base_color_sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // metallic_roughness_texture
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
                // metallic_roughness_sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // normal_texture
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
                // normal_sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // occlusion_texture
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
                // occlusion_sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // emissive_texture
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
                // emissive_sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 79,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // material uniform
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

        let default_texture = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Material default texture"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Material default sampler"),
            ..Default::default()
        });

        Self {
            materials,
            bind_group_layout,
            default_texture,
            default_sampler,
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn builder<'a>(&'a mut self, name: Option<&'a str>) -> MaterialBuilder<'a> {
        MaterialBuilder {
            manager: self,
            label: name,
            uniform: MaterialUniform {
                base_color_factor: [1.0; 4],
                base_color_tex_coord: TEX_COORD_COUNT,
                metallic_factor: 1.0,
                roughness_factor: 1.0,
                metallic_roughness_tex_coord: TEX_COORD_COUNT,
                normal_scale: 1.0,
                normal_tex_coord: TEX_COORD_COUNT,
                occlusion_strength: 1.0,
                occlusion_tex_coord: TEX_COORD_COUNT,
                emissive_factor: [0.0; 3],
                emissive_tex_coord: TEX_COORD_COUNT,
            },
            base_color_texture: None,
            base_color_sampler: None,
            metallic_roughness_texture: None,
            metallic_roughness_sampler: None,
            normal_texture: None,
            normal_sampler: None,
            occlusion_texture: None,
            occlusion_sampler: None,
            emissive_texture: None,
            emissive_sampler: None,
        }
    }
}

pub type MaterialId = Id<Material>;
pub struct Material {
    bind_group: wgpu::BindGroup,
}

pub struct MaterialBuilder<'a> {
    manager: &'a mut MaterialManager,
    label: Option<&'a str>,
    uniform: MaterialUniform,
    base_color_texture: Option<&'a wgpu::TextureView>,
    base_color_sampler: Option<&'a wgpu::Sampler>,
    metallic_roughness_texture: Option<&'a wgpu::TextureView>,
    metallic_roughness_sampler: Option<&'a wgpu::Sampler>,
    normal_texture: Option<&'a wgpu::TextureView>,
    normal_sampler: Option<&'a wgpu::Sampler>,
    occlusion_texture: Option<&'a wgpu::TextureView>,
    occlusion_sampler: Option<&'a wgpu::Sampler>,
    emissive_texture: Option<&'a wgpu::TextureView>,
    emissive_sampler: Option<&'a wgpu::Sampler>,
}

impl<'a> MaterialBuilder<'a> {
    pub fn build(self, device: &wgpu::Device) -> MaterialId {
        let material_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{} uniform", self.label.unwrap_or(""))),
            contents: cast_slice(&[self.uniform]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{} bind group", self.label.unwrap_or(""))),
            layout: &self.manager.bind_group_layout,
            entries: &[
                // base_color_texture
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        self.base_color_texture
                            .unwrap_or(&self.manager.default_texture),
                    ),
                },
                // base_color_sampler
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(
                        self.base_color_sampler
                            .unwrap_or(&self.manager.default_sampler),
                    ),
                },
                // metallic_roughness_texture
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        self.metallic_roughness_texture
                            .unwrap_or(&self.manager.default_texture),
                    ),
                },
                // metallic_roughness_sampler
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(
                        self.metallic_roughness_sampler
                            .unwrap_or(&self.manager.default_sampler),
                    ),
                },
                // normal_texture
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(
                        self.normal_texture.unwrap_or(&self.manager.default_texture),
                    ),
                },
                // normal_sampler
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(
                        self.normal_sampler.unwrap_or(&self.manager.default_sampler),
                    ),
                },
                // occlusion_texture
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(
                        self.occlusion_texture
                            .unwrap_or(&self.manager.default_texture),
                    ),
                },
                // occlusion_sampler
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(
                        self.occlusion_sampler
                            .unwrap_or(&self.manager.default_sampler),
                    ),
                },
                // emissive_texture
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(
                        self.emissive_texture
                            .unwrap_or(&self.manager.default_texture),
                    ),
                },
                // emissive_sampler
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(
                        self.emissive_sampler
                            .unwrap_or(&self.manager.default_sampler),
                    ),
                },
                // material uniform
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: material_uniform.as_entire_binding(),
                },
            ],
        });

        self.manager.materials.add(Material { bind_group })
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialUniform {
    base_color_factor: [f32; 4],
    base_color_tex_coord: u32,
    metallic_factor: f32,
    roughness_factor: f32,
    metallic_roughness_tex_coord: u32,
    normal_scale: f32,
    normal_tex_coord: u32,
    occlusion_strength: f32,
    occlusion_tex_coord: u32,
    emissive_factor: [f32; 3],
    emissive_tex_coord: u32,
}
