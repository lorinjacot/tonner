use std::ops::Index;

use bitflags::bitflags;
use bytemuck::{Pod, Zeroable, cast_slice};
use wgpu::util::DeviceExt;

use crate::{DenseEntry, Id, Resources, storage::SparseSet};

pub struct Texture {
    id: Id<Self>,
    name: String,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

impl Texture {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }
}

impl DenseEntry for Texture {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

#[must_use]
#[derive(Default)]
pub struct TextureBuilder {
    name: Option<String>,
    texture: Option<wgpu::Texture>,
    sampler: Option<wgpu::Sampler>,
}

impl TextureBuilder {
    pub fn name(mut self, name: impl Into<Option<String>>) -> Self {
        self.name = name.into();
        self
    }

    pub fn texture(mut self, texture: impl Into<Option<wgpu::Texture>>) -> Self {
        self.texture = texture.into();
        self
    }

    pub fn sampler(mut self, sampler: impl Into<Option<wgpu::Sampler>>) -> Self {
        self.sampler = sampler.into();
        self
    }

    pub fn build<'r>(self, resources: &'r mut Resources) -> &'r Texture {
        let label = self.name.as_deref();

        let view = self.texture.expect("texture should be set").create_view(
            &wgpu::TextureViewDescriptor {
                label,
                ..Default::default()
            },
        );

        let sampler = self.sampler.unwrap_or_else(|| {
            resources.device.create_sampler(&wgpu::SamplerDescriptor {
                label,
                ..Default::default()
            })
        });

        let manager = &mut resources.materials;
        let id = manager.textures.next_id();
        manager.textures.insert(Texture {
            id,
            name: self.name.unwrap_or("".to_string()),
            view,
            sampler,
        })
    }
}

pub struct Material {
    id: Id<Self>,
    layout: MaterialLayout,
    normal_tex_coord: Option<u32>,
    bind_group: wgpu::BindGroup,
}

impl Material {
    pub fn has_base_color_texture(&self) -> bool {
        self.layout.textures.contains(Textures::BASE_COLOR)
    }

    pub fn has_metallic_roughness_texture(&self) -> bool {
        self.layout.textures.contains(Textures::METALLIC_ROUGHNESS)
    }

    pub fn has_occlusion_texture(&self) -> bool {
        self.layout.textures.contains(Textures::OCCLUSION)
    }

    pub fn has_emissive_texture(&self) -> bool {
        self.layout.textures.contains(Textures::EMISSIVE)
    }

    pub fn has_normal_texture(&self) -> bool {
        self.layout.textures.contains(Textures::NORMAL)
    }

    pub fn normal_tex_coord(&self) -> Option<u32> {
        self.normal_tex_coord
    }

    pub fn alpha_mode(&self) -> AlphaMode {
        self.layout.alpha_mode
    }

    pub fn double_sided(&self) -> bool {
        self.layout.double_sided
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct Textures: u8 {
        const BASE_COLOR = 1 << 0;
        const METALLIC_ROUGHNESS = 1 << 1;
        const NORMAL = 1 << 2;
        const OCCLUSION = 1 << 3;
        const EMISSIVE = 1 << 4;
    }
}

impl DenseEntry for Material {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

#[must_use]
pub struct MaterialBuilder {
    base_color_texture: Option<Id<Texture>>,
    metallic_roughness_texture: Option<Id<Texture>>,
    normal_texture: Option<Id<Texture>>,
    occlusion_texture: Option<Id<Texture>>,
    emissive_texture: Option<Id<Texture>>,
    uniform: MaterialUniform,
    layout: MaterialLayout,
}

impl MaterialBuilder {
    pub fn base_color_factor(mut self, base_color_factor: [f32; 4]) -> Self {
        self.uniform.base_color_factor = base_color_factor;
        self
    }

    pub fn base_color_tex_coord(mut self, tex_coord: u32) -> Self {
        self.uniform.base_color_tex_coord = tex_coord;
        self
    }

    pub fn base_color_texture(mut self, texture: impl Into<Option<Id<Texture>>>) -> Self {
        self.base_color_texture = texture.into();
        self.layout.textures.insert(Textures::BASE_COLOR);
        self
    }

    pub fn metallic_factor(mut self, metallic_factor: f32) -> Self {
        self.uniform.metallic_factor = metallic_factor;
        self
    }

    pub fn roughness_factor(mut self, roughness_factor: f32) -> Self {
        self.uniform.roughness_factor = roughness_factor;
        self
    }

    pub fn metallic_roughness_texture(mut self, texture: impl Into<Option<Id<Texture>>>) -> Self {
        self.metallic_roughness_texture = texture.into();
        self.layout.textures.insert(Textures::METALLIC_ROUGHNESS);
        self
    }

    pub fn metallic_roughness_tex_coord(mut self, tex_coord: u32) -> Self {
        self.uniform.metallic_roughness_tex_coord = tex_coord;
        self
    }

    pub fn normal_scale(mut self, scale: f32) -> Self {
        self.uniform.normal_scale = scale;
        self
    }

    pub fn normal_texture(mut self, texture: impl Into<Option<Id<Texture>>>) -> Self {
        self.normal_texture = texture.into();
        self.layout.textures.insert(Textures::NORMAL);
        self
    }

    pub fn normal_tex_coord(mut self, tex_coord: u32) -> Self {
        self.uniform.normal_tex_coord = tex_coord;
        self
    }

    pub fn occlusion_strength(mut self, strength: f32) -> Self {
        self.uniform.occlusion_strength = strength;
        self
    }

