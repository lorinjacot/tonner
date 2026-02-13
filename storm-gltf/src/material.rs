use std::path::Path;

use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};

use super::transforms::{
    default_4x10, default_05, default_10, is_0, is_3x00, is_4x10, is_05, is_10, is_false,
};

/// The material appearance of a primitive.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Material {
    /// [Some] if already loaded.
    #[serde(skip)]
    loaded: Option<storm::mesh::material::Material>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,

    /// A set of parameter values that are used to define the metallic-roughness material
    /// model from Physically Based Rendering (PBR) methodology. When undefined, all the
    /// default values of [PbrMetallicRoughness] **MUST** apply.
    #[serde(rename = "pbrMetallicRoughness")]
    #[serde(default)]
    #[serde(skip_serializing_if = "PbrMetallicRoughness::is_default")]
    pbr_metallic_roughness: PbrMetallicRoughness,

    /// The tangent space normal texture. The texture encodes RGB components with linear
    /// transfer function. Each texel represents the XYZ components of a normal vector in
    /// tangent space. The normal vectors use the convention +X is right and +Y is up. +Z
    /// points toward the viewer. If a fourth component (A) is present, it **MUST** be ignored.
    /// When undefined, the material does not have a tangent space normal texture.
    #[serde(rename = "normalTexture")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    normal_texture: Option<NormalTextureInfo>,

    /// The occlusion texture. The occlusion values are linearly sampled from the R channel.
    /// Higher values indicate areas that receive full indirect lighting and lower values
    /// indicate no indirect lighting. If other channels are present (GBA), they **MUST** be ignored
    /// for occlusion calculations. When undefined, the material does not have an occlusion texture.
    #[serde(rename = "occlusionTexture")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    occlusion_texture: Option<OcclusionTextureInfo>,

    /// The emissive texture. It controls the color and intensity of the light being emitted by the material. $
    /// This texture contains RGB components encoded with the sRGB transfer function. If a fourth component (A) is present,
    /// it **MUST** be ignored. When undefined, the texture **MUST** be sampled as having 1.0 in RGB components.
    #[serde(rename = "emissiveTexture")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    emissive_texture: Option<TextureInfo>,

    /// The factors for the emissive color of the material. This value defines
    /// linear multipliers for the sampled texels of the emissive texture.
    #[serde(rename = "emissiveFactor")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_3x00")]
    emissive_factor: [f32; 3],

    /// The material’s alpha rendering mode enumeration specifying the interpretation of the alpha value of the base color.
    #[serde(rename = "alphaMode")]
    #[serde(default)]
    #[serde(skip_serializing_if = "AlphaMode::is_default")]
    alpha_mode: AlphaMode,

    /// Specifies the cutoff threshold when in [MASK](AlphaMode::Mask). If the alpha value is
    /// greater than or equal to this value then it is rendered as fully opaque, otherwise,
    /// it is rendered as fully transparent. A value greater than 1.0 will render the entire
    /// material as fully transparent. This value **MUST** be ignored for other alpha modes.
    /// When [alphaMode](Material::alpha_mode) is not defined, this value **MUST NOT** be defined.
    #[serde(rename = "alphaCutoff")]
    #[serde(default = "default_05")]
    #[serde(skip_serializing_if = "is_05")]
    alpha_cutoff: f32,

    /// Specifies whether the material is double sided. When this value is false,
    /// back-face culling is enabled. When this value is true, back-face culling is
    /// disabled and double-sided lighting is enabled. The back-face **MUST** have
    /// its normals reversed before the lighting equation is evaluated.
    #[serde(rename = "doubleSided")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    double_sided: bool,
}

