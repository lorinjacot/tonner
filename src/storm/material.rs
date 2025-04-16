use std::{
    collections::HashMap,
    iter::{once, repeat_n},
    ops::Index,
};

use bitflags::bitflags;
use bytemuck::cast_slice;
use wgpu::util::DeviceExt;

use super::{
    storage::{Id, SparseMap, SparseSet},
    texture::{Texture, TextureManager},
    Asset,
};

pub const TEX_COORD_COUNT: u32 = 2;

const UNIFORM_BINDING: u32 = 10;

const BASE_COLOR_TEXTURE_OVERRIDE: &str = "has_base_color_texture";
const METALLIC_ROUGHNESS_TEXTURE_OVERRIDE: &str = "has_metallic_roughness_texture";
const NORMAL_TEXTURE_OVERRIDE: &str = "has_normal_texture";
const OCCLUSION_TEXTURE_OVERRIDE: &str = "has_occlusion_texture";
const EMISSIVE_TEXTURE_OVERRIDE: &str = "has_emissive_texture";

bitflags! {
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
    pub struct MaterialFlags: u32 {
        const BASE_COLOR_TEXTURE = 0b00000001;
        const METALLIC_ROUGHNESS_TEXTURE = 0b00000010;
        const NORMAL_TEXTURE = 0b00000100;
        const OCCLUSION_TEXTURE = 0b00001000;
        const EMISSIVE_TEXTURE = 0b00010000;
    }
}

impl MaterialFlags {
    pub fn insert_constants(&self, constants: &mut HashMap<String, f64>) {
        constants.insert(
            BASE_COLOR_TEXTURE_OVERRIDE.to_string(),
            self.contains(MaterialFlags::BASE_COLOR_TEXTURE) as u64 as f64,
        );
        constants.insert(
            METALLIC_ROUGHNESS_TEXTURE_OVERRIDE.to_string(),
            self.contains(MaterialFlags::METALLIC_ROUGHNESS_TEXTURE) as u64 as f64,
        );
        constants.insert(
            NORMAL_TEXTURE_OVERRIDE.to_string(),
            self.contains(MaterialFlags::NORMAL_TEXTURE) as u64 as f64,
        );
        constants.insert(
            OCCLUSION_TEXTURE_OVERRIDE.to_string(),
            self.contains(MaterialFlags::OCCLUSION_TEXTURE) as u64 as f64,
        );
        constants.insert(
            EMISSIVE_TEXTURE_OVERRIDE.to_string(),
            self.contains(MaterialFlags::EMISSIVE_TEXTURE) as u64 as f64,
        );
    }
}

