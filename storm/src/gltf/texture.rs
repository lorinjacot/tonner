use std::{io::Cursor, path::Path};

use anyhow::{Context, bail};
use data_url::{DataUrl, DataUrlError};
use image::{ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::Resources;
use crate::storage::{DenseEntry, Id};

/// Image data used to create a texture. Image **MAY** be referenced by an URI (or IRI) or a buffer view index.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Image {
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

impl Image {
    fn load(
        &mut self,
        srgb: bool,
        base_path: &Path,
        buffer_views: &[super::BufferView],
        buffers: &[super::Buffer],
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<wgpu::Texture> {
        if let Some(image) = &self.wgpu {
            return Ok(image.clone());
        }

        let name = self.name.as_deref();
        let image = if let Some(uri) = &self.uri {
            match DataUrl::process(&uri) {
                Ok(url) => {
                    let mime_type = url.mime_type();
                    let (body, _fragment) = url.decode_to_vec()?;
                    ImageReader::with_format(
                        Cursor::new(body),
                        match (mime_type.type_.as_str(), mime_type.subtype.as_str()) {
                            ("image", "png") => ImageFormat::Png,
                            ("image", "jpeg") => ImageFormat::Jpeg,
                            (type_, subtype) => {
                                bail!("Unsupported image format {type_}/{subtype}.")
                            }
                        },
                    )
                    .decode()?
                }
                Err(DataUrlError::NoComma) => bail!("Invalid data url."),
                Err(DataUrlError::NotADataUrl) => {
                    let path = base_path.join(uri);
                    ImageReader::open(&path)
                        .with_context(|| format!("Failed to open image at {path:?}."))?
                        .decode()?
                }
            }
        } else {
            let buffer_view = self.buffer_view.with_context(|| {
                format!("One of image.uri or image.buffer_view must be defined.'")
            })?;
            let format = match self.mime_type {
                ImageMimeType::ImageJpeg => ImageFormat::Jpeg,
                ImageMimeType::ImagePng => ImageFormat::Png,
                ImageMimeType::None => {
                    bail!("image.mime_type must be defined when image.buffer_view is defined.")
                }
            };
            let bytes = buffer_views
                .get(buffer_view)
                .with_context(|| format!("image.buffer_view {buffer_view} is out of range."))?
                .bytes(buffers)
                .with_context(|| format!("Failed load image.buffer_view {buffer_view}."))?;

            let reader = Cursor::new(bytes);

            ImageReader::with_format(reader, format).decode()?
        };

        let texture = crate::texture::TextureBuilder::default()
            .name(name)
            .from_dynamic_image(&image, srgb)
            // .generate_mips()
            .build(resources, encoder);
        self.wgpu = Some(texture.clone());
        Ok(texture)
    }
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

/// Texture sampler properties for filtering and wrapping modes.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Sampler {
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

impl Sampler {
    fn load(&mut self, resources: &mut Resources) -> anyhow::Result<wgpu::Sampler> {
        if let Some(sampler) = &self.wgpu {
            return Ok(sampler.clone());
        }

        let mag_filter = match self.mag_filter {
            MagFilter::Linear => wgpu::FilterMode::Linear,
            MagFilter::Nearest | MagFilter::None => wgpu::FilterMode::Nearest,
        };
        let (min_filter, mipmap_filter) = match self.min_filter {
            MinFilter::LinearMipmapNearest | MinFilter::Linear => {
                (wgpu::FilterMode::Linear, wgpu::FilterMode::Nearest)
            }
            MinFilter::LinearMipmapLinear => (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear),
            MinFilter::NearestMipmapNearest | MinFilter::Nearest | MinFilter::None => {
                (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest)
            }
            MinFilter::NearestMipmapLinear => (wgpu::FilterMode::Nearest, wgpu::FilterMode::Linear),
        };

        let sampler = resources.device.create_sampler(&wgpu::SamplerDescriptor {
            label: self.name.as_deref(),
            address_mode_u: wrapping_mode_to_address_mode(self.wrap_s),
            address_mode_v: wrapping_mode_to_address_mode(self.wrap_t),
            mag_filter,
            min_filter,
            mipmap_filter,
            ..Default::default()
        });
        self.wgpu = Some(sampler.clone());
        Ok(sampler)
    }
}

fn wrapping_mode_to_address_mode(wrapping_mode: WrappingMode) -> wgpu::AddressMode {
    match wrapping_mode {
        WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
        WrappingMode::Repeat => wgpu::AddressMode::Repeat,
        WrappingMode::None => wgpu::AddressMode::Repeat,
    }
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

/// A texture and its sampler.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Texture {
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

impl Texture {
    pub(super) fn load(
        &mut self,
        srgb: bool,
        base_path: &Path,
        samplers: &mut [super::Sampler],
        images: &mut [super::Image],
        buffer_views: &[super::BufferView],
        buffers: &[super::Buffer],
        resources: &mut Resources,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<Id<crate::material::Texture>> {
        if let Some(id) = self.id {
            return Ok(id);
        }

        let name = self.name.clone();
        let sampler = self.sampler;
        let source = self.source.context("image.source must be defined.")?;

        let sampler = match sampler {
            Some(index) => Some(
                samplers
                    .get_mut(index)
                    .with_context(|| format!("texture.sampler {index} is out of range."))?
                    .load(resources)
                    .with_context(|| format!("Failed to load texture.sampler {index}."))?,
            ),
            None => None,
        };

        let source = images
            .get_mut(source)
            .with_context(|| format!("texture.image {source} is out of range."))?
            .load(srgb, base_path, buffer_views, buffers, resources, encoder)
            .with_context(|| format!("Failed to load texture.image {source}."))?;

        let id = crate::material::TextureBuilder::default()
            .name(name)
            .sampler(sampler)
            .texture(source)
            .build(resources)
            .id();
        self.id = Some(id);
        Ok(id)
    }
}
