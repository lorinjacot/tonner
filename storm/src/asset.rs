use std::collections::BTreeMap;

use glam::{Mat4, Quat};
use wgpu::util::DeviceExt;

use crate::{
    Id, Resources,
    mesh::{IndexBuffer, Mesh, MeshDescriptor, Primitive},
    scene::{Node, Scene},
    storage::{DenseEntry, SparseSet},
};

pub fn open_gltf<'r>(
    path: impl AsRef<std::path::Path>,
    resources: &'r mut Resources,
) -> Result<(Vec<Scene>, Option<usize>), gltf::Error> {
    let (document, buffers, _images) = gltf::import(path)?;

    let mesh_mapping = resources.load_meshes(&document, &buffers);

    let scenes = document
        .scenes()
        .map(|gltf_scene| {
            let mut scene = Scene::new(
                gltf_scene
                    .name()
                    .map_or_else(|| gltf_scene.index().to_string(), |name| name.to_string()),
                resources,
            );
            for node in gltf_scene.nodes() {
                scene.build_gltf_node(node, None, &mut resources.meshes, &mesh_mapping);
            }
            scene
        })
        .collect();

    let default_scene = document.default_scene().map(|scene| scene.index());
    Ok((scenes, default_scene))
}

impl Resources {
    fn load_meshes(
        &mut self,
        document: &gltf::Document,
        buffers: &Vec<gltf::buffer::Data>,
    ) -> Vec<Id<Mesh>> {
        let mut accessors: Vec<Option<Accessor>> = vec![None; document.accessors().len()];
        let mut views: Vec<Option<View>> = vec![None; document.views().len()];

        document
            .meshes()
            .map(|gltf_mesh| {
                let name = format!(
                    "mesh({}) {}",
                    gltf_mesh.index(),
                    gltf_mesh.name().unwrap_or("")
                );
                let mut primitives = Vec::with_capacity(gltf_mesh.primitives().len());
                for primitive in gltf_mesh.primitives() {
                    let mut vertex_count = match primitive.get(&gltf::Semantic::Positions) {
                        Some(positions) => positions.count() as u32,
                        None => continue,
                    };

                    if primitive.get(&gltf::Semantic::Normals).is_none() {
                        todo!("generate normals");
                    }

                    let index_buffer = primitive.indices().map(|indices| {
                        vertex_count = indices.count() as u32;
                        match &accessors[indices.index()] {
                            Some(Accessor::IndexBuffer(index_buffer)) => index_buffer.clone(),
                            None => {
                                let index_buffer = IndexBuffer::from_gltf(
                                    &indices,
                                    &buffers,
                                    &mut views,
                                    &self.device,
                                );
                                accessors[indices.index()] =
                                    Some(Accessor::IndexBuffer(index_buffer.clone()));
                                index_buffer
                            }
                            _ => {
                                panic!(
                                    "primitive indices accessors cannot be used for other purposes"
                                )
                            }
                        }
                    });

                    let mut vertex_buffers: BTreeMap<wgpu::Buffer, VertexBufferLayout> =
                        BTreeMap::new();
                    for (semantic, accessor) in primitive.attributes() {
                        let attribute = match &accessors[accessor.index()] {
                            Some(Accessor::Attribute(attribute)) => attribute.clone(),
                            None => {
                                let attribute =
                                    Attribute::from(&accessor, &buffers, &mut views, &self.device);
                                accessors[accessor.index()] =
                                    Some(Accessor::Attribute(attribute.clone()));
                                attribute
                            }
                            _ => panic!("attributes accessors cannot be used for other purposes"),
                        };
                        let shader_location = match semantic {
                            gltf::Semantic::Positions => 1,
                            gltf::Semantic::Normals => 2,
                            // gltf::Semantic::Tangents => 3,
                            // gltf::Semantic::TexCoords(0) => 4,
                            // gltf::Semantic::TexCoords(1) => 5,
                            // gltf::Semantic::Colors(0) => 6,
                            _ => panic!("unsupported primitive attribute {semantic:?}"),
                        };
                        vertex_buffers
                            .entry(attribute.buffer)
                            .or_insert_with(|| VertexBufferLayout {
                                array_stride: attribute.array_stride,
                                attributes: Vec::with_capacity(1),
                            })
                            .attributes
                            .push(wgpu::VertexAttribute {
                                format: attribute.format,
                                offset: attribute.offset,
                                shader_location,
                            });
                    }
                    let mut vertex_buffer_layouts = Vec::with_capacity(1 + vertex_buffers.len());
                    vertex_buffer_layouts.push(wgpu::VertexBufferLayout {
                        array_stride: 4,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![0 => Uint32],
                    });
                    vertex_buffer_layouts.extend(vertex_buffers.values().map(|layout| {
                        wgpu::VertexBufferLayout {
                            array_stride: layout.array_stride,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &layout.attributes,
                        }
                    }));

                    let pipeline_layout =
                        self.device
                            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                                label: Some(&format!("{name} pipeline layout")),
                                bind_group_layouts: &[&self.render_bind_group_layout],
                                push_constant_ranges: &[],
                            });

                    let pipeline =
                        self.device
                            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                                label: Some(&format!("{name} pipeline")),
                                layout: Some(&pipeline_layout),
                                vertex: wgpu::VertexState {
                                    module: &self.primitive_shader_module,
                                    entry_point: Some("vs_main"),
                                    compilation_options: wgpu::PipelineCompilationOptions::default(
                                    ),
                                    buffers: &vertex_buffer_layouts,
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
                                depth_stencil: Some(wgpu::DepthStencilState {
                                    format: wgpu::TextureFormat::Depth24Plus,
                                    depth_write_enabled: true,
                                    depth_compare: wgpu::CompareFunction::LessEqual,
                                    stencil: wgpu::StencilState::default(),
                                    bias: wgpu::DepthBiasState::default(),
                                }),
                                multisample: wgpu::MultisampleState {
                                    count: 1,
                                    mask: !0,
                                    alpha_to_coverage_enabled: false,
                                },
                                fragment: Some(wgpu::FragmentState {
                                    module: &self.primitive_shader_module,
                                    entry_point: Some("fs_main"),
                                    compilation_options: wgpu::PipelineCompilationOptions::default(
                                    ),
                                    targets: &[Some(self.render_texture_format.into())],
                                }),
                                multiview: None,
                                cache: None,
                            });

                    let vertex_buffers = vertex_buffers.into_keys().collect();

                    primitives.push(Primitive {
                        pipeline,
                        index_buffer,
                        vertex_buffers,
                        vertex_count,
                    });
                }
                self.meshes
                    .push(MeshDescriptor {
                        name: gltf_mesh.name().map(|name| name.to_string()),
                        primitives,
                    })
                    .id()
            })
            .collect()
    }
}

