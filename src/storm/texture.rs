use std::{
    f32::consts::FRAC_PI_2,
    iter::{once, repeat_n},
};

use glam::{vec3, Mat4, Vec3};
use image::DynamicImage;
use wgpu::util::DeviceExt;

use super::{
    storage::{Id, SparseMap, SparseSet},
    Asset, Iter, Name, Storm,
};

use TextureInner::*;

pub const CUBE_VERTICES: &[Vec3] = &[
    // front face
    vec3(-1.0, 1.0, 1.0),
    vec3(-1.0, -1.0, 1.0),
    vec3(1.0, 1.0, 1.0),
    vec3(1.0, -1.0, 1.0),
    // right face
    vec3(1.0, 1.0, -1.0),
    vec3(1.0, 1.0, 1.0),
    vec3(1.0, -1.0, -1.0),
    vec3(1.0, -1.0, 1.0),
    // back face
    vec3(1.0, 1.0, -1.0),
    vec3(1.0, -1.0, -1.0),
    vec3(-1.0, 1.0, -1.0),
    vec3(-1.0, -1.0, -1.0),
    // left face
    vec3(-1.0, 1.0, 1.0),
    vec3(-1.0, 1.0, -1.0),
    vec3(-1.0, -1.0, 1.0),
    vec3(-1.0, -1.0, -1.0),
    // bottom face
    vec3(1.0, -1.0, 1.0),
    vec3(-1.0, -1.0, 1.0),
    vec3(1.0, -1.0, -1.0),
    vec3(-1.0, -1.0, -1.0),
    // top face
    vec3(-1.0, 1.0, 1.0),
    vec3(1.0, 1.0, 1.0),
    vec3(-1.0, 1.0, -1.0),
    vec3(1.0, 1.0, -1.0),
];

pub const CUBE_INDICES: &[u16] = &[
    0, 1, 2, 2, 1, 3, // front
    4, 5, 6, 6, 5, 7, // right
    8, 9, 10, 10, 9, 11, // back
    12, 13, 14, 14, 13, 15, // left
    16, 17, 18, 18, 17, 19, // bottom
    20, 21, 22, 22, 21, 23, // top
];

pub const CUBE_VERTEX_BUFFER_LAYOUT: &[wgpu::VertexBufferLayout] = &[wgpu::VertexBufferLayout {
    array_stride: size_of::<Vec3>() as u64,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &[wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0,
    }],
}];

pub struct TextureManager {
    textures: SparseSet<Texture>,
    images: SparseSet<Image>,
    samplers: SparseSet<Sampler>,
    cubemaps: SparseSet<Cubemap>,
    environment_maps: SparseSet<EnvironmentMap>,
    default_sampler: Option<Id<Sampler>>,
    cubemap_sampler: wgpu::Sampler,
    shader_module: wgpu::ShaderModule,
    equirectangular_bind_group_layout: wgpu::BindGroupLayout,
    equirectangular_to_cube_pipeline_layout: wgpu::PipelineLayout,
    cube_vertex_buffer: wgpu::Buffer,
    cube_index_buffer: wgpu::Buffer,
    view_projection_bind_groups: [wgpu::BindGroup; 6],
    assets: SparseMap<Asset, AssetData>,
}

