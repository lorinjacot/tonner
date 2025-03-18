use std::iter::{once, repeat_n};

use bytemuck::{cast_slice, Pod, Zeroable};
use wgpu::util::DeviceExt;

use super::{
    buffer::BufferManager,
    material::{Material, MaterialManager},
    storage::{Id, SparseMap, SparseSet},
    texture::TextureManager,
    Asset,
};

const ATTRIBUTES_STRIDE: u64 = 72;
const WORKGROUP_SIZE: u32 = 64;

pub struct MeshManager {
    meshes: SparseSet<Mesh>,
    assets: SparseMap<Asset, Vec<Option<Id<Mesh>>>>,
    write_attributes_bind_group_layouts: [wgpu::BindGroupLayout; 2],
    write_attributes_pipeline: wgpu::ComputePipeline,
}

impl MeshManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let meshes = SparseSet::new();
        let assets = SparseMap::new();

        let shader = device.create_shader_module(wgpu::include_wgsl!("mesh.wgsl"));

        let write_attributes_bind_group_layouts = [
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Write attributes bind group 0 layout"),
                entries: &[
                    // positions
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // normals
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // tangents
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // tex_coords_0
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            }),
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Write attributes bind group 1 layout"),
                entries: &[
                    // tex_coords_1
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // colors_0
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // attributes
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // accessors
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            }),
        ];

        let write_attributes_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Write attributes pipeline layout"),
                bind_group_layouts: &[
                    &write_attributes_bind_group_layouts[0],
                    &write_attributes_bind_group_layouts[1],
                ],
                push_constant_ranges: &[],
            });

        let write_attributes_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Write attributes pipeline"),
                layout: Some(&write_attributes_pipeline_layout),
                module: &shader,
                entry_point: Some("writeAttributes"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });

        MeshManager {
            meshes,
            assets,
            write_attributes_bind_group_layouts,
            write_attributes_pipeline,
        }
    }

    pub fn load_mesh(
        &mut self,
        asset: Id<Asset>,
        mesh: gltf::Mesh,
        buffers: &mut BufferManager,
        textures: &mut TextureManager,
        materials: &mut MaterialManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Id<Mesh> {
        match self.assets[asset].get(mesh.index()) {
            Some(Some(id)) => *id,
            _ => self.create_mesh(
                asset, mesh, buffers, textures, materials, device, queue, encoder,
            ),
        }
    }

    fn create_mesh(
        &mut self,
        asset: Id<Asset>,
        mesh: gltf::Mesh,
        buffers: &mut BufferManager,
        textures: &mut TextureManager,
        materials: &mut MaterialManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Id<Mesh> {
        use gltf::mesh::Semantic::*;

        let mut primitives = Vec::with_capacity(mesh.primitives().len());

        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Write attributes compute pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&self.write_attributes_pipeline);

        for primitive in mesh.primitives() {
            if let Some(positions) = primitive.get(&Positions) {
                let mut accessors = Vec::with_capacity(6);
                let attributes_count = positions.count() as u64;

                accessors.push(Accessor {
                    offset: positions.offset() as u32,
                    component_type: component_type(positions.data_type()),
                    component_number: positions.dimensions().multiplicity() as u32,
                    stride: positions
                        .view()
                        .unwrap()
                        .stride()
                        .expect("no stride for POSITION") as u32,
                });
                let positions = buffers.load_buffer_view(
                    asset,
                    positions.view().expect("sparse accessor not supported"),
                    wgpu::BufferUsages::STORAGE,
                    device,
                );

                let normals = primitive.get(&Normals).expect("primitive normals required");
                accessors.push(Accessor {
                    offset: normals.offset() as u32,
                    component_type: component_type(normals.data_type()),
                    component_number: normals.dimensions().multiplicity() as u32,
                    stride: normals
                        .view()
                        .unwrap()
                        .stride()
                        .expect("no stride for NORMAL") as u32,
                });
                let normals = buffers.load_buffer_view(
                    asset,
                    normals.view().expect("sparse accessor not supported"),
                    wgpu::BufferUsages::STORAGE,
                    device,
                );

                let tangents = primitive
                    .get(&Tangents)
                    .expect("primitive tangents required");
                accessors.push(Accessor {
                    offset: tangents.offset() as u32,
                    component_type: component_type(tangents.data_type()),
                    component_number: tangents.dimensions().multiplicity() as u32,
                    stride: tangents
                        .view()
                        .unwrap()
                        .stride()
                        .expect("no stride for NORMAL") as u32,
                });
                let tangents = buffers.load_buffer_view(
                    asset,
                    tangents.view().expect("sparse accessor not supported"),
                    wgpu::BufferUsages::STORAGE,
                    device,
                );

                let tex_coords_0 = primitive
                    .get(&TexCoords(0))
                    .expect("primitive TEX_COORD_0 required");
                accessors.push(Accessor {
                    offset: tex_coords_0.offset() as u32,
                    component_type: component_type(tex_coords_0.data_type()),
                    component_number: tex_coords_0.dimensions().multiplicity() as u32,
                    stride: tex_coords_0
                        .view()
                        .unwrap()
                        .stride()
                        .expect("no stride for NORMAL") as u32,
                });
                let tex_coords_0 = buffers.load_buffer_view(
                    asset,
                    tex_coords_0.view().expect("sparse accessor not supported"),
                    wgpu::BufferUsages::STORAGE,
                    device,
                );

                let tex_coords_1 = primitive
                    .get(&TexCoords(1))
                    .expect("primitive TEX_COORD_1 required");
                accessors.push(Accessor {
                    offset: tex_coords_1.offset() as u32,
                    component_type: component_type(tex_coords_1.data_type()),
                    component_number: tex_coords_1.dimensions().multiplicity() as u32,
                    stride: tex_coords_1
                        .view()
                        .unwrap()
                        .stride()
                        .expect("no stride for NORMAL") as u32,
                });
                let tex_coords_1 = buffers.load_buffer_view(
                    asset,
                    tex_coords_1.view().expect("sparse accessor not supported"),
                    wgpu::BufferUsages::STORAGE,
                    device,
                );

                let colors_0 = primitive
                    .get(&Colors(0))
                    .expect("primitive COLOR_0 required");
                accessors.push(Accessor {
                    offset: colors_0.offset() as u32,
                    component_type: component_type(colors_0.data_type()),
                    component_number: colors_0.dimensions().multiplicity() as u32,
                    stride: colors_0
                        .view()
                        .unwrap()
                        .stride()
                        .expect("no stride for NORMAL") as u32,
                });
                let colors_0 = buffers.load_buffer_view(
                    asset,
                    colors_0.view().expect("sparse accessor not supported"),
                    wgpu::BufferUsages::STORAGE,
                    device,
                );

                let attributes = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("{} primitive buffer", mesh.name().unwrap_or(""))),
                    size: attributes_count as u64 * ATTRIBUTES_STRIDE,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
                    mapped_at_creation: false,
                });

                let accessors = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!(
                        "{} primitive accessors buffer",
                        mesh.name().unwrap_or("")
                    )),
                    contents: cast_slice(&accessors),
                    usage: wgpu::BufferUsages::STORAGE,
                });

                let bind_groups = [
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some(&format!(
                            "{} write attribute bind group 0",
                            mesh.name().unwrap_or("")
                        )),
                        layout: &self.write_attributes_bind_group_layouts[0],
                        entries: &[
                            // positions
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: buffers[positions].as_entire_binding(),
                            },
                            // normals
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: buffers[normals].as_entire_binding(),
                            },
                            // tangents
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: buffers[tangents].as_entire_binding(),
                            },
                            // tex_coords_0
                            wgpu::BindGroupEntry {
                                binding: 3,
                                resource: buffers[tex_coords_0].as_entire_binding(),
                            },
                        ],
                    }),
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some(&format!(
                            "{} write attribute bind group 1",
                            mesh.name().unwrap_or("")
                        )),
                        layout: &self.write_attributes_bind_group_layouts[1],
                        entries: &[
                            // tex_coords_1
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: buffers[tex_coords_1].as_entire_binding(),
                            },
                            // colors_0
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: buffers[colors_0].as_entire_binding(),
                            },
                            // attributes
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: attributes.as_entire_binding(),
                            },
                            // accessors
                            wgpu::BindGroupEntry {
                                binding: 3,
                                resource: accessors.as_entire_binding(),
                            },
                        ],
                    }),
                ];

                cpass.set_bind_group(0, &bind_groups[0], &[]);
                cpass.set_bind_group(1, &bind_groups[1], &[]);
                cpass.dispatch_workgroups(attributes_count as u32 / WORKGROUP_SIZE, 1, 1);

                let (indices, vertex_count) =
                    primitive
                        .indices()
                        .map_or((None, attributes_count), |indices| {
                            let id = buffers.load_buffer_view(
                                asset,
                                indices.view().expect("Sparse accessor not supported"),
                                wgpu::BufferUsages::INDEX,
                                device,
                            );
                            let vertex_count = indices.count() as u64;
                            let indices = Some((
                                buffers[id].clone(),
                                match indices.data_type() {
                                    gltf::accessor::DataType::U16 => wgpu::IndexFormat::Uint16,
                                    gltf::accessor::DataType::U32 => wgpu::IndexFormat::Uint32,
                                    _ => unimplemented!("unsupported index format"),
                                },
                            ));
                            (indices, vertex_count)
                        });

                let material =
                    materials.load_material(asset, primitive.material(), textures, device, queue);

                primitives.push(Primitive {
                    indices,
                    attributes,
                    vertex_count,
                    material,
                });
            }
        }

        let id = self.meshes.push(Mesh { primitives });

        let mapping = &mut self.assets[asset];
        match mapping.get_mut(mesh.index()) {
            Some(entry) => *entry = Some(id),
            None => {
                let iter = repeat_n(None, mesh.index() - mapping.len()).chain(once(Some(id)));
                mapping.extend(iter);
            }
        }

        id
    }
}

pub struct Mesh {
    primitives: Vec<Primitive>,
}

struct Primitive {
    indices: Option<(wgpu::Buffer, wgpu::IndexFormat)>,
    attributes: wgpu::Buffer,
    vertex_count: u64,
    material: Id<Material>,
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Accessor {
    offset: u32,
    component_type: u32,
    component_number: u32,
    stride: u32,
}

fn component_type(data_type: gltf::accessor::DataType) -> u32 {
    use gltf::accessor::DataType::*;

    match data_type {
        I8 => 5120,
        u8 => 5121,
        I16 => 5122,
        U16 => 5123,
        U32 => 5125,
        F32 => 5126,
    }
}
