use data_url::forgiving_base64::InvalidBase64;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::{fmt::Display, num::NonZeroUsize, path::PathBuf};
use thiserror::Error;

use crate::Id;

use accessor::{Accessor, AccessorComponentType, AccessorType};
use transforms::{
    default_4x10, default_05, default_10, is_0, is_3x00, is_4x10, is_05, is_10, is_false,
};

mod accessor;
mod load;
mod transforms;

#[derive(Error, Debug)]
pub enum GltfError {
    #[error("Invalid binary gltf container: {0}")]
    Glb(#[from] GlbError),
    #[error("Failed to parse json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid {entity} index: {index}")]
    InvalidIndex { entity: GltfEntity, index: usize },
    #[error(
        "{usage} cannot have accessor with {accessor_type} of {component_type} (normalized: {normalized})"
    )]
    InvalidAccessorDataType {
        accessor_type: AccessorType,
        component_type: AccessorComponentType,
        normalized: bool,
        usage: AccessorUsage,
    },
    #[error("Failed to read external image: {0}")]
    Image(#[from] image::ImageError),
    #[error("Invalid base64 encoding")]
    InvalidBase64(#[from] InvalidBase64),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug)]
pub enum GltfEntity {
    Accessor,
    AnimationSampler,
    Node,
    Scene,
    Skin,
}

impl Display for GltfEntity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accessor => write!(f, "accessor"),
            Self::AnimationSampler => write!(f, "animation sampler"),
            Self::Node => write!(f, "node"),
            Self::Scene => write!(f, "scene"),
            Self::Skin => write!(f, "skin"),
        }
    }
}

#[derive(Debug)]
pub enum AccessorUsage {
    Indices,
    AnimationOutpus { path: AnimationTargetPath },
}

impl Display for AccessorUsage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Indices => write!(f, "Primitive indices"),
            Self::AnimationOutpus { path } => write!(f, "Animation outputs value for {path}"),
        }
    }
}

#[derive(Error, Debug)]
pub enum GlbError {
    #[error("Binary glTF container version should be 2")]
    UnknownVersion,
    #[error("A glb asset must have a JSON chunk as first chunk")]
    JsonChunkMissing,
    #[error("This glb asset must have a BIN chunk as second chunk")]
    BinChunkMissing,
    #[error("Invalid chunk length")]
    InvalidChunkLength,
}

pub struct GltfAsset {
    parent: PathBuf,
    json: Gltf,
    default_material: Option<Id<crate::material::Material>>,
}

const GLB_HEADER_SIZE: u32 = 3 * size_of::<u32>() as u32;
const GLTF: u32 = 0x46546C67;
const JSON: u32 = 0x4E4F534A;
const BIN: u32 = 0x004E4942;

struct GlbChunk {
    chunk_type: u32,
    chunk_data: Vec<u8>,
}

/// The root object for a glTF asset.
#[derive(Debug, Serialize, Deserialize)]
struct Gltf {
    /// Names of glTF extensions used in this asset.
    #[serde(rename = "extensionsUsed")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    extensions_used: Vec<String>,

    /// Names of glTF extensions required to properly load this asset.
    #[serde(rename = "extensionsRequired")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    extensions_required: Vec<String>,

    /// An array of accessors. An accessor is a typed view into a bufferView.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    accessors: Vec<Accessor>,

    /// An array of keyframe animations.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    animations: Vec<Animation>,

    /// Metadata about the glTF asset.
    asset: Asset,

    /// An array of buffers. A buffer points to binary geometry, animation, or skins.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    buffers: Vec<Buffer>,

    /// An array of bufferViews. A bufferView is a view into a buffer generally representing a subset of the buffer.
    #[serde(rename = "bufferViews")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    buffer_views: Vec<BufferView>,

    /// An array of cameras. A camera defines a projection matrix.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cameras: Vec<Camera>,

    /// An array of images. An image defines data used to create a texture.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    images: Vec<Image>,

