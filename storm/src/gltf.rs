use std::{
    fs::File,
    io::{BufReader, Read},
    num::NonZeroUsize,
    path::Path,
};

use bytemuck::bytes_of_mut;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GltfError {
    #[error("Failed to read the asset: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid binary gltf container: {0}")]
    Glb(#[from] GlbError),
    #[error("Failed to parse json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Unsupported asset: {0}")]
    Unsupported(String),
}

#[derive(Error, Debug)]
pub enum GlbError {
    #[error("Binary glTF container version should be 2")]
    UnknownVersion,
    #[error("Glb asset should have at least one chunk")]
    FirstChunkMissing,
    #[error("Invalid chunk length")]
    InvalidChunkLength,
    #[error("The first glb chunk should be the structured JSON content of the asset")]
    InvalidFirstChunkType,
}

pub struct GltfAsset {
    json: Gltf,
    binary_buffer: Option<Vec<u8>>,
}

const GLB_HEADER_SIZE: u32 = 3 * size_of::<u32>() as u32;
const MIN_CHUNK_SIZE: u32 = 2 * size_of::<u32>() as u32;
const ASCII_GLTF: u32 = 0x46546C67;
const ASCII_JSON: u32 = 0x4E4F534A;
const ASCII_BIN: u32 = 0x004E4942;

impl GltfAsset {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GltfError> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let mut magic: u32 = 0;
        reader.read_exact(bytes_of_mut(&mut magic))?;
        if magic != ASCII_GLTF {
            return Err(GltfError::Unsupported(
                "Only Binary glTF (.glb) asset are supported".to_string(),
            ));
        }

        let mut version: u32 = 0;
        reader.read_exact(bytes_of_mut(&mut version))?;
        if version != 2 {
            return Err(GlbError::UnknownVersion.into());
        }

        let mut length: u32 = 0;
        reader.read_exact(bytes_of_mut(&mut length))?;
        length -= GLB_HEADER_SIZE;

        let mut reader = reader.take(length as u64);

        if length < MIN_CHUNK_SIZE {
            return Err(GlbError::FirstChunkMissing.into());
        }
        let json = GlbChunk::from_reader(&mut reader)?;
        length -= json.chunk_length;
        if json.chunk_type != ASCII_JSON {
            return Err(GlbError::InvalidFirstChunkType.into());
        }
        let json = serde_json::from_slice(&json.chunk_data)?;

        let binary_buffer = if length >= MIN_CHUNK_SIZE {
            let binary_buffer = GlbChunk::from_reader(&mut reader)?;
            // length -= binary_buffer.chunk_length;
            if binary_buffer.chunk_type != ASCII_BIN {
                return Err(GlbError::InvalidFirstChunkType.into());
            }
            Some(binary_buffer.chunk_data)
        } else {
            None
        };

        Ok(Self {
            json,
            binary_buffer,
        })
    }
}

struct GlbChunk {
    chunk_length: u32,
    chunk_type: u32,
    chunk_data: Vec<u8>,
}

impl GlbChunk {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self, GltfError> {
        let mut chunk_length: u32 = 0;
        reader.read_exact(bytes_of_mut(&mut chunk_length))?;

        let mut chunk_type: u32 = 0;
        reader.read_exact(bytes_of_mut(&mut chunk_type))?;

        let mut chunk_data = Vec::with_capacity(chunk_length as usize);
        reader
            .take(chunk_length as u64)
            .read_to_end(&mut chunk_data)?;

        if (chunk_data.len() as u32) < chunk_length {
            return Err(GlbError::InvalidChunkLength.into());
        }

        Ok(Self {
            chunk_length,
            chunk_type,
            chunk_data,
        })
    }
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

/// A typed view into a buffer view that contains raw binary data.
#[derive(Debug, Serialize, Deserialize)]
struct Accessor {
    /// The index of the buffer view. When undefined, the accessor **MUST**
    /// be initialized with zeros; `sparse` property or extensions **MAY**
    /// override zeros with actual values.
    #[serde(rename = "bufferView")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    buffer_view: Option<usize>,

