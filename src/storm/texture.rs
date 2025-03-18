use std::iter::{once, repeat_n};

use wgpu::util::DeviceExt;

use super::{
    storage::{Id, SparseMap, SparseSet},
    Asset,
};

use TextureInner::*;

pub struct TextureManager {
    textures: SparseSet<Texture>,
    images: SparseSet<Image>,
    samplers: SparseSet<Sampler>,
    default_sampler: Option<Id<Sampler>>,
    assets: SparseMap<Asset, AssetData>,
}

impl TextureManager {
    pub fn new() -> Self {
        Self {
            textures: SparseSet::new(),
            images: SparseSet::new(),
            samplers: SparseSet::new(),
            default_sampler: None,
            assets: SparseMap::new(),
        }
    }

    pub fn register_asset(&mut self, id: Id<Asset>, data: Vec<gltf::image::Data>) {
        self.assets.insert(
            id,
            AssetData {
                data,
                texture_mapping: Vec::new(),
                image_mapping: Vec::new(),
                sampler_mapping: Vec::new(),
            },
        );
    }

    pub fn create_view_sampler(
        &mut self,
        view: wgpu::TextureView,
        sampler: wgpu::Sampler,
    ) -> Id<Texture> {
        self.textures.push(Texture(ViewSampler(view, sampler)))
    }