    /// An array of materials. A material defines the appearance of a primitive.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    materials: Vec<Material>,

    /// An array of meshes. A mesh is a set of primitives to be rendered.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    meshes: Vec<Mesh>,

    /// An array of nodes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    nodes: Vec<Node>,

    /// An array of samplers. A sampler contains properties for texture filtering and wrapping modes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    samplers: Vec<Sampler>,

    /// The index of the default scene. This property **MUST NOT** be defined, when [scenes](Gltf::scenes) is undefined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    scene: Option<usize>,

    /// An array of scenes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    scenes: Vec<Scene>,

    /// An array of skins. A skin is defined by joints and matrices.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    skins: Vec<Skin>,

    /// An array of textures.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    textures: Vec<Texture>,
}

/// Metadata about the glTF asset.
#[derive(Debug, Serialize, Deserialize)]
struct Asset {
    /// A copyright message suitable for display to credit the content creator.
    copyright: Option<String>,

    /// Tool that generated this glTF model. Useful for debugging.
    generator: Option<String>,

    /// The glTF version in the form of `<major>.<minor>` that this asset targets.
    version: String,

    /// The minimum glTF version in the form of `<major>.<minor>` that this asset targets.
    /// This property **MUST NOT** be greater than the asset version.
    #[serde(rename = "minVersion")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    min_version: Option<String>,
}

/// A keyframe animation.
#[derive(Debug, Serialize, Deserialize)]
struct Animation {
    /// An array of animation channels. An animation channel combines an
    /// animation sampler with a target property being animated. Different
    /// channels of the same animation **MUST NOT** have the same targets.
    channels: Vec<AnimationChannel>,

    /// An array of animation samplers. An animation sampler combines timestamps
    /// with a sequence of output values and defines an interpolation algorithm.
    samplers: Vec<AnimationSampler>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// An animation channel combines an animation sampler with a target property being animated.
#[derive(Debug, Serialize, Deserialize)]
struct AnimationChannel {
    /// The index of a sampler in this animation used to compute the value for the target,
    /// e.g., a node’s translation, rotation, or scale (TRS).
    sampler: usize,

    /// The descriptor of the animated property.
    target: AnimationTarget,
}

/// The descriptor of the animated property.
#[derive(Debug, Serialize, Deserialize)]
struct AnimationTarget {
    /// The index of the node to animate. When undefined, the animated object
    /// **MAY** be defined by an extension.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    node: Option<usize>,

    /// The name of the node’s TRS property to animate, or the "weights" of
    /// the Morph Targets it instantiates. For the [Translation](AnimationTargetPath::Translation)
    /// property, the values that are provided by the sampler are the translation along
    /// the X, Y, and Z axes. For the [Rotation](AnimationTargetPath::Rotation) property,
    /// the values are a quaternion in the order (x, y, z, w), where w is the scalar.
    /// For the [Scale](AnimationTargetPath::Scale) property, the values are the scaling
    /// factors along the X, Y, and Z axes.
    path: AnimationTargetPath,
}

/// The name of the node’s TRS property to animate, or the "weights" of
/// the Morph Targets it instantiates. For the [Translation](AnimationTargetPath::Translation)
/// property, the values that are provided by the sampler are the translation along
/// the X, Y, and Z axes. For the [Rotation](AnimationTargetPath::Rotation) property,
/// the values are a quaternion in the order (x, y, z, w), where w is the scalar.
/// For the [Scale](AnimationTargetPath::Scale) property, the values are the scaling
/// factors along the X, Y, and Z axes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AnimationTargetPath {
    #[serde(rename = "translation")]
    Translation,

    #[serde(rename = "rotation")]
    Rotation,

    #[serde(rename = "scale")]
    Scale,

    #[serde(rename = "weights")]
    Weights,
}

impl Display for AnimationTargetPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Translation => "translation",
            Self::Rotation => "rotation",
            Self::Scale => "scale",
            Self::Weights => "weights",
        })
    }
}

