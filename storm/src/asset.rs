use std::iter::repeat_n;

use glam::{Mat4, Quat};
use gltf::texture::WrappingMode;
use wgpu::AddressMode;

use crate::{
    Id, Resources,
    geometry::MorphTargetBuilder,
    material::{AlphaMode, Material},
    mesh::Mesh,
    scene::{Node, Scene, animation},
    storage::DenseEntry,
};

const SUPPORTED_EXTENSIONS: &[&str] = &[];

pub fn open_gltf<'r>(
    path: impl AsRef<std::path::Path>,
    resources: &'r mut Resources,
    encoder: &mut wgpu::CommandEncoder,
    render_width: u32,
    render_height: u32,
) -> Result<(Vec<Scene>, Option<usize>), gltf::Error> {
    let (document, buffers, images_data) = gltf::import(path)?;

    for extension in document.extensions_required() {
        if !SUPPORTED_EXTENSIONS.contains(&extension) {
            panic!("unsupported gltf extension {extension}");
        }
    }

    let mut images = vec![None; document.images().len()];
    let mut samplers = vec![None; document.samplers().len()];
    let mut default_sampler = None;
    let mut textures = vec![None; document.textures().len()];

    let materials: Vec<_> = document
        .materials()
        .map(|material| {
            create_material(
                material,
                &images_data,
                resources,
                &mut images,
                &mut samplers,
                &mut default_sampler,
                &mut textures,
                encoder,
            )
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
                    let topology = match primitive.mode() {
                        gltf::mesh::Mode::Points => wgpu::PrimitiveTopology::PointList,
                        gltf::mesh::Mode::LineStrip => wgpu::PrimitiveTopology::LineStrip,
                        gltf::mesh::Mode::Lines => wgpu::PrimitiveTopology::LineList,
                        gltf::mesh::Mode::Triangles => wgpu::PrimitiveTopology::TriangleList,
                        gltf::mesh::Mode::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
                        _ => panic!("unsupported primitive topology {:?}", primitive.mode()),
                    };
                    let mut geometry_builder = resources
                        .geometry_builder()
                        .positions(positions)
                        .topology(topology);
                    if let Some(indices) = reader.read_indices() {
                        geometry_builder =
                            geometry_builder.indices_u32(indices.into_u32().collect());
                    }
                    if let Some(normals) = reader.read_normals() {
                        geometry_builder = geometry_builder.normals(normals);
                    }
                    if let Some(tangents) = reader.read_tangents() {
                        geometry_builder = geometry_builder.tangents(tangents);
                    }
                    if let Some(normal_texture) = primitive.material().normal_texture() {
                        geometry_builder =
                            geometry_builder.normal_tex_coord(normal_texture.tex_coord());
                    }
                    for set in 0.. {
                        match reader.read_tex_coords(set) {
                            Some(tex_coords) => {
                                geometry_builder =
                                    geometry_builder.tex_coords(tex_coords.into_f32());
                            }
                            None => break,
                        }
                    }
                    for set in 0.. {
                        match reader.read_colors(set) {
                            Some(colors) => {
                                geometry_builder = geometry_builder.colors(colors.into_rgba_f32());
                            }
                            None => break,
                        }
                    }
                    for set in 0.. {
                        match reader.read_joints(set) {
                            Some(joints) => {
                                geometry_builder = match joints {
                                    gltf::mesh::util::ReadJoints::U8(iter) => geometry_builder
                                        .joints(iter.map(|[a, b, c, d]| {
                                            [a as u32, b as u32, c as u32, d as u32]
                                        })),
                                    gltf::mesh::util::ReadJoints::U16(iter) => geometry_builder
                                        .joints(iter.map(|[a, b, c, d]| {
                                            [a as u32, b as u32, c as u32, d as u32]
                                        })),
                                }
                            }
                            None => break,
                        }
                    }
                    for set in 0.. {
                        match reader.read_weights(set) {
                            Some(weights) => {
                                geometry_builder = geometry_builder.weights(weights.into_f32());
                            }
                            None => break,
                        }
                    }
                    for (positions, normals, tangents) in reader.read_morph_targets() {
                        let mut builder = MorphTargetBuilder::new();
                        if let Some(positions) = positions {
                            builder = builder.positions(positions);
                        }
                        if let Some(normals) = normals {
                            builder = builder.normals(normals);
                        }
                        if let Some(tangents) = tangents {
                            builder = builder.tangents(tangents);
                        }
                        geometry_builder = geometry_builder.morph_target(builder);
                    }
                    let geometry = geometry_builder.build(encoder).id();
                    let material = match primitive.material().index() {
                        Some(index) => materials[index],
                        None => *default_material.get_or_insert_with(|| {
                            create_material(
                                primitive.material(),
                                &images_data,
                                resources,
                                &mut images,
                                &mut samplers,
                                &mut default_sampler,
                                &mut textures,
                                encoder,
                            )
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

    let scenes =
        document
            .scenes()
            .map(|gltf_scene| {
                let mut scene = Scene::new(
                    gltf_scene
                        .name()
                        .map_or_else(|| gltf_scene.index().to_string(), |name| name.to_string()),
                    resources,
                    encoder,
                    render_width,
                    render_height,
                );
                let mut node_mapping = vec![None; document.nodes().len()];
                let mut skins = vec![None; document.skins().len()];
                for node in gltf_scene.nodes() {
                    scene.build_gltf_node(
                        node,
                        None,
                        &mesh_mapping,
                        &mut node_mapping,
                        &mut skins,
                        resources,
                    );
                }

                skins
                    .into_iter()
                    .enumerate()
                    .filter_map(|(skin, nodes)| {
                        nodes.map(|nodes| (document.skins().nth(skin).unwrap(), nodes))
                    })
                    .for_each(|(skin, nodes)| {
                        let mut builder = scene.skin_builder().nodes(skin.joints().map(|joint| {
                            node_mapping[joint.index()]
                                .expect("skin joints must belong to same scene as skinned node")
                        }));
                        if let Some(inverse_bind_matrices) = skin
                            .reader(|buffer| Some(&buffers[buffer.index()].0))
                            .read_inverse_bind_matrices()
                        {
                            builder = builder.inverse_bind_matrices(
                                inverse_bind_matrices.map(|mat| Mat4::from_cols_array_2d(&mat)),
                            )
                        }
                        let skin = builder.build().id();
                        nodes.iter().for_each(|node| {
                            scene.add_skin_to_node(skin, *node);
                        });
                    });

                document.animations().for_each(|animation| {
                    let mut channels = Vec::new();
                    for channel in animation.channels() {
                        match node_mapping[channel.target().node().index()] {
                            Some(id) => {
                                let morph_targets_count = scene[id].weights().len();
                                channels.push((id, morph_targets_count, channel));
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
                        .channels(channels.into_iter().map(
                            |(node, morph_targets_count, channel)| {
                                let reader =
                                    channel.reader(|buffer| Some(&buffers[buffer.index()].0));
                                let inputs = reader
                                    .read_inputs()
                                    .expect("gltf animation sampler missing inputs")
                                    .collect();
                                let interpolation = match channel.sampler().interpolation() {
                                    gltf::animation::Interpolation::Step => {
                                        animation::Interpolation::Step
                                    }
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
                                        animation::Outputs::Rotations(
                                            rotations.into_f32().collect(),
                                        )
                                    }
                                    gltf::animation::util::ReadOutputs::Scales(iter) => {
                                        animation::Outputs::Scales(iter.collect())
                                    }
                                    gltf::animation::util::ReadOutputs::MorphTargetWeights(
                                        weights,
                                    ) => animation::Outputs::Weights(
                                        weights.into_f32().collect(),
                                        morph_targets_count,
                                    ),
                                };
                                animation::Channel {
                                    node,
                                    inputs,
                                    interpolation,
                                    outputs,
                                }
                            },
                        ))
                        .build();
                });
                scene
            })
            .collect();

    let default_scene = document.default_scene().map(|scene| scene.index());
    Ok((scenes, default_scene))
}

fn create_material(
    material: gltf::Material,
    images_data: &[gltf::image::Data],
    resources: &mut Resources,
    images: &mut [Option<wgpu::TextureView>],
    samplers: &mut [Option<wgpu::Sampler>],
    default_sampler: &mut Option<wgpu::Sampler>,
    textures: &mut [Option<(wgpu::TextureView, wgpu::Sampler)>],
    encoder: &mut wgpu::CommandEncoder,
) -> Id<Material> {
    let pbr_metallic_roughness = material.pbr_metallic_roughness();
    if let Some(info) = pbr_metallic_roughness.base_color_texture() {
        let texture = info.texture();
        textures[texture.index()].get_or_insert_with(|| {
            create_texture(
                texture,
                images_data,
                true,
                resources,
                images,
                samplers,
                default_sampler,
                encoder,
            )
        });
    }
    if let Some(info) = pbr_metallic_roughness.metallic_roughness_texture() {
        let texture = info.texture();
        textures[texture.index()].get_or_insert_with(|| {
            create_texture(
                texture,
                images_data,
                false,
                resources,
                images,
                samplers,
                default_sampler,
                encoder,
            )
        });
    }
    if let Some(info) = material.normal_texture() {
        let texture = info.texture();
        textures[texture.index()].get_or_insert_with(|| {
            create_texture(
                texture,
                images_data,
                false,
                resources,
                images,
                samplers,
                default_sampler,
                encoder,
            )
        });
    }
    if let Some(info) = material.occlusion_texture() {
        let texture = info.texture();
        textures[texture.index()].get_or_insert_with(|| {
            create_texture(
                texture,
                images_data,
                false,
                resources,
                images,
                samplers,
                default_sampler,
                encoder,
            )
        });
    }
    if let Some(info) = material.emissive_texture() {
        let texture = info.texture();
        textures[texture.index()].get_or_insert_with(|| {
            create_texture(
                texture,
                images_data,
                true,
                resources,
                images,
                samplers,
                default_sampler,
                encoder,
            )
        });
    }
    let mut builder = resources
        .material_builder()
        .base_color_factor(pbr_metallic_roughness.base_color_factor())
        .metallic_factor(pbr_metallic_roughness.metallic_factor())
        .roughness_factor(pbr_metallic_roughness.roughness_factor())
        .emissive_factor(material.emissive_factor())
        .alpha_mode(match material.alpha_mode() {
            gltf::material::AlphaMode::Blend => AlphaMode::Blend,
            gltf::material::AlphaMode::Mask => AlphaMode::Mask,
            gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
        });
    if let Some(base_color_texture) = pbr_metallic_roughness.base_color_texture() {
        let (texture, sampler) = textures[base_color_texture.texture().index()]
            .as_ref()
            .unwrap();
        builder = builder
            .base_color_tex_coord(base_color_texture.tex_coord())
            .base_color_texture(texture)
            .base_color_sampler(sampler);
    }
    if let Some(metallic_roughness_texture) = pbr_metallic_roughness.metallic_roughness_texture() {
        let (texture, sampler) = textures[metallic_roughness_texture.texture().index()]
            .as_ref()
            .unwrap();
        builder = builder
            .metallic_roughness_tex_coord(metallic_roughness_texture.tex_coord())
            .metallic_roughness_texture(texture)
            .metallic_roughness_sampler(sampler);
    }
    if let Some(normal_texture) = material.normal_texture() {
        let (texture, sampler) = textures[normal_texture.texture().index()].as_ref().unwrap();
        builder = builder
            .normal_scale(normal_texture.scale())
            .normal_tex_coord(normal_texture.tex_coord())
            .normal_texture(texture)
            .normal_sampler(sampler);
    }
    if let Some(occlusion_texture) = material.occlusion_texture() {
        let (texture, sampler) = textures[occlusion_texture.texture().index()]
            .as_ref()
            .unwrap();
        builder = builder
            .occlusion_strength(occlusion_texture.strength())
            .occlusion_tex_coord(occlusion_texture.tex_coord())
            .occlusion_texture(texture)
            .occlusion_sampler(sampler);
    }
    if let Some(emissive_texture) = material.emissive_texture() {
        let (texture, sampler) = textures[emissive_texture.texture().index()]
            .as_ref()
            .unwrap();
        builder = builder
            .emissive_tex_coord(emissive_texture.tex_coord())
            .emissive_texture(texture)
            .emissive_sampler(sampler);
    }
    if let Some(alpha_cutoff) = material.alpha_cutoff() {
        builder = builder.alpha_cutoff(alpha_cutoff);
    }
    if material.double_sided() {
        builder = builder.double_sided();
    }
    builder.build().id()
}

impl Scene {
    fn build_gltf_node<'a>(
        &mut self,
        node: gltf::Node<'a>,
        parent: Option<Id<Node>>,
        mesh_mapping: &[Id<Mesh>],
        node_mapping: &mut [Option<Id<Node>>],
        skins: &mut [Option<Vec<Id<Node>>>],
        resources: &Resources,
    ) -> Id<Node> {
        let mut builder = self
            .node_builder()
            .name(format!(
                "Gltf node {} {}",
                node.index(),
                node.name().unwrap_or("")
            ))
            .parent(parent);
        let mut default_weights = None;
        if let Some(mesh) = node.mesh() {
            default_weights = mesh.weights();
            builder = builder.mesh(mesh_mapping[mesh.index()]);
        }
        if let Some(weights) = node.weights().or(default_weights) {
            builder = builder.weights(weights.into());
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
        let id = builder.build(resources).id();
        if let Some(skin) = node.skin() {
            skins[skin.index()]
                .get_or_insert_with(|| Vec::new())
                .push(id);
        }
        node_mapping[node.index()] = Some(id);
        for child in node.children() {
            self.build_gltf_node(
                child,
                Some(id),
                mesh_mapping,
                node_mapping,
                skins,
                resources,
            );
        }
        id
    }
}

fn create_texture(
    texture: gltf::Texture,
    images_data: &[gltf::image::Data],
    srgb: bool,
    resources: &mut Resources,
    images: &mut [Option<wgpu::TextureView>],
    samplers: &mut [Option<wgpu::Sampler>],
    default_sampler: &mut Option<wgpu::Sampler>,
    encoder: &mut wgpu::CommandEncoder,
) -> (wgpu::TextureView, wgpu::Sampler) {
    let image = texture.source();
    let image = images[image.index()]
        .get_or_insert_with(|| create_image(image, srgb, images_data, resources, encoder))
        .clone();
    let sampler = texture.sampler();
    let sampler = match sampler.index() {
        Some(index) => &mut samplers[index],
        None => default_sampler,
    }
    .get_or_insert_with(|| create_sampler(sampler, resources))
    .clone();
    (image, sampler)
}

fn create_image(
    image: gltf::Image,
    srgb: bool,
    images_data: &[gltf::image::Data],
    resources: &mut Resources,
    encoder: &mut wgpu::CommandEncoder,
) -> wgpu::TextureView {
    let data = &images_data[image.index()];
    let (bytes, format) = match data.format {
        gltf::image::Format::R8 => (&data.pixels, wgpu::TextureFormat::R8Unorm),
        gltf::image::Format::R8G8 => (&data.pixels, wgpu::TextureFormat::Rg8Unorm),
        gltf::image::Format::R8G8B8 => (
            &rgb_to_rgba(&data.pixels, 1),
            if srgb {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            },
        ),
        gltf::image::Format::R8G8B8A8 => (
            &data.pixels,
            if srgb {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            },
        ),
        gltf::image::Format::R16 => (&data.pixels, wgpu::TextureFormat::R16Unorm),
        gltf::image::Format::R16G16 => (&data.pixels, wgpu::TextureFormat::Rg16Unorm),
        gltf::image::Format::R16G16B16 => (
            &rgb_to_rgba(&data.pixels, 2),
            wgpu::TextureFormat::Rgba16Unorm,
        ),
        gltf::image::Format::R16G16B16A16 => (&data.pixels, wgpu::TextureFormat::Rgba16Unorm),
        gltf::image::Format::R32G32B32FLOAT => (
            &rgb_to_rgba(&data.pixels, 4),
            wgpu::TextureFormat::Rgba32Float,
        ),
        gltf::image::Format::R32G32B32A32FLOAT => (&data.pixels, wgpu::TextureFormat::Rgba32Float),
    };
    let name = image
        .name()
        .map_or_else(|| format!("Image {}", image.index()), str::to_string);
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
        .generate_mips()
        .build(encoder)
        .create_view(&wgpu::TextureViewDescriptor {
            label: Some(&name),
            ..Default::default()
        })
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

fn create_sampler(sampler: gltf::texture::Sampler, resources: &mut Resources) -> wgpu::Sampler {
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