pub struct MaterialManager {
    materials: SparseSet<Material>,
    bind_group_layout: wgpu::BindGroupLayout,
    default_material: Option<Id<Material>>,
    mappings: SparseMap<Asset, Vec<Option<Id<Material>>>>,
    dummy_texture: Id<Texture>,
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
                    binding: 9,
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
            default_material: None,
            mappings: SparseMap::new(),
            dummy_texture,
        }
    }

    pub fn load_material(
        &mut self,
        asset: Id<Asset>,
        material: gltf::Material,
        textures: &mut TextureManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<Material> {
        match material.index() {
            Some(index) => match self.mappings.entry(asset).or_default().get(index) {
                Some(Some(id)) => *id,
                _ => self.create_material(asset, material, textures, device, queue),
            },
            None => self.create_material(asset, material, textures, device, queue),
        }
    }

    fn create_material(
        &mut self,
        asset: Id<Asset>,
        material: gltf::Material,
        textures: &mut TextureManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<Material> {
        let label = format!("Material {}", material.name().unwrap_or(""));

        let mut flags = MaterialFlags::empty();
        const TEXTURE_COUNT: usize = 5;
        let mut entries = Vec::with_capacity(TEXTURE_COUNT);

        let (base_color_tex_coord, id) = material
            .pbr_metallic_roughness()
            .base_color_texture()
            .map_or_else(
                || (0, self.dummy_texture),
                |texture| {
                    assert!(texture.tex_coord() < TEX_COORD_COUNT);
                    flags.insert(MaterialFlags::BASE_COLOR_TEXTURE);
                    (
                        texture.tex_coord(),
                        textures.load_texture(asset, texture.texture(), true, device, queue),
                    )
                },
            );
        entries.push(id);

        let (metallic_roughness_tex_coord, id) = material
            .pbr_metallic_roughness()
            .metallic_roughness_texture()
            .map_or_else(
                || (0, self.dummy_texture),
                |texture| {
                    assert!(texture.tex_coord() < TEX_COORD_COUNT);
                    flags.insert(MaterialFlags::METALLIC_ROUGHNESS_TEXTURE);
                    (
                        texture.tex_coord(),
                        textures.load_texture(asset, texture.texture(), true, device, queue),
                    )
                },
            );
        entries.push(id);

        let (normal_texture_scale, normal_tex_coord, id) = material.normal_texture().map_or_else(
            || (1.0, 0, self.dummy_texture),
            |texture| {
                assert!(texture.tex_coord() < TEX_COORD_COUNT);
                flags.insert(MaterialFlags::NORMAL_TEXTURE);
                (
                    texture.scale(),
                    texture.tex_coord(),
                    textures.load_texture(asset, texture.texture(), false, device, queue),
                )
            },
        );
        entries.push(id);

        let (occlusion_texture_strength, occlusion_tex_coord, id) =
            material.occlusion_texture().map_or_else(
                || (1.0, 0, self.dummy_texture),
                |texture| {
                    assert!(texture.tex_coord() < TEX_COORD_COUNT);
                    flags.insert(MaterialFlags::OCCLUSION_TEXTURE);
                    (
                        texture.strength(),
                        texture.tex_coord(),
                        textures.load_texture(asset, texture.texture(), false, device, queue),
                    )
                },
            );
        entries.push(id);

        let (emissive_tex_coord, id) = material.emissive_texture().map_or_else(
            || (0, self.dummy_texture),
            |texture| {
                assert!(texture.tex_coord() < TEX_COORD_COUNT);
                flags.insert(MaterialFlags::EMISSIVE_TEXTURE);
                (
                    texture.tex_coord(),
                    textures.load_texture(asset, texture.texture(), true, device, queue),
                )
            },
        );
        entries.push(id);

        let uniform = MaterialUniform {
            base_color_factor: material.pbr_metallic_roughness().base_color_factor(),
            base_color_tex_coord,
            metallic_factor: material.pbr_metallic_roughness().metallic_factor(),
            roughness_factor: material.pbr_metallic_roughness().roughness_factor(),
            metallic_roughness_tex_coord,
            normal_texture_scale,
            normal_tex_coord,
            occlusion_texture_strength,
            occlusion_tex_coord,
            emissive_factor: material.emissive_factor(),
            emissive_tex_coord,
        };
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} uniform")),
            contents: cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let entries: Vec<_> = entries
            .iter()
            .enumerate()
            .flat_map(|(i, texture)| {
                [
                    wgpu::BindGroupEntry {
                        binding: i as u32 * 2,
                        resource: wgpu::BindingResource::TextureView(
                            textures.view(*texture).unwrap(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: i as u32 * 2 + 1,
                        resource: wgpu::BindingResource::Sampler(
                            textures.sampler(*texture).unwrap(),
                        ),
                    },
                ]
            })
            .chain(once(wgpu::BindGroupEntry {
                binding: UNIFORM_BINDING,
                resource: uniform.as_entire_binding(),
            }))
            .collect();

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{label} bind group")),
            layout: &self.bind_group_layout,
            entries: &entries,
        });

        let id = self.materials.push(Material { bind_group, flags });

        match material.index() {
            Some(index) => {
                let mapping = &mut self.mappings[asset];
                match mapping.get_mut(index) {
                    Some(entry) => *entry = Some(id),
                    None => {
                        let iter = repeat_n(None, index - mapping.len()).chain(once(Some(id)));
                        mapping.extend(iter);
                    }
                }
            }
            None => self.default_material = Some(id),
        }

        id
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }
}

impl Index<Id<Material>> for MaterialManager {
    type Output = Material;

    fn index(&self, index: Id<Material>) -> &Self::Output {
        &self.materials[index]
    }
}

pub struct Material {
    bind_group: wgpu::BindGroup,
    flags: MaterialFlags,
}

impl Material {
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn flags(&self) -> MaterialFlags {
        self.flags
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
    normal_texture_scale: f32,
    normal_tex_coord: u32,
    occlusion_texture_strength: f32,
    occlusion_tex_coord: u32,
    emissive_factor: [f32; 3],
    emissive_tex_coord: u32,
}
