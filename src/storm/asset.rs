use std::{fs, io, path::Path};

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use thiserror::Error;

use crate::storage::{Id, Storage};

pub struct AssetManager {
    assets: Storage<Asset>,
}

impl AssetManager {
    pub fn new() -> Self {
        let assets = Storage::new();
        AssetManager { assets }
    }

    pub fn load(&mut self, path: impl AsRef<Path>) -> Result<AssetId, Error> {
        let string = fs::read_to_string(path)?;
        let asset = serde_json::from_str(&string)?;
        dbg!(&asset);
        // dbg!(serde_json::to_string(&asset).unwrap());
        Ok(self.assets.add(asset))
    }
}

pub type AssetId = Id<Asset>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Asset {
    accessors: Vec<Accessor>,
}

/// A typed view into a buffer view that contains raw binary data.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Accessor {
    /// The index of the buffer view. When undefined, the accessor MUST be
    /// initialized with zeros; `sparse` property or extensions MAY override
    /// zeros with actual values.
    #[serde(skip_serializing_if = "Option::is_none")]
    buffer_view: Option<usize>,

    /// The offset relative to the start of the buffer view in bytes. This MUST
    /// be a multiple of the size of the component datatype. This property MUST
    /// NOT be defined when `bufferView` is undefined.
    #[serde(default = "usize_0", skip_serializing_if = "usize_is_0")]
    byte_offset: usize,

    /// The datatype of the accessor's components. UNSIGNED_INT type MUST NOT
    /// be used for any accessor that is not referenced by `mesh.primitive.indices`.
    component_type: ComponentType,

    /// Specifies whether integer data values are normalized (`true`) to [0, 1]
    /// (for unsigned types) or to [-1, 1] (for signed types) when they are accessed.
    /// This property MUST NOT be set to `true` for accessors with `F32` or `U32`
    /// component type.
    #[serde(default = "bool_false", skip_serializing_if = "is_false")]
    normalized: bool,

    /// The number of elements referenced by this accessor, not to be confused with
    /// the number of bytes or number of components.
    count: u32,

    /// Specifies if the accessor’s elements are scalars, vectors, or matrices.
    #[serde(rename = "type")]
    type_: Type,

    /// Maximum value of each component in this accessor. Array elements MUST be
    /// treated as having the same data type as accessor’s 'component_type'. Both
    /// `min` and `max` arrays have the same length. The length is determined by the
    /// value of the `type` property; it can be 1, 2, 3, 4, 9, or 16.
    ///
    /// `normalized` property has no effect on array values: they always correspond
    /// to the actual values stored in the buffer. When the accessor is sparse, this
    /// property MUST contain maximum values of accessor data with sparse substitution
    /// applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<Vec<f32>>,

    /// Minimum value of each component in this accessor. Array elements MUST be
    /// treated as having the same data type as accessor’s `component_type`. Both
    /// `min` and `max` arrays have the same length. The length is determined by the
    /// value of the `type` property; it can be 1, 2, 3, 4, 9, or 16.
    ///
    /// `normalized` property has no effect on array values: they always correspond
    /// to the actual values stored in the buffer. When the accessor is sparse, this
    /// property MUST contain minimum values of accessor data with sparse substitution
    /// applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<Vec<f32>>,

    /// Sparse storage of elements that deviate from their initialization value.
    #[serde(skip_serializing_if = "Option::is_none")]
    sparse: Option<Sparse>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize_repr, Deserialize_repr)]
#[repr(u16)]
enum ComponentType {
    /// signed byte
    I8 = 5120,
    /// unsigned byte
    U8 = 5121,
    /// signed short
    I16 = 5122,
    /// unsigned short
    U16 = 5123,
    /// unsigned int
    U32 = 5125,
    /// float
    F32 = 5126,
}

impl ComponentType {
    fn bits(&self) -> usize {
        match self {
            Self::I8 | Self::U8 => 8,
            Self::I16 | Self::U16 => 16,
            Self::U32 | Self::F32 => 32,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum Type {
    SCALAR,
    VEC2,
    VEC3,
    VEC4,
    MAT2,
    MAT3,
    MAT4,
}

impl Type {
    fn component_count(&self) -> u32 {
        match self {
            Self::SCALAR => 1,
            Self::VEC2 => 2,
            Self::VEC3 => 3,
            Self::VEC4 | Self::MAT2 => 4,
            Self::MAT3 => 9,
            Self::MAT4 => 16,
        }
    }
}

/// Sparse storage of accessor values that deviate from their initialization value.
#[derive(Debug, Serialize, Deserialize)]
struct Sparse {
    /// Number of deviating accessor values stored in the sparse array.
    count: u32,

    /// An object pointing to a buffer view containing the indices of deviating accessor
    /// values. The number of indices is equal to `count`. Indices MUST strictly increase.
    indices: SparseIndices,

    /// An object pointing to a buffer view containing the deviating accessor values.
    values: SparseValues,
}

/// An object pointing to a buffer view containing the indices of deviating accessor values.
/// The number of indices is equal to `accessor.sparse.count`. Indices MUST strictly increase.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SparseIndices {
    /// The index of the buffer view with sparse indices. The referenced buffer view MUST NOT
    /// have its `target` or `byteStride` properties defined. The buffer view and the optional
    /// `byteOffset` MUST be aligned to the `componentType` byte length.
    buffer_view: usize,

    /// The offset relative to the start of the buffer view in bytes.
    #[serde(default = "usize_0", skip_serializing_if = "usize_is_0")]
    byte_offset: usize,

    /// The indices data type.
    component_type: SparseComponentType,
}

#[derive(Debug, Serialize_repr, Deserialize_repr)]
#[repr(u16)]
enum SparseComponentType {
    U8 = 5121,
    U16 = 5123,
    U32 = 5125,
}

/// An object pointing to a buffer view containing the deviating accessor values.
/// The number of elements is equal to `accessor.sparse.count` times number of
/// components. The elements have the same component type as the base accessor.
/// The elements are tightly packed. Data MUST be aligned following the same rules
/// as the base accessor.
#[derive(Debug, Serialize, Deserialize)]
struct SparseValues {
    /// The index of the buffer_view with sparse values. The referenced buffer view
    /// MUST NOT have its target or `byteStride` properties defined.
    buffer_view: usize,

    /// The offset relative to the start of the bufferView in bytes.
    #[serde(default = "usize_0", skip_serializing_if = "usize_is_0")]
    byte_offset: usize,
}

fn bool_false() -> bool {
    false
}

fn is_false(value: &bool) -> bool {
    !value
}

fn usize_0() -> usize {
    0
}

fn usize_is_0(value: &usize) -> bool {
    *value == 0
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to read file: {0}")]
    Io(#[from] io::Error),
    #[error("invalid file: {0}")]
    Json(#[from] serde_json::Error),
}
