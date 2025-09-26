use std::{
    fmt::Display,
    iter::{Zip, zip},
    marker::PhantomData,
    num::NonZeroUsize,
    slice,
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use bytemuck::{Pod, cast_slice, from_bytes};
use glam::{
    I8Vec2, I8Vec3, I8Vec4, I16Vec2, I16Vec3, I16Vec4, U8Vec2, U8Vec3, U8Vec4, U16Vec2, U16Vec3,
    U16Vec4, UVec4, Vec2, Vec3, Vec4,
};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::gltf::transforms::{is_0, is_false};
use crate::gltf::{Buffer, BufferView};

/// A typed view into a buffer view that contains raw binary data.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Accessor {
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

macro_rules! generate_iter {
    ($vis:vis $name:ident, item = $target:ty, [$((
        ($accessor_type:ident, $component_type:ident, $normalized:ident) =>
        $source:ty $(: $via:ty, $transformation:expr)?
   ) ),+]) => {
        /// Iterate over the gtTL accessor entries using the given `IteratorConsumer`.
        /// Works with both dense and sparse accessors.
        ///
        #[doc = concat!(
            "This method will automatically convert the following types into a `", stringify!($target), "`:"
        )]
        $(#[doc = concat!("- `&", stringify!($source), "`")])+
        ///
        /// If the glTF accessor contains any other type, this method will fail.
        $vis fn $name<'a, C: IteratorConsumer<'a, $target>>(
            &'a self,
            buffer_views: &[BufferView],
            buffers: &'a [Buffer],
            consumer: C,
        ) -> Result<C::Return> {
            macro_rules! from {
                ($source2:ty) => {{
                    struct Consumer<'b, D: IteratorConsumer<'b, $target>> {
                        consumer: D,
                        lifetype: PhantomData<&'b ()>,
                    }

                    impl<'b, D: IteratorConsumer<'b, $target>> IteratorConsumer<'b, &'b $source2>
                        for Consumer<'b, D>
                    {
                        type Return = D::Return;

                        fn consume<I: Iterator<Item = &'b $source2> + 'b>(
                            self,
                            iter: I,
                        ) -> Result<Self::Return> {
                            self.consumer
                                .consume(iter.cloned().map(<$target>::from_array))
                        }
                    }

                    self.iter_unchecked(
                        buffer_views,
                        buffers,
                        Consumer {
                            consumer,
                            lifetype: PhantomData,
                        },
                    )
                }};
                ($source2:ty : $via2:ty, $transformation2:expr) => {{
                    struct Consumer<'b, D: IteratorConsumer<'b, $target>> {
                        consumer: D,
                        lifetype: PhantomData<&'b ()>,
                    }

                    impl<'b, D: IteratorConsumer<'b, $target>> IteratorConsumer<'b, &'b $source2> for Consumer<'b, D> {
                        type Return = D::Return;

                        fn consume<I: Iterator<Item = &'b $source2> + 'b>(
                            self,
                            iter: I,
                        ) -> Result<Self::Return> {
                            self.consumer.consume(
                                iter.cloned()
                                    .map(<$via2>::from_array)
                                    .map($transformation2),
                            )
                        }
                    }

                    self.iter_unchecked(
                        buffer_views,
                        buffers,
                        Consumer {
                            consumer,
                            lifetype: PhantomData,
                        },
                    )
                }};
            }

            match (self.type_, self.component_type, self.normalized) {
                $((AccessorType::$accessor_type, AccessorComponentType::$component_type, $normalized) => {
                    from!($source $(: $via, $transformation)?)
                })+
                (type_, component_type, normalized) => {
                    bail!(
                        concat!("Cannot create a ", stringify!($target), " from a {} of {} {}"),
                        type_,
                        if normalized { "" } else { " normalized" },
                        component_type
                    );
                }
            }
        }
    };
}

impl Accessor {
    /// The number of elements referenced by this accessor, not to be confused with the number of
    /// bytes or number of components.
    pub(super) fn count(&self) -> usize {
        self.count
    }

