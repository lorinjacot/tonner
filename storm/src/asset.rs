use std::{collections::BTreeMap, ops::Range};

use wgpu::util::DeviceExt;

use crate::{
    Id, Storm,
    mesh::{IndexBuffer, Mesh, MeshDescriptor, Primitive},
    scene::{Node, Scene, SceneDescriptor},
    storage::{DenseEntry, SetEntry, SparseSet},
};

impl Storm {
    pub fn open_gltf(
        &mut self,
        path: impl AsRef<std::path::Path>,
        device: &wgpu::Device,
    ) -> Result<&Asset, gltf::Error> {
        let (document, buffers, _images) = gltf::import(path)?;

        let mut accessors: Vec<Option<Accessor>> = vec![None; document.accessors().len()];
        let mut views: Vec<Option<View>> = vec![None; document.views().len()];

        let meshes: Vec<_> = document
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

                    let index_buffer = primitive.indices().map(|indices| {
                        vertex_count = indices.count() as u32;
                        match &accessors[indices.index()] {
                            Some(Accessor::IndexBuffer(index_buffer)) => index_buffer.clone(),
                            None => {
                                let index_buffer =
                                    IndexBuffer::from_gltf(&indices, &buffers, &mut views, device);
                                accessors[indices.index()] =
                                    Some(Accessor::IndexBuffer(index_buffer.clone()));
                                index_buffer
                            }
                            _ => panic!(
                                "primitive indices accessors cannot be used for other purposes"
                            ),
                        }
                    });

                    let mut vertex_buffers: BTreeMap<usize, VertexBufferLayout> = BTreeMap::new();
                    for (semantic, accessor) in primitive.attributes() {
                        let attribute = match &accessors[accessor.index()] {
                            Some(Accessor::Attribute(attribute)) => attribute.clone(),
                            None => {
                                let attribute =
                                    Attribute::from(&accessor, &buffers, &mut views, device);
                                accessors[accessor.index()] =
                                    Some(Accessor::Attribute(attribute.clone()));
                                attribute
                            }
                            _ => panic!("attributes accessors cannot be used for other purposes"),
                        };
                        let shader_location = match semantic {
                            gltf::Semantic::Positions => 1,
                            // gltf::Semantic::Normals => 2,
                            // gltf::Semantic::Tangents => 3,
                            // gltf::Semantic::TexCoords(0) => 4,
                            // gltf::Semantic::TexCoords(1) => 5,
                            // gltf::Semantic::Colors(0) => 6,
                            _ => panic!("unsupported primitive attribute"),
                        };
                        vertex_buffers
                            .entry(attribute.view)
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
                    let (vertex_buffers, mut vertex_buffer_layouts): (_, Vec<_>) = vertex_buffers
                        .iter()
                        .map(|(view, layout)| {
                            (
                                views[*view].as_ref().unwrap().buffer.clone(),
                                wgpu::VertexBufferLayout {
                                    array_stride: layout.array_stride,
                                    step_mode: wgpu::VertexStepMode::Vertex,
                                    attributes: &layout.attributes,
                                },
                            )
                        })
                        .unzip();
                    vertex_buffer_layouts.push(wgpu::VertexBufferLayout {
                        array_stride: 4,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![0 => Uint32],
                    });

                    let pipeline_layout =
                        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                            label: Some(&format!("{name} pipeline layout")),
                            bind_group_layouts: &[&self.scene_bind_group_layout],
                            push_constant_ranges: &[],
                        });

                    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some(&format!("{name} pipeline")),
                        layout: Some(&pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: &self.primitive_shader_module,
                            entry_point: Some("vs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
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
                        depth_stencil: None,
                        multisample: wgpu::MultisampleState {
                            count: 1,
                            mask: !0,
                            alpha_to_coverage_enabled: false,
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: &self.primitive_shader_module,
                            entry_point: Some("fs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            targets: &[Some(self.render_texture_format.into())],
                        }),
                        multiview: None,
                        cache: None,
                    });

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
            .collect();

        let scenes: Vec<Id<Scene>> = document
            .scenes()
            .map(|gltf_scene| {
                let scene = self.scenes.push(SceneDescriptor {
                    name: gltf_scene.name().map(|name| name.to_string()),
                    bind_group_layout: self.scene_bind_group_layout.clone(),
                });
                for gltf_node in gltf_scene.nodes() {
                    Node::from_gltf(&gltf_node, None, scene, &mut self.meshes, &meshes, device);
                }
                scene.id()
            })
            .collect();
        self.scene = document.default_scene().map(|scene| scenes[scene.index()]);

        Ok(self.assets.push(()))
    }
}

impl Node {
    fn from_gltf<'a>(
        node: &gltf::Node,
        parent: Option<Id<Node>>,
        scene: &'a mut Scene,
        meshes: &mut SparseSet<Mesh>,
        meshes_mapping: &[Id<Mesh>],
        device: &wgpu::Device,
    ) -> Id<Node> {
        let mesh = node
            .mesh()
            .map(|gltf_mesh| &meshes[meshes_mapping[gltf_mesh.index()]]);
        let id = scene
            .node_builder()
            .name(node.name().map(|name| name.to_string()))
            .parent(parent)
            .mesh(mesh)
            .build(device)
            .id();
        for child in node.children() {
            Node::from_gltf(&child, Some(id), scene, meshes, meshes_mapping, device);
        }
        id
    }
}

pub struct Asset {
    id: Id<Self>,
}

impl DenseEntry for Asset {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

impl SetEntry for Asset {
    type Descriptor = ();

    fn new(id: Id<Self::Key>, _desc: Self::Descriptor) -> Self {
        Self { id }
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
            let start = indices.offset() as u64;
            let end = start + (indices.count() * indices.size()) as u64;
            let bounds = start..end;
            Self {
                buffer,
                bounds,
                format,
            }
        }
    }
}

#[derive(Debug, Clone)]
struct Attribute {
    view: usize,
    buffer: wgpu::Buffer,
    bounds: Range<u64>,
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
            let start = accessor.offset() as u64;
            let end = start + (accessor.count() * accessor.size()) as u64;
            let bounds = start..end;
            let offset = accessor.offset() as u64;
            Self {
                view: view_idx,
                buffer,
                bounds,
                array_stride,
                format,
                offset,
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
