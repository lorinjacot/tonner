use std::ops::{Index, IndexMut};

use wgpu::util::DeviceExt;

use crate::{
    storage::{Id, Storage},
    texture::{Texture2d, TextureManager},
};

pub struct MaterialManager {
    materials: Storage<Material>,
    bind_group_layout: wgpu::BindGroupLayout,
    default_texture: Texture2d,
    default_sampler: wgpu::Sampler,
    device: wgpu::Device,
}

impl MaterialManager {
    pub fn new(textures: &mut TextureManager, device: wgpu::Device, _queue: wgpu::Queue) -> Self {
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        const WHITE_PIXEL: [u8; 4] = [255, 255, 255, 255];
        let default_texture = textures.create_from_pixels(
            Some("Material default base texture"),
            wgpu::TextureUsages::TEXTURE_BINDING,
            1,
            1,
            &WHITE_PIXEL,
            wgpu::TextureFormat::Rgba8Unorm,
        );

        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Material default sampler"),
            ..Default::default()
        });

        Self {
            materials: Storage::new(),
            bind_group_layout,
            default_texture,
            default_sampler,
            device,
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn create(&mut self, material: &MaterialDescriptor) -> MaterialId {
        let base_color_texture = match material.base_color_texture.as_ref() {
            Some(texture) => texture,
            None => &TextureDescriptor {
                texture: &self.default_texture,
                sampler: &self.default_sampler,
                tex_coord: 0,
            },
        };
        let metallic_roughness_texture = match material.metallic_roughness_texture.as_ref() {
            Some(texture) => texture,
            None => &TextureDescriptor {
                texture: &self.default_texture,
                sampler: &self.default_sampler,
                tex_coord: 0,
            },
        };
        let (normal_texture, normal_sampler, normal_tex_coord, normal_scale) =
            match material.normal_texture.as_ref() {
                Some(texture) => (
                    texture.texture,
                    texture.sampler,
                    texture.tex_coord,
                    texture.scale,
                ),
                None => (&self.default_texture, &self.default_sampler, 0, 1.0),
            };
        let emissive_texture = match material.emissive_texture.as_ref() {
            Some(texture) => texture,
            None => &TextureDescriptor {
                texture: &self.default_texture,
                sampler: &self.default_sampler,
                tex_coord: 0,
            },
        };

        let material_uniform = MaterialUniform {
            base_color_factor: material.base_color_factor,
            base_color_tex_coord: base_color_texture.tex_coord,
            metallic_factor: material.metallic_factor,
            roughness_factor: material.roughness_factor,
            metallic_roughness_tex_coord: metallic_roughness_texture.tex_coord,
            normal_tex_coord,
            normal_scale,
            _padding: [0; 2],
            emissive_factor: material.emissive_factor,
            emissive_tex_coord: emissive_texture.tex_coord,
        };
        let material_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Material uniform buffer"),
                contents: bytemuck::cast_slice(&[material_uniform]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &base_color_texture.texture.view(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&base_color_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &metallic_roughness_texture.texture.view(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&metallic_roughness_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(normal_texture.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(normal_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&emissive_texture.texture.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&emissive_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
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
    normal_scale: f32,
    normal_tex_coord: u32,
    _padding: [u32; 2],
    emissive_factor: [f32; 3],
    emissive_tex_coord: u32,
}

pub struct TextureDescriptor<'a> {
    pub texture: &'a Texture2d,
    pub sampler: &'a wgpu::Sampler,
    pub tex_coord: u32,
}

pub struct NormalTextureDescriptor<'a> {
    pub texture: &'a Texture2d,
    pub sampler: &'a wgpu::Sampler,
    pub tex_coord: u32,
    pub scale: f32,
}

pub struct MaterialDescriptor<'a> {
    pub base_color_factor: [f32; 4],
    pub base_color_texture: Option<TextureDescriptor<'a>>,
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub metallic_roughness_texture: Option<TextureDescriptor<'a>>,
    pub normal_texture: Option<NormalTextureDescriptor<'a>>,
    pub emissive_texture: Option<TextureDescriptor<'a>>,
    pub emissive_factor: [f32; 3],
}