    /// Specifies if the accessor’s elements are scalars, vectors, or matrices.
    pub(super) fn type_(&self) -> AccessorType {
        self.type_
    }

    /// The datatype of the accessor’s components. [UnsignedInt](AccessorComponentType::UnsignedInt)
    /// type **MUST NOT** be used for any accessor that is not referenced by
    /// [mesh.primitive.indices](MeshPrimitive::indices).
    pub(super) fn component_type(&self) -> AccessorComponentType {
        self.component_type
    }

    /// Specifies whether integer data values are normalized (`true`) to [0, 1] (for unsigned types)
    /// or to [-1, 1] (for signed types) when they are accessed. This property **MUST NOT** be set
    /// to `true` for accessors with [Float](AccessorComponentType::Float) or
    /// [UnsignedInt](AccessorComponentType::UnsignedInt) component type.
    pub(super) fn normalized(&self) -> bool {
        self.normalized
    }

    /// Return an iterator visiting the glTF accessor.
    /// Works with both dense and sparse accessors.
    /// This method will interpret the bytes contained in
    /// the accessor as `V` without doing any checks.
    pub(super) fn iter_unchecked<'a, V: Value, C: IteratorConsumer<'a, &'a V>>(
        &'a self,
        buffer_views: &[BufferView],
        buffers: &'a [Buffer],
        consumer: C,
    ) -> Result<C::Return> {
        match &self.sparse {
            Some(sparse) => match sparse.indices.component_type {
                SparseAccessorComponentType::UnsignedByte => {
                    let mut sparse_iter =
                        sparse_iter_unchecked::<u8, V>(sparse, buffer_views, buffers)?;
                    let next_sparse_entry = sparse_iter.next().map(|(idx, v)| (idx.as_usize(), v));

                    match self.buffer_view {
                        Some(buffer_view) => {
                            let default_values = dense_iter_unchecked(
                                buffer_view,
                                self.byte_offset,
                                self.count,
                                buffer_views,
                                buffers,
                            )?;

                            consumer.consume(SparseAccessorIter {
                                default_values,
                                sparse_iter,
                                next_sparse_entry,
                            })
                        }
                        None => consumer.consume(PureSparseAccessorIter {
                            default_value: Value::DEFAULT,
                            sparse_iter,
                            next_sparse_entry,
                            next: 0,
                            count: self.count,
                        }),
                    }
                }
                SparseAccessorComponentType::UnsignedShort => {
                    let mut sparse_iter =
                        sparse_iter_unchecked::<u16, V>(sparse, buffer_views, buffers)?;
                    let next_sparse_entry = sparse_iter.next().map(|(idx, v)| (idx.as_usize(), v));

                    match self.buffer_view {
                        Some(buffer_view) => {
                            let default_values = dense_iter_unchecked(
                                buffer_view,
                                self.byte_offset,
                                self.count,
                                buffer_views,
                                buffers,
                            )?;

                            consumer.consume(SparseAccessorIter {
                                default_values,
                                sparse_iter,
                                next_sparse_entry,
                            })
                        }
                        None => consumer.consume(PureSparseAccessorIter {
                            default_value: Value::DEFAULT,
                            sparse_iter,
                            next_sparse_entry,
                            next: 0,
                            count: self.count,
                        }),
                    }
                }
                SparseAccessorComponentType::UnsignedInt => {
                    let mut sparse_iter =
                        sparse_iter_unchecked::<u32, V>(sparse, buffer_views, buffers)?;
                    let next_sparse_entry = sparse_iter.next().map(|(idx, v)| (idx.as_usize(), v));

                    match self.buffer_view {
                        Some(buffer_view) => {
                            let default_values = dense_iter_unchecked(
                                buffer_view,
                                self.byte_offset,
                                self.count,
                                buffer_views,
                                buffers,
                            )?;

                            consumer.consume(SparseAccessorIter {
                                default_values,
                                sparse_iter,
                                next_sparse_entry,
                            })
                        }
                        None => consumer.consume(PureSparseAccessorIter {
                            default_value: Value::DEFAULT,
                            sparse_iter,
                            next_sparse_entry,
                            next: 0,
                            count: self.count,
                        }),
                    }
                }
            },
            None => {
                let buffer_view = self
                    .buffer_view
                    .context("one of accessor.buffer_view or accessor.sparse has to be defined")?;

                consumer.consume(dense_iter_unchecked(
                    buffer_view,
                    self.byte_offset,
                    self.count,
                    buffer_views,
                    buffers,
                )?)
            }
        }
    }

    /// Return a view over the bytes of all the entries from this glTF accessor.
    /// This method will fail if the accessor is not sparse or if the data are not tighly packed.
    pub(super) fn bytes_dense_tighly_packed<'a>(
        &'a self,
        buffer_views: &[BufferView],
        buffers: &'a [Buffer],
    ) -> anyhow::Result<&'a [u8]> {
        ensure!(self.sparse.is_none(), "accessor must not be sparse.");

        let start = self.byte_offset;
        let stride = self.type_.dim() * self.component_type.size();
        let end = start + self.count * stride;

        let buffer_view_idx = self.buffer_view.ok_or(anyhow!(
            "accessor.buffer_view must be defined for dense accessor."
        ))?;
        let buffer_view = buffer_views.get(buffer_view_idx).ok_or(anyhow!(
            "accessor.buffer_view {buffer_view_idx} is out of range."
        ))?;

        if let Some(byte_stride) = buffer_view.byte_stride() {
            ensure!(byte_stride.get() == stride, "accessor data must be tightly packed.");
        }

        buffer_view
            .bytes(buffers)
            .with_context(|| format!("Failed to load accessor.buffer_view {buffer_view_idx}."))?
            .get(start..end)
            .with_context(|| format!("accessor.buffer_view {buffer_view_idx} is too short."))
    }

    generate_iter! {pub(super) iter_vec2, item = Vec2, [
        ((Vec2, Byte, true) => [i8; 2] : I8Vec2, |a| (a.as_vec2() / 127.0).max(-Vec2::ONE)),
        ((Vec2, UnsignedByte, true) => [u8; 2] : U8Vec2, |a| a.as_vec2() / 255.0),
        ((Vec2, Short, true) => [i16; 2] : I16Vec2, |a| (a.as_vec2() / 32767.0).max(-Vec2::ONE)),
        ((Vec2, UnsignedShort, true) => [u16; 2] : U16Vec2, |a| a.as_vec2() / 65535.0),
        ((Vec2, Float, false) => [f32; 2])
    ]}

    generate_iter! {pub(super) iter_vec3, item = Vec3, [
        ((Vec3, Byte, true) => [i8; 3] : I8Vec3, |a| (a.as_vec3() / 127.0).max(-Vec3::ONE)),
        ((Vec3, UnsignedByte, true) => [u8; 3] : U8Vec3, |a| a.as_vec3() / 255.0),
        ((Vec3, Short, true) => [i16; 3] : I16Vec3, |a| (a.as_vec3() / 32767.0).max(-Vec3::ONE)),
        ((Vec3, UnsignedShort, true) => [u16; 3] : U16Vec3, |a| a.as_vec3() / 65535.0),
        ((Vec3, Float, false) => [f32; 3])
    ]}

    generate_iter! {pub(super) iter_vec4, item = Vec4, [
        ((Vec4, Byte, true) => [i8; 4] : I8Vec4, |a| (a.as_vec4() / 127.0).max(-Vec4::ONE)),
        ((Vec4, UnsignedByte, true) => [u8; 4] : U8Vec4, |a| a.as_vec4() / 255.0),
        ((Vec4, Short, true) => [i16; 4] : I16Vec4, |a| (a.as_vec4() / 32767.0).max(-Vec4::ONE)),
        ((Vec4, UnsignedShort, true) => [u16; 4] : U16Vec4, |a| a.as_vec4() / 65535.0),
        ((Vec4, Float, false) => [f32; 4])
    ]}

    generate_iter! {pub(super) iter_uvec4, item = UVec4, [
        ((Vec4, UnsignedByte, false) => [u8; 4] : U8Vec4, |a| a.as_uvec4()),
        ((Vec4, UnsignedShort, false) => [u16; 4] : U16Vec4, |a| a.as_uvec4()),
        ((Vec4, UnsignedInt, false) => [u32; 4])
    ]}
}