impl Scene {
    fn build_gltf_node(
        &mut self,
        node: gltf::Node,
        parent: Option<Id<Node>>,
        meshes: &mut SparseSet<Mesh>,
        mesh_mapping: &[Id<Mesh>],
    ) -> Id<Node> {
        let mesh = node
            .mesh()
            .map(|gltf_mesh| &meshes[mesh_mapping[gltf_mesh.index()]]);
        let mut builder = self
            .node_builder()
            .name(node.name().map(|name| name.to_string()))
            .parent(parent);
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
        let id = builder.mesh(mesh).build().id();
        for child in node.children() {
            self.build_gltf_node(child, Some(id), meshes, mesh_mapping);
        }
        id
    }
}

#[derive(Clone)]
enum Accessor {
    IndexBuffer(IndexBuffer),
    Attribute(Attribute),
}

impl IndexBuffer {
    fn from_gltf(
        indices: &gltf::Accessor,
        buffers: &Vec<gltf::buffer::Data>,
        views: &mut Vec<Option<View>>,
        device: &wgpu::Device,
    ) -> Self {
        if indices.sparse().is_some() {
            todo!("sparse primitive indices")
        } else {
            let view = indices
                .view()
                .expect("dense gltf accessor should have a view");
            let format = match indices.data_type() {
                gltf::accessor::DataType::U16 => wgpu::IndexFormat::Uint16,
                gltf::accessor::DataType::U32 => wgpu::IndexFormat::Uint32,
                _ => panic!("index buffer format should be one of u16 or u32"),
            };
            let view_idx = view.index();
            let buffer = match &views[view_idx] {
                Some(view) => view.buffer.clone(),
                None => {
                    let view = View::from(&view, buffers, wgpu::BufferUsages::INDEX, device);
                    let buffer = view.buffer.clone();
                    views[view_idx] = Some(view);
                    buffer
                }
            };
            Self {
                buffer,
                offset: indices.offset() as u64,
                format,
            }
        }
    }
}

#[derive(Debug, Clone)]
struct Attribute {
    buffer: wgpu::Buffer,
    array_stride: wgpu::BufferAddress,
    format: wgpu::VertexFormat,
    offset: u64,
}