/// An animation sampler combines timestamps with a sequence of output values and defines an interpolation algorithm.
#[derive(Debug, Serialize, Deserialize)]
struct AnimationSampler {
    /// The index of an accessor containing keyframe timestamps. The accessor **MUST** be of scalar type with
    /// floating-point components. The values represent time in seconds with `time[0] >= 0.0`, and strictly
    /// increasing values, i.e., `time[n + 1] > time[n]`.
    input: usize,

    /// Interpolation algorithm.
    #[serde(default)]
    #[serde(skip_serializing_if = "AnimationInterpolation::is_default")]
    interpolation: AnimationInterpolation,

    /// The index of an accessor, containing keyframe output values.
    output: usize,
}

/// Interpolation algorithm.
#[derive(Debug, Default, Serialize, Deserialize)]
enum AnimationInterpolation {
    /// The animated values are linearly interpolated between keyframes.
    /// When targeting a rotation, spherical linear interpolation (slerp)
    /// **SHOULD** be used to interpolate quaternions. The number of
    /// output elements **MUST** equal the number of input elements.
    #[default]
    #[serde(rename = "LINEAR")]
    Linear,

    /// The animated values remain constant to the output of the first keyframe,
    /// until the next keyframe. The number of output elements **MUST** equal the
    /// number of input elements.
    #[serde(rename = "STEP")]
    Step,

    /// The animation’s interpolation is computed using a cubic spline with
    /// specified tangents. The number of output elements **MUST** equal three
    /// times the number of input elements. For each input element, the output
    /// stores three elements, an in-tangent, a spline vertex, and an out-tangent.
    /// There **MUST** be at least two keyframes when using this interpolation.
    #[serde(rename = "CUBICSPLINE")]
    Cubicspline,
}

impl AnimationInterpolation {
    fn is_default(&self) -> bool {
        match self {
            AnimationInterpolation::Linear => true,
            _ => false,
        }
    }
}

/// A buffer points to binary geometry, animation, or skins.
#[derive(Debug, Serialize, Deserialize)]
struct Buffer {
    /// The content of the buffer. Empty if unsupported uri.
    #[serde(skip)]
    bytes: Vec<u8>,

    /// The URI (or IRI) of the buffer. Relative paths are relative to
    /// the current glTF asset. Instead of referencing an external file,
    /// this field **MAY** contain a `data:`-URI.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    uri: Option<String>,

    /// The length of the buffer in bytes.
    #[serde(rename = "byteLength")]
    byte_length: usize,

    /// The user-defined name of this object. This is not necessarily unique,
    /// e.g., an accessor and a buffer could have the same name, or two accessors
    /// could even have the same name.
    name: Option<String>,
}

/// A view into a buffer generally representing a subset of the buffer.
#[derive(Debug, Serialize, Deserialize)]
struct BufferView {
    /// The index of the buffer.
    buffer: usize,

    /// The offset into the buffer in bytes.
    #[serde(rename = "byteOffset")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_0")]
    byte_offset: usize,

    /// The length of the bufferView in bytes.
    #[serde(rename = "byteLength")]
    byte_length: usize,

    /// The stride, in bytes, between vertex attributes. When this is not
    /// defined, data is tightly packed. When two or more accessors use the
    /// same buffer view, this field **MUST** be defined.
    #[serde(rename = "byteStride")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    byte_stride: Option<NonZeroUsize>,

    /// The hint representing the intended GPU buffer type to use with this buffer view.
    #[serde(default)]
    #[serde(skip_serializing_if = "BufferViewTarget::is_none")]
    target: BufferViewTarget,

