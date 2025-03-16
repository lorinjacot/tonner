use std::iter::repeat_n;

use wgpu::util::DeviceExt;

use super::{
    storage::{Id, SparseMap, SparseSet},
    Asset,
};

pub struct TextureManager {
    textures: SparseSet<Texture>,
    images: SparseSet<Image>,
    samplers: SparseSet<Sampler>,
    mappings: SparseMap<Asset, AssetMappings>,
}

impl TextureManager {
    pub fn new() -> Self {
        Self {
            textures: SparseSet::new(),
            images: SparseSet::new(),
            samplers: SparseSet::new(),
            mappings: SparseMap::new(),
        }
    }

    pub fn load(
        &mut self,
        asset: Id<Asset>,
        texture: gltf::Texture,
        srgb: bool,
        images: Vec<gltf::image::Data>,
        device: &wgpu::Device,
    ) -> Id<Texture> {
        todo!()
    }

    fn load_image(
        &mut self,
        asset: Id<Asset>,
        image: gltf::Image,
        srgb: bool,
        images: Vec<gltf::image::Data>,
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
}

pub struct Texture {}

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

struct Sampler {}

#[derive(Default)]
struct AssetMappings {
    textures: Vec<Option<Id<Texture>>>,
    images: Vec<Option<Id<Image>>>,
    samplers: Vec<Option<Id<Sampler>>>,
}
