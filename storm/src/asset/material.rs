use std::sync::Arc;
use std::{collections::HashMap, sync::Mutex};

use bitflags::bitflags;
use bytemuck::{Pod, Zeroable, cast_slice};
use glam::{Vec3, Vec4};
use uuid::Uuid;
use wgpu::util::DeviceExt;

use crate::Engine;

pub struct Texture {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

/// A unique id for [`Material`]. A `Material` has one and only one id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialId(Uuid);

#[derive(Clone)]
pub struct Material(Arc<MaterialData>);

struct MaterialData {
    id: MaterialId,
    name: Mutex<String>,
    alpha_mode: AlphaMode,
    double_sided: bool,
    textures_flags: TextureFlags,
    uniform_buffer: wgpu::Buffer,
    normal_tex_coord: Option<u32>,
    base_color_texture: Texture,
    metallic_roughness_texture: Texture,
    occlusion_texture: Texture,
    emissive_texture: Texture,
    normal_texture: Texture,
}

impl Material {
    pub fn has_base_color_texture(&self) -> bool {
        self.0.textures_flags.contains(TextureFlags::BASE_COLOR)
    }

    pub fn has_metallic_roughness_texture(&self) -> bool {
        self.0
            .textures_flags
            .contains(TextureFlags::METALLIC_ROUGHNESS)
    }

    pub fn has_occlusion_texture(&self) -> bool {
        self.0.textures_flags.contains(TextureFlags::OCCLUSION)
    }

    pub fn has_emissive_texture(&self) -> bool {
        self.0.textures_flags.contains(TextureFlags::EMISSIVE)
    }

    pub fn has_normal_texture(&self) -> bool {
        self.0.textures_flags.contains(TextureFlags::NORMAL)
    }

    pub fn normal_tex_coord(&self) -> Option<u32> {
        self.0.normal_tex_coord
    }

    pub fn alpha_mode(&self) -> AlphaMode {
        self.0.alpha_mode
    }

    pub fn double_sided(&self) -> bool {
        self.0.double_sided
    }
}

bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct TextureFlags: u8 {
        const BASE_COLOR = 1 << 0;
        const METALLIC_ROUGHNESS = 1 << 1;
        const NORMAL = 1 << 2;
        const OCCLUSION = 1 << 3;
        const EMISSIVE = 1 << 4;
    }
}

/// A builder for [`Material`].
#[must_use]
pub struct MaterialBuilder {
    name: String,
    alpha_mode: AlphaMode,
    double_sided: bool,
    uniform: MaterialUniform,
    textures_flags: TextureFlags,
    base_color_texture: Option<wgpu::TextureView>,
    base_color_sampler: Option<wgpu::Sampler>,
    metallic_roughness_texture: Option<wgpu::TextureView>,
    metallic_roughness_sampler: Option<wgpu::Sampler>,
    normal_texture: Option<wgpu::TextureView>,
    normal_sampler: Option<wgpu::Sampler>,
    occlusion_texture: Option<wgpu::TextureView>,
    occlusion_sampler: Option<wgpu::Sampler>,
    emissive_texture: Option<wgpu::TextureView>,
    emissive_sampler: Option<wgpu::Sampler>,
}

