use std::{
    cmp::Ordering::*,
    collections::HashMap,
    iter::{once, repeat_n, zip},
    ops::Index,
};

use bitflags::bitflags;
use bytemuck::{Pod, Zeroable, cast_slice};
use glam::Vec3;
use wgpu::util::DeviceExt;

use crate::buffer::Buffer;

use super::{
    Asset,
    buffer::{Accessor, BufferManager},
    material::{Material, MaterialFlags, MaterialManager},
    storage::{Id, SparseMap, SparseSet},
    texture::TextureManager,
};

const POSITION_LOCATION: u32 = 7;
const NORMAL_LOCATION: u32 = 8;
const TANGENT_LOCATION: u32 = 9;
const TEX_COORD_0_LOCATION: u32 = 10;
const TEX_COORD_1_LOCATION: u32 = 11;
const COLOR_0_LOCATION: u32 = 12;

const TRANSFORM_ATTRIBUTES: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
    0 => Float32x4,
    1 => Float32x4,
    2 => Float32x4,
    3 => Float32x4,
    4 => Float32x3,
    5 => Float32x3,
    6 => Float32x3,
];

pub struct MeshManager {
    meshes: SparseSet<Mesh>,
    assets: SparseMap<Asset, Vec<Option<Id<Mesh>>>>,
    shader_module: wgpu::ShaderModule,
    pipelines: SparseSet<PrimitivePipeline>,
    pipeline_layout: wgpu::PipelineLayout,
    render_format: wgpu::TextureFormat,
    dummy_vertex_buffer: Id<Buffer>,
}

impl MeshManager {
    pub fn new(
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        materials: &MaterialManager,
        buffers: &mut BufferManager,
        render_format: wgpu::TextureFormat,
        device: &wgpu::Device,
    ) -> Self {
        let meshes = SparseSet::new();
        let assets = SparseMap::new();

        let shader_module = device.create_shader_module(wgpu::include_wgsl!("primitive.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Primitive pipeline layout"),
            bind_group_layouts: &[&camera_bind_group_layout, materials.bind_group_layout()],
            push_constant_ranges: &[],
        });

        let pipelines = SparseSet::new();

        let dummy_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy vertex buffer"),
            size: 16,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        let dummy_vertex_buffer = buffers.create_buffer(dummy_vertex_buffer, 0);