    /// The user-defined name of this object. This is not necessarily unique,
    /// e.g., an accessor and a buffer could have the same name, or two accessors
    /// could even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Default, Serialize_repr, Deserialize_repr)]
#[repr(u32)]
enum BufferViewTarget {
    #[default]
    None = 0,
    ArrayBuffer = 34962,
    ElementArrayBuffer = 34963,
}

impl BufferViewTarget {
    fn is_none(&self) -> bool {
        match self {
            BufferViewTarget::None => true,
            _ => false,
        }
    }
}

///A camera’s projection. A node **MAY** reference a camera to apply a transform to place the camera in the scene.
#[derive(Debug, Serialize, Deserialize)]
struct Camera {
    /// An orthographic camera containing properties to create an orthographic projection matrix.
    /// This property **MUST NOT** be defined when [perspective](Camera::perspective) is defined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    orthographic: Option<OrthographicCamera>,

    /// A perspective camera containing properties to create a perspective projection matrix.
    /// This property **MUST NOT** be defined when [orthographic](Camera::orthographic) is defined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    perspective: Option<PerspectiveCamera>,

    /// Specifies if the camera uses a perspective or orthographic projection.
    /// Based on this, either the camera’s [perspective](Camera::perspective)
    /// or [orthographic](Camera::orthographic) property **MUST** be defined.
    #[serde(rename = "type")]
    type_: CameraType,

    /// The user-defined name of this object. This is not necessarily unique,
    /// e.g., an accessor and a buffer could have the same name, or two accessors
    /// could even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// An orthographic camera containing properties to create an orthographic projection matrix.
#[derive(Debug, Serialize, Deserialize)]
struct OrthographicCamera {
    /// The floating-point horizontal magnification of the view. This value **MUST NOT**
    /// be equal to zero. This value **SHOULD NOT** be negative.
    xmag: f32,

    /// The floating-point vertical magnification of the view. This value **MUST NOT**
    /// be equal to zero. This value **SHOULD NOT** be negative.
    ymag: f32,

    /// The floating-point distance to the far clipping plane. This value **MUST NOT**
    /// be equal to zero. zfar **MUST** be greater than znear.
    zfar: f32,

    /// The floating-point distance to the near clipping plane.
    znear: f32,
}

/// A perspective camera containing properties to create a perspective projection matrix.
#[derive(Debug, Serialize, Deserialize)]
struct PerspectiveCamera {
    /// The floating-point aspect ratio of the field of view. When undefined, the aspect
    /// ratio of the rendering viewport **MUST** be used.
    #[serde(rename = "aspectRatio")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    aspect_ratio: Option<f32>,

    /// The floating-point vertical field of view in radians. This value **SHOULD** be less than π.
    yfov: f32,

    /// The floating-point distance to the far clipping plane. When defined, `zfar` **MUST** be greater
    /// than [znear](PerspectiveCamera::znear). If `zfar` is undefined, client implementations **SHOULD**
    /// use infinite projection matrix.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    zfar: Option<f32>,

    /// The floating-point distance to the near clipping plane.
    znear: f32,
}

/// Specifies if the camera uses a perspective or orthographic projection.
/// Based on this, either the camera’s [perspective](Camera::perspective)
/// or [orthographic](Camera::orthographic) property **MUST** be defined.
#[derive(Debug, Serialize, Deserialize)]
enum CameraType {
    #[serde(rename = "perspective")]
    Perspective,

    #[serde(rename = "orthographic")]
    Orthographic,
}

/// Image data used to create a texture. Image **MAY** be referenced by an URI (or IRI) or a buffer view index.
#[derive(Debug, Serialize, Deserialize)]
struct Image {
    /// wgpu texture, if the resource has been loaded.
    #[serde(skip)]
    wgpu: Option<wgpu::Texture>,

    /// The URI (or IRI) of the image. Relative paths are relative to the current glTF asset.
    /// Instead of referencing an external file, this field **MAY** contain a `data:`-URI.
    /// This field **MUST NOT** be defined when [bufferView](Image::buffer_view) is defined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    uri: Option<String>,

    /// The image’s media type. This field **MUST** be defined when
    /// [buffer_view](Image::buffer_view) is defined.
    #[serde(rename = "mimeType")]
    #[serde(default)]
    #[serde(skip_serializing_if = "ImageMimeType::is_none")]
    mime_type: ImageMimeType,

