use std::ops::{Index, IndexMut};

use wgpu::util::DeviceExt;

use crate::storage::{Id, Storage};

pub struct MaterialManager {
    materials: Storage<Material>,
    bind_group_layout: wgpu::BindGroupLayout,
    default_sampler: wgpu::Sampler,
    default_base_texture_view: wgpu::TextureView,
}

impl MaterialManager {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Material default sampler"),
            ..Default::default()
        });

        const WHITE_PIXEL: [u8; 4] = [255, 255, 255, 255];
        let default_base_texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("Material default base texture"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &WHITE_PIXEL,
        );
        let default_base_texture_view =
            default_base_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            materials: Storage::new(),
            bind_group_layout,
            default_sampler,
            default_base_texture_view,
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn create(&mut self, material: &MaterialDescriptor, device: &wgpu::Device) -> MaterialId {
        let base_color_texture = match material.base_color_texture.as_ref() {
            Some(texture) => texture,
            None => &TextureDescriptor {
                view: &self.default_base_texture_view,
                sampler: &self.default_sampler,
                tex_coord: 0,
            },
        };
        let emissive_texture = match material.emissive_texture.as_ref() {
            Some(texture) => texture,
            None => &TextureDescriptor {
                view: &self.default_base_texture_view,
                sampler: &self.default_sampler,
                tex_coord: 0
            }
        };

        let material_uniform = MaterialUniform {
            base_color_factor: material.base_color_factor,
            base_color_tex_coord: base_color_texture.tex_coord,
            metallic_factor: material.metallic_factor,
            roughness_factor: material.roughness_factor,
            _padding: [0.0; 1],
            emissive_factor: material.emissive_factor,
            emissive_tex_coord: emissive_texture.tex_coord,
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
                    resource: wgpu::BindingResource::TextureView(&base_color_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&base_color_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&emissive_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&emissive_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
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
    _padding: [f32; 1],
    emissive_factor: [f32; 3],
    emissive_tex_coord: u32,
}

pub struct TextureDescriptor<'a> {
    pub view: &'a wgpu::TextureView,
    pub sampler: &'a wgpu::Sampler,
    pub tex_coord: u32,
}

pub struct MaterialDescriptor<'a> {
    pub base_color_factor: [f32; 4],
    pub base_color_texture: Option<TextureDescriptor<'a>>,
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub emissive_texture: Option<TextureDescriptor<'a>>,
    pub emissive_factor: [f32; 3],
}
