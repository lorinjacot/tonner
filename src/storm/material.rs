use std::iter::repeat_n;

use bytemuck::cast_slice;
use wgpu::util::DeviceExt;

use super::{
    storage::{Id, SparseMap, SparseSet},
    texture::{Texture, TextureManager},
    Asset,
};

pub const TEX_COORD_COUNT: u32 = 2;

pub struct MaterialManager {
    materials: SparseSet<Material>,
    bind_group_layout: wgpu::BindGroupLayout,
    dummy_texture: Id<Texture>,
    default_material: Option<Id<Material>>,
    mappings: SparseMap<Asset, Vec<Option<Id<Material>>>>,
}

impl MaterialManager {
    pub fn new(textures: &mut TextureManager, device: &wgpu::Device) -> Self {
        let materials = SparseSet::new();

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

        let dummy_view = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Material dummy texture"),
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

        let dummy_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Material dummy sampler"),
            ..Default::default()
        });

        let dummy_texture = textures.create_view_sampler(dummy_view, dummy_sampler);

        Self {
            materials,
            bind_group_layout,
            dummy_texture,
            default_material: None,
            mappings: SparseMap::new(),
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn load_material(
        &mut self,
        asset: Id<Asset>,
        material: gltf::Material,
        images: &Vec<gltf::image::Data>,
        textures: &mut TextureManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<Material> {
        let index = match material.index() {
            Some(index) => index,
            None => {
                let id = self.create_material(asset, material, images, textures, device, queue);
                self.default_material = Some(id);
                return id;
            }
        };
        if let Some(Some(id)) = self.mappings.entry(asset).or_default().get(index) {
            return *id;
        }

        let id = self.create_material(asset, material, images, textures, device, queue);

        let mapping = &mut self.mappings[asset];
        match mapping.get_mut(index) {
            Some(entry) => *entry = Some(id),
            None => {
                let iter = repeat_n(None, index - mapping.len()).chain(Some(Some(id)));
                mapping.extend(iter);
            }
        }

        id
    }

    fn create_material(
        &mut self,
        asset: Id<Asset>,
        material: gltf::Material,
        images: &Vec<gltf::image::Data>,
        textures: &mut TextureManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<Material> {
        let label = format!("Material {}", material.name().unwrap_or(""));

        let base_color_texture = material
            .pbr_metallic_roughness()
            .base_color_texture()
            .map(|texture| {
                assert!(texture.tex_coord() < TEX_COORD_COUNT);
                (
                    textures.load_texture(asset, texture.texture(), true, images, device, queue),
                    texture.tex_coord(),
                )
            })
            .unwrap_or((self.dummy_texture, TEX_COORD_COUNT));

        let metallic_roughness_texture = material
            .pbr_metallic_roughness()
            .metallic_roughness_texture()
            .map(|texture| {
                assert!(texture.tex_coord() < TEX_COORD_COUNT);
                (
                    textures.load_texture(asset, texture.texture(), true, images, device, queue),
                    texture.tex_coord(),
                )
            })
            .unwrap_or((self.dummy_texture, TEX_COORD_COUNT));

        let normal_texture = material
            .normal_texture()
            .map(|texture| {
                assert!(texture.tex_coord() < TEX_COORD_COUNT);
                (
                    textures.load_texture(asset, texture.texture(), false, images, device, queue),
                    texture.tex_coord(),
                    texture.scale(),
                )
            })
            .unwrap_or((self.dummy_texture, TEX_COORD_COUNT, 1.0));

        let occlusion_texture = material
            .occlusion_texture()
            .map(|texture| {
                assert!(texture.tex_coord() < TEX_COORD_COUNT);
                (
                    textures.load_texture(asset, texture.texture(), false, images, device, queue),
                    texture.tex_coord(),
                    texture.strength(),
                )
            })
            .unwrap_or((self.dummy_texture, TEX_COORD_COUNT, 1.0));

        let emissive_texture = material
            .emissive_texture()
            .map(|texture| {
                assert!(texture.tex_coord() < TEX_COORD_COUNT);
                (
                    textures.load_texture(asset, texture.texture(), true, images, device, queue),
                    texture.tex_coord(),
                )
            })
            .unwrap_or((self.dummy_texture, TEX_COORD_COUNT));

        let uniform = MaterialUniform {
            base_color_factor: material.pbr_metallic_roughness().base_color_factor(),
            base_color_tex_coord: base_color_texture.1,
            metallic_factor: material.pbr_metallic_roughness().metallic_factor(),
            roughness_factor: material.pbr_metallic_roughness().roughness_factor(),
            metallic_roughness_tex_coord: metallic_roughness_texture.1,
            normal_scale: normal_texture.2,
            normal_tex_coord: normal_texture.1,
            occlusion_strength: occlusion_texture.2,
            occlusion_tex_coord: occlusion_texture.1,
            emissive_factor: material.emissive_factor(),
            emissive_tex_coord: emissive_texture.1,
        };
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} uniform")),
            contents: cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{label} bind group")),
            layout: &self.bind_group_layout,
            entries: &[
                // base_color_texture
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        textures.view(base_color_texture.0).unwrap(),
                    ),
                },
                // base_color_sampler
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(
                        textures.sampler(base_color_texture.0).unwrap(),
                    ),
                },
                // metallic_roughness_texture
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        textures.view(metallic_roughness_texture.0).unwrap(),
                    ),
                },
                // metallic_roughness_sampler
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(
                        textures.sampler(metallic_roughness_texture.0).unwrap(),
                    ),
                },
                // normal_texture
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(
                        textures.view(normal_texture.0).unwrap(),
                    ),
                },
                // normal_sampler
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(
                        textures.sampler(normal_texture.0).unwrap(),
                    ),
                },
                // occlusion_texture
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(
                        textures.view(occlusion_texture.0).unwrap(),
                    ),
                },
                // occlusion_sampler
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(
                        textures.sampler(occlusion_texture.0).unwrap(),
                    ),
                },
                // emissive_texture
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(
                        textures.view(emissive_texture.0).unwrap(),
                    ),
                },
                // emissive_sampler
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(
                        textures.sampler(emissive_texture.0).unwrap(),
                    ),
                },
                // material uniform
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });

        self.materials.push(Material { bind_group })
    }
}

pub struct Material {
    bind_group: wgpu::BindGroup,
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