impl Material {
    pub(super) fn load(
        &mut self,
        base_path: &Path,
        textures: &mut [super::Texture],
        samplers: &mut [super::Sampler],
        buffer_views: &[super::BufferView],
        buffers: &[super::Buffer],
        images: &mut [super::Image],
        ctx: &storm::Context,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<storm::mesh::material::Material> {
        if let Some(material) = self.loaded.clone() {
            return Ok(material);
        }

        let pbr = &self.pbr_metallic_roughness;

        let mut builder = storm::mesh::material::MaterialBuilder::default()
            .base_color_factor(pbr.base_color_factor)
            .metallic_factor(pbr.metallic_factor)
            .roughness_factor(pbr.roughness_factor)
            .emissive_factor(self.emissive_factor)
            .alpha_mode(match self.alpha_mode {
                AlphaMode::Opaque => storm::mesh::material::AlphaMode::Opaque,
                AlphaMode::Mask => storm::mesh::material::AlphaMode::Mask,
                AlphaMode::Blend => storm::mesh::material::AlphaMode::Blend,
            })
            .alpha_cutoff(self.alpha_cutoff)
            .double_sided(self.double_sided);

        if let Some(info) = &pbr.base_color_texture {
            let (texture, sampler) = textures
                .get_mut(info.index)
                .ok_or(anyhow!(
                    "material.base_color_texture {} is out of range",
                    info.index
                ))?
                .load(
                    true,
                    base_path,
                    samplers,
                    images,
                    buffer_views,
                    buffers,
                    ctx,
                    encoder,
                )
                .with_context(|| {
                    format!("Failed to load material.base_color_texture {}", info.index)
                })?;
            builder = builder
                .base_color_texture(texture)
                .base_color_sampler(sampler)
                .base_color_tex_coord(info.tex_coord as u32);
        }

        if let Some(info) = &pbr.metallic_roughness_texture {
            let (view, sampler) = textures
                .get_mut(info.index)
                .ok_or(anyhow!(
                    "material.metallic_roughness_texture {} is out of range",
                    info.index
                ))?
                .load(
                    false,
                    base_path,
                    samplers,
                    images,
                    buffer_views,
                    buffers,
                    ctx,
                    encoder,
                )
                .with_context(|| {
                    format!(
                        "Failed to load material.metallic_roughness_texture {}",
                        info.index
                    )
                })?;
            builder = builder
                .metallic_roughness_texture(view)
                .metallic_roughness_sampler(sampler)
                .metallic_roughness_tex_coord(info.tex_coord as u32);
        }

        if let Some(info) = &self.normal_texture {
            let (view, sampler) = textures
                .get_mut(info.index)
                .ok_or(anyhow!(
                    "material.normal_texture {} is out of range",
                    info.index
                ))?
                .load(
                    false,
                    base_path,
                    samplers,
                    images,
                    buffer_views,
                    buffers,
                    ctx,
                    encoder,
                )
                .with_context(|| {
                    format!("Failed to load material.normal_texture {}", info.index)
                })?;
            builder = builder
                .normal_texture(view)
                .normal_sampler(sampler)
                .normal_tex_coord(info.tex_coord as u32)
                .normal_scale(info.scale);
        }

        if let Some(info) = &self.occlusion_texture {
            let (view, sampler) = textures
                .get_mut(info.index)
                .ok_or(anyhow!(
                    "material.occlusion_texture {} is out of range",
                    info.index
                ))?
                .load(
                    true,
                    base_path,
                    samplers,
                    images,
                    buffer_views,
                    buffers,
                    ctx,
                    encoder,
                )
                .with_context(|| {
                    format!("Failed to load material.occlusion_texture {}", info.index)
                })?;
            builder = builder
                .occlusion_texture(view)
                .occlusion_sampler(sampler)
                .occlusion_tex_coord(info.tex_coord as u32)
                .occlusion_strength(info.strength);
        }

        if let Some(info) = &self.emissive_texture {
            let (view, sampler) = textures
                .get_mut(info.index)
                .ok_or(anyhow!(
                    "material.emissive_texture {} is out of range",
                    info.index
                ))?
                .load(
                    true,
                    base_path,
                    samplers,
                    images,
                    buffer_views,
                    buffers,
                    ctx,
                    encoder,
                )
                .with_context(|| {
                    format!("Failed to load material.emissive_texture {}", info.index)
                })?;
            builder = builder
                .emissive_texture(view)
                .emissive_sampler(sampler)
                .emissive_tex_coord(info.tex_coord as u32);
        }

        let material = builder.build(ctx);
        self.loaded = Some(material.clone());
        Ok(material)
    }
}

impl Default for Material {
    fn default() -> Self {
        Self {
            loaded: None,
            name: None,
            pbr_metallic_roughness: PbrMetallicRoughness::default(),
            normal_texture: None,
            occlusion_texture: None,
            emissive_texture: None,
            emissive_factor: Default::default(),
            alpha_mode: AlphaMode::default(),
            alpha_cutoff: 0.5,
            double_sided: false,
        }
    }
}

/// A set of parameter values that are used to define the metallic-roughness material model
/// from Physically-Based Rendering (PBR) methodology.
#[derive(Debug, Serialize, Deserialize)]
struct PbrMetallicRoughness {
    /// The factors for the base color of the material. This value defines linear multipliers
    /// for the sampled texels of the base color texture.
    #[serde(rename = "baseColorFactor")]
    #[serde(default = "default_4x10")]
    #[serde(skip_serializing_if = "is_4x10")]
    base_color_factor: [f32; 4],

    /// The base color texture. The first three components (RGB) **MUST** be encoded with the
    /// sRGB transfer function. They specify the base color of the material. If the fourth
    /// component (A) is present, it represents the linear alpha coverage of the material.
    /// Otherwise, the alpha coverage is equal to 1.0. The [material.alphaMode](Material::alpha_mode)
    /// property specifies how alpha is interpreted. The stored texels **MUST NOT** be premultiplied.
    /// When undefined, the texture **MUST** be sampled as having `1.0` in all components.
    #[serde(rename = "baseColorTexture")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    base_color_texture: Option<TextureInfo>,

    /// The factor for the metalness of the material. This value defines a linear multiplier
    /// or the sampled metalness values of the metallic-roughness texture.
    #[serde(rename = "metallicFactor")]
    #[serde(default = "default_10")]
    #[serde(skip_serializing_if = "is_10")]
    metallic_factor: f32,

