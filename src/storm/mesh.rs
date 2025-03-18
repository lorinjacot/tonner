use std::{
    iter::{once, repeat_n},
    ops::Deref,
};

use crate::storm::buffer::Buffer;

use super::{
    buffer::BufferManager,
    material::{Material, MaterialManager},
    storage::{Id, SparseMap, SparseSet},
    texture::TextureManager,
    Asset,
};

const ATTRIBUTES_STRIDE: u64 = 72;
const WORKGROUP_SIZE: u32 = 64;

const POSITION_LOCATION: u32 = 7;
const NORMAL_LOCATION: u32 = 8;
const TANGENT_LOCATION: u32 = 9;
const TEX_COORD_0_LOCATION: u32 = 10;
const TEX_COORD_1_LOCATION: u32 = 11;
const COLOR_0_LOCATION: u32 = 12;

pub struct MeshManager {
    meshes: SparseSet<Mesh>,
    assets: SparseMap<Asset, Vec<Option<Id<Mesh>>>>,
    shader_module: wgpu::ShaderModule,
    pipeline_layout: wgpu::PipelineLayout,
    pipelines: SparseSet<PrimitivePipeline>,
}

impl MeshManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let meshes = SparseSet::new();
        let assets = SparseMap::new();

        let shader_module = device.create_shader_module(wgpu::include_wgsl!("primitive.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Primitive pipeline layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        let pipelines = SparseSet::new();

        MeshManager {
            meshes,
            assets,
            shader_module,
            pipeline_layout,
            pipelines,
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

        let mut primitives = Vec::with_capacity(mesh.primitives().len());

        for primitive in mesh.primitives() {
            if let Some(positions) = primitive.get(&Positions) {
                let attributes_count = positions.count() as u64;
                let mut attributes_buffers: SparseMap<Buffer, Vec<wgpu::VertexAttribute>> =
                    SparseMap::new();

                // POSITION
                let id = buffers.load_buffer_view(
                    asset,
                    positions.view().expect("sparse accessor not supported"),
                    wgpu::BufferUsages::VERTEX,
                    device,
                );
                let attribute = wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: positions.offset() as u64,
                    shader_location: POSITION_LOCATION,
                };
                attributes_buffers.entry(id).or_default().push(attribute);

                // NORMAL
                let id = buffers.load_buffer_view(
                    asset,
                    primitive
                        .get(&Normals)
                        .expect("primive NORMAL required")
                        .view()
                        .expect("sparse accessor not supported"),
                    wgpu::BufferUsages::VERTEX,
                    device,
                );
                let attribute = wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: positions.offset() as u64,
                    shader_location: NORMAL_LOCATION,
                };
                attributes_buffers.entry(id).or_default().push(attribute);

                // TANGENT
                let id = buffers.load_buffer_view(
                    asset,
                    primitive
                        .get(&Normals)
                        .expect("primive TANGENT required")
                        .view()
                        .expect("sparse accessor not supported"),
                    wgpu::BufferUsages::VERTEX,
                    device,
                );
                let attribute = wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: positions.offset() as u64,
                    shader_location: TANGENT_LOCATION,
                };
                attributes_buffers.entry(id).or_default().push(attribute);

                // TEXCOORD_0
                let id = buffers.load_buffer_view(
                    asset,
                    primitive
                        .get(&Normals)
                        .expect("primive TEXCOORD_0 required")
                        .view()
                        .expect("sparse accessor not supported"),
                    wgpu::BufferUsages::VERTEX,
                    device,
                );
                let attribute = wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: positions.offset() as u64,
                    shader_location: TEX_COORD_0_LOCATION,
                };
                attributes_buffers.entry(id).or_default().push(attribute);

                // TEXCOORD_1
                let id = buffers.load_buffer_view(
                    asset,
                    primitive
                        .get(&Normals)
                        .expect("primive TEXCOORD_1 required")
                        .view()
                        .expect("sparse accessor not supported"),
                    wgpu::BufferUsages::VERTEX,
                    device,
                );
                let attribute = wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: positions.offset() as u64,
                    shader_location: TEX_COORD_1_LOCATION,
                };
                attributes_buffers.entry(id).or_default().push(attribute);

                // COLOR_0
                let id = buffers.load_buffer_view(
                    asset,
                    primitive
                        .get(&Normals)
                        .expect("primive COLOR_0 required")
                        .view()
                        .expect("sparse accessor not supported"),
                    wgpu::BufferUsages::VERTEX,
                    device,
                );
                let attribute = wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: positions.offset() as u64,
                    shader_location: COLOR_0_LOCATION,
                };
                attributes_buffers.entry(id).or_default().push(attribute);

                let (vertex_buffers, vertex_layouts): (Vec<_>, Vec<_>) = attributes_buffers
                    .into_iter()
                    .map(|(id, attributes)| {
                        let buffer = &buffers[id];
                        let layout = VertexBufferLayout {
                            array_stride: buffer.stride().unwrap_or(attributes[0].format.size()),
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes,
                        };
                        (buffer.deref().clone(), layout)
                    })
                    .unzip();

                let pipeline = self
                    .pipelines
                    .iter()
                    .find(|(_, pipeline)| vertex_layouts == pipeline.vertex_layouts);
                let pipeline = match pipeline {
                    Some((id, _)) => id,
                    None => {
                        let buffers: Vec<_> = vertex_layouts
                            .iter()
                            .map(|layout| wgpu::VertexBufferLayout {
                                array_stride: layout.array_stride,
                                step_mode: layout.step_mode,
                                attributes: &layout.attributes,
                            })
                            .collect();
                        let pipeline =
                            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                                label: Some("Primitive render pipeline"),
                                layout: Some(&self.pipeline_layout),
                                vertex: wgpu::VertexState {
                                    module: &self.shader_module,
                                    entry_point: Some("vs_main"),
                                    compilation_options: wgpu::PipelineCompilationOptions::default(
                                    ),
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
                                    compilation_options: wgpu::PipelineCompilationOptions::default(
                                    ),
                                    targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
                                }),
                                multiview: None,
                                cache: None,
                            });
                        self.pipelines.push(PrimitivePipeline {
                            pipeline,
                            vertex_layouts,
                        })
                    }
                };

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
                    vertex_buffers,
                    pipeline,
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
    vertex_buffers: Vec<wgpu::Buffer>,
    vertex_count: u64,
    pipeline: Id<PrimitivePipeline>,
    material: Id<Material>,
}

struct PrimitivePipeline {
    pipeline: wgpu::RenderPipeline,
    vertex_layouts: Vec<VertexBufferLayout>,
}

#[derive(PartialEq)]
struct VertexBufferLayout {
    array_stride: wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode,
    attributes: Vec<wgpu::VertexAttribute>,
}

fn component_type(data_type: gltf::accessor::DataType) -> u32 {
    use gltf::accessor::DataType::*;

    match data_type {
        I8 => 5120,
        U8 => 5121,
        I16 => 5122,
        U16 => 5123,
        U32 => 5125,
        F32 => 5126,
    }
}