/// The datatype of the accessor’s components. [UnsignedInt](AccessorComponentType::UnsignedInt)
/// type **MUST NOT** be used for any accessor that is not referenced by
/// [mesh.primitive.indices](MeshPrimitive::indices).
#[derive(Debug, Clone, Copy, Serialize_repr, Deserialize_repr)]
#[repr(u32)]
pub enum AccessorComponentType {
    Byte = 5120,
    UnsignedByte = 5121,
    Short = 5122,
    UnsignedShort = 5123,
    UnsignedInt = 5125,
    Float = 5126,
}

impl AccessorComponentType {
    /// Size of the described type in bytes.
    fn size(&self) -> usize {
        match self {
            Self::Byte | Self::UnsignedByte => 1,
            Self::Short | Self::UnsignedShort => 2,
            Self::Float | Self::UnsignedInt => 4,
        }
    }
}

impl Display for AccessorComponentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Byte => "byte",
            Self::UnsignedByte => "unsigned byte",
            Self::Short => "short",
            Self::UnsignedShort => "unsigned short",
            Self::UnsignedInt => "unsigned int",
            Self::Float => "float",
        })
    }
}

/// Specifies if the accessor’s elements are scalars, vectors, or matrices.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AccessorType {
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

impl AccessorType {
    /// Dimension (e.g. number of components) of the described type.
    fn dim(&self) -> usize {
        match self {
            Self::Scalar => 1,
            Self::Vec2 => 2,
            Self::Vec3 => 3,
            Self::Vec4 => 4,
            Self::Mat2 => 2 * 2,
            Self::Mat3 => 3 * 3,
            Self::Mat4 => 4 * 4,
        }
    }
}

