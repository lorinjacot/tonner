use std::iter::repeat_n;

use glam::{Mat4, Quat};
use gltf::texture::WrappingMode;
use wgpu::AddressMode;

use crate::{
    Id, Resources,
    mesh::{MaterialBuilder, Mesh},
    scene::{Node, Scene, animation},
    storage::{DenseEntry, SparseSet},
};

const SUPPORTED_EXTENSIONS: &[&str] = &[];

pub fn open_gltf<'r>(
    path: impl AsRef<std::path::Path>,
    resources: &'r mut Resources,
    encoder: &mut wgpu::CommandEncoder,
) -> Result<(Vec<Scene>, Option<usize>), gltf::Error> {
    let (document, buffers, images) = gltf::import(path)?;

    for extension in document.extensions_required() {
        if !SUPPORTED_EXTENSIONS.contains(&extension) {
            panic!("unsupported gltf extension {extension}");
        }
    }

    let images: Vec<_> = document
        .images()
        .map(|image| {
            let data = &images[image.index()];
            let (bytes, format) = match data.format {
                gltf::image::Format::R8 => (&data.pixels, wgpu::TextureFormat::R8Unorm),
                gltf::image::Format::R8G8 => (&data.pixels, wgpu::TextureFormat::Rg8Unorm),
                gltf::image::Format::R8G8B8 => (
                    &rgb_to_rgba(&data.pixels, 1),
                    wgpu::TextureFormat::Rgba8Unorm,
                ),
                gltf::image::Format::R8G8B8A8 => (&data.pixels, wgpu::TextureFormat::Rgba8Unorm),
                gltf::image::Format::R16 => (&data.pixels, wgpu::TextureFormat::R16Unorm),
                gltf::image::Format::R16G16 => (&data.pixels, wgpu::TextureFormat::Rg16Unorm),
                gltf::image::Format::R16G16B16 => (
                    &rgb_to_rgba(&data.pixels, 2),
                    wgpu::TextureFormat::Rgba16Unorm,
                ),
                gltf::image::Format::R16G16B16A16 => {
                    (&data.pixels, wgpu::TextureFormat::Rgba16Unorm)
                }
                gltf::image::Format::R32G32B32FLOAT => (
                    &rgb_to_rgba(&data.pixels, 4),
                    wgpu::TextureFormat::Rgba32Float,
                ),
                gltf::image::Format::R32G32B32A32FLOAT => {
                    (&data.pixels, wgpu::TextureFormat::Rgba32Float)
                }
            };
            let name = format!(
                "Gltf image {} {}",
                image.index(),
                image.name().unwrap_or("")
            );
            resources
                .texture_builder()
                .name(&name)
                .bytes(
                    wgpu::Extent3d {
                        width: data.width,
                        height: data.height,
                        depth_or_array_layers: 1,
                    },
                    format,
                    &bytes,
                )
                .build(encoder)
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some(&name),
                    ..Default::default()
                })
        })
        .collect();

    let samplers: Vec<_> = document
        .samplers()
        .map(|sampler| from_gltf_sampler(sampler, resources))
        .collect();
    let default_sampler = document.textures().find_map(|texture| {
        let sampler = texture.sampler();
        match sampler.index() {
            Some(_) => None,
            None => Some(from_gltf_sampler(sampler, resources)),
        }
    });

    let textures: Vec<_> = document
        .textures()
        .map(|texture| {
            let sampler = match texture.sampler().index() {
                Some(index) => &samplers[index],
                None => default_sampler.as_ref().unwrap(),
            };
            (&images[texture.source().index()], sampler)
        })
        .collect();

    let materials: Vec<_> = document
        .materials()
        .map(|material| {
            resources
                .material_builder()
                .from_gltf(material, &textures)
                .build()
                .id()
        })
        .collect();
    let mut default_material = None;

    let mesh_mapping: Vec<_> = document
        .meshes()
        .map(|mesh| {
            let mut primitives = Vec::with_capacity(mesh.primitives().len());
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                if let Some(positions) = reader.read_positions() {
                    let mut geometry_builder =
                        resources.geometry_builder().positions(positions.collect());
                    if let Some(indices) = reader.read_indices() {
                        geometry_builder =
                            geometry_builder.indices_u32(indices.into_u32().collect());
                    }
                    if let Some(normals) = reader.read_normals() {
                        geometry_builder = geometry_builder.normals(normals.collect());
                    }
                    for set in 0.. {
                        match reader.read_tex_coords(set) {
                            Some(tex_coords) => {
                                geometry_builder = match tex_coords {
                                    gltf::mesh::util::ReadTexCoords::U8(iter) => {
                                        geometry_builder.tex_coords_u8(set, iter.collect())
                                    }
                                    gltf::mesh::util::ReadTexCoords::U16(iter) => {
                                        geometry_builder.tex_coords_u16(set, iter.collect())
                                    }
                                    gltf::mesh::util::ReadTexCoords::F32(iter) => {
                                        geometry_builder.tex_coords_f32(set, iter.collect())
                                    }
                                };
                            }
                            None => break,
                        }
                    }
                    for set in 0.. {
                        match reader.read_colors(set) {
                            Some(colors) => {
                                geometry_builder = match colors {
                                    gltf::mesh::util::ReadColors::RgbU8(_) => geometry_builder
                                        .colors_u8(set, colors.into_rgba_u8().collect()),
                                    gltf::mesh::util::ReadColors::RgbaU8(iter) => {
                                        geometry_builder.colors_u8(set, iter.collect())
                                    }
                                    gltf::mesh::util::ReadColors::RgbU16(_) => geometry_builder
                                        .colors_u16(set, colors.into_rgba_u16().collect()),
                                    gltf::mesh::util::ReadColors::RgbaU16(iter) => {
                                        geometry_builder.colors_u16(set, iter.collect())
                                    }
                                    gltf::mesh::util::ReadColors::RgbF32(_) => geometry_builder
                                        .colors_f32(set, colors.into_rgba_f32().collect()),
                                    gltf::mesh::util::ReadColors::RgbaF32(iter) => {
                                        geometry_builder.colors_f32(set, iter.collect())
                                    }
                                };
                            }
                            None => break,
                        }
                    }
                    let geometry = geometry_builder.build(encoder).id();
                    let material = match primitive.material().index() {
                        Some(index) => materials[index],
                        None => *default_material.get_or_insert_with(|| {
                            resources
                                .material_builder()
                                .from_gltf(primitive.material(), &textures)
                                .build()
                                .id()
                        }),
                    };
                    primitives.push((geometry, material));
                }
            }
            resources
                .mesh_builder()
                .name(format!(
                    "Gltf mesh {} {}",
                    mesh.index(),
                    mesh.name().unwrap_or("")
                ))
                .primitives(primitives)
                .build()
                .id()
        })
        .collect();

    let scenes = document
        .scenes()
        .map(|gltf_scene| {
            let mut scene = Scene::new(
                gltf_scene
                    .name()
                    .map_or_else(|| gltf_scene.index().to_string(), |name| name.to_string()),
                resources,
                encoder,
            );
            let mut node_mapping = vec![None; document.nodes().len()];
            for node in gltf_scene.nodes() {
                scene.build_gltf_node(
                    node,
                    None,
                    &mut resources.meshes,
                    &mesh_mapping,
                    &mut node_mapping,
                );
            }
            document.animations().for_each(|animation| {
                let mut channels = Vec::new();
                for channel in animation.channels() {
                    match node_mapping[channel.target().node().index()] {
                        Some(id) => {
                            channels.push((id, channel));
                        }
                        None => {
                            return;
                        }
                    }
                }
                scene
                    .animation_builder()
                    .name(format!(
                        "Gltf animation {} {}",
                        animation.index(),
                        animation.name().unwrap_or("")
                    ))
                    .repeat()
                    .channels(channels.into_iter().map(|(node, channel)| {
                        let reader = channel.reader(|buffer| Some(&buffers[buffer.index()].0));
                        let inputs = reader
                            .read_inputs()
                            .expect("gltf animation sampler missing inputs")
                            .collect();
                        let interpolation = match channel.sampler().interpolation() {
                            gltf::animation::Interpolation::Step => animation::Interpolation::Step,
                            gltf::animation::Interpolation::Linear => {
                                animation::Interpolation::Linear
                            }
                            gltf::animation::Interpolation::CubicSpline => {
                                animation::Interpolation::CubicSpline
                            }
                        };
                        let outputs = match reader
                            .read_outputs()
                            .expect("gltf animation sampler missing outputs")
                        {
                            gltf::animation::util::ReadOutputs::Translations(iter) => {
                                animation::Outputs::Translations(iter.collect())
                            }
                            gltf::animation::util::ReadOutputs::Rotations(rotations) => {
                                animation::Outputs::Rotations(rotations.into_f32().collect())
                            }
                            gltf::animation::util::ReadOutputs::Scales(iter) => {
                                animation::Outputs::Scales(iter.collect())
                            }
                            gltf::animation::util::ReadOutputs::MorphTargetWeights(_) => {
                                todo!("morph target weights animation")
                            }
                        };
                        animation::Channel {
                            node,
                            inputs,
                            interpolation,
                            outputs,
                        }
                    }))
                    .build();
            });
            scene
        })
        .collect();

    let default_scene = document.default_scene().map(|scene| scene.index());
    Ok((scenes, default_scene))
}