impl TextureManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader_module = device.create_shader_module(wgpu::include_wgsl!("texture.wgsl"));

        let view_projection_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture manager view projection bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let equirectangular_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture manager equirectangular bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                ],
            });

        let equirectangular_to_cube_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Textures manager equirectangular to cube pipeline layout"),
                bind_group_layouts: &[
                    &view_projection_bind_group_layout,
                    &equirectangular_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let cube_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Texture manager cube vertex buffer"),
            contents: bytemuck::cast_slice(&CUBE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let cube_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Texture manager cube index buffer"),
            contents: bytemuck::cast_slice(&CUBE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let projection = Mat4::perspective_rh(FRAC_PI_2, 1.0, 0.1, 10.0);
        let create_bind_group = |view| {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Texture manager view projection buffer"),
                contents: bytemuck::cast_slice(&[projection * view]),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Texture manager view projection bind group"),
                layout: &view_projection_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            })
        };

        let view_projection_bind_groups = [
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, Vec3::X, Vec3::Y)),
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, -Vec3::X, Vec3::Y)),
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, Vec3::Y, Vec3::Z)),
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, -Vec3::Y, -Vec3::Z)),
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, -Vec3::Z, Vec3::Y)), // the z-axis of wgpu is our -z
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, Vec3::Z, Vec3::Y)),
        ];

        let cubemap_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Cubemap sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            textures: SparseSet::new(),
            images: SparseSet::new(),
            samplers: SparseSet::new(),
            cubemaps: SparseSet::new(),
            environment_maps: SparseSet::new(),
            shader_module,
            equirectangular_bind_group_layout,
            equirectangular_to_cube_pipeline_layout,
            cube_vertex_buffer,
            cube_index_buffer,
            view_projection_bind_groups,
            cubemap_sampler,
            default_sampler: None,
            assets: SparseMap::new(),
        }
    }

    pub fn register_asset(&mut self, id: Id<Asset>, images: Vec<gltf::image::Data>) {
        self.assets.insert(
            id,
            AssetData {
                data: images,
                texture_mapping: Vec::new(),
                image_mapping: Vec::new(),
                sampler_mapping: Vec::new(),
            },
        );
    }

    pub fn create_texture_view_sampler(
        &mut self,
        texture: wgpu::Texture,
        view: wgpu::TextureView,
        sampler: wgpu::Sampler,
    ) -> Id<Texture> {
        self.textures
            .push(Texture(TextureViewSampler(texture, view, sampler)))
    }

    pub fn create_dynamic_image(
        &mut self,
        name: Option<&str>,
        dynamic_image: &DynamicImage,
        srgb: bool,
        usage: wgpu::TextureUsages,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Id<Texture> {
        use DynamicImage::*;
        let (dynamic_image, format) = match dynamic_image {
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

        let name = Name::from_name_or_else(|| self.textures.next_id(), name);
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some(&format!("{} texture", name.0)),
                size: wgpu::Extent3d {
                    width: dynamic_image.width(),
                    height: dynamic_image.height(),
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
                usage,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            dynamic_image.as_bytes(),
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&format!("{} view", name.0)),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(&format!("{} sampler", name.0)),
            ..Default::default()
        });
        self.create_texture_view_sampler(texture, view, sampler)
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

    pub fn create_environment_map(
        &mut self,
        equirectangular_map: Id<Texture>,
    ) -> Id<EnvironmentMap> {
        todo!()
    }

    pub fn environment_map(&self, id: Id<EnvironmentMap>) -> Option<&EnvironmentMap> {
        self.environment_maps.get(id)
    }

    pub fn environment_maps(&self) -> Iter<'_, EnvironmentMap, EnvironmentMap> {
        self.environment_maps.iter()
    }

    pub fn texture(&self, id: Id<Texture>) -> Option<&wgpu::Texture> {
        self.textures.get(id).map(|texture| match &texture.0 {
            ImageSampler(image, _) => &self.images[*image].texture,
            TextureViewSampler(texture, _, _) => texture,
        })
    }

    pub fn view(&self, id: Id<Texture>) -> Option<&wgpu::TextureView> {
        self.textures.get(id).map(|texture| match &texture.0 {
            ImageSampler(image, _) => &self.images[*image].view,
            TextureViewSampler(_, view, _) => view,
        })
    }

    pub fn sampler(&self, id: Id<Texture>) -> Option<&wgpu::Sampler> {
        self.textures.get(id).map(|texture| match &texture.0 {
            ImageSampler(_, sampler) => &self.samplers[*sampler].inner,
            TextureViewSampler(_, _, sampler) => sampler,
        })
    }
}

pub struct Texture(TextureInner);

enum TextureInner {
    ImageSampler(Id<Image>, Id<Sampler>),
    TextureViewSampler(wgpu::Texture, wgpu::TextureView, wgpu::Sampler),
}

struct Image {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct Sampler {
    inner: wgpu::Sampler,
}

pub struct Cubemap {
    view: wgpu::TextureView,
}

impl Cubemap {
    pub fn from_equirectangular_map(
        name: Option<&str>,
        equirectangular_map: Id<Texture>,
        storm: &mut Storm,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Id<Self> {
        let texture = storm.textures.texture(equirectangular_map).unwrap();
        let view = storm.textures.view(equirectangular_map).unwrap();
        let sampler = storm.textures.sampler(equirectangular_map).unwrap();

        let name = Name::from_name_or_else(|| storm.textures.cubemaps.next_id(), name);
        let pipeline = storm
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("{name} creation pipeline")),
                layout: Some(&storm.textures.equirectangular_to_cube_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &storm.textures.shader_module,
                    entry_point: Some("vs_cube"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: CUBE_VERTEX_BUFFER_LAYOUT,
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &storm.textures.shader_module,
                    entry_point: Some("fs_equirectangular_to_cube"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(texture.format().into())],
                }),
                multiview: None,
                cache: None,
            });

        let equirectangular_bind_group =
            storm.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("equirectangular bind group"),
                layout: &storm.textures.equirectangular_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            });

        Self::from_pipeline(
            Some(&name.0),
            texture.width(),
            texture.height(),
            texture.format(),
            &pipeline,
            &equirectangular_bind_group,
            storm,
            encoder,
        )
    }

    fn from_pipeline(
        name: Option<&str>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        pipeline: &wgpu::RenderPipeline,
        source_bind_group: &wgpu::BindGroup,
        storm: &mut Storm,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Id<Self> {
        let name = Name::from_name_or_else(|| storm.textures.cubemaps.next_id(), name);

        let texture = storm.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("{name} texture")),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        for base_array_layer in 0..6 {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("create cubemap pipeline"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture.create_view(&wgpu::TextureViewDescriptor {
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        base_array_layer,
                        array_layer_count: Some(1),
                        ..Default::default()
                    }),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(
                0,
                &storm.textures.view_projection_bind_groups[base_array_layer as usize],
                &[],
            );
            render_pass.set_bind_group(1, source_bind_group, &[]);
            render_pass.set_vertex_buffer(0, storm.textures.cube_vertex_buffer.slice(..));
            render_pass.set_index_buffer(
                storm.textures.cube_index_buffer.slice(..),
                wgpu::IndexFormat::Uint16,
            );
            render_pass.draw_indexed(0..CUBE_INDICES.len() as u32, 0, 0..1);
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&format!("{name} texture view")),
            ..Default::default()
        });

        storm.textures.cubemaps.push(Cubemap { view })
    }
}

pub struct EnvironmentMap {
    pub name: Name,
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
