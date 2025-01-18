use std::{
    collections::HashSet,
    ops::{Index, IndexMut},
};

use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::storage::{Id, Storage};

use super::{material::MaterialManager, MaterialId, NodeId};

pub const TEX_COORDS_LEN: usize = 2;
pub const COLORS_LEN: usize = 1;

pub struct MeshManager {
    meshes: Storage<Mesh>,
    primitive_pipeline: wgpu::RenderPipeline,
}

impl MeshManager {
    pub fn new(
        device: &wgpu::Device,
        nodes_bind_group_layout: &wgpu::BindGroupLayout,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        lights_bind_group_layout: &wgpu::BindGroupLayout,
        material_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let primitive_module = device.create_shader_module(wgpu::include_wgsl!("primitive.wgsl"));

        let primitive_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Primitive pipeline layout"),
                bind_group_layouts: &[
                    nodes_bind_group_layout,
                    camera_bind_group_layout,
                    lights_bind_group_layout,
                    material_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let primitive_attributes = wgpu::vertex_attr_array![
            1 => Float32x3,
            2 => Float32x3,
            3 => Float32x2,
            4 => Float32x2,
            5 => Float32x4,
        ];
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
                        array_stride: size_of::<PrimitiveAttributes>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &primitive_attributes,
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &primitive_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
            }),
            multiview: None,
            cache: None,
        });
        log::debug!("primitive pipeline created");

        Self {
            meshes: Storage::new(),
            primitive_pipeline,
        }
    }

    pub fn create(
        &mut self,
        mesh: MeshDescriptor,
        device: &wgpu::Device,
    ) -> Result<MeshId, MeshCreationError> {
        let primitives = mesh
            .primitives
            .into_iter()
            .map(|primitive| Primitive {
                vertex_count: primitive.vertex_count,
                indices: primitive.indices,
                attributes: primitive.attributes,
                material: primitive.material,
            })
            .collect();

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

    // pub fn get(&self, mesh: MeshId) -> Option<&Mesh> {
    //     self.meshes.get(mesh)
    // }

    pub fn get_mut(&mut self, mesh: MeshId) -> Option<&mut Mesh> {
        self.meshes.get_mut(mesh)
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
    indices: Option<PrimitiveIndices>,
    attributes: wgpu::Buffer,
    material: MaterialId,
}

pub struct PrimitiveIndices {
    pub buffer: wgpu::Buffer,
    pub format: wgpu::IndexFormat,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PrimitiveAttributes {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coords: [[f32; 2]; TEX_COORDS_LEN],
    pub colors: [[f32; 4]; COLORS_LEN],
}

pub struct MeshDescriptor {
    pub primitives: Vec<PrimitiveDescriptor>,
}

pub struct PrimitiveDescriptor {
    pub vertex_count: u32,
    pub indices: Option<PrimitiveIndices>,
    pub attributes: wgpu::Buffer,
    pub material: MaterialId,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MeshCreationError {}

pub trait DrawMeshes {
    fn draw_meshes(
        &mut self,
        meshes: &MeshManager,
        materials: &MaterialManager,
        nodes_bind_group: &wgpu::BindGroup,
        camera_bind_group: &wgpu::BindGroup,
        light_bind_group: &wgpu::BindGroup,
    );
}

impl<'a> DrawMeshes for wgpu::RenderPass<'a> {
    fn draw_meshes(
        &mut self,
        meshes_manager: &MeshManager,
        materials: &MaterialManager,
        nodes_bind_group: &wgpu::BindGroup,
        camera_bind_group: &wgpu::BindGroup,
        light_bind_group: &wgpu::BindGroup,
    ) {
        self.set_pipeline(&meshes_manager.primitive_pipeline);
        self.set_bind_group(0, nodes_bind_group, &[]);
        self.set_bind_group(1, camera_bind_group, &[]);
        self.set_bind_group(2, light_bind_group, &[]);

        for mesh in meshes_manager.meshes.values() {
            let instance_count = mesh.nodes.len() as u32;
            self.set_vertex_buffer(0, mesh.nodes_buffer.slice(..));

            for primitive in &mesh.primitives {
                self.set_bind_group(3, materials[primitive.material].bind_group(), &[]);
                self.set_vertex_buffer(1, primitive.attributes.slice(..));
                match &primitive.indices {
                    Some(indices) => {
                        self.set_index_buffer(indices.buffer.slice(..), indices.format);
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
