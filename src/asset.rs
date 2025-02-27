use std::{
    collections::{hash_map::Entry, HashMap},
    path::Path,
};

use glam::{Mat4, Quat, Vec3};
use image::{DynamicImage, EncodableLayout, RgbImage};
use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::{
    scene::{
        MaterialDescriptor, MaterialId, MeshCreationError, MeshDescriptor, MeshId,
        NodeCreationError, NodeDescriptor, NodeId, NodeTransform, NormalTextureDescriptor,
        PrimitiveAttributes, PrimitiveDescriptor, PrimitiveIndices, Scene, TextureDescriptor,
        COLORS_LEN, TEX_COORDS_LEN,
    },
    texture::{Texture2d, Texture2dDescriptor, Texture2dSource, TextureCreationError},
};

pub struct Asset {
    buffers: Vec<gltf::buffer::Data>,
    images: Vec<gltf::image::Data>,
    scenes_mapping: HashMap<usize, SceneMapping>,
    texture_samplers: HashMap<usize, (Texture2d, wgpu::Sampler)>,
}

impl Asset {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<(Self, gltf::Document), gltf::Error> {
        let (document, buffers, images) = gltf::import(path)?;

        Ok((
            Self {
                buffers,
                images,
                scenes_mapping: HashMap::new(),
                texture_samplers: HashMap::new(),
            },
            document,
        ))
    }

    pub fn create_scene(
        &mut self,
        gltf_scene: gltf::Scene,
        scene_id: usize,
        scene: &mut Scene,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), CreationError> {
        for gltf_node in gltf_scene.nodes() {
            self.create_node(&gltf_node, None, scene_id, scene, device, queue)?;
        }

        Ok(())
    }

