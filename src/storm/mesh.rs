use std::{
    collections::HashMap,
    iter::{once, repeat_n},
    ops::Deref,
};

use crate::storm::buffer::Buffer;

use super::{
    buffer::BufferManager,
    material::{Material, MaterialFlags, MaterialManager},
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
    dummy_vertex_buffer: Id<Buffer>,
}

impl MeshManager {
    pub fn new(
        materials: &MaterialManager,
        buffers: &mut BufferManager,
        device: &wgpu::Device,
    ) -> Self {
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
                primitive.get(&Normals).map_or_else(
                    || todo!("generate normals"),
                    |accessor| {
                        let id = buffers.load_accessor(asset, accessor, usage, device);
                        let normal = &buffers[id];
                        attributes_buffers
                            .entry(normal.buffer())
                            .or_default()
                            .push(normal.vertex_attribute_layout(NORMAL_LOCATION));
                    },
                );

                let mut init_attribute = |semantic, shader_location: u32, default_format| {
                    let (has, buffer, layout) = primitive.get(&semantic).map_or_else(
                        || {
                            (
                                0.0,
                                self.dummy_vertex_buffer,
                                wgpu::VertexAttribute {
                                    format: default_format,
                                    offset: 0,
                                    shader_location,
                                },
                            )
                        },
                        |accessor| {
                            let id = buffers.load_accessor(asset, accessor, usage, device);
                            let tangent = &buffers[id];
                            (
                                1.0,
                                tangent.buffer(),
                                tangent.vertex_attribute_layout(shader_location),
                            )
                        },
                    );
                    attributes_buffers.entry(buffer).or_default().push(layout);
                    has
                };

                let has_tangent =
                    init_attribute(Tangents, TANGENT_LOCATION, wgpu::VertexFormat::Float32x4);

                let has_tex_coord_0 = init_attribute(
                    TexCoords(0),
                    TEX_COORD_0_LOCATION,
                    wgpu::VertexFormat::Float32x2,
                );
                let has_tex_coord_1 = init_attribute(
                    TexCoords(1),
                    TEX_COORD_1_LOCATION,
                    wgpu::VertexFormat::Float32x2,
                );
                let has_color_0 =
                    init_attribute(Colors(0), COLOR_0_LOCATION, wgpu::VertexFormat::Float32x4);

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

                let material_id =
                    materials.load_material(asset, primitive.material(), textures, device, queue);
                let material = &materials[material_id];
                let material_flags = material.flags();

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

                    let mut constants = HashMap::with_capacity(5);
                    constants.insert("has_tangent".to_string(), has_tangent);
                    constants.insert("has_tex_coord_0".to_string(), has_tex_coord_0);
                    constants.insert("has_tex_coord_1".to_string(), has_tex_coord_1);
                    constants.insert("has_color_0".to_string(), has_color_0);
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
                            targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
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

                primitives.push(Primitive {
                    indices,
                    vertex_buffers,
                    pipeline: self.pipelines[pipeline].pipeline.clone(),
                    vertex_count,
                    material: material.bind_group().clone(),
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
    pipeline: wgpu::RenderPipeline,
    material: wgpu::BindGroup,
}

struct PrimitivePipeline {
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