impl Display for AccessorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Scalar => "scalar",
            Self::Vec2 => "vec2",
            Self::Vec3 => "vec3",
            Self::Vec4 => "vec4",
            Self::Mat2 => "mat2",
            Self::Mat3 => "mat3",
            Self::Mat4 => "mat4",
        })
    }
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

/// This trait is used by the `Accessor.iter_*` methods. The generated iterators
/// are given as argument to the `consume`. The value returned by the `consume` method
/// is then returned by the `Accessor.iter_*` methods.
///
/// This trait is a workaround to some limitation of rust higher order functions.
/// At the time of writing, it is not possible to have a function/method taking a
/// callable which take a generic argument.
pub(super) trait IteratorConsumer<'a, T: 'a> {
    /// The type return after having consume an iterator successfully.
    type Return;

    /// This method take ownership of the iterator and can consume it in any suitable ways.
    fn consume<I: Iterator<Item = T> + 'a>(self, iter: I) -> Result<Self::Return>;
}

/// A iterator over a dense glTF accessor's entries.
pub(super) struct DenseAccessorIter<'a, V: Value> {
    bytes: &'a [u8],
    next: usize,
    byte_stride: usize,
    data_type: PhantomData<V>,
}

impl<'a, V: Value> Iterator for DenseAccessorIter<'a, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.next * self.byte_stride;
        let end = start + size_of::<V>();
        let next = from_bytes(self.bytes.get(start..end)?);
        self.next += 1;
        Some(next)
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.next += n;
        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = (self.bytes.len() - size_of::<V>()) / self.byte_stride + 1 - self.next;
        (len, Some(len))
    }
}