struct VertexBufferLayout {
    array_stride: wgpu::BufferAddress,
    attributes: Vec<wgpu::VertexAttribute>,
}

impl Attribute {
    fn from(
        accessor: &gltf::Accessor,
        buffers: &Vec<gltf::buffer::Data>,
        views: &mut Vec<Option<View>>,
        device: &wgpu::Device,
    ) -> Self {
        #[rustfmt::skip]
        let format = match (accessor.data_type(), accessor.dimensions()) {
            (gltf::accessor::DataType::U8, gltf_json::accessor::Type::Scalar) => wgpu::VertexFormat::Uint8,
            (gltf::accessor::DataType::U8, gltf_json::accessor::Type::Vec2) => wgpu::VertexFormat::Uint8x2,
            (gltf::accessor::DataType::U8, gltf_json::accessor::Type::Vec4) => wgpu::VertexFormat::Uint8x4,
            (gltf::accessor::DataType::U16, gltf_json::accessor::Type::Scalar) => wgpu::VertexFormat::Uint16,
            (gltf::accessor::DataType::U16, gltf_json::accessor::Type::Vec2) => wgpu::VertexFormat::Uint16x2,
            (gltf::accessor::DataType::U16, gltf_json::accessor::Type::Vec4) => wgpu::VertexFormat::Uint16x4,
            (gltf::accessor::DataType::U32, gltf_json::accessor::Type::Scalar) => wgpu::VertexFormat::Uint32,
            (gltf::accessor::DataType::U32, gltf_json::accessor::Type::Vec2) => wgpu::VertexFormat::Uint32x2,
            (gltf::accessor::DataType::U32, gltf_json::accessor::Type::Vec3) => wgpu::VertexFormat::Uint32x3,
            (gltf::accessor::DataType::U32, gltf_json::accessor::Type::Vec4) => wgpu::VertexFormat::Uint32x4,
            (gltf::accessor::DataType::I8, gltf_json::accessor::Type::Scalar) => wgpu::VertexFormat::Sint8,
            (gltf::accessor::DataType::I8, gltf_json::accessor::Type::Vec2) => wgpu::VertexFormat::Sint8x2,
            (gltf::accessor::DataType::I8, gltf_json::accessor::Type::Vec4) => wgpu::VertexFormat::Sint8x4,
            (gltf::accessor::DataType::I16, gltf_json::accessor::Type::Scalar) => wgpu::VertexFormat::Sint16,
            (gltf::accessor::DataType::I16, gltf_json::accessor::Type::Vec2) => wgpu::VertexFormat::Sint16x2,
            (gltf::accessor::DataType::I16, gltf_json::accessor::Type::Vec4) => wgpu::VertexFormat::Sint16x4,
            (gltf::accessor::DataType::F32, gltf_json::accessor::Type::Scalar) => wgpu::VertexFormat::Float32,
            (gltf::accessor::DataType::F32, gltf_json::accessor::Type::Vec2) => wgpu::VertexFormat::Float32x2,
            (gltf::accessor::DataType::F32, gltf_json::accessor::Type::Vec3) => wgpu::VertexFormat::Float32x3,
            (gltf::accessor::DataType::F32, gltf_json::accessor::Type::Vec4) => wgpu::VertexFormat::Float32x4,
            _ => panic!("unsupported vertex format")
        };

        if accessor.sparse().is_some() {
            todo!("sparse attribute accessor")
        } else {
            let view = accessor.view().expect("dense accessor must have a view");
            let array_stride = view.stride().unwrap_or(accessor.size()) as wgpu::BufferAddress;
            let view_idx = view.index();
            let buffer = match &views[view_idx] {
                Some(view) => view.buffer.clone(),
                None => {
                    let view = View::from(&view, buffers, wgpu::BufferUsages::VERTEX, device);
                    let buffer = view.buffer.clone();
                    views[view_idx] = Some(view);
                    buffer
                }
            };
            Self {
                buffer,
                array_stride,
                format,
                offset: accessor.offset() as u64,
            }
        }
    }
}

#[derive(Clone)]
struct View {
    buffer: wgpu::Buffer,
}

impl View {
    fn from(
        view: &gltf::buffer::View,
        buffers: &Vec<gltf::buffer::Data>,
        usage: wgpu::BufferUsages,
        device: &wgpu::Device,
    ) -> Self {
        let name = format!("view({}) {}", view.index(), view.name().unwrap_or(""));
        let start = view.offset();
        let end = start + view.length();
        let contents = &buffers[view.buffer().index()].0[start..end];
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&name),
            contents,
            usage,
        });
        Self { buffer }
    }
}