impl<'a, 'r> MaterialBuilder<'a, 'r> {
    fn from_gltf(
        mut self,
        material: gltf::Material,
        textures: &'a [(&wgpu::TextureView, &wgpu::Sampler)],
    ) -> Self {
        let pbr_metallic_roughness = material.pbr_metallic_roughness();
        if let Some(base_color_texture) = pbr_metallic_roughness.base_color_texture() {
            let (texture, sampler) = textures[base_color_texture.texture().index()];
            self = self
                .base_color_tex_coord(base_color_texture.tex_coord())
                .base_color_texture(texture)
                .base_color_sampler(sampler);
        }
        if let Some(metallic_roughness_texture) =
            pbr_metallic_roughness.metallic_roughness_texture()
        {
            let (texture, sampler) = textures[metallic_roughness_texture.texture().index()];
            self = self
                .metallic_roughness_tex_coord(metallic_roughness_texture.tex_coord())
                .metallic_roughness_texture(texture)
                .metallic_roughness_sampler(sampler);
        }
        self.base_color_factor(pbr_metallic_roughness.base_color_factor())
            .metallic_factor(pbr_metallic_roughness.metallic_factor())
            .roughness_factor(pbr_metallic_roughness.roughness_factor())
    }
}

impl Scene {
    fn build_gltf_node(
        &mut self,
        node: gltf::Node,
        parent: Option<Id<Node>>,
        meshes: &mut SparseSet<Mesh>,
        mesh_mapping: &[Id<Mesh>],
        node_mapping: &mut [Option<Id<Node>>],
    ) -> Id<Node> {
        let mut builder = self
            .node_builder()
            .name(format!(
                "Gltf node {} {}",
                node.index(),
                node.name().unwrap_or("")
            ))
            .parent(parent);
        if let Some(mesh) = node.mesh() {
            builder = builder.mesh(&meshes[mesh_mapping[mesh.index()]]);
        }
        builder = match node.transform() {
            gltf::scene::Transform::Decomposed {
                translation,
                rotation,
                scale,
            } => builder.translation_rotation_scale(
                translation.into(),
                Quat::from_array(rotation),
                scale.into(),
            ),
            gltf::scene::Transform::Matrix { matrix } => {
                builder.local_matrix(Mat4::from_cols_array_2d(&matrix))
            }
        };
        let id = builder.build().id();
        node_mapping[node.index()] = Some(id);
        for child in node.children() {
            self.build_gltf_node(child, Some(id), meshes, mesh_mapping, node_mapping);
        }
        id
    }
}

