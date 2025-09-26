use std::num::NonZeroUsize;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use super::transforms::is_0;

/// A buffer points to binary geometry, animation, or skins.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Buffer {
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

impl Buffer {
    /// The content of the buffer. Empty if unsupported uri.
    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The content of the buffer. Empty if unsupported uri.
    pub(super) fn bytes_mut(&mut self) -> &mut Vec<u8> {
        &mut self.bytes
    }

    /// The length of the buffer in bytes.
    pub(super) fn byte_length(&self) -> usize {
        self.byte_length
    }

    /// The URI (or IRI) of the buffer. Relative paths are relative to
    /// the current glTF asset. Instead of referencing an external file,
    /// this field **MAY** contain a `data:`-URI.
    pub(super) fn uri(&self) -> &Option<String> {
        &self.uri
    }
}

/// A view into a buffer generally representing a subset of the buffer.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct BufferView {
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

impl BufferView {
    pub(super) fn bytes<'a>(&self, buffers: &'a [super::Buffer]) -> Result<&'a [u8]> {
        let start = self.byte_offset;
        let end = start + self.byte_length;
        buffers
            .get(self.buffer)
            .ok_or_else(|| anyhow!("buffer_view.buffer {} is out of range.", self.buffer))?
            .bytes
            .get(start..end)
            .with_context(|| {
                format!(
                    "buffer_view.buffer {} is shorter than the buffer view.",
                    self.buffer
                )
            })
    }

    /// The stride, in bytes, between vertex attributes. When this is not
    /// defined, data is tightly packed. When two or more accessors use the
    /// same buffer view, this field **MUST** be defined.
    pub(super) fn byte_stride(&self) -> Option<NonZeroUsize> {
        self.byte_stride
    }
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
