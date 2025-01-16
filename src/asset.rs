use std::{collections::HashMap, path::Path};

use glam::{Mat4, Quat, Vec3};
use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::{
    camera::Camera,
    scene::{
        MaterialDescriptor, MaterialId, MeshCreationError, MeshDescriptor, MeshId,
        NodeCreationError, NodeDescriptor, NodeTransform, PrimitiveAttributes, PrimitiveDescriptor,
        PrimitiveIndices, Scene,
    },
};

pub struct Asset {
    pub document: gltf::Document,
    buffers: Vec<gltf::buffer::Data>,
    images: Vec<gltf::image::Data>,
}

impl Asset {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, gltf::Error> {
        let (document, buffers, images) = gltf::import(path)?;
        Ok(Self {
            document,
            buffers,
            images,
        })
    }

    pub fn create_scene(
        &self,
        gltf_scene: gltf::Scene,
        camera: Camera,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Scene, SceneCreationError> {
        log::debug!("Creating empty scene...");
        let mut scene = Scene::new(device, camera);
        log::debug!("Empty scene created");

        let mut mesh_mapping = HashMap::new();
        let mut material_mapping = HashMap::new();

        log::debug!("Creating nodes descriptors...");
        let mut nodes = Vec::with_capacity(gltf_scene.nodes().len());
        for node in gltf_scene.nodes() {
            nodes.push(self.create_node(
                &node,
                &mut scene,
                &mut mesh_mapping,
                &mut material_mapping,
                device,
                queue,
            )?);
        }
        log::debug!("Creating nodes...");
        scene.create_node(nodes, device)?;

        Ok(scene)
    }

    fn create_material(
        &self,
        gltf_material: &gltf::Material,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<MaterialDescriptor, SceneCreationError> {
        let (base_color_texture, base_color_sampler, base_color_tex_coord) =
            match gltf_material.pbr_metallic_roughness().base_color_texture() {
                Some(info) => {
                    let texture = info.texture();
                    let image = &self.images[texture.source().index()];

                    (
                        self.create_texture_view(
                            Some("Base color texture"),
                            &image.pixels,
                            image.width,
                            image.height,
                            wgpu::TextureFormat::Rgba8UnormSrgb,
                            device,
                            queue,
                        ),
                        self.create_sampler(
                            &texture.sampler(),
                            Some("Material base color sampler"),
                            device,
                        ),
                        info.tex_coord(),
                    )
                }
                None => (
                    self.create_texture_view(
                        Some("Material default base color texture"),
                        &[255, 255, 255, 255],
                        1,
                        1,
                        wgpu::TextureFormat::Rgba8UnormSrgb,
                        device,
                        queue,
                    ),
                    device.create_sampler(&wgpu::SamplerDescriptor {
                        label: Some("Material default base color sampler"),
                        ..Default::default()
                    }),
                    0,
                ),
            };

        let (metallic_roughness_texture, metallic_roughness_sampler, metallic_roughness_tex_coord) =
            match gltf_material
                .pbr_metallic_roughness()
                .metallic_roughness_texture()
            {
                Some(info) => {
                    let texture = info.texture();
                    let image = &self.images[texture.source().index()];

                    let format = match image.format {
                        gltf::image::Format::R8G8B8A8 => wgpu::TextureFormat::Rgba8Unorm,
                        gltf::image::Format::R16G16B16A16 => wgpu::TextureFormat::Rgba16Unorm,
                        gltf::image::Format::R32G32B32A32FLOAT => wgpu::TextureFormat::Rgba32Float,
                        _ => return Err(SceneCreationError::InvalidAsset),
                    };

                    (
                        self.create_texture_view(
                            Some("Material metallic roughness texture"),
                            &image.pixels,
                            image.width,
                            image.height,
                            format,
                            device,
                            queue,
                        ),
                        self.create_sampler(
                            &texture.sampler(),
                            Some("Material metallic roughness sampler"),
                            device,
                        ),
                        info.tex_coord(),
                    )
                }
                None => (
                    self.create_texture_view(
                        Some("Material default metallic roughness texture"),
                        &[255, 255, 255, 255],
                        1,
                        1,
                        wgpu::TextureFormat::Rgba8Unorm,
                        device,
                        queue,
                    ),
                    device.create_sampler(&wgpu::SamplerDescriptor {
                        label: Some("Material default metallic roughness sampler"),
                        ..Default::default()
                    }),
                    0,
                ),
            };

        let (normal_texture, normal_sampler, normal_tex_coord, normal_texture_scale) =
            match gltf_material.normal_texture() {
                Some(_gltf_normal_texture) => {
                    todo!()
                }
                None => {
                    let contents: [f32; 4] = [0.0, 0.0, 1.0, 0.0];
                    (
                        self.create_texture_view(
                            Some("Material default normal texture"),
                            bytemuck::cast_slice(&contents),
                            1,
                            1,
                            wgpu::TextureFormat::Rgba32Float,
                            device,
                            queue,
                        ),
                        device.create_sampler(&wgpu::SamplerDescriptor {
                            label: Some("Material default normal sampler"),
                            ..Default::default()
                        }),
                        0,
                        1.0,
                    )
                }
            };

        let (occlusion_texture, occlusion_sampler, occlusion_tex_coord, occlusion_strength) =
            match gltf_material.occlusion_texture() {
                Some(gltf_occlusion_texture) => {
                    let texture = gltf_occlusion_texture.texture();
                    let image = &self.images[texture.source().index()];

                    let format = match image.format {
                        gltf::image::Format::R8 => wgpu::TextureFormat::R8Unorm,
                        gltf::image::Format::R16 => wgpu::TextureFormat::R16Unorm,
                        _ => {
                            return Err(SceneCreationError::Unsupported(
                                "occlusion texture image format".to_string(),
                            ))
                        }
                    };

                    (
                        self.create_texture_view(
                            Some("Material occlusion texture"),
                            &image.pixels,
                            image.width,
                            image.height,
                            format,
                            device,
                            queue,
                        ),
                        self.create_sampler(
                            &texture.sampler(),
                            Some("Material occlusion sampler"),
                            device,
                        ),
                        gltf_occlusion_texture.tex_coord(),
                        gltf_occlusion_texture.strength(),
                    )
                }
                None => (
                    self.create_texture_view(
                        Some("Material default occlusion texture"),
                        &[0],
                        1,
                        1,
                        wgpu::TextureFormat::R8Unorm,
                        device,
                        queue,
                    ),
                    device.create_sampler(&wgpu::SamplerDescriptor {
                        label: Some("Material default occlusion sampler"),
                        ..Default::default()
                    }),
                    0,
                    1.0,
                ),
            };

        let (emissive_texture, emissive_sampler, emissive_tex_coord) =
            match gltf_material.emissive_texture() {
                Some(info) => {
                    let texture = info.texture();
                    let image = &self.images[texture.source().index()];

                    (
                        self.create_texture_view(
                            Some("Material emissive texture"),
                            &image.pixels,
                            image.width,
                            image.height,
                            wgpu::TextureFormat::Rgba8UnormSrgb,
                            device,
                            queue,
                        ),
                        self.create_sampler(
                            &texture.sampler(),
                            Some("Material emissive sampler"),
                            device,
                        ),
                        info.tex_coord(),
                    )
                }
                None => (
                    self.create_texture_view(
                        Some("Material default emissive texture"),
                        &[0, 0, 0, 0],
                        1,
                        1,
                        wgpu::TextureFormat::Rgba8UnormSrgb,
                        device,
                        queue,
                    ),
                    device.create_sampler(&wgpu::SamplerDescriptor {
                        label: Some("Material default emissive sampler"),
                        ..Default::default()
                    }),
                    0,
                ),
            };

        Ok(MaterialDescriptor {
            base_color_factor: gltf_material.pbr_metallic_roughness().base_color_factor(),
            base_color_tex_coord,
            base_color_texture,
            base_color_sampler,
            metallic_factor: gltf_material.pbr_metallic_roughness().metallic_factor(),
            roughness_factor: gltf_material.pbr_metallic_roughness().roughness_factor(),
            metallic_roughness_tex_coord,
            metallic_roughness_texture,
            metallic_roughness_sampler,
            normal_texture_scale,
            normal_tex_coord,
            normal_texture,
            normal_sampler,
            occlusion_strength,
            occlusion_tex_coord,
            occlusion_texture,
            occlusion_sampler,
            emissive_tex_coord,
            emissive_factor: gltf_material.emissive_factor(),
            emissive_texture,
            emissive_sampler,
        })
    }

    fn create_texture_view(
        &self,
        label: Option<&str>,
        pixels: &[u8],
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> wgpu::TextureView {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            pixels,
        );

        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn create_sampler(
        &self,
        gltf_sampler: &gltf::texture::Sampler,
        label: Option<&str>,
        device: &wgpu::Device,
    ) -> wgpu::Sampler {
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
                gltf::texture::MinFilter::Linear | gltf::texture::MinFilter::LinearMipmapLinear,
            )
            | None => (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear),
            Some(gltf::texture::MinFilter::LinearMipmapNearest) => {
                (wgpu::FilterMode::Linear, wgpu::FilterMode::Nearest)
            }
            Some(
                gltf::texture::MinFilter::Nearest | gltf::texture::MinFilter::NearestMipmapLinear,
            ) => (wgpu::FilterMode::Nearest, wgpu::FilterMode::Linear),
            Some(gltf::texture::MinFilter::NearestMipmapNearest) => {
                (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest)
            }
        };
        device.create_sampler(&wgpu::SamplerDescriptor {
            label,
            address_mode_u,
            address_mode_v,
            mag_filter,
            min_filter,
            mipmap_filter,
            ..Default::default()
        })
    }

    fn create_mesh(
        &self,
        gltf_mesh: &gltf::Mesh,
        scene: &mut Scene,
        material_mapping: &mut HashMap<Option<usize>, MaterialId>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<MeshDescriptor, SceneCreationError> {
        let mut primitives = Vec::with_capacity(gltf_mesh.primitives().len());
        for primitive in gltf_mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&self.buffers.get(buffer.index())?));

            dbg!("Creating primitives");

            if let Some(positions) = reader.read_positions() {
                let mut vertex_count = positions.len() as u32;

                let indices = match reader.read_indices() {
                    Some(indices) => match indices {
                        gltf::mesh::util::ReadIndices::U8(_) => {
                            return Err(SceneCreationError::Unsupported(
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

                let normals = reader
                    .read_normals()
                    .ok_or(SceneCreationError::Unsupported(
                        "Attributes NORMALS is required".to_string(),
                    ))?;

                let attributes: Vec<_> = positions
                    .zip(normals)
                    .map(|(position, normal)| PrimitiveAttributes {
                        position,
                        normal,
                        tex_coords: [[0.0, 0.0], [0.0, 0.0]],
                    })
                    .collect();

                let attributes = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Attributes buffer"),
                    contents: bytemuck::cast_slice(&attributes),
                    usage: wgpu::BufferUsages::VERTEX,
                });

                dbg!("attributes created");

                let material = primitive.material();
                let material = match material_mapping.entry(material.index()) {
                    std::collections::hash_map::Entry::Occupied(entry) => *entry.get(),
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let material = scene.create_material(
                            self.create_material(&primitive.material(), device, queue)?,
                            device,
                        );
                        *entry.insert(material)
                    }
                };

                primitives.push(PrimitiveDescriptor {
                    vertex_count,
                    indices,
                    attributes,
                    material,
                });
            }
        }

        Ok(MeshDescriptor { primitives })
    }

    fn create_node(
        &self,
        gltf_node: &gltf::Node,
        scene: &mut Scene,
        meshes_mapping: &mut HashMap<usize, MeshId>,
        material_mapping: &mut HashMap<Option<usize>, MaterialId>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<NodeDescriptor, SceneCreationError> {
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
            Some(mesh) => Some(match meshes_mapping.entry(mesh.index()) {
                std::collections::hash_map::Entry::Occupied(entry) => *entry.get(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let mesh = self.create_mesh(&mesh, scene, material_mapping, device, queue)?;
                    *entry.insert(scene.create_mesh(mesh, device)?)
                }
            }),
            None => None,
        };

        dbg!("Mesh created");

        let mut children = Vec::with_capacity(gltf_node.children().len());
        for child in gltf_node.children() {
            children.push(self.create_node(
                &child,
                scene,
                meshes_mapping,
                material_mapping,
                device,
                queue,
            )?);
        }

        Ok(NodeDescriptor {
            local_transform,
            children,
            mesh,
        })
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SceneCreationError {
    #[error(transparent)]
    NodeCreationError(#[from] NodeCreationError),
    #[error(transparent)]
    MeshCreationError(#[from] MeshCreationError),
    #[error("Invalid asset")]
    InvalidAsset,
    #[error("unsupported: {0}")]
    Unsupported(String),
}