impl Default for MaterialBuilder {
    fn default() -> Self {
        MaterialBuilder {
            name: String::new(),
            alpha_mode: AlphaMode::Opaque,
            double_sided: false,
            uniform: MaterialUniform::default(),
            textures_flags: TextureFlags::empty(),
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

impl MaterialBuilder {
    /// Give a name to the material. Default to no name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// The factors for the base color of the material. Default to [`Vec4::ONE`].
    pub fn base_color_factor(mut self, base_color_factor: impl Into<Vec4>) -> Self {
        self.uniform.base_color_factor = base_color_factor.into();
        self
    }

    /// Sample the base color from the texture. Default to uniform base color.
    pub fn base_color_texture(mut self, texture: impl Into<wgpu::TextureView>) -> Self {
        self.base_color_texture = Some(texture.into());
        self.textures_flags.insert(TextureFlags::BASE_COLOR);
        self
    }

    /// Defines how to sample the base color texture. Only used if a base color texture is set.
    /// Default to the default [`wgpu::Sampler`].
    pub fn base_color_sampler(mut self, sampler: impl Into<wgpu::Sampler>) -> Self {
        self.base_color_sampler = Some(sampler.into());
        self
    }

    /// Which geometry texture coordinates set should be used for texture coordinate mapping. Default to `0`.
    pub fn base_color_tex_coord(mut self, tex_coord: u32) -> Self {
        self.uniform.base_color_tex_coord = tex_coord;
        self
    }

    /// The factor for the metalness of the material. Default to `1.0`.
    pub fn metallic_factor(mut self, metallic_factor: f32) -> Self {
        self.uniform.metallic_factor = metallic_factor;
        self
    }

    /// The factor for the roughness of the material. Default to `1.0`.
    pub fn roughness_factor(mut self, roughness_factor: f32) -> Self {
        self.uniform.roughness_factor = roughness_factor;
        self
    }

    /// Sample the metallic and roughness factors from the texture. Default to uniform factors.
    pub fn metallic_roughness_texture(mut self, texture: impl Into<wgpu::TextureView>) -> Self {
        self.metallic_roughness_texture = Some(texture.into());
        self.textures_flags.insert(TextureFlags::METALLIC_ROUGHNESS);
        self
    }

    /// Defines how to sample the metallic roughness texture. Only used if a metallic roughness texture is set.
    /// Default to the default [`wgpu::Sampler`].
    pub fn metallic_roughness_sampler(mut self, sampler: impl Into<wgpu::Sampler>) -> Self {
        self.metallic_roughness_sampler = Some(sampler.into());
        self
    }

    /// Which geometry texture coordinates set should be used for texture coordinate mapping. Default to `0`.
    pub fn metallic_roughness_tex_coord(mut self, tex_coord: u32) -> Self {
        self.uniform.metallic_roughness_tex_coord = tex_coord;
        self
    }

    /// The scalar parameter applied to each normal vector of the normal texture.
    /// Only used when a normal texture is set. Default to `1.0`.
    pub fn normal_scale(mut self, scale: f32) -> Self {
        self.uniform.normal_scale = scale;
        self
    }

    /// Sample the normals from the texture. Default to normal interpolated from geometry vertices.
    pub fn normal_texture(mut self, texture: impl Into<wgpu::TextureView>) -> Self {
        self.normal_texture = Some(texture.into());
        self.textures_flags.insert(TextureFlags::NORMAL);
        self
    }

    /// Defines how to sample the normal texture. Only used if a normal texture is set.
    /// Default to the default [`wgpu::Sampler`].
    pub fn normal_sampler(mut self, sampler: impl Into<wgpu::Sampler>) -> Self {
        self.normal_sampler = Some(sampler.into());
        self
    }

    /// Which geometry texture coordinates set should be used for texture coordinate mapping. Default to `0`.
    pub fn normal_tex_coord(mut self, tex_coord: u32) -> Self {
        self.uniform.normal_tex_coord = tex_coord;
        self
    }

    /// A scalar multiplier controlling the amount of occlusion applied.
    /// Only used when a occlusion texture is set. Default to `1.0`.
    pub fn occlusion_strength(mut self, strength: f32) -> Self {
        self.uniform.occlusion_strength = strength;
        self
    }

    /// Sample the ambiance occlusion factor from the texture. Default to no ambiance occlusion.
    pub fn occlusion_texture(mut self, texture: impl Into<wgpu::TextureView>) -> Self {
        self.occlusion_texture = Some(texture.into());
        self.textures_flags.insert(TextureFlags::OCCLUSION);
        self
    }

    /// Defines how to sample the occlusion texture. Only used if a occlusion texture is set.
    /// Default to the default [`wgpu::Sampler`].
    pub fn occlusion_sampler(mut self, sampler: impl Into<wgpu::Sampler>) -> Self {
        self.occlusion_sampler = Some(sampler.into());
        self
    }

    /// Which geometry texture coordinates set should be used for texture coordinate mapping. Default to `0`.
    pub fn occlusion_tex_coord(mut self, tex_coord: u32) -> Self {
        self.uniform.occlusion_tex_coord = tex_coord;
        self
    }

    /// The factors for the emissive color of the material. Default to [`Vec3::ZERO`].
    pub fn emissive_factor(mut self, factor: impl Into<Vec3>) -> Self {
        self.uniform.emissive_factor = factor.into();
        self
    }

    /// Sample the emissive color from the texture. Default to uniform emissive factor.
    pub fn emissive_texture(mut self, texture: impl Into<wgpu::TextureView>) -> Self {
        self.emissive_texture = Some(texture.into());
        self.textures_flags.insert(TextureFlags::EMISSIVE);
        self
    }

    /// Defines how to sample the emissive texture. Only used if a emissive texture is set.
    /// Default to the default [`wgpu::Sampler`].
    pub fn emissive_sampler(mut self, sampler: impl Into<wgpu::Sampler>) -> Self {
        self.emissive_sampler = Some(sampler.into());
        self
    }

    /// Which geometry texture coordinates set should be used for texture coordinate mapping. Default to `0`.
    pub fn emissive_tex_coord(mut self, tex_coord: u32) -> Self {
        self.uniform.emissive_tex_coord = tex_coord;
        self
    }

    /// The alpha rendering mode of the material. Default to [`AlphaMode::Opaque`].
    pub fn alpha_mode(mut self, alpha_mode: impl Into<AlphaMode>) -> Self {
        self.alpha_mode = alpha_mode.into();
        self
    }

    /// The alpha cutoff value of the material. Only used when the alpha mode is set to [`AlphaMode::Mask`].
    /// Default to `0.5`.
    pub fn alpha_cutoff(mut self, alpha_cutoff: f32) -> Self {
        self.uniform.alpha_cutoff = alpha_cutoff;
        self
    }

    /// Specifies whether the material is double sided. Default to `false`.
    pub fn double_sided(mut self, double_sided: bool) -> Self {
        self.double_sided = double_sided;
        self
    }

    pub fn build(self, engine: &mut Engine) -> Material {
        let uniform_buffer = engine
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Material uniform buffer"),
                contents: cast_slice(&[self.uniform]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let normal_tex_coord = if self.normal_texture.is_some() {
            Some(self.uniform.normal_tex_coord)
        } else {
            None
        };

        let manager = &mut engine.material_manager;
        let default_texture = &manager.dummy_texture;
        let default_sampler = &manager.default_sampler;

        let base_color_texture = Texture {
            view: self
                .base_color_texture
                .unwrap_or_else(|| default_texture.clone()),
            sampler: self
                .base_color_sampler
                .unwrap_or_else(|| default_sampler.clone()),
        };

        let metallic_roughness_texture = Texture {
            view: self
                .metallic_roughness_texture
                .unwrap_or_else(|| default_texture.clone()),
            sampler: self
                .metallic_roughness_sampler
                .unwrap_or_else(|| default_sampler.clone()),
        };

        let normal_texture = Texture {
            view: self
                .normal_texture
                .unwrap_or_else(|| default_texture.clone()),
            sampler: self
                .normal_sampler
                .unwrap_or_else(|| default_sampler.clone()),
        };

        let occlusion_texture = Texture {
            view: self
                .occlusion_texture
                .unwrap_or_else(|| default_texture.clone()),
            sampler: self
                .occlusion_sampler
                .unwrap_or_else(|| default_sampler.clone()),
        };

        let emissive_texture = Texture {
            view: self
                .emissive_texture
                .unwrap_or_else(|| default_texture.clone()),
            sampler: self
                .emissive_sampler
                .unwrap_or_else(|| default_sampler.clone()),
        };

        let id = MaterialId(Uuid::new_v4());
        let data = MaterialData {
            id,
            name: Mutex::new(self.name),
            alpha_mode: self.alpha_mode,
            double_sided: self.double_sided,
            textures_flags: self.textures_flags,
            uniform_buffer,
            normal_tex_coord,
            base_color_texture,
            metallic_roughness_texture,
            normal_texture,
            occlusion_texture,
            emissive_texture,
        };
        let material = Material(Arc::new(data));
        manager.materials.insert(id, material.clone());

        material
    }
}

pub(crate) struct MaterialManager {
    materials: HashMap<MaterialId, Material>,
    dummy_texture: wgpu::TextureView,
    default_sampler: wgpu::Sampler,
}

impl MaterialManager {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
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

        Self {
            materials: HashMap::new(),
            dummy_texture,
            default_sampler,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct MaterialUniform {
    base_color_factor: Vec4,
    base_color_tex_coord: u32,
    metallic_factor: f32,
    roughness_factor: f32,
    metallic_roughness_tex_coord: u32,
    normal_scale: f32,
    normal_tex_coord: u32,
    occlusion_strength: f32,
    occlusion_tex_coord: u32,
    emissive_factor: Vec3,
    emissive_tex_coord: u32,
    alpha_cutoff: f32,
    _pad: [u32; 3],
}

impl Default for MaterialUniform {
    fn default() -> Self {
        MaterialUniform {
            base_color_factor: Vec4::ONE,
            base_color_tex_coord: 0,
            metallic_factor: 1.0,
            roughness_factor: 1.0,
            metallic_roughness_tex_coord: 0,
            normal_scale: 1.0,
            normal_tex_coord: 0,
            occlusion_strength: 1.0,
            occlusion_tex_coord: 0,
            emissive_factor: Vec3::ZERO,
            emissive_tex_coord: 0,
            alpha_cutoff: 0.5,
            _pad: [0; 3],
        }
    }
}