    /// The index of the [BufferView] that contains the image.
    /// This field **MUST NOT** be defined when [uri](Image::uri) is defined.
    #[serde(rename = "bufferView")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    buffer_view: Option<usize>,

    /// The user-defined name of this object. This is not necessarily unique,
    /// e.g., an accessor and a buffer could have the same name, or two accessors
    /// could even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// The image’s media type. This field **MUST** be defined when
/// [bufferView](Image::buffer_view) is defined.
#[derive(Debug, Default, Serialize, Deserialize)]
enum ImageMimeType {
    #[default]
    None,

    #[serde(rename = "image/jpeg")]
    ImageJpeg,

    #[serde(rename = "image/png")]
    ImagePng,
}

impl ImageMimeType {
    fn is_none(&self) -> bool {
        match self {
            ImageMimeType::None => true,
            _ => false,
        }
    }
}

/// The material appearance of a primitive.
#[derive(Debug, Serialize, Deserialize)]
struct Material {
    /// Storm storage id, if the resource has been loaded.
    #[serde(skip)]
    id: Option<Id<crate::material::Material>>,

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

impl Default for Material {
    fn default() -> Self {
        Self {
            id: None,
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

/// A set of primitives to be rendered. Its global transform is defined by a node that references it.
#[derive(Debug, Serialize, Deserialize)]
struct Mesh {
    /// Storm storage id, if the resource has been loaded.
    #[serde(skip)]
    id: Option<Id<crate::mesh::Mesh>>,

    /// An array of primitives, each defining geometry to be rendered.
    primitives: Vec<MeshPrimitive>,

    /// Array of weights to be applied to the morph targets. The number of array
    /// elements **MUST** match the number of morph targets.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    weights: Option<Vec<f32>>,

    /// The user-defined name of this object. This is not necessarily unique,
    /// e.g., an accessor and a buffer could have the same name, or two accessors
    /// could even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// Geometry to be rendered with the given material.
#[derive(Debug, Serialize, Deserialize)]
struct MeshPrimitive {
    /// A plain JSON object, where each key corresponds to a mesh attribute semantic
    /// and each value is the index of the accessor containing attribute’s data.
    attributes: PrimitiveAttributes,

    /// The index of the accessor that contains the vertex indices. When this is undefined,
    /// the primitive defines non-indexed geometry. When defined, the accessor **MUST** have
    /// [SCALAR](AccessorType::Scalar) type and an unsigned integer component type.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    indices: Option<usize>,

    /// The index of the material to apply to this primitive when rendering.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    material: Option<usize>,

    /// The topology type of primitives to render.
    #[serde(default)]
    #[serde(skip_serializing_if = "PrimitiveMode::is_default")]
    mode: PrimitiveMode,

    /// An array of morph targets.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    targets: Vec<MorphTarget>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PrimitiveAttributes {
    /// Unitless XYZ vertex positions
    #[serde(rename = "POSITION")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<usize>,

    /// Normalized XYZ vertex normals
    #[serde(rename = "NORMAL")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    normal: Option<usize>,

    /// XYZW vertex tangents where the XYZ portion is normalized,
    /// and the W component is a sign value (-1 or +1) indicating
    /// handedness of the tangent basis
    #[serde(rename = "TANGENT")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    tangent: Option<usize>,

    /// ST texture coordinates
    #[serde(rename = "TEXCOORD_0")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    tex_coord_0: Option<usize>,

    /// ST texture coordinates
    #[serde(rename = "TEXCOORD_1")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    tex_coord_1: Option<usize>,

    /// RGB or RGBA vertex color linear multiplier
    #[serde(rename = "COLOR_0")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    color_0: Option<usize>,

    /// Ondices of the skin joints
    #[serde(rename = "JOINTS_0")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    joints_0: Option<usize>,

    /// How strongly the skin joint influences the vertex
    #[serde(rename = "WEIGHTS_0")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    weights_0: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MorphTarget {
    /// XYZ vertex position displacements
    #[serde(rename = "POSITION")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<usize>,

    /// XYZ vertex normal displacements
    #[serde(rename = "NORMAL")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    normal: Option<usize>,

    /// XYZ vertex tangent displacements
    #[serde(rename = "TANGENT")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    tangent: Option<usize>,

    /// ST texture coordinate displacements
    #[serde(rename = "TEXCOORD_0")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    tex_coord_0: Option<usize>,

    /// ST texture coordinate displacements
    #[serde(rename = "TEXCOORD_1")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    tex_coord_1: Option<usize>,

    /// RGB or RGBA color deltas
    #[serde(rename = "COLOR_n")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    color_0: Option<usize>,
}

/// The topology type of primitives to render.
#[derive(Debug, Clone, Copy, Default, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
enum PrimitiveMode {
    Points = 0,
    Lines = 1,
    LineLoop = 2,
    LineStrip = 3,
    #[default]
    Triangles = 4,
    TriangleStrip = 5,
    TriangleFan = 6,
}

impl PrimitiveMode {
    fn is_default(&self) -> bool {
        match self {
            PrimitiveMode::Triangles => true,
            _ => false,
        }
    }
}

impl Display for PrimitiveMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Points => "points",
                Self::Lines => "lines",
                Self::LineLoop => "line loop",
                Self::LineStrip => "line strip",
                Self::Triangles => "triangles",
                Self::TriangleStrip => "triangle strip",
                Self::TriangleFan => "triangle fan",
            }
        )
    }
}

/// A node in the node hierarchy. When the node contains [skin](Node::skin),
/// all [mesh.primitives](Mesh::primitives) **MUST** contain [JOINTS_0](PrimitiveAttributes::joints_0)
/// and [WEIGHTS_0](PrimitiveAttributes::weights_0) attributes.
/// A node **MAY** have either a `matrix` or any combination of
/// [translation](Node::translation)/[rotation](Node::rotation)/[scale](Node::scale)
/// (TRS) properties. TRS properties are converted to matrices and postmultiplied in the
/// `T * R * S` order to compose the transformation matrix; first the scale is applied to
/// the vertices, then the rotation, and then the translation. If none are provided, the
/// transform is the identity. When a node is targeted for animation (referenced by an
/// [animation.channel.target](AnimationChannel::target)), [matrix](Node::matrix)
/// **MUST NOT** be present.
#[derive(Debug, Serialize, Deserialize)]
struct Node {
    /// Storm storage id, if the resource has been loaded. Cleared once the scene has been loaded.
    #[serde(skip)]
    id: Option<Id<crate::Node>>,