    /// The offset relative to the start of the buffer view in bytes. This **MUST**
    /// be a multiple of the size of the component datatype. This property **MUST NOT**
    /// be defined when [bufferView](Accessor::buffer_view) is `undefined`.
    #[serde(rename = "byteOffset")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_0")]
    byte_offset: usize,

    /// The datatype of the accessor’s components. [UnsignedInt](AccessorComponentType::UnsignedInt)
    /// type **MUST NOT** be used for any accessor that is not referenced by
    /// [mesh.primitive.indices](MeshPrimitive::indices).
    #[serde(rename = "componentType")]
    component_type: AccessorComponentType,

    /// Specifies whether integer data values are normalized (`true`) to [0, 1] (for unsigned types)
    /// or to [-1, 1] (for signed types) when they are accessed. This property **MUST NOT** be set
    /// to `true` for accessors with [Float](AccessorComponentType::Float) or
    /// [UnsignedInt](AccessorComponentType::UnsignedInt) component type.
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    normalized: bool,

    /// The number of elements referenced by this accessor, not to be confused with the number of
    /// bytes or number of components.
    count: usize,

    /// Specifies if the accessor’s elements are scalars, vectors, or matrices.
    #[serde(rename = "type")]
    type_: AccessorType,

    /// Maximum value of each component in this accessor. Array elements
    /// **MUST** be treated as having the same data type as [componentType](Accessor::component_type).
    /// Both [min](Accessor::min) and [max](Accessor::max) arrays have the same length.
    /// The length is determined by the value of the type property;
    /// it can be 1, 2, 3, 4, 9, or 16.
    ///
    /// [normalized](Accessor::normalized) property has no effect on array values:
    /// they always correspond to the actual values stored in the buffer.
    /// When the accessor is sparse, this property **MUST** contain maximum
    /// values of accessor data with sparse substitution applied.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<Vec<f32>>,

    /// Minimum value of each component in this accessor. Array elements
    /// **MUST** be treated as having the same data type as [componentType](Accessor::component_type).
    /// Both [min](Accessor::min) and [max](Accessor::max) arrays have the same length.
    /// The length is determined by the value of the type property;
    /// it can be 1, 2, 3, 4, 9, or 16.
    ///
    /// [normalized](Accessor::normalized) property has no effect on array values:
    /// they always correspond to the actual values stored in the buffer.
    /// When the accessor is sparse, this property **MUST** contain maximum
    /// values of accessor data with sparse substitution applied.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<Vec<f32>>,

    /// Sparse storage of elements that deviate from their initialization value.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    sparse: Option<SparseAccessor>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could even
    /// have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// The datatype of the accessor’s components. [UnsignedInt](AccessorComponentType::UnsignedInt)
/// type **MUST NOT** be used for any accessor that is not referenced by
/// [mesh.primitive.indices](MeshPrimitive::indices).
#[derive(Debug, Serialize_repr, Deserialize_repr)]
#[repr(u32)]
enum AccessorComponentType {
    Byte = 5120,
    UnsignedByte = 5121,
    Short = 5122,
    UnsignedShort = 5123,
    UnsignedInt = 5125,
    Float = 5126,
}

/// Specifies if the accessor’s elements are scalars, vectors, or matrices.
#[derive(Debug, Serialize, Deserialize)]
enum AccessorType {
    #[serde(rename = "SCALAR")]
    Scalar,

    #[serde(rename = "VEC2")]
    Vec2,

    #[serde(rename = "VEC3")]
    Vec3,

    #[serde(rename = "VEC4")]
    Vec4,

    #[serde(rename = "MAT2")]
    Mat2,

    #[serde(rename = "MAT3")]
    Mat3,