    pub fn load_texture(
        &mut self,
        asset: Id<Asset>,
        texture: gltf::Texture,
        srgb: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<Texture> {
        match self.assets[asset].texture_mapping.get(texture.index()) {
            Some(Some(id)) => *id,
            _ => self.create_texture(asset, texture, srgb, device, queue),
        }
    }

    fn create_texture(
        &mut self,
        asset: Id<Asset>,
        texture: gltf::Texture,
        srgb: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<Texture> {
        let image = self.load_image(asset, texture.source(), srgb, device, queue);
        let sampler = self.load_sampler(asset, texture.sampler(), device);
        let id = self.textures.push(Texture(ImageSampler(image, sampler)));

        let mapping = &mut self.assets[asset].texture_mapping;
        match mapping.get_mut(texture.index()) {
            Some(entry) => *entry = Some(id),
            None => {
                let iter = repeat_n(None, texture.index() - mapping.len()).chain(once(Some(id)));
                mapping.extend(iter);
            }
        }

        id.into()
    }

    fn load_image(
        &mut self,
        asset: Id<Asset>,
        image: gltf::Image,
        srgb: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<Image> {
        match self.assets[asset].image_mapping.get(image.index()) {
            Some(Some(id)) => *id,
            _ => self.create_image(asset, image, srgb, device, queue),
        }
    }

    fn create_image(
        &mut self,
        asset: Id<Asset>,
        image: gltf::Image,
        srgb: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<Image> {
        let asset = &mut self.assets[asset];

        let data = &asset.data[image.index()];
        let mut create = |format: wgpu::TextureFormat, bytes| {
            let texture = device.create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some(&format!("Image {}", image.name().unwrap_or(""))),
                    size: wgpu::Extent3d {
                        width: data.width,
                        height: data.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: if srgb {
                        format.add_srgb_suffix()
                    } else {
                        format
                    },
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::MipMajor,
                bytes,
            );

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

            let id = self.images.push(Image { texture, view });

            match asset.image_mapping.get_mut(image.index()) {
                Some(entry) => *entry = Some(id),
                None => {
                    let iter = repeat_n(None, image.index() - asset.image_mapping.len())
                        .chain(once(Some(id)));
                    asset.image_mapping.extend(iter);
                }
            }

            id
        };

        match data.format {
            gltf::image::Format::R8 => create(wgpu::TextureFormat::R8Unorm, &data.pixels),
            gltf::image::Format::R8G8 => create(wgpu::TextureFormat::Rg8Unorm, &data.pixels),
            gltf::image::Format::R8G8B8 => {
                let bytes: Vec<_> = data
                    .pixels
                    .chunks(3)
                    .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
                    .collect();
                create(wgpu::TextureFormat::Rgba8Unorm, &bytes)
            }
            gltf::image::Format::R8G8B8A8 => create(wgpu::TextureFormat::Rgba8Unorm, &data.pixels),
            gltf::image::Format::R16 => create(wgpu::TextureFormat::R16Unorm, &data.pixels),
            gltf::image::Format::R16G16 => create(wgpu::TextureFormat::Rg16Unorm, &data.pixels),
            gltf::image::Format::R16G16B16 => {
                let bytes: Vec<_> = data
                    .pixels
                    .chunks(6)
                    .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], rgb[3], rgb[4], rgb[5], 255, 255])
                    .collect();
                create(wgpu::TextureFormat::Rgba16Unorm, &bytes)
            }
            gltf::image::Format::R16G16B16A16 => {
                create(wgpu::TextureFormat::Rgba16Unorm, &data.pixels)
            }
            gltf::image::Format::R32G32B32FLOAT => {
                let alpha = f32::to_le_bytes(1.0);
                let bytes: Vec<_> = data
                    .pixels
                    .chunks(12)
                    .flat_map(|rgb| {
                        [
                            rgb[0], rgb[1], rgb[2], rgb[3], // red
                            rgb[4], rgb[5], rgb[6], rgb[7], // greed
                            rgb[8], rgb[9], rgb[10], rgb[11], // blue
                            alpha[0], alpha[1], alpha[2], alpha[3],
                        ]
                    })
                    .collect();
                create(wgpu::TextureFormat::Rgba32Float, &bytes)
            }
            gltf::image::Format::R32G32B32A32FLOAT => {
                create(wgpu::TextureFormat::Rgba32Float, &data.pixels)
            }
        }
    }

    fn load_sampler(
        &mut self,
        asset: Id<Asset>,
        sampler: gltf::texture::Sampler,
        device: &wgpu::Device,
    ) -> Id<Sampler> {
        match sampler.index() {
            Some(index) => match self.assets[asset].sampler_mapping.get(index) {
                Some(Some(id)) => *id,
                _ => self.create_sampler(asset, sampler, device),
            },
            None => self.create_sampler(asset, sampler, device),
        }
    }

    fn create_sampler(
        &mut self,
        asset: Id<Asset>,
        sampler: gltf::texture::Sampler,
        device: &wgpu::Device,
    ) -> Id<Sampler> {
        use wgpu::FilterMode::*;

        let mag_filter = match sampler.mag_filter() {
            Some(gltf::texture::MagFilter::Nearest) => Nearest,
            Some(gltf::texture::MagFilter::Linear) => Linear,
            None => wgpu::FilterMode::default(),
        };
        let (min_filter, mipmap_filter) = match sampler.min_filter() {
            Some(gltf::texture::MinFilter::Nearest) => (Nearest, wgpu::FilterMode::default()),
            Some(gltf::texture::MinFilter::Linear) => (Linear, wgpu::FilterMode::default()),
            Some(gltf::texture::MinFilter::NearestMipmapNearest) => (Nearest, Nearest),
            Some(gltf::texture::MinFilter::LinearMipmapNearest) => (Linear, Nearest),
            Some(gltf::texture::MinFilter::NearestMipmapLinear) => (Nearest, Linear),
            Some(gltf::texture::MinFilter::LinearMipmapLinear) => (Linear, Linear),
            None => (wgpu::FilterMode::default(), wgpu::FilterMode::default()),
        };
        let inner = device.create_sampler(&wgpu::SamplerDescriptor {
            label: sampler.name(),
            address_mode_u: address_mode(sampler.wrap_s()),
            address_mode_v: address_mode(sampler.wrap_t()),
            mag_filter,
            min_filter,
            mipmap_filter,
            ..Default::default()
        });

        let id = self.samplers.push(Sampler { inner });

        match sampler.index() {
            Some(index) => {
                let mapping = &mut self.assets[asset].sampler_mapping;
                match mapping.get_mut(index) {
                    Some(entry) => *entry = Some(id),
                    None => {
                        let iter = repeat_n(None, index - mapping.len()).chain(once(Some(id)));
                        mapping.extend(iter);
                    }
                }
            }
            None => self.default_sampler = Some(id),
        }

        id
    }

    pub fn view(&self, id: Id<Texture>) -> Option<&wgpu::TextureView> {
        self.textures.get(id).map(|texture| match &texture.0 {
            ImageSampler(image, _) => &self.images[*image].view,
            ViewSampler(view, _) => view,
        })
    }

    pub fn sampler(&self, id: Id<Texture>) -> Option<&wgpu::Sampler> {
        self.textures.get(id).map(|texture| match &texture.0 {
            ImageSampler(_, sampler) => &self.samplers[*sampler].inner,
            ViewSampler(_, sampler) => sampler,
        })
    }
}

pub struct Texture(TextureInner);

enum TextureInner {
    ImageSampler(Id<Image>, Id<Sampler>),
    ViewSampler(wgpu::TextureView, wgpu::Sampler),
}

struct Image {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct Sampler {
    inner: wgpu::Sampler,
}

fn address_mode(wrap: gltf::texture::WrappingMode) -> wgpu::AddressMode {
    match wrap {
        gltf::texture::WrappingMode::Repeat => wgpu::AddressMode::Repeat,
        gltf::texture::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
        gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
    }
}

struct AssetData {
    data: Vec<gltf::image::Data>,
    texture_mapping: Vec<Option<Id<Texture>>>,
    image_mapping: Vec<Option<Id<Image>>>,
    sampler_mapping: Vec<Option<Id<Sampler>>>,
}