    /// The index of the camera referenced by this node.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    camera: Option<usize>,

    /// The indices of this node's children.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<usize>,

    /// The index of the skin referenced by this node.
    /// When a skin is referenced by a node within a scene,
    /// all joints used by the skin **MUST** belong to the same scene.
    /// When defined, [mesh](Node::mesh) **MUST** also be defined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    skin: Option<usize>,

    /// A floating-point 4x4 transformation matrix stored in column-major order.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    matrix: Option<[f32; 16]>,

    /// The index of the mesh in this node.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    mesh: Option<usize>,

    /// The node’s unit quaternion rotation in the order (x, y, z, w), where w is the scalar.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    rotation: Option<[f32; 4]>,

    /// The node’s non-uniform scale, given as the scaling factors along the x, y, and z axes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    scale: Option<[f32; 3]>,

    /// The node’s translation along the x, y, and z axes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    translation: Option<[f32; 3]>,

    /// The weights of the instantiated morph target. The number of array elements
    /// **MUST** match the number of morph targets of the referenced mesh.
    /// When defined, [mesh](Node::mesh) **MUST** also be defined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    weights: Option<Vec<f32>>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// Texture sampler properties for filtering and wrapping modes.
#[derive(Debug, Serialize, Deserialize)]
struct Sampler {
    /// wgpu sampler, if the resource has been loaded.
    #[serde(skip)]
    wgpu: Option<wgpu::Sampler>,