    pub fn occlusion_texture(mut self, texture: impl Into<Option<Id<Texture>>>) -> Self {
        self.occlusion_texture = texture.into();
        self.layout.textures.insert(Textures::OCCLUSION);
        self
    }

    pub fn occlusion_tex_coord(mut self, tex_coord: u32) -> Self {
        self.uniform.occlusion_tex_coord = tex_coord;
        self
    }

    pub fn emissive_factor(mut self, factor: [f32; 3]) -> Self {
        self.uniform.emissive_factor = factor;
        self
    }

    pub fn emissive_texture(mut self, texture: impl Into<Option<Id<Texture>>>) -> Self {
        self.emissive_texture = texture.into();
        self.layout.textures.insert(Textures::EMISSIVE);
        self
    }

    pub fn emissive_tex_coord(mut self, tex_coord: u32) -> Self {
        self.uniform.emissive_tex_coord = tex_coord;
        self
    }

    pub fn alpha_mode(mut self, alpha_mode: AlphaMode) -> Self {
        self.layout.alpha_mode = alpha_mode;
        self
    }

    pub fn alpha_cutoff(mut self, alpha_cutoff: f32) -> Self {
        self.uniform.alpha_cutoff = alpha_cutoff;
        self
    }

    pub fn double_sided(mut self, double_sided: bool) -> Self {
        self.layout.double_sided = double_sided;
        self
    }

    pub fn build<'r>(self, resources: &'r mut Resources) -> &'r mut Material {
        let uniform_buffer =
            resources
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Material uniform buffer"),
                    contents: cast_slice(&[self.uniform]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

        let manager = &mut resources.materials;

        let view_samper = |texture: Option<Id<Texture>>| {
            texture.map_or((&manager.dummy_texture, &manager.default_sampler), |id| {
                let texture = &manager.textures[id];
                (texture.view(), texture.sampler())
            })
        };

        let (base_color_view, base_color_sampler) = view_samper(self.base_color_texture);
        let (metallic_roughness_view, metallic_roughness_sampler) =
            view_samper(self.metallic_roughness_texture);
        let (normal_view, normal_sampler) = view_samper(self.normal_texture);
        let (occlusion_view, occlusion_sampler) = view_samper(self.occlusion_texture);
        let (emissive_view, emissive_sampler) = view_samper(self.emissive_texture);

        let bind_group = resources
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Material bind group"),
                layout: &manager.bind_group_layout,
                entries: &[
                    // Base color texture
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(base_color_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(base_color_sampler),
                    },
                    // Metallic roughness texture
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(metallic_roughness_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(metallic_roughness_sampler),
                    },
                    // Normal texture
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(normal_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(normal_sampler),
                    },
                    // Occlusion texture
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::TextureView(occlusion_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: wgpu::BindingResource::Sampler(occlusion_sampler),
                    },
                    // Emissive texture
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: wgpu::BindingResource::TextureView(emissive_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 9,
                        resource: wgpu::BindingResource::Sampler(emissive_sampler),
                    },
                    // Material uniform
                    wgpu::BindGroupEntry {
                        binding: 10,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                ],
            });

        let id = manager.materials.next_id();
        manager.materials.insert(Material {
            id,
            normal_tex_coord: self.normal_texture.map(|_| self.uniform.normal_tex_coord),
            bind_group,
            layout: self.layout,
        })
    }
}

impl Default for MaterialBuilder {
    fn default() -> Self {
        let uniform = MaterialUniform {
            base_color_factor: [1.0; 4],
            base_color_tex_coord: 0,
            metallic_factor: 1.0,
            roughness_factor: 1.0,
            metallic_roughness_tex_coord: 0,
            normal_scale: 1.0,
            normal_tex_coord: 0,
            occlusion_strength: 1.0,
            occlusion_tex_coord: 0,
            emissive_factor: [0.0; 3],
            emissive_tex_coord: 0,
            alpha_cutoff: 0.5,
            _pad: [0; 3],
        };

        let layout = MaterialLayout {
            textures: Textures::empty(),
            alpha_mode: AlphaMode::Opaque,
            double_sided: false,
        };

        Self {
            base_color_texture: None,
            metallic_roughness_texture: None,
            normal_texture: None,
            occlusion_texture: None,
            emissive_texture: None,
            uniform,
            layout,
        }
    }
}

pub struct MaterialManager {
    materials: SparseSet<Material>,
    dummy_texture: wgpu::TextureView,
    default_sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    textures: SparseSet<Texture>,
}

impl MaterialManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let dummy_texture = device
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
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Material dummy texture view"),
                ..Default::default()
            });

        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Material default sampler"),
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Material bind group layout"),
            entries: &[
                // Base color texture
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
                // metallic roughness texture
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
                // normal texture
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
                // occlusion texture
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
                // emissive texture
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
                // Material Uniform
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
            materials: SparseSet::new(),
            dummy_texture,
            default_sampler,
            bind_group_layout,
            textures: SparseSet::new(),
        }
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AlphaMode {
    /// The rendered material is fully opaque and any `alpha` value is ignored.
    Opaque = 0,
    /// The rendered material is either fully opaque or fully transparent depending
    /// on the alpha value and the specified `alpha_cutoff` value.
    Mask = 1,
    /// The rendered material is combined with the background using the specified
    /// `alpha` value as transparency
    Blend = 2,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MaterialLayout {
    textures: Textures,
    alpha_mode: AlphaMode,
    double_sided: bool,
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
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
    alpha_cutoff: f32,
    _pad: [u32; 3],
}
