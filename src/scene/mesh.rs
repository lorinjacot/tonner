use std::collections::HashMap;

use wgpu::util::DeviceExt;

use crate::asset::Asset;

use super::{DrawScene, Node};

pub struct MeshManager {
    nodes_bind_group_layout: wgpu::BindGroupLayout,
    camera_bind_group_layout: wgpu::BindGroupLayout,
    primitive_pipeline: wgpu::RenderPipeline,

    gltf_mesh_mapping: HashMap<usize, usize>,
    meshes: Vec<Mesh>,
}

impl MeshManager {
    pub fn new(device: &wgpu::Device, targets: &[Option<wgpu::ColorTargetState>]) -> Self {
        let primitive_module = device.create_shader_module(wgpu::include_wgsl!("primitive.wgsl"));

        let nodes_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Nodes bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let primitive_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Primitive pipeline layout"),
                bind_group_layouts: &[&nodes_bind_group_layout, &camera_bind_group_layout],
                push_constant_ranges: &[],
            });

        let primitive_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Primitive render pipeline"),
            layout: Some(&primitive_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &primitive_module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: 4,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Uint32,
                            offset: 0,
                            shader_location: 0,
                        }],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: 3 * 4,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 1,
                        }],
                    },
                ],
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
                module: &primitive_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets,
            }),
            multiview: None,
            cache: None,
        });

        Self {
            nodes_bind_group_layout,
            camera_bind_group_layout,
            primitive_pipeline,
            gltf_mesh_mapping: HashMap::new(),
            meshes: Vec::new(),
        }
    }

    pub fn nodes_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.nodes_bind_group_layout
    }

    pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.camera_bind_group_layout
    }

    pub fn add_mesh_to_nodes(
        &mut self,
        gltf_mesh: &gltf::Mesh,
        nodes_id: &[usize],
        nodes: &mut [Node],
        device: &wgpu::Device,
        asset: &Asset,
    ) {
        if let Some(mesh_id) = self.gltf_mesh_mapping.get(&gltf_mesh.index()) {
            for node_id in nodes_id {
                let node = &mut nodes[*node_id];
                if let Some(_old_mesh) = node.mesh {
                    panic!("changing mesh currently not supported");
                }
                node.mesh = Some(*mesh_id);
                self.meshes[*mesh_id].nodes.push(*node_id);
            }
            return;
        }

        let mesh_id = self.meshes.len();
        let mut primitives = Vec::with_capacity(gltf_mesh.primitives().len());
        for gltf_primitive in gltf_mesh.primitives() {
            if let Some(positions) = gltf_primitive.get(&gltf::Semantic::Positions) {
                let name = format!("mesh_{} primitive_{}", gltf_mesh.index(), gltf_mesh.index());

                let (index_buffer, vertex_count) = if let Some(indices) = gltf_primitive.indices() {
                    let buffer_view = indices.view().expect("sparse accessor not supported");
                    let start = buffer_view.offset() + indices.offset();
                    let end = start + indices.count() * indices.size();
                    let contents = &asset.buffers[buffer_view.buffer().index()][start..end];

                    let index_buffer = Some(device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("{name} index buffer")),
                            contents,
                            usage: wgpu::BufferUsages::INDEX,
                        },
                    ));

                    (index_buffer, indices.count() as u32)
                } else {
                    (None, positions.count() as u32)
                };

                let buffer_view = positions.view().expect("sparse accessor not supported");
                if buffer_view
                    .stride()
                    .is_some_and(|stride| stride != positions.size())
                {
                    panic!("only tightly packed positions are supported");
                }
                let start = buffer_view.offset() + positions.offset();
                let end = start + positions.count() * positions.size();
                let contents = &asset.buffers[buffer_view.buffer().index()][start..end];

                let attributes_buffer =
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("{name} attributes buffer")),
                        contents,
                        usage: wgpu::BufferUsages::VERTEX,
                    });

                primitives.push(Primitive {
                    vertex_count,
                    index_buffer,
                    attributes_buffer,
                });
            }
        }
        let nodes_id_32 = nodes_id
            .iter()
            .map(|node_id| {
                let node = &mut nodes[*node_id];
                if let Some(_old_mesh) = node.mesh {
                    panic!("changing mesh currently not supported");
                }
                node.mesh = Some(mesh_id);
                *node_id as u32
            })
            .collect::<Vec<_>>();
        let node_id_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("mesh_{} nodes buffer", gltf_mesh.index())),
            contents: bytemuck::cast_slice(&nodes_id_32),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.meshes.push(Mesh {
            nodes: nodes_id.to_vec(),
            node_id_buffer,
            primitives,
        });
        self.gltf_mesh_mapping.insert(gltf_mesh.index(), mesh_id);
    }
}

struct Mesh {
    nodes: Vec<usize>,
    node_id_buffer: wgpu::Buffer,
    primitives: Vec<Primitive>,
}

struct Primitive {
    vertex_count: u32,
    index_buffer: Option<wgpu::Buffer>,
    attributes_buffer: wgpu::Buffer,
}

impl<'a> DrawScene for wgpu::RenderPass<'a> {
    fn draw_scene(&mut self, scene: &super::Scene) {
        let manager = &scene.meshes;

        self.set_pipeline(&manager.primitive_pipeline);
        self.set_bind_group(0, &scene.nodes_bind_group, &[]);
        self.set_bind_group(1, scene.camera.bind_group(), &[]);

        for mesh in &manager.meshes {
            let instance_count = mesh.nodes.len() as u32;
            self.set_vertex_buffer(0, mesh.node_id_buffer.slice(..));

            for primitive in &mesh.primitives {
                self.set_vertex_buffer(1, primitive.attributes_buffer.slice(..));
                match &primitive.index_buffer {
                    Some(index_buffer) => {
                        self.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                        self.draw_indexed(0..primitive.vertex_count, 0, 0..instance_count);
                    }
                    None => {
                        self.draw(0..primitive.vertex_count, 0..instance_count);
                    }
                }
            }
        }
    }
}