    /// The factor for the roughness of the material. This value defines a linear multiplier
    /// for the sampled roughness values of the metallic-roughness texture.
    #[serde(rename = "roughnessFactor")]
    #[serde(default = "default_10")]
    #[serde(skip_serializing_if = "is_10")]
    roughness_factor: f32,

    /// The metallic-roughness texture. The metalness values are sampled from the B channel.
    /// The roughness values are sampled from the G channel. These values **MUST** be encoded
    /// with a linear transfer function. If other channels are present (R or A), they **MUST**
    /// be ignored for metallic-roughness calculations. When undefined, the texture **MUST**
    /// be sampled as having 1.0 in G and B components.
    #[serde(rename = "metallicRoughnessTexture")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    metallic_roughness_texture: Option<TextureInfo>,
}

impl PbrMetallicRoughness {
    fn is_default(&self) -> bool {
        self.base_color_factor == [1.0; 4]
            && self.base_color_texture.is_none()
            && self.metallic_factor == 1.0
            && self.roughness_factor == 1.0
            && self.metallic_roughness_texture.is_none()
    }
}

impl Default for PbrMetallicRoughness {
    fn default() -> Self {
        PbrMetallicRoughness {
            base_color_factor: [1.0; 4],
            base_color_texture: None,
            metallic_factor: 1.0,
            roughness_factor: 1.0,
            metallic_roughness_texture: None,
        }
    }
}

/// Reference to a texture.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TextureInfo {
    /// The index of the texture.
    index: usize,

    /// This integer value is used to construct a string in the format `TEXCOORD_<set index>`
    /// which is a reference to a key in [mesh.primitives.attributes](MeshPrimitive::attributes)
    /// (e.g. a value of `0` corresponds to [TEXCOORD_0](PrimitiveAttributes::tex_coord_0)).
    /// A mesh primitive **MUST** have the corresponding texture coordinate attributes for
    /// the material to be applicable to it.
    #[serde(rename = "texCoord")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_0")]
    tex_coord: usize,
}

/// Reference to a texture.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NormalTextureInfo {
    /// The index of the texture.
    index: usize,

    /// This integer value is used to construct a string in the format `TEXCOORD_<set index>`
    /// which is a reference to a key in [mesh.primitives.attributes](MeshPrimitive::attributes)
    /// (e.g. a value of `0` corresponds to [TEXCOORD_0](PrimitiveAttributes::tex_coord_0)).
    /// A mesh primitive **MUST** have the corresponding texture coordinate attributes for
    /// the material to be applicable to it.
    #[serde(rename = "texCoord")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_0")]
    tex_coord: usize,

    /// The scalar parameter applied to each normal vector of the texture. This value scales
    /// the normal vector in X and Y directions using the formula:
    /// `scaledNormal = normalize(<sampled normal texture value> * 2.0 - 1.0) * vec3(<normal scale>, <normal scale>, 1.0)`.
    #[serde(default = "default_10")]
    #[serde(skip_serializing_if = "is_10")]
    scale: f32,
}

/// Reference to a texture.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OcclusionTextureInfo {
    /// The index of the texture.
    index: usize,

    /// This integer value is used to construct a string in the format `TEXCOORD_<set index>`
    /// which is a reference to a key in [mesh.primitives.attributes](MeshPrimitive::attributes)
    /// (e.g. a value of `0` corresponds to [TEXCOORD_0](PrimitiveAttributes::tex_coord_0)).
    /// A mesh primitive **MUST** have the corresponding texture coordinate attributes for
    /// the material to be applicable to it.
    #[serde(rename = "texCoord")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_0")]
    tex_coord: usize,

    /// A scalar parameter controlling the amount of occlusion applied. A value of `0.0` means no occlusion.
    /// A value of `1.0` means full occlusion. This value affects the final occlusion value as:
    /// `1.0 + strength * (<sampled occlusion texture value> - 1.0)`.
    #[serde(default = "default_10")]
    #[serde(skip_serializing_if = "is_10")]
    strength: f32,
}

/// The material’s alpha rendering mode enumeration specifying the interpretation of the alpha value of the base color.
#[derive(Debug, Default, Serialize, Deserialize)]
enum AlphaMode {
    /// The alpha value is ignored, and the rendered output is fully opaque.
    #[default]
    #[serde(rename = "OPAQUE")]
    Opaque,

    /// The rendered output is either fully opaque or fully transparent depending
    /// on the alpha value and the specified [alphaCutoff](Material::alpha_cutoff) value;
    /// the exact appearance of the edges **MAY** be subject to implementation-specific
    /// techniques such as “Alpha-to-Coverage”.
    #[serde(rename = "MASK")]
    Mask,

    /// The alpha value is used to composite the source and destination areas.
    /// The rendered output is combined with the background using the normal painting operation
    /// (i.e. the Porter and Duff over operator).
    #[serde(rename = "BLEND")]
    Blend,
}

impl AlphaMode {
    fn is_default(&self) -> bool {
        match self {
            AlphaMode::Opaque => true,
            _ => false,
        }
    }
}