    pub fn create_texture(
        &mut self,
        label: Option<&str>,
        gltf_texture: &gltf::Texture,
        is_srgb: bool,
        scene: &mut Scene,
        device: &wgpu::Device,
    ) -> Result<(), CreationError> {
        match self.texture_samplers.entry(gltf_texture.index()) {
            Entry::Occupied(_) => Ok(()),
            Entry::Vacant(entry) => {
                let image = self
                    .images
                    .get(gltf_texture.source().index())
                    .ok_or(CreationError::InvalidAsset)?;

                let mut create_texture = |pixels: &[u8], format: wgpu::TextureFormat| {
                    scene.create_texture2d(&Texture2dDescriptor {
                        label,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING,
                        source: Texture2dSource::Pixel {
                            width: image.width,
                            height: image.height,
                            pixels,
                            format: if is_srgb {
                                format.add_srgb_suffix()
                            } else {
                                format
                            },
                        },
                    })
                };

                let texture = match image.format {
                    gltf::image::Format::R8G8B8 => {
                        // rgb => rgba conversion needed
                        let pixels = DynamicImage::ImageRgb8(
                            RgbImage::from_vec(image.width, image.height, image.pixels.clone())
                                .ok_or(CreationError::InvalidAsset)?,
                        )
                        .into_rgba8();
                        create_texture(pixels.as_bytes(), wgpu::TextureFormat::Rgba8Unorm)
                    }
                    gltf::image::Format::R8G8B8A8 => {
                        create_texture(&image.pixels, wgpu::TextureFormat::Rgba8Unorm)
                    }
                    _ => {
                        return Err(CreationError::Unsupported(format!(
                            "Image format {:?}",
                            image.format
                        )))
                    }
                }?;

                let gltf_sampler = gltf_texture.sampler();
                let address_mode_u = match gltf_sampler.wrap_s() {
                    gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
                    gltf::texture::WrappingMode::Repeat => wgpu::AddressMode::Repeat,
                    gltf::texture::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
                };
                let address_mode_v = match gltf_sampler.wrap_t() {
                    gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
                    gltf::texture::WrappingMode::Repeat => wgpu::AddressMode::Repeat,
                    gltf::texture::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
                };
                let mag_filter = match gltf_sampler.mag_filter() {
                    Some(gltf::texture::MagFilter::Nearest) => wgpu::FilterMode::Nearest,
                    Some(gltf::texture::MagFilter::Linear) | None => wgpu::FilterMode::Linear,
                };
                let (min_filter, mipmap_filter) = match gltf_sampler.min_filter() {
                    Some(
                        gltf::texture::MinFilter::Linear
                        | gltf::texture::MinFilter::LinearMipmapLinear,
                    )
                    | None => (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear),
                    Some(gltf::texture::MinFilter::LinearMipmapNearest) => {
                        (wgpu::FilterMode::Linear, wgpu::FilterMode::Nearest)
                    }
                    Some(
                        gltf::texture::MinFilter::Nearest
                        | gltf::texture::MinFilter::NearestMipmapLinear,
                    ) => (wgpu::FilterMode::Nearest, wgpu::FilterMode::Linear),
                    Some(gltf::texture::MinFilter::NearestMipmapNearest) => {
                        (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest)
                    }
                };
                let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                    label,
                    address_mode_u,
                    address_mode_v,
                    mag_filter,
                    min_filter,
                    mipmap_filter,
                    ..Default::default()
                });

                entry.insert((texture, sampler));

                Ok(())
            }
        }
    }

    pub fn create_material(
        &mut self,
        gltf_material: &gltf::Material,
        scene_id: usize,
        scene: &mut Scene,
        device: &wgpu::Device,
    ) -> Result<MaterialId, CreationError> {
        if let Some(material) = self
            .scenes_mapping
            .entry(scene_id)
            .or_default()
            .materials
            .get(&gltf_material.index())
        {
            return Ok(*material);
        }

        let pbr_metallic_roughness = gltf_material.pbr_metallic_roughness();
        if let Some(base_color_texture) = pbr_metallic_roughness.base_color_texture() {
            self.create_texture(
                Some("Material base color texture"),
                &base_color_texture.texture(),
                true,
                scene,
                device,
            )?;
        }
        if let Some(metallic_roughness_texture) =
            pbr_metallic_roughness.metallic_roughness_texture()
        {
            self.create_texture(
                Some("Material metallic roughness texture"),
                &metallic_roughness_texture.texture(),
                false,
                scene,
                device,
            )?;
        }
        if let Some(normal_texture) = gltf_material.normal_texture() {
            self.create_texture(
                Some("Material normal texture"),
                &normal_texture.texture(),
                false,
                scene,
                device,
            )?;
        }
        if let Some(emissive_texture) = gltf_material.emissive_texture() {
            self.create_texture(
                Some("Material emissive texture"),
                &emissive_texture.texture(),
                true,
                scene,
                device,
            )?;
        }

        let base_color_texture: Option<TextureDescriptor<'_>> =
            pbr_metallic_roughness.base_color_texture().map(|info| {
                let texture_sampler = &self.texture_samplers[&info.texture().index()];
                TextureDescriptor {
                    texture: &texture_sampler.0,
                    sampler: &texture_sampler.1,
                    tex_coord: info.tex_coord(),
                }
            });
        let metallic_roughness_texture =
            pbr_metallic_roughness
                .metallic_roughness_texture()
                .map(|info| {
                    let texture_sampler = &self.texture_samplers[&info.texture().index()];
                    TextureDescriptor {
                        texture: &texture_sampler.0,
                        sampler: &texture_sampler.1,
                        tex_coord: info.tex_coord(),
                    }
                });
        let normal_texture = gltf_material.normal_texture().map(|info| {
            let texture_sampler = &self.texture_samplers[&info.texture().index()];
            NormalTextureDescriptor {
                texture: &texture_sampler.0,
                sampler: &texture_sampler.1,
                tex_coord: info.tex_coord(),
                scale: info.scale(),
            }
        });
        let emissive_texture = gltf_material.emissive_texture().map(|info| {
            let texture_sampler = &self.texture_samplers[&info.texture().index()];
            TextureDescriptor {
                texture: &texture_sampler.0,
                sampler: &texture_sampler.1,
                tex_coord: info.tex_coord(),
            }
        });

        let material = MaterialDescriptor {
            base_color_factor: pbr_metallic_roughness.base_color_factor(),
            base_color_texture,
            metallic_factor: pbr_metallic_roughness.metallic_factor(),
            roughness_factor: pbr_metallic_roughness.roughness_factor(),
            metallic_roughness_texture,
            normal_texture,
            emissive_texture,
            emissive_factor: gltf_material.emissive_factor(),
        };

        let material = scene.create_material(&material);
        self.scenes_mapping
            .get_mut(&scene_id)
            .unwrap()
            .materials
            .insert(gltf_material.index(), material);
        Ok(material)
    }

    pub fn create_mesh(
        &mut self,
        gltf_mesh: &gltf::Mesh,
        scene_id: usize,
        scene: &mut Scene,
        device: &wgpu::Device,
    ) -> Result<MeshId, CreationError> {
        if let Some(mesh) = self
            .scenes_mapping
            .entry(scene_id)
            .or_default()
            .meshes
            .get(&gltf_mesh.index())
        {
            return Ok(*mesh);
        }

        let gltf_primitives = gltf_mesh.primitives();
        let mut primitives = Vec::with_capacity(gltf_primitives.len());
        for gltf_primitive in gltf_primitives {
            let reader = gltf_primitive.reader(|buffer| Some(&self.buffers.get(buffer.index())?));

            if let Some(mut positions) = reader.read_positions() {
                let attributes_count = positions.len();
                let mut vertex_count = attributes_count as u32;

                let indices = match reader.read_indices() {
                    Some(indices) => match indices {
                        gltf::mesh::util::ReadIndices::U8(_) => {
                            return Err(CreationError::Unsupported(
                                "Only u16 and u32 index format are supported".to_string(),
                            ))
                        }
                        gltf::mesh::util::ReadIndices::U16(indices) => {
                            let indices: Vec<_> = indices.collect();
                            vertex_count = indices.len() as u32;
                            Some(PrimitiveIndices {
                                buffer: device.create_buffer_init(
                                    &wgpu::util::BufferInitDescriptor {
                                        label: Some("Index buffer"),
                                        contents: bytemuck::cast_slice(&indices),
                                        usage: wgpu::BufferUsages::INDEX,
                                    },
                                ),
                                format: wgpu::IndexFormat::Uint16,
                            })
                        }
                        gltf::mesh::util::ReadIndices::U32(indices) => {
                            let indices: Vec<_> = indices.collect();
                            vertex_count = indices.len() as u32;
                            Some(PrimitiveIndices {
                                buffer: device.create_buffer_init(
                                    &wgpu::util::BufferInitDescriptor {
                                        label: Some("Index buffer"),
                                        contents: bytemuck::cast_slice(&indices),
                                        usage: wgpu::BufferUsages::INDEX,
                                    },
                                ),
                                format: wgpu::IndexFormat::Uint32,
                            })
                        }
                    },
                    None => None,
                };

                let mut tex_coords: Vec<Box<dyn Iterator<Item = [f32; 2]>>> =
                    Vec::with_capacity(TEX_COORDS_LEN);
                for set in 0..TEX_COORDS_LEN as u32 {
                    tex_coords.push(
                        reader
                            .read_tex_coords(set)
                            .map_or(Box::new(std::iter::repeat([0.0, 0.0])), |tex_coord| {
                                Box::new(tex_coord.into_f32())
                            }),
                    );
                }

                let mut normals = reader.read_normals().ok_or(CreationError::Unsupported(
                    "Attributes NORMAL is required".to_string(),
                ))?;
                let mut tangents = reader.read_tangents().ok_or(CreationError::Unsupported(
                    "Attributes TANGENT is required".to_string(),
                ))?;

                let mut colors: Vec<Box<dyn Iterator<Item = [f32; 4]>>> =
                    Vec::with_capacity(COLORS_LEN);
                for set in 0..COLORS_LEN as u32 {
                    colors.push(reader.read_colors(set).map_or(
                        Box::new(std::iter::repeat([1.0, 1.0, 1.0, 1.0])),
                        |colors| Box::new(colors.into_rgba_f32()),
                    ));
                }

                let mut attributes = Vec::with_capacity(attributes_count);
                for _ in 0..attributes_count {
                    attributes.push(PrimitiveAttributes {
                        position: positions.next().ok_or(CreationError::InvalidAsset)?,
                        normal: normals.next().ok_or(CreationError::InvalidAsset)?,
                        tangent: tangents.next().ok_or(CreationError::InvalidAsset)?,
                        tex_coords: [
                            tex_coords[0].next().ok_or(CreationError::InvalidAsset)?,
                            tex_coords[1].next().ok_or(CreationError::InvalidAsset)?,
                        ],
                        colors: [colors[0].next().ok_or(CreationError::InvalidAsset)?],
                    });
                }

                drop(tex_coords);
                drop(colors);

                let attributes = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Attributes buffer"),
                    contents: bytemuck::cast_slice(&attributes),
                    usage: wgpu::BufferUsages::VERTEX,
                });

                let material =
                    self.create_material(&gltf_primitive.material(), scene_id, scene, device)?;

                primitives.push(PrimitiveDescriptor {
                    vertex_count,
                    indices,
                    attributes,
                    material,
                });
            }
        }

        let mesh = MeshDescriptor { primitives };

        Ok(scene.create_mesh(mesh, device)?)
    }

    pub fn create_node(
        &mut self,
        gltf_node: &gltf::Node,
        parent: Option<NodeId>,
        scene_id: usize,
        scene: &mut Scene,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<NodeId, CreationError> {
        if let Some(node) = self
            .scenes_mapping
            .entry(scene_id)
            .or_default()
            .nodes
            .get(&gltf_node.index())
        {
            return Ok(*node);
        }

        let local_transform = match gltf_node.transform() {
            gltf::scene::Transform::Decomposed {
                translation,
                rotation,
                scale,
            } => NodeTransform::TRS {
                translation: Vec3::from_array(translation),
                rotation: Quat::from_array(rotation),
                scale: Vec3::from_array(scale),
            },
            gltf::scene::Transform::Matrix { matrix } => {
                NodeTransform::Matrix(Mat4::from_cols_array_2d(&matrix))
            }
        };

        let mesh = match gltf_node.mesh() {
            Some(gltf_mesh) => Some(self.create_mesh(&gltf_mesh, scene_id, scene, device)?),
            None => None,
        };

        let node = NodeDescriptor {
            local_transform,
            parent,
            mesh,
        };
        let node = scene.create_node(&node, device)?;

        for child_node in gltf_node.children() {
            self.create_node(&child_node, Some(node), scene_id, scene, device, queue)?;
        }

        Ok(node)
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CreationError {
    #[error(transparent)]
    NodeCreationError(#[from] NodeCreationError),
    #[error(transparent)]
    MeshCreationError(#[from] MeshCreationError),
    #[error(transparent)]
    TextureCreationError(#[from] TextureCreationError),
    #[error("Invalid asset")]
    InvalidAsset,
    #[error("unsupported: {0}")]
    Unsupported(String),
}

#[derive(Debug, Default)]
struct SceneMapping {
    nodes: HashMap<usize, NodeId>,
    meshes: HashMap<usize, MeshId>,
    materials: HashMap<Option<usize>, MaterialId>,
}