    #[serde(rename = "MAT4")]
    Mat4,
}

/// Sparse storage of accessor values that deviate from their initialization value.
#[derive(Debug, Serialize, Deserialize)]
struct SparseAccessor {
    /// Number of deviating accessor values stored in the sparse array.
    count: usize,

    /// An object pointing to a buffer view containing the indices of deviating
    /// accessor values. The number of indices is equal to [count](SparseAccessor::count).
    /// Indices **MUST** strictly increase.
    indices: SparseAccessorIndices,

    /// An object pointing to a buffer view containing the deviating accessor values.
    values: SparseAccessorValues,
}

/// An object pointing to a buffer view containing the indices of deviating accessor
/// values. The number of indices is equal to [accessor.sparse.count](SparseAccessor::count).
/// Indices **MUST** strictly increase.
#[derive(Debug, Serialize, Deserialize)]
struct SparseAccessorIndices {
    /// The index of the buffer view with sparse indices. The referenced buffer view
    /// **MUST NOT** have its [target](BufferView::target) or [byteStride](BufferView::byte_stride)
    /// properties defined. The buffer view and the optional [byteOffset](BufferView::byte_offset)
    /// **MUST** be aligned to the [componentType](SparseAccessorIndices::component_type) byte length.
    #[serde(rename = "bufferView")]
    buffer_view: usize,

    /// The offset relative to the start of the buffer view in bytes.
    #[serde(rename = "byteOffset")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_0")]
    byte_offset: usize,

    /// The indices data type.
    #[serde(rename = "componentType")]
    component_type: SparseAccessorComponentType,
}

/// The indices data type.
#[derive(Debug, Serialize_repr, Deserialize_repr)]
#[repr(u32)]
enum SparseAccessorComponentType {
    UnsignedByte = 5121,
    UnsignedShort = 5123,
    UnsignedInt = 5125,
}

/// An object pointing to a buffer view containing the deviating accessor values.
/// The number of elements is equal to [accessor.sparse.count](SparseAccessor::count)
/// times number of components. The elements have the same component type as the base
/// accessor. The elements are tightly packed. Data **MUST** be aligned following the
/// same rules as the base accessor.
#[derive(Debug, Serialize, Deserialize)]
struct SparseAccessorValues {
    /// The index of the bufferView with sparse values. The referenced buffer
    /// view **MUST NOT** have its [target](BufferView::target) or
    /// [byteStride](BufferView::byte_stride) properties defined.
    #[serde(rename = "bufferView")]
    buffer_view: usize,

    /// The offset relative to the start of the
    /// [buffer_view](SparseAccessorValues::buffer_view) in bytes.
    #[serde(rename = "byteOffset")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_0")]
    byte_offset: usize,
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
#[derive(Debug, Serialize, Deserialize)]
enum AnimationTargetPath {
    #[serde(rename = "translation")]
    Translation,

    #[serde(rename = "rotation")]
    Rotation,

    #[serde(rename = "scale")]
    Scale,

    #[serde(rename = "weights")]
    Weights,
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
#[derive(Debug, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    targets: Option<Vec<MorphTarget>>,
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
    #[serde(rename = "COLOR_n")]
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
#[derive(Debug, Default, Serialize_repr, Deserialize_repr)]
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

#[derive(Debug, Default, Serialize_repr, Deserialize_repr)]
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

fn is_0(value: &usize) -> bool {
    *value == 0
}

fn is_false(value: &bool) -> bool {
    *value == false
}

fn is_3x00(value: &[f32; 3]) -> bool {
    *value == [0.0; 3]
}

fn default_05() -> f32 {
    0.5
}

fn is_05(value: &f32) -> bool {
    *value == 0.5
}

fn default_10() -> f32 {
    1.0
}

fn is_10(value: &f32) -> bool {
    *value == 1.0
}

fn default_4x10() -> [f32; 4] {
    [1.0; 4]
}

fn is_4x10(value: &[f32; 4]) -> bool {
    *value == [1.0; 4]
}
