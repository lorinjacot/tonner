use std::iter::repeat_n;

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
    mappings: SparseMap<Asset, AssetMappings>,
}

impl TextureManager {
    pub fn new() -> Self {
        Self {
            textures: SparseSet::new(),
            images: SparseSet::new(),
            samplers: SparseSet::new(),
            default_sampler: None,
            mappings: SparseMap::new(),
        }
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
        images: &Vec<gltf::image::Data>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<Texture> {
        let mapping = &mut self.mappings.entry(asset).or_default().textures;
        if let Some(Some(id)) = mapping.get(texture.index()) {
            return *id;
        }

        let image = self.load_image(asset, texture.source(), srgb, images, device, queue);
        let sampler = self.load_sampler(asset, texture.sampler(), device);
        let id = self.textures.push(Texture(ImageSampler(image, sampler)));

        let mapping = &mut self.mappings[asset].textures;
        match mapping.get_mut(texture.index()) {
            Some(entry) => *entry = Some(id),
            None => {
                let iter = repeat_n(None, texture.index() - mapping.len()).chain(Some(Some(id)));
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
        images: &Vec<gltf::image::Data>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<Image> {
        let mapping = &mut self.mappings.entry(asset).or_default().images;

        match mapping.get_mut(image.index()) {
            Some(Some(id)) => *id,
            Some(entry) => {
                let id =
                    self.images
                        .create(image.name(), &images[image.index()], srgb, device, queue);
                *entry = Some(id);
                id
            }
            None => {
                let id =
                    self.images
                        .create(image.name(), &images[image.index()], srgb, device, queue);
                let iter = repeat_n(None, image.index() - mapping.len()).chain(Some(Some(id)));
                mapping.extend(iter);
                id
            }
        }
    }

    fn load_sampler(
        &mut self,
        asset: Id<Asset>,
        sampler: gltf::texture::Sampler,
        device: &wgpu::Device,
    ) -> Id<Sampler> {
        let mapping = &mut self.mappings.entry(asset).or_default().samplers;

        let index = match sampler.index() {
            Some(index) => index,
            None => match self.default_sampler {
                Some(id) => return id,
                None => {
                    let id = self.samplers.create(sampler, device);
                    self.default_sampler = Some(id);
                    return id;
                }
            },
        };

        match mapping.get_mut(index) {
            Some(Some(id)) => *id,
            Some(entry) => {
                let id = self.samplers.create(sampler, device);
                *entry = Some(id);
                id
            }
            None => {
                let id = self.samplers.create(sampler, device);
                let iter = repeat_n(None, index - mapping.len()).chain(Some(Some(id)));
                mapping.extend(iter);
                id
            }
        }
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

impl SparseSet<Image> {
    fn create(
        &mut self,
        label: Option<&str>,
        data: &gltf::image::Data,
        srgb: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<Image> {
        let mut create = |format: wgpu::TextureFormat, bytes| {
            let texture = device.create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label,
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

            self.push(Image { texture, view })
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
}

struct Sampler {
    inner: wgpu::Sampler,
}

impl SparseSet<Sampler> {
    fn create(&mut self, sampler: gltf::texture::Sampler, device: &wgpu::Device) -> Id<Sampler> {
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: sampler.name(),
            address_mode_u: Self::address_mode(sampler.wrap_s()),
            address_mode_v: Self::address_mode(sampler.wrap_t()),
            mag_filter,
            min_filter,
            mipmap_filter,
            ..Default::default()
        });

        self.push(Sampler { inner: sampler })
    }

    fn address_mode(wrap: gltf::texture::WrappingMode) -> wgpu::AddressMode {
        match wrap {
            gltf::texture::WrappingMode::Repeat => wgpu::AddressMode::Repeat,
            gltf::texture::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
            gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        }
    }
}

#[derive(Default)]
struct AssetMappings {
    textures: Vec<Option<Id<Texture>>>,
    images: Vec<Option<Id<Image>>>,
    samplers: Vec<Option<Id<Sampler>>>,
}
