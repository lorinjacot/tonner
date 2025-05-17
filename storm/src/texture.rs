use image::DynamicImage;
use wgpu::util::DeviceExt;

use crate::Resources;

#[must_use]
pub struct TextureBuilder<'a> {
    resources: &'a Resources,
    name: Option<&'a str>,
    source: Source<'a>,
    mip_level_count: u32,
    generate_mips: bool,
    usage: wgpu::TextureUsages,
}

impl<'a> TextureBuilder<'a> {
    pub fn new(resources: &'a Resources) -> Self {
        Self {
            resources,
            name: None,
            source: Source::Empty {
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                format: wgpu::TextureFormat::R8Unorm,
            },
            mip_level_count: 1,
            generate_mips: false,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
        }
    }

    pub fn name(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }

    pub fn mip_level_count(mut self, mip_level_count: u32) -> Self {
        self.mip_level_count = mip_level_count;
        self
    }

    pub fn usage(mut self, usage: wgpu::TextureUsages) -> Self {
        self.usage = usage;
        self
    }

    pub fn empty(mut self, size: wgpu::Extent3d, format: wgpu::TextureFormat) -> Self {
        self.source = Source::Empty { size, format };
        self
    }

    pub fn bytes(
        mut self,
        size: wgpu::Extent3d,
        format: wgpu::TextureFormat,
        bytes: &'a [u8],
    ) -> Self {
        self.source = Source::Bytes {
            size,
            format,
            bytes,
        };
        self
    }

    pub fn from_dynamic_image(mut self, dynamic_image: &'a DynamicImage, srgb: bool) -> Self {
        self.source = Source::DynamicImage {
            dynamic_image,
            srgb,
        };
        self
    }

    pub fn build(self, _encoder: &mut wgpu::CommandEncoder) -> wgpu::Texture {
        let texture = match self.source {
            Source::Empty { size, format } => {
                self.resources
                    .device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: self.name,
                        size,
                        mip_level_count: self.mip_level_count,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: self.usage,
                        view_formats: &[],
                    })
            }
            Source::Bytes {
                size,
                format,
                bytes,
            } => self.resources.device.create_texture_with_data(
                &self.resources.queue,
                &wgpu::TextureDescriptor {
                    label: self.name,
                    size,
                    mip_level_count: self.mip_level_count,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: self.usage,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                bytes,
            ),
            Source::DynamicImage {
                dynamic_image,
                srgb,
            } => {
                use DynamicImage::*;
                let (dynamic_image, mut format) = match dynamic_image {
                    ImageRgb8(_) => (
                        &ImageRgba8(dynamic_image.to_rgba8()),
                        wgpu::TextureFormat::Rgba8Unorm,
                    ),
                    ImageRgba8(_) => (dynamic_image, wgpu::TextureFormat::Rgba8Unorm),
                    ImageRgb16(_) => (
                        &ImageRgba16(dynamic_image.to_rgba16()),
                        wgpu::TextureFormat::Rgba16Unorm,
                    ),
                    ImageRgba16(_) => (dynamic_image, wgpu::TextureFormat::Rgba16Unorm),
                    ImageRgb32F(_) => (
                        &ImageRgba32F(dynamic_image.to_rgba32f()),
                        wgpu::TextureFormat::Rgba32Float,
                    ),
                    ImageRgba32F(_) => (dynamic_image, wgpu::TextureFormat::Rgba32Float),
                    _ => unimplemented!(),
                };
                if srgb {
                    format = format.add_srgb_suffix();
                }
                self.resources.device.create_texture_with_data(
                    &self.resources.queue,
                    &wgpu::TextureDescriptor {
                        label: self.name,
                        size: wgpu::Extent3d {
                            width: dynamic_image.width(),
                            height: dynamic_image.height(),
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: self.mip_level_count,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: self.usage,
                        view_formats: &[],
                    },
                    wgpu::util::TextureDataOrder::LayerMajor,
                    dynamic_image.as_bytes(),
                )
            }
        };
        if self.generate_mips {
            todo!("generate mips");
        }
        texture
    }
}

enum Source<'a> {
    Empty {
        size: wgpu::Extent3d,
        format: wgpu::TextureFormat,
    },
    Bytes {
        size: wgpu::Extent3d,
        format: wgpu::TextureFormat,
        bytes: &'a [u8],
    },
    DynamicImage {
        dynamic_image: &'a DynamicImage,
        srgb: bool,
    },
}