        MeshManager {
            meshes,
            assets,
            shader_module,
            pipelines,
            pipeline_layout,
            render_format,
            dummy_vertex_buffer,
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
    ) -> Id<Mesh> {
        match self.assets.entry(asset).or_default().get(mesh.index()) {
            Some(Some(id)) => *id,
            _ => self.create_mesh(asset, mesh, buffers, textures, materials, device, queue),
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
    ) -> Id<Mesh> {
        use gltf::mesh::Semantic::*;

        let mut primitives: SparseMap<PrimitivePipeline, Vec<Primitive>> = SparseMap::new();

        for primitive in mesh.primitives() {
            if let Some(positions) = primitive.get(&Positions) {
                let attributes_count = positions.count() as u32;
                let usage = wgpu::BufferUsages::VERTEX;
                let mut attributes_buffers: SparseMap<Buffer, Vec<wgpu::VertexAttribute>> =
                    SparseMap::new();

                let (vertex_buffers, vertex_layouts, primitive_flats, indices, vertex_count): (
                    Vec<_>,
                    Vec<_>,
                    PrimitiveFlags,
                    Option<(Id<Accessor>, wgpu::IndexFormat)>,
                    u32,
                ) = match primitive.get(&Normals) {
                    Some(normals) => {
                        {
                            let id = buffers.load_accessor(asset, positions, usage, device);
                            let position = &buffers[id];
                            attributes_buffers
                                .entry(position.buffer())
                                .or_default()
                                .push(position.vertex_attribute_layout(POSITION_LOCATION));
                        }

                        {
                            let id = buffers.load_accessor(asset, normals, usage, device);
                            let normal = &buffers[id];
                            attributes_buffers
                                .entry(normal.buffer())
                                .or_default()
                                .push(normal.vertex_attribute_layout(NORMAL_LOCATION));
                        }

                        let mut flags = PrimitiveFlags::empty();
                        let mut init_attribute =
                            |semantic: gltf::Semantic,
                             flag: PrimitiveFlags,
                             shader_location: u32,
                             default_format: wgpu::VertexFormat| {
                                let (buffer, layout) = primitive.get(&semantic).map_or_else(
                                    || {
                                        (
                                            self.dummy_vertex_buffer,
                                            wgpu::VertexAttribute {
                                                format: default_format,
                                                offset: 0,
                                                shader_location,
                                            },
                                        )
                                    },
                                    |accessor| {
                                        let id =
                                            buffers.load_accessor(asset, accessor, usage, device);
                                        let tangent = &buffers[id];
                                        flags.insert(flag);
                                        (
                                            tangent.buffer(),
                                            tangent.vertex_attribute_layout(shader_location),
                                        )
                                    },
                                );
                                attributes_buffers.entry(buffer).or_default().push(layout)
                            };

                        init_attribute(
                            Tangents,
                            PrimitiveFlags::TANGENT,
                            TANGENT_LOCATION,
                            wgpu::VertexFormat::Float32x4,
                        );
                        init_attribute(
                            TexCoords(0),
                            PrimitiveFlags::TEX_COORD_0,
                            TEX_COORD_0_LOCATION,
                            wgpu::VertexFormat::Float32x2,
                        );
                        init_attribute(
                            TexCoords(1),
                            PrimitiveFlags::TEX_COORD_1,
                            TEX_COORD_1_LOCATION,
                            wgpu::VertexFormat::Float32x2,
                        );
                        init_attribute(
                            Colors(0),
                            PrimitiveFlags::COLOR_0,
                            COLOR_0_LOCATION,
                            wgpu::VertexFormat::Float32x4,
                        );

                        let (vertex_buffers, vertex_layouts) = attributes_buffers
                            .into_iter()
                            .map(|(id, attributes)| {
                                let buffer = &buffers[id];
                                let layout = VertexBufferLayout {
                                    array_stride: buffer.stride(),
                                    step_mode: wgpu::VertexStepMode::Vertex,
                                    attributes,
                                };
                                (id, layout)
                            })
                            .unzip();

                        let (indices, vertex_count) =
                            primitive
                                .indices()
                                .map_or((None, attributes_count), |indices| {
                                    let indices_count = indices.count() as u32;
                                    let id = buffers.load_accessor(
                                        asset,
                                        indices,
                                        wgpu::BufferUsages::INDEX,
                                        device,
                                    );
                                    let accessor = &buffers[id];
                                    let indices = Some((id, accessor.index_format()));
                                    (indices, indices_count)
                                });

                        (vertex_buffers, vertex_layouts, flags, indices, vertex_count)
                    }
                    None => {
                        let reader = primitive.reader(|buffer| {
                            Some(&buffers.buffer_data(asset)?.get(buffer.index())?.0)
                        });

                        let positions: Vec<_> = match reader.read_indices() {
                            Some(indices) => {
                                let positions: Vec<_> = reader.read_positions().unwrap().collect();
                                indices
                                    .into_u32()
                                    .map(|index| positions[index as usize])
                                    .collect()
                            }
                            None => reader.read_positions().unwrap().collect(),
                        };

                        let normals = generate_normals(&positions);

                        let flags = PrimitiveFlags::empty();
                        if reader.read_tangents().is_some()
                            || reader.read_tex_coords(0).is_some()
                            || reader.read_colors(0).is_some()
                        {
                            todo!("include other vertex data");
                        }

                        let vertices = zip(positions, normals).map(|(position, normal)| {
                            VertexData([
                                position[0],
                                position[1],
                                position[2],
                                normal[0],
                                normal[1],
                                normal[2],
                            ])
                        });

                        let (indices, vertices) = merge_vertices(vertices);

                        let vertex_buffer =
                            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some(&format!(
                                    "{} vertex buffer",
                                    mesh.name().unwrap_or("")
                                )),
                                contents: cast_slice(&vertices),
                                usage: wgpu::BufferUsages::VERTEX,
                            });

                        use wgpu::VertexFormat::*;

                        let stride = size_of::<VertexData<6>>() as u64;
                        let vertex_buffers = vec![
                            buffers.create_buffer(vertex_buffer, stride),
                            self.dummy_vertex_buffer,
                        ];
                        let vertex_layouts = vec![
                            VertexBufferLayout {
                                array_stride: stride,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: wgpu::vertex_attr_array![POSITION_LOCATION => Float32x3, NORMAL_LOCATION => Float32x3].into(),
                            },
                            VertexBufferLayout {
                                array_stride: 0,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: [
                                    (Float32x3, TANGENT_LOCATION),
                                    (Float32x2, TEX_COORD_0_LOCATION),
                                    (Float32x2, TEX_COORD_1_LOCATION),
                                    (Float32x4, COLOR_0_LOCATION),
                                ].into_iter().map(|(format, shader_location)| {
                                    wgpu::VertexAttribute {
                                        format,
                                        offset: 0,
                                        shader_location,
                                    }
                                }).collect(),
                            }
                        ];

                        let vertex_count = indices.len();
                        const INDEX_STRIDE: u64 = size_of::<u32>() as u64;
                        let indices_buffer = buffers.create_buffer(
                            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some(&format!("{} index buffer", mesh.name().unwrap_or(""))),
                                contents: cast_slice(&indices),
                                usage: wgpu::BufferUsages::INDEX,
                            }),
                            INDEX_STRIDE,
                        );
                        let indices = (
                            buffers.create_accessor(
                                indices_buffer,
                                0,
                                vertex_count as u64 * INDEX_STRIDE,
                                gltf::accessor::DataType::U32,
                                false,
                                gltf::accessor::Dimensions::Scalar,
                            ),
                            wgpu::IndexFormat::Uint32,
                        );

                        (
                            vertex_buffers,
                            vertex_layouts,
                            flags,
                            Some(indices),
                            vertex_count as u32,
                        )
                    }
                };

