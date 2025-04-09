use std::{
    collections::BTreeMap,
    iter::{once, repeat_n},
};

use bitflags::bitflags;
use bytemuck::cast_slice;
use wgpu::util::DeviceExt;

use super::{
    storage::{Id, SparseMap, SparseSet},
    texture::TextureManager,
    Asset,
};

pub const TEX_COORD_COUNT: u32 = 2;

const BASE_COLOR_TEXTURE_BINDING: u32 = 0;
const METALLIC_ROUGHNESS_TEXTURE_BINDING: u32 = 2;
const NORMAL_TEXTURE_BINDING: u32 = 4;
const OCCLUSION_TEXTURE_BINDING: u32 = 6;
const EMISSIVE_TEXTURE_BINDING: u32 = 8;
const UNIFORM_BINDING: u32 = 10;

bitflags! {
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
    struct MaterialFlags: u32 {
        const BASE_COLOR_TEXTURE = 0b00000001;
        const METALLIC_ROUGHNESS_TEXTURE = 0b00000010;
        const NORMAL_TEXTURE = 0b00000100;
        const OCCLUSION_TEXTURE = 0b00001000;
        const EMISSIVE_TEXTURE = 0b00010000;
    }
}

pub struct MaterialManager {
    materials: SparseSet<Material>,
    bind_group_layouts: BTreeMap<MaterialFlags, wgpu::BindGroupLayout>,
    default_material: Option<Id<Material>>,
    mappings: SparseMap<Asset, Vec<Option<Id<Material>>>>,
}

impl MaterialManager {
    pub fn new() -> Self {
        let materials = SparseSet::new();
        let bind_group_layouts = BTreeMap::new();

        Self {
            materials,
            bind_group_layouts,
            default_material: None,
            mappings: SparseMap::new(),
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
        let mut entries = Vec::new();

        let base_color_tex_coord = material
            .pbr_metallic_roughness()
            .base_color_texture()
            .map(|texture| {
                assert!(texture.tex_coord() < TEX_COORD_COUNT);
                flags.insert(MaterialFlags::BASE_COLOR_TEXTURE);
                entries.push((
                    BASE_COLOR_TEXTURE_BINDING,
                    textures.load_texture(asset, texture.texture(), true, device, queue),
                ));
                texture.tex_coord()
            })
            .unwrap_or(0);

        let metallic_roughness_tex_coord = material
            .pbr_metallic_roughness()
            .metallic_roughness_texture()
            .map(|texture| {
                assert!(texture.tex_coord() < TEX_COORD_COUNT);
                flags.insert(MaterialFlags::METALLIC_ROUGHNESS_TEXTURE);
                entries.push((
                    METALLIC_ROUGHNESS_TEXTURE_BINDING,
                    textures.load_texture(asset, texture.texture(), true, device, queue),
                ));
                texture.tex_coord()
            })
            .unwrap_or(0);

        let (normal_tex_coord, normal_texture_scale) = material
            .normal_texture()
            .map(|texture| {
                assert!(texture.tex_coord() < TEX_COORD_COUNT);
                flags.insert(MaterialFlags::NORMAL_TEXTURE);
                entries.push((
                    NORMAL_TEXTURE_BINDING,
                    textures.load_texture(asset, texture.texture(), false, device, queue),
                ));
                (texture.tex_coord(), texture.scale())
            })
            .unwrap_or((0, 1.0));

        let (occlusion_tex_coord, occlusion_texture_strength) = material
            .occlusion_texture()
            .map(|texture| {
                assert!(texture.tex_coord() < TEX_COORD_COUNT);
                flags.insert(MaterialFlags::OCCLUSION_TEXTURE);
                entries.push((
                    OCCLUSION_TEXTURE_BINDING,
                    textures.load_texture(asset, texture.texture(), false, device, queue),
                ));
                (texture.tex_coord(), texture.strength())
            })
            .unwrap_or((0, 1.0));

        let emissive_tex_coord = material
            .emissive_texture()
            .map(|texture| {
                assert!(texture.tex_coord() < TEX_COORD_COUNT);
                flags.insert(MaterialFlags::EMISSIVE_TEXTURE);
                entries.push((
                    EMISSIVE_TEXTURE_BINDING,
                    textures.load_texture(asset, texture.texture(), true, device, queue),
                ));
                texture.tex_coord()
            })
            .unwrap_or(0);

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

        let layout = self.bind_group_layouts.entry(flags).or_insert_with(|| {
            let entries: Vec<_> = flags
                .iter()
                .flat_map(|entry| {
                    let binding = match entry {
                        MaterialFlags::BASE_COLOR_TEXTURE => BASE_COLOR_TEXTURE_BINDING,
                        MaterialFlags::METALLIC_ROUGHNESS_TEXTURE => {
                            METALLIC_ROUGHNESS_TEXTURE_BINDING
                        }
                        MaterialFlags::NORMAL_TEXTURE => NORMAL_TEXTURE_BINDING,
                        MaterialFlags::OCCLUSION_TEXTURE => OCCLUSION_TEXTURE_BINDING,
                        MaterialFlags::EMISSIVE_TEXTURE => EMISSIVE_TEXTURE_BINDING,
                        _ => unreachable!(),
                    };
                    [
                        wgpu::BindGroupLayoutEntry {
                            binding,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: binding + 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ]
                })
                .collect();
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(&format!("{flags:?} material bind group layout")),
                entries: &entries,
            })
        });

        let entries: Vec<_> = entries
            .iter()
            .copied()
            .flat_map(|(binding, texture)| {
                [
                    wgpu::BindGroupEntry {
                        binding,
                        resource: wgpu::BindingResource::TextureView(
                            textures.view(texture).unwrap(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: binding + 1,
                        resource: wgpu::BindingResource::Sampler(
                            textures.sampler(texture).unwrap(),
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
            layout: &layout,
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

    pub fn bind_group_layout(&self, material: Id<Material>) -> Option<&wgpu::BindGroupLayout> {
        self.bind_group_layouts
            .get(&self.materials.get(material)?.flags)
    }
}

pub struct Material {
    bind_group: wgpu::BindGroup,
    flags: MaterialFlags,
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
