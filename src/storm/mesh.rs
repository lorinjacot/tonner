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
    pipelines: SparseSet<PrimitivePipeline>,
}

impl MeshManager {
    pub fn new(device: &wgpu::Device, materials: &MaterialManager) -> Self {
        let meshes = SparseSet::new();
        let assets = SparseMap::new();

        let shader_module = device.create_shader_module(wgpu::include_wgsl!("primitive.wgsl"));

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let pipelines = SparseSet::new();

        MeshManager {
            meshes,
            assets,
            shader_module,
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
                let usage = wgpu::BufferUsages::VERTEX;
                let mut attributes_buffers: SparseMap<Buffer, Vec<wgpu::VertexAttribute>> =
                    SparseMap::new();

                {
                    let id = buffers.load_accessor(asset, positions, usage, device);
                    let position = &buffers[id];
                    attributes_buffers
                        .entry(position.buffer())
                        .or_default()
                        .push(position.vertex_attribute_layout(POSITION_LOCATION));
                }
                primitive.get(&Normals).map(|accessor| {
                    let id = buffers.load_accessor(asset, accessor, usage, device);
                    let normal = &buffers[id];
                    attributes_buffers
                        .entry(normal.buffer())
                        .or_default()
                        .push(normal.vertex_attribute_layout(NORMAL_LOCATION));
                });
                primitive.get(&Tangents).map(|accessor| {
                    let id = buffers.load_accessor(asset, accessor, usage, device);
                    let tangent = &buffers[id];
                    attributes_buffers
                        .entry(tangent.buffer())
                        .or_default()
                        .push(tangent.vertex_attribute_layout(TANGENT_LOCATION));
                });
                primitive.get(&TexCoords(0)).map(|accessor| {
                    let id = buffers.load_accessor(asset, accessor, usage, device);
                    let tex_coord = &buffers[id];
                    attributes_buffers
                        .entry(tex_coord.buffer())
                        .or_default()
                        .push(tex_coord.vertex_attribute_layout(TEX_COORD_0_LOCATION));
                });
                primitive.get(&TexCoords(1)).map(|accessor| {
                    let id = buffers.load_accessor(asset, accessor, usage, device);
                    let tex_coord = &buffers[id];
                    attributes_buffers
                        .entry(tex_coord.buffer())
                        .or_default()
                        .push(tex_coord.vertex_attribute_layout(TEX_COORD_1_LOCATION));
                });
                primitive.get(&Colors(0)).map(|accessor| {
                    let id = buffers.load_accessor(asset, accessor, usage, device);
                    let color = &buffers[id];
                    attributes_buffers
                        .entry(color.buffer())
                        .or_default()
                        .push(color.vertex_attribute_layout(COLOR_0_LOCATION));
                });

                let (vertex_buffers, vertex_layouts): (Vec<_>, Vec<_>) = attributes_buffers
                    .into_iter()
                    .map(|(id, attributes)| {
                        let buffer = &buffers[id];
                        let layout = VertexBufferLayout {
                            array_stride: buffer.stride(),
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
                                layout: None,
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
                            let indices_count = indices.count() as u64;
                            let id = buffers.load_accessor(
                                asset,
                                indices,
                                wgpu::BufferUsages::INDEX,
                                device,
                            );
                            let accessor = &buffers[id];
                            let indices =
                                Some((buffers[accessor.buffer()].clone(), accessor.index_format()));
                            (indices, indices_count)
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
