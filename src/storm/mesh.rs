use std::collections::HashMap;

use crate::storage::{Id, Storage};

use super::material::MaterialId;

pub struct MeshManager {
    meshes: Storage<Mesh>,
    shader_module: wgpu::ShaderModule,
    attributes_bind_group_layout: wgpu::BindGroupLayout,
    primitive_builder_pipeline_layout: wgpu::PipelineLayout,
}

impl MeshManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let meshes = Storage::new();

        let shader_module = device.create_shader_module(wgpu::include_wgsl!("mesh.wgsl"));

        let attributes_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Primitive attributes bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let primitive_builder_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Primitive builder pipeline layout"),
                bind_group_layouts: &[&attributes_bind_group_layout],
                push_constant_ranges: &[],
            });

        Self {
            meshes,
            shader_module,
            attributes_bind_group_layout,
            primitive_builder_pipeline_layout,
        }
    }

    pub fn builder<'a>(&'a mut self, name: Option<&'a str>) -> MeshBuilder<'a> {
        MeshBuilder {
            manager: self,
            label: name,
            primitives: Vec::new(),
        }
    }
}

pub type MeshId = Id<Mesh>;
pub struct Mesh {
    primitives: Vec<Primitive>,
}

pub struct MeshBuilder<'a> {
    manager: &'a mut MeshManager,
    label: Option<&'a str>,
    primitives: Vec<Primitive>,
}

impl<'a> MeshBuilder<'a> {
    pub fn primitive(
        &'a mut self,
        vertex_count: u32,
        material: MaterialId,
    ) -> PrimitiveBuilder<'a> {
        let label = format!(
            "{} primitive {}",
            self.label.unwrap_or("mesh"),
            self.primitives.len()
        );
        PrimitiveBuilder {
            mesh: self,
            label,
            vertex_count,
            constants: HashMap::new(),
            indices: None,
            vertex_buffers: Vec::new(),
            vertex_buffer_layouts: Vec::new(),
            material,
        }
    }

    pub fn build(self) -> MeshId {
        self.manager.meshes.add(Mesh {
            primitives: self.primitives,
        })
    }
}

pub struct PrimitiveBuilder<'a> {
    mesh: &'a mut MeshBuilder<'a>,
    label: String,
    vertex_count: u32,
    constants: HashMap<String, f64>,
    indices: Option<(wgpu::Buffer, wgpu::IndexFormat)>,
    vertex_buffers: Vec<wgpu::BufferSlice<'a>>,
    vertex_buffer_layouts: Vec<wgpu::VertexBufferLayout<'a>>,
    material: MaterialId,
}

impl<'a> PrimitiveBuilder<'a> {
    pub fn build(self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder) {
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("{} builder pipeline", self.label)),
            layout: Some(&self.mesh.manager.primitive_builder_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.mesh.manager.shader_module,
                entry_point: Some("vs_attributes"),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &self.constants,
                    ..Default::default()
                },
                buffers: &self.vertex_buffer_layouts,
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Point,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: None,
            multiview: None,
            cache: None,
        });

        let attributes = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{} attributes buffer", self.label)),
            size: self.vertex_count as u64 * 4 * 18,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{} attributes bind group", self.label)),
            layout: &self.mesh.manager.attributes_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: attributes.as_entire_binding(),
            }],
        });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(&format!("{} primitive builder render pass", self.label)),
            color_attachments: &[],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_pipeline(&pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        for (slot, buffer_slice) in self.vertex_buffers.into_iter().enumerate() {
            render_pass.set_vertex_buffer(slot as u32, buffer_slice);
        }
        match self.indices {
            Some((ref buffer, index_format)) => {
                render_pass.set_index_buffer(buffer.slice(..), index_format);
                render_pass.draw_indexed(0..self.vertex_count, 0, 0..1);
            }
            None => {
                render_pass.draw(0..self.vertex_count, 0..1);
            }
        }

        self.mesh.primitives.push(Primitive {
            vertex_count: self.vertex_count,
            attributes,
            material: self.material,
        });
    }
}

struct Primitive {
    vertex_count: u32,
    attributes: wgpu::Buffer,
    material: MaterialId,
}