    /// Magnification filter.
    #[serde(rename = "magFilter")]
    #[serde(default)]
    #[serde(skip_serializing_if = "MagFilter::is_none")]
    mag_filter: MagFilter,

    /// Minification filter.
    #[serde(rename = "minFilter")]
    #[serde(default)]
    #[serde(skip_serializing_if = "MinFilter::is_none")]
    min_filter: MinFilter,

    /// S (U) wrapping mode. All valid values correspond to WebGL enums.
    #[serde(rename = "wrapS")]
    #[serde(default)]
    #[serde(skip_serializing_if = "WrappingMode::is_none")]
    wrap_s: WrappingMode,

    /// T (V) wrapping mode.
    #[serde(rename = "wrapT")]
    #[serde(default)]
    #[serde(skip_serializing_if = "WrappingMode::is_none")]
    wrap_t: WrappingMode,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// Magnification filter.
#[derive(Debug, Default, Serialize_repr, Deserialize_repr)]
#[repr(u32)]
enum MagFilter {
    #[default]
    None = 0,
    Nearest = 9728,
    Linear = 9729,
}

impl MagFilter {
    fn is_none(&self) -> bool {
        match self {
            MagFilter::None => true,
            _ => false,
        }
    }
}

/// Minification filter.
#[derive(Debug, Default, Serialize_repr, Deserialize_repr)]
#[repr(u32)]
enum MinFilter {
    #[default]
    None = 0,
    Nearest = 9728,
    Linear = 9729,
    NearestMipmapNearest = 9984,
    LinearMipmapNearest = 9985,
    NearestMipmapLinear = 9986,
    LinearMipmapLinear = 9987,
}

impl MinFilter {
    fn is_none(&self) -> bool {
        match self {
            MinFilter::None => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize_repr, Deserialize_repr)]
#[repr(u32)]
enum WrappingMode {
    #[default]
    None = 0,
    ClampToEdge = 33071,
    MirroredRepeat = 33648,
    Repeat = 10497,
}

impl WrappingMode {
    fn is_none(&self) -> bool {
        match self {
            WrappingMode::None => true,
            _ => false,
        }
    }
}

/// The root nodes of a scene.
#[derive(Debug, Serialize, Deserialize)]
struct Scene {
    /// The indices of each root node.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    nodes: Vec<usize>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

// Joints and matrices defining a skin.
#[derive(Debug, Serialize, Deserialize)]
struct Skin {
    /// Nodes using this skin. Cleared once the scene has been loaded.
    #[serde(skip)]
    nodes: Vec<Id<crate::Node>>,

    /// The index of the accessor containing the floating-point 4x4 inverse-bind matrices.
    /// Its [accessor.count](Accessor::count) property **MUST** be greater than or equal to
    /// the number of elements of the joints array. When undefined, each matrix is a 4x4
    /// identity matrix.
    #[serde(rename = "inverseBindMatrices")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    inverse_bind_matrices: Option<usize>,

    /// The index of the node used as a skeleton root. The node **MUST** be the closest common
    /// root of the joints hierarchy or a direct or indirect parent node of the closest common root.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    skeleton: Option<usize>,

    /// Indices of skeleton nodes, used as joints in this skin.
    joints: Vec<usize>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// A texture and its sampler.
#[derive(Debug, Serialize, Deserialize)]
struct Texture {
    /// Storm storage id, if the resource has been loaded.
    #[serde(skip)]
    id: Option<Id<crate::material::Texture>>,

    /// The index of the sampler used by this texture. When undefined, a sampler
    /// with repeat wrapping and auto filtering **SHOULD** be used.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    sampler: Option<usize>,

    /// The index of the image used by this texture. When undefined, an extension or
    /// other mechanism **SHOULD** supply an alternate texture source, otherwise behavior is undefined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<usize>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}