/// A iterator over a sparse glTF accessor's entries.
pub(super) struct SparseAccessorIter<'a, I: Index, V: Value> {
    default_values: DenseAccessorIter<'a, V>,
    sparse_iter: Zip<slice::Iter<'a, I>, slice::Iter<'a, V>>,
    next_sparse_entry: Option<(usize, &'a V)>,
}

impl<'a, I: Index, V: Value> Iterator for SparseAccessorIter<'a, I, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.default_values.next;
        let default_value = self.default_values.next()?;
        match self.next_sparse_entry {
            Some(entry) if entry.0 == next => {
                let value = entry.1;
                self.next_sparse_entry = self
                    .sparse_iter
                    .next()
                    .map(|(idx, value)| (idx.as_usize(), value));
                Some(value)
            }
            _ => Some(default_value),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.default_values.size_hint()
    }
}

/// A iterator over a pure sparse glTF accessor's entries.
pub(super) struct PureSparseAccessorIter<'a, I: Index, V: Value> {
    default_value: &'a V,
    sparse_iter: Zip<slice::Iter<'a, I>, slice::Iter<'a, V>>,
    next_sparse_entry: Option<(usize, &'a V)>,
    next: usize,
    count: usize,
}

impl<'a, I: Index, V: Value> Iterator for PureSparseAccessorIter<'a, I, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next < self.count {
            Some(match self.next_sparse_entry {
                Some(entry) if entry.0 == self.next => {
                    let value = entry.1;
                    self.next_sparse_entry = self
                        .sparse_iter
                        .next()
                        .map(|(idx, value)| (idx.as_usize(), value));
                    value
                }
                _ => self.default_value,
            })
        } else {
            None
        }
    }
}

/// Create an iterator over the sparse part of a glTF accessor.
/// This function will interpret the bytes contained in
/// the accessor as `V` without doing any checks.
fn sparse_iter_unchecked<'a, I: Index, V: Value>(
    sparse: &SparseAccessor,
    buffer_views: &[BufferView],
    buffers: &'a [Buffer],
) -> Result<Zip<slice::Iter<'a, I>, slice::Iter<'a, V>>> {
    let indices = buffer_views
        .get(sparse.indices.buffer_view)
        .with_context(|| {
            format!(
                "accessor.sparse.indices.buffer_view {} is out of range.",
                sparse.indices.buffer_view
            )
        })?;
    let start = sparse.indices.byte_offset;
    let end = start + sparse.count * size_of::<I>();
    let indices: &[I] = cast_slice(
        indices
            .bytes(buffers)
            .with_context(|| {
                format!(
                    "Failed to load accessor.sparse.indices.buffer_view {}.",
                    sparse.indices.buffer_view
                )
            })?
            .get(start..end)
            .with_context(|| {
                format!(
                    "accessor.sparse.indices.buffer_view {} is too short.",
                    sparse.indices.buffer_view
                )
            })?,
    );

    let start = sparse.values.byte_offset;
    let end = start + sparse.count * size_of::<V>();
    let values: &[V] = cast_slice(
        buffer_views
            .get(sparse.values.buffer_view)
            .with_context(|| {
                format!(
                    "accessor.sparse.values.buffer_view {} is out of range.",
                    sparse.values.buffer_view
                )
            })?
            .bytes(buffers)
            .with_context(|| {
                format!(
                    "Failed to load accessor.sparse.values.buffer_view {}.",
                    sparse.values.buffer_view
                )
            })?
            .get(start..end)
            .with_context(|| {
                format!(
                    "accessor.sparse.values.buffer_view {} is too short.",
                    sparse.values.buffer_view
                )
            })?,
    );

    Ok(zip(indices, values))
}