fn rgb_to_rgba(bytes: &Vec<u8>, bytes_per_channel: usize) -> Vec<u8> {
    bytes
        .chunks_exact(3 * bytes_per_channel)
        .flat_map(|rgb| {
            let mut rgba = Vec::with_capacity(4 * bytes_per_channel);
            for byte in rgb {
                rgba.push(*byte);
            }
            rgba.extend(repeat_n(0, bytes_per_channel));
            rgba
        })
        .collect()
}

fn wrapping_mode_to_address_mode(wrapping_mode: WrappingMode) -> AddressMode {
    match wrapping_mode {
        WrappingMode::ClampToEdge => AddressMode::ClampToEdge,
        WrappingMode::MirroredRepeat => AddressMode::MirrorRepeat,
        WrappingMode::Repeat => AddressMode::Repeat,
    }
}

fn from_gltf_sampler(sampler: gltf::texture::Sampler, resources: &mut Resources) -> wgpu::Sampler {
    let name = format!(
        "Gltf sampler {:?} {}",
        sampler.index(),
        sampler.name().unwrap_or("")
    );
    let mag_filter = match sampler.mag_filter() {
        Some(gltf::texture::MagFilter::Linear) => wgpu::FilterMode::Linear,
        Some(gltf::texture::MagFilter::Nearest) | None => wgpu::FilterMode::Nearest,
    };
    let (min_filter, mipmap_filter) = match sampler.min_filter() {
        Some(gltf::texture::MinFilter::LinearMipmapNearest)
        | Some(gltf::texture::MinFilter::Linear) => {
            (wgpu::FilterMode::Linear, wgpu::FilterMode::Nearest)
        }
        Some(gltf::texture::MinFilter::LinearMipmapLinear) => {
            (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear)
        }
        Some(gltf::texture::MinFilter::NearestMipmapNearest)
        | Some(gltf::texture::MinFilter::Nearest)
        | None => (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest),
        Some(gltf::texture::MinFilter::NearestMipmapLinear) => {
            (wgpu::FilterMode::Nearest, wgpu::FilterMode::Linear)
        }
    };
    resources.device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some(&name),
        address_mode_u: wrapping_mode_to_address_mode(sampler.wrap_s()),
        address_mode_v: wrapping_mode_to_address_mode(sampler.wrap_t()),
        mag_filter,
        min_filter,
        mipmap_filter,
        ..Default::default()
    })
}
