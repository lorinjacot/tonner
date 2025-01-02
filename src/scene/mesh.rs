use std::{
    collections::HashSet,
    ops::{Index, IndexMut},
};

use glam::Vec3;
use wgpu::util::DeviceExt;

use crate::storage::{Id, Storage};

use super::NodeId;

pub struct MeshManager {
    meshes: Storage<Mesh>,
    primitive_pipeline: wgpu::RenderPipeline,
}

impl MeshManager {
    pub fn new(
        device: &wgpu::Device,
        nodes_bind_group_layout: &wgpu::BindGroupLayout,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        targets: &[Option<wgpu::ColorTargetState>],
    ) -> Self {
        let primitive_module = device.create_shader_module(wgpu::include_wgsl!("primitive.wgsl"));

        let primitive_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Primitive pipeline layout"),
                bind_group_layouts: &[nodes_bind_group_layout, camera_bind_group_layout],
                push_constant_ranges: &[],
            });

        let primitive_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Primitive pipeline"),
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
            meshes: Storage::new(),
            primitive_pipeline,
        }
    }

    pub fn create(&mut self, mesh: MeshBuilder, device: &wgpu::Device) -> Result<MeshId, ()> {
        let mut primitives = Vec::with_capacity(mesh.primitives.len());
        for primitive in mesh.primitives {
            let positions = primitive.positions.ok_or(())?;

            let (vertex_count, indices) = match primitive.indices {
                Some(indices) => (
                    indices.len() as u32,
                    Some(
                        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Primitive indices buffer"),
                            contents: bytemuck::cast_slice(&indices),
                            usage: wgpu::BufferUsages::INDEX,
                        }),
                    ),
                ),
                None => (positions.len() as u32, None),
            };

            let attributes = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Primitive attributes buffer"),
                contents: bytemuck::cast_slice(&positions),
                usage: wgpu::BufferUsages::VERTEX,
            });

            primitives.push(Primitive {
                vertex_count,
                indices,
                attributes,
            });
        }

        let nodes_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Nodes buffer"),
            contents: &[],
            usage: wgpu::BufferUsages::VERTEX,
        });
        let mesh_id = self.meshes.add(Mesh {
            nodes: HashSet::new(),
            nodes_buffer,
            primitives,
        });

        Ok(mesh_id)
    }
}

impl Index<MeshId> for MeshManager {
    type Output = Mesh;

    fn index(&self, index: MeshId) -> &Self::Output {
        &self.meshes[index]
    }
}

impl IndexMut<MeshId> for MeshManager {
    fn index_mut(&mut self, index: MeshId) -> &mut Self::Output {
        &mut self.meshes[index]
    }
}

pub struct Mesh {
    pub(super) nodes: HashSet<NodeId>,
    nodes_buffer: wgpu::Buffer,
    primitives: Vec<Primitive>,
}

impl Mesh {
    pub(super) fn update_nodes_buffer(&mut self, dense_indices: Vec<u32>, device: &wgpu::Device) {
        self.nodes_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Nodes buffer"),
            contents: bytemuck::cast_slice(&dense_indices),
            usage: wgpu::BufferUsages::VERTEX,
        });
    }
}

pub type MeshId = Id<Mesh>;

struct Primitive {
    vertex_count: u32,
    indices: Option<wgpu::Buffer>,
    attributes: wgpu::Buffer,
}

pub struct MeshBuilder {
    primitives: Vec<PrimitiveBuilder>,
}

impl MeshBuilder {
    pub fn new() -> Self {
        Self {
            primitives: Vec::new(),
        }
    }

    pub fn set_primitives(mut self, primitives: Vec<PrimitiveBuilder>) -> Self {
        self.primitives = primitives;
        self
    }
}

pub struct PrimitiveBuilder {
    indices: Option<Vec<u32>>,
    positions: Option<Vec<Vec3>>,
}

impl PrimitiveBuilder {
    pub fn new() -> Self {
        Self {
            indices: None,
            positions: None,
        }
    }

    pub fn set_indices(mut self, indices: Option<Vec<u32>>) -> Self {
        self.indices = indices;
        self
    }

    pub fn set_positions(mut self, positions: Vec<Vec3>) -> Self {
        self.positions = Some(positions);
        self
    }
}

pub trait DrawMeshes {
    fn draw_meshes(
        &mut self,
        meshes: &MeshManager,
        nodes_bind_group: &wgpu::BindGroup,
        camera_bind_group: &wgpu::BindGroup,
    );
}

impl<'a> DrawMeshes for wgpu::RenderPass<'a> {
    fn draw_meshes(
        &mut self,
        meshes_manager: &MeshManager,
        nodes_bind_group: &wgpu::BindGroup,
        camera_bind_group: &wgpu::BindGroup,
    ) {
        self.set_pipeline(&meshes_manager.primitive_pipeline);
        self.set_bind_group(0, nodes_bind_group, &[]);
        self.set_bind_group(1, camera_bind_group, &[]);

        for mesh in meshes_manager.meshes.values() {
            let instance_count = mesh.nodes.len() as u32;
            self.set_vertex_buffer(0, mesh.nodes_buffer.slice(..));

            for primitive in &mesh.primitives {
                self.set_vertex_buffer(1, primitive.attributes.slice(..));
                match &primitive.indices {
                    Some(index_buffer) => {
                        self.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
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