                let material =
                    materials.load_material(asset, primitive.material(), textures, device, queue);
                let material_flags = materials[material].flags();

                let pipeline = self.pipelines.iter().find_map(|(id, pipeline)| {
                    if vertex_layouts == pipeline.vertex_layouts
                        && material_flags == pipeline.material_flags
                    {
                        Some(id)
                    } else {
                        None
                    }
                });
                let pipeline = pipeline.unwrap_or_else(|| {
                    let mut buffers = vec![wgpu::VertexBufferLayout {
                        array_stride: 100,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &TRANSFORM_ATTRIBUTES,
                    }];
                    buffers.extend(
                        vertex_layouts
                            .iter()
                            .map(|layout| wgpu::VertexBufferLayout {
                                array_stride: layout.array_stride,
                                step_mode: layout.step_mode,
                                attributes: &layout.attributes,
                            }),
                    );

                    let mut constants = HashMap::with_capacity(9);
                    primitive_flats.insert_constants(&mut constants);
                    material_flags.insert_constants(&mut constants);

                    let compilation_options = wgpu::PipelineCompilationOptions {
                        constants: &constants,
                        ..Default::default()
                    };

                    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("Primitive render pipeline"),
                        layout: Some(&self.pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: &self.shader_module,
                            entry_point: Some("vs_main"),
                            compilation_options: compilation_options.clone(),
                            buffers: &buffers,
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
                            module: &self.shader_module,
                            entry_point: Some("fs_main"),
                            compilation_options,
                            targets: &[Some(self.render_format.into())],
                        }),
                        multiview: None,
                        cache: None,
                    });
                    self.pipelines.push(PrimitivePipeline {
                        pipeline,
                        vertex_layouts,
                        material_flags,
                    })
                });

                primitives.entry(pipeline).or_default().push(Primitive {
                    indices,
                    vertex_buffers,
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

fn generate_normals(positions: &[[f32; 3]]) -> Vec<[f32; 3]> {
    positions
        .chunks_exact(3)
        .into_iter()
        .flat_map(|positions| {
            let a = Vec3::from_array(positions[0]);
            let b = Vec3::from_array(positions[1]);
            let c = Vec3::from_array(positions[2]);

            [Vec3::cross(b - a, c - b).to_array(); 3]
        })
        .collect()
}

bitflags! {
    struct PrimitiveFlags: u32 {
        const TANGENT = 0b00000001;
        const TEX_COORD_0 = 0b00000010;
        const TEX_COORD_1 = 0b00000100;
        const COLOR_0 = 0b00001000;
    }
}

impl PrimitiveFlags {
    fn insert_constants(&self, constants: &mut HashMap<String, f64>) {
        constants.insert(
            "has_tangent".to_string(),
            self.contains(Self::TANGENT) as u64 as f64,
        );
        constants.insert(
            "has_tex_coord_0".to_string(),
            self.contains(Self::TEX_COORD_0) as u64 as f64,
        );
        constants.insert(
            "has_tex_coord_1".to_string(),
            self.contains(Self::TEX_COORD_1) as u64 as f64,
        );
        constants.insert(
            "has_color_0".to_string(),
            self.contains(Self::COLOR_0) as u64 as f64,
        );
    }
}

#[derive(Clone, Copy, Zeroable)]
#[repr(C)]
struct VertexData<const D: usize>([f32; D]);

unsafe impl<const D: usize> Pod for VertexData<D> {}

const TOLERANCE: f32 = 1e-8;

impl<const D: usize> PartialEq for VertexData<D> {
    fn eq(&self, other: &Self) -> bool {
        zip(self.0, other.0).all(|(a, b)| (a - b).abs() <= TOLERANCE)
    }
}

impl<const D: usize> Eq for VertexData<D> {}

impl<const D: usize> PartialOrd for VertexData<D> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        for (a, b) in zip(self.0, other.0) {
            if a < b - TOLERANCE {
                return Some(Less);
            }
            if a > b + TOLERANCE {
                return Some(Greater);
            }
        }
        Some(Equal)
    }
}

