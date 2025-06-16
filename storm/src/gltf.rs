use serde::{Deserialize, Serialize};

/// Metadata about the glTF asset.
#[derive(Serialize, Deserialize)]
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
#[derive(Serialize, Deserialize)]
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
    /// be defined when [buffer_view](Accessor::buffer_view) is `undefined`.
    #[serde(rename = "byteOffset")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_0")]
    byte_offset: usize,

    /// The datatype of the accessor’s components. [UnsignedInt](AccessorComponentType::UnsignedInt)
    /// type **MUST NOT** be used for any accessor that is not referenced by [mesh.primitive.indices].
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
    ///
    /// Also contains the maximum and mininum values of each component in this accessor.
    /// Array elements **MUST** be treated as having the same data type as [component_type](Accessor::component_type).
    type_min_max: AccessorType,

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
/// type **MUST NOT** be used for any accessor that is not referenced by [mesh.primitive.indices].
#[derive(Serialize, Deserialize)]
enum AccessorComponentType {
    Byte = 5120,
    UnsignedByte = 5121,
    Short = 5122,
    UnsignedShort = 5123,
    UnsignedInt = 5125,
    Float = 5126,
}

/// Specifies if the accessor’s elements are scalars, vectors, or matrices.
#[derive(Serialize, Deserialize)]
enum AccessorType {
    SCALAR {
        max: Option<[f32; 1]>,
        min: Option<[f32; 1]>,
    },
    VEC2 {
        max: Option<[f32; 2]>,
        min: Option<[f32; 2]>,
    },
    VEC3 {
        max: Option<[f32; 3]>,
        min: Option<[f32; 3]>,
    },
    VEC4 {
        max: Option<[f32; 4]>,
        min: Option<[f32; 4]>,
    },
    MAT2 {
        max: Option<[f32; 4]>,
        min: Option<[f32; 4]>,
    },
    MAT3 {
        max: Option<[f32; 9]>,
        min: Option<[f32; 9]>,
    },
    MAT4 {
        max: Option<[f32; 16]>,
        min: Option<[f32; 16]>,
    },
}

/// Sparse storage of accessor values that deviate from their initialization value.
#[derive(Serialize, Deserialize)]
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
#[derive(Serialize, Deserialize)]
struct SparseAccessorIndices {
    /// The index of the buffer view with sparse indices. The referenced buffer view
    /// **MUST NOT** have its [target] or [byte_stride] properties defined. The buffer
    /// view and the optional [byte_offset] **MUST** be aligned to the
    /// [component_type](SparseAccessorIndices::component_type) byte length.
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
#[derive(Serialize, Deserialize)]
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
#[derive(Serialize, Deserialize)]
struct SparseAccessorValues {
    /// The index of the bufferView with sparse values. The referenced buffer
    /// view **MUST NOT** have its [target] or [byte_stride] properties defined.
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
#[derive(Serialize, Deserialize)]
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
#[derive(Serialize, Deserialize)]
struct AnimationChannel {
    /// The index of a sampler in this animation used to compute the value for the target,
    /// e.g., a node’s translation, rotation, or scale (TRS).
    sampler: usize,

    /// The descriptor of the animated property.
    target: AnimationTarget,
}

/// The descriptor of the animated property.
#[derive(Serialize, Deserialize)]
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
#[derive(Serialize, Deserialize)]
enum AnimationTargetPath {
    Translation,
    Rotation,
    Scale,
    Weights,
}

/// An animation sampler combines timestamps with a sequence of output values and defines an interpolation algorithm.
#[derive(Serialize, Deserialize)]
struct AnimationSampler {
    /// The index of an accessor containing keyframe timestamps. The accessor **MUST** be of scalar type with
    /// floating-point components. The values represent time in seconds with `time[0] >= 0.0`, and strictly
    /// increasing values, i.e., `time[n + 1] > time[n]`.
    input: usize,

    /// Interpolation algorithm.
    #[serde(default)]
    interpolation: AnimationInterpolation,

    /// The index of an accessor, containing keyframe output values.
    output: usize,
}

/// Interpolation algorithm.
#[derive(Default, Serialize, Deserialize)]
enum AnimationInterpolation {
    /// The animated values are linearly interpolated between keyframes.
    /// When targeting a rotation, spherical linear interpolation (slerp)
    /// **SHOULD** be used to interpolate quaternions. The number of
    /// output elements **MUST** equal the number of input elements.
    #[default]
    LINEAR,

    /// The animated values remain constant to the output of the first keyframe,
    /// until the next keyframe. The number of output elements **MUST** equal the
    /// number of input elements.
    STEP,

    /// The animation’s interpolation is computed using a cubic spline with
    /// specified tangents. The number of output elements **MUST** equal three
    /// times the number of input elements. For each input element, the output
    /// stores three elements, an in-tangent, a spline vertex, and an out-tangent.
    /// There **MUST** be at least two keyframes when using this interpolation.
    CUBICSPLINE,
}

fn is_0(value: &usize) -> bool {
    *value == 0
}

fn is_false(value: &bool) -> bool {
    *value == false
}