/// Create an iterator over the dense part of a glTF accessor.
/// This function will interpret the bytes contained in
/// the accessor as `V` without doing any checks.
fn dense_iter_unchecked<'a, V: Value>(
    buffer_view: usize,
    byte_offset: usize,
    count: usize,
    buffer_views: &[BufferView],
    buffers: &'a [Buffer],
) -> Result<DenseAccessorIter<'a, V>> {
    let view = buffer_views.get(buffer_view).ok_or(anyhow!(
        "accessor.buffer_view {buffer_view} is out of range."
    ))?;

    let byte_stride = view.byte_stride().map_or(size_of::<V>(), NonZeroUsize::get);
    let start = byte_offset;
    let end = start + (count - 1) * byte_stride + size_of::<V>();

    let bytes = view
        .bytes(buffers)
        .with_context(|| format!("Failed to get buffer_view.buffer {buffer_view}."))?
        .get(start..end)
        .with_context(|| format!("buffer_view.buffer {buffer_view} is too short."))?;

    Ok(DenseAccessorIter {
        bytes,
        next: 0,
        byte_stride,
        data_type: PhantomData,
    })
}

/// Type implementing this trait can be used as an index for a glTF sparse accessor.
pub trait Index: Copy + Pod {
    fn as_usize(&self) -> usize;
}

impl Index for u8 {
    fn as_usize(&self) -> usize {
        *self as usize
    }
}

impl Index for u16 {
    fn as_usize(&self) -> usize {
        *self as usize
    }
}

impl Index for u32 {
    fn as_usize(&self) -> usize {
        *self as usize
    }
}

/// Any type that can be stored inside a glTF accessor should implement this trait.
pub trait Value: Pod + 'static {
    /// Default value. Used as default value for pure sparse accessor.
    const DEFAULT: &'static Self;
}

impl Value for [i8; 1] {
    const DEFAULT: &'static Self = &[0; 1];
}

impl Value for [i8; 2] {
    const DEFAULT: &'static Self = &[0; 2];
}

impl Value for [i8; 3] {
    const DEFAULT: &'static Self = &[0; 3];
}

impl Value for [i8; 4] {
    const DEFAULT: &'static Self = &[0; 4];
}

impl Value for [u8; 1] {
    const DEFAULT: &'static Self = &[0; 1];
}

impl Value for [u8; 2] {
    const DEFAULT: &'static Self = &[0; 2];
}

impl Value for [u8; 3] {
    const DEFAULT: &'static Self = &[0; 3];
}

impl Value for [u8; 4] {
    const DEFAULT: &'static Self = &[0; 4];
}

impl Value for [i16; 1] {
    const DEFAULT: &'static Self = &[0; 1];
}

impl Value for [i16; 2] {
    const DEFAULT: &'static Self = &[0; 2];
}

impl Value for [i16; 3] {
    const DEFAULT: &'static Self = &[0; 3];
}

impl Value for [i16; 4] {
    const DEFAULT: &'static Self = &[0; 4];
}

impl Value for [u16; 1] {
    const DEFAULT: &'static Self = &[0; 1];
}

impl Value for [u16; 2] {
    const DEFAULT: &'static Self = &[0; 2];
}

impl Value for [u16; 3] {
    const DEFAULT: &'static Self = &[0; 3];
}

impl Value for [u16; 4] {
    const DEFAULT: &'static Self = &[0; 4];
}

impl Value for [u32; 1] {
    const DEFAULT: &'static Self = &[0; 1];
}

impl Value for [u32; 2] {
    const DEFAULT: &'static Self = &[0; 2];
}

impl Value for [u32; 3] {
    const DEFAULT: &'static Self = &[0; 3];
}

impl Value for [u32; 4] {
    const DEFAULT: &'static Self = &[0; 4];
}

impl Value for [f32; 1] {
    const DEFAULT: &'static Self = &[0.0; 1];
}

impl Value for [f32; 2] {
    const DEFAULT: &'static Self = &[0.0; 2];
}

impl Value for [f32; 3] {
    const DEFAULT: &'static Self = &[0.0; 3];
}

impl Value for [f32; 4] {
    const DEFAULT: &'static Self = &[0.0; 4];
}

impl Value for [f32; 4 * 4] {
    const DEFAULT: &'static Self = &[0.0; 4 * 4];
}