impl<const D: usize> Ord for VertexData<D> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        for (a, b) in zip(self.0, other.0) {
            if a < b - TOLERANCE {
                return Less;
            }
            if a > b + TOLERANCE {
                return Greater;
            }
        }
        Equal
    }
}

fn merge_vertices<V>(vertices: V) -> (Vec<u32>, Vec<V::Item>)
where
    V: IntoIterator,
    V::Item: Ord,
{
    let mut vertices: Vec<_> = vertices.into_iter().enumerate().collect();
    vertices.sort_by(|a, b| (a.1.cmp(&b.1)));

    let mut unique_vertices = Vec::with_capacity(vertices.len());
    let mut indices = vec![0; vertices.len()];
    for (pos, data) in vertices {
        match unique_vertices.last() {
            Some(last) => match data.cmp(last) {
                Greater => {
                    indices[pos] = unique_vertices.len() as u32;
                    unique_vertices.push(data);
                }
                Equal => indices[pos] = unique_vertices.len() as u32 - 1,
                Less => unreachable!(),
            },
            None => {
                indices[pos] = 0;
                unique_vertices.push(data);
            }
        }
    }
    (indices, unique_vertices)
}

impl Index<Id<Mesh>> for MeshManager {
    type Output = Mesh;

    fn index(&self, index: Id<Mesh>) -> &Self::Output {
        &self.meshes[index]
    }
}

impl Index<Id<PrimitivePipeline>> for MeshManager {
    type Output = wgpu::RenderPipeline;

    fn index(&self, index: Id<PrimitivePipeline>) -> &Self::Output {
        &self.pipelines[index].pipeline
    }
}

pub struct Mesh {
    pub primitives: SparseMap<PrimitivePipeline, Vec<Primitive>>,
}

pub struct Primitive {
    pub indices: Option<(Id<Accessor>, wgpu::IndexFormat)>,
    pub vertex_buffers: Vec<Id<Buffer>>,
    pub vertex_count: u32,
    pub material: Id<Material>,
}

pub struct PrimitivePipeline {
    pipeline: wgpu::RenderPipeline,
    vertex_layouts: Vec<VertexBufferLayout>,
    material_flags: MaterialFlags,
}

#[derive(PartialEq)]
struct VertexBufferLayout {
    array_stride: wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode,
    attributes: Vec<wgpu::VertexAttribute>,
}
