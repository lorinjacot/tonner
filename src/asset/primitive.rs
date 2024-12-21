use std::collections::HashMap;

use wgpu::util::DeviceExt;

use crate::camera::Camera;

use super::scene::Scene;

pub struct PrimitiveManager {
    pipeline: wgpu::RenderPipeline,
    node_bind_group_layout: wgpu::BindGroupLayout,
    indices: HashMap<usize, wgpu::Buffer>,
    vertices: HashMap<usize, wgpu::Buffer>,
    materials: HashMap<Option<usize>, wgpu::BindGroup>,
    material_bind_group_layout: wgpu::BindGroupLayout,
    primitives: HashMap<usize, Vec<Primitive>>,
}

impl PrimitiveManager {
    pub fn new(device: &wgpu::Device, targets: &[Option<wgpu::ColorTargetState>]) -> Self {
        let node_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Node bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Matrial bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Primitive camera bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let module = device.create_shader_module(wgpu::include_wgsl!("primitive.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Primitive pipeline layout"),
            bind_group_layouts: &[
                &node_bind_group_layout,
                &material_bind_group_layout,
                &camera_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Primitive pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 3 * 4,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    }],
                }],
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
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets,
            }),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            node_bind_group_layout,
            indices: HashMap::new(),
            vertices: HashMap::new(),
            materials: HashMap::new(),
            material_bind_group_layout,
            primitives: HashMap::new(),
        }
    }

    pub fn node_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.node_bind_group_layout
    }

    pub fn init_mesh(
        &mut self,
        mesh: &gltf::Mesh,
        device: &wgpu::Device,
        buffers: &Vec<gltf::buffer::Data>,
    ) {
        let mesh_index = mesh.index();
        if self.primitives.contains_key(&mesh_index) {
            return;
        }

        for primitive in mesh.primitives() {
            assert_eq!(
                primitive.mode(),
                gltf::mesh::Mode::Triangles,
                "only mode 4 currently supported"
            );

            if let Some(positions) = primitive.get(&gltf::Semantic::Positions) {
                let (indices, vertices_count) = if let Some(indices) = primitive.indices() {
                    let index_format = match indices.data_type() {
                        gltf::accessor::DataType::U16 => wgpu::IndexFormat::Uint16,
                        gltf::accessor::DataType::U32 => wgpu::IndexFormat::Uint32,
                        _ => panic!("unsupported index format"),
                    };
                    let vertices_count = indices.count() as u32;
                    let buffer_slice = self.create_buffer_slice(&indices, false, device, buffers);
                    (Some((buffer_slice, index_format)), vertices_count)
                } else {
                    (None, positions.count() as u32)
                };

                let positions = self.create_buffer_slice(&positions, true, device, buffers);

                let material = primitive.material();
                self.init_material(&material, device);
                let material = material.index();

                self.primitives
                    .entry(mesh_index)
                    .or_default()
                    .push(Primitive {
                        indices,
                        positions,
                        vertices_count,
                        material,
                    });
            }
        }
    }

    fn create_buffer_slice(
        &mut self,
        accessor: &gltf::Accessor,
        is_vertex: bool,
        device: &wgpu::Device,
        buffers: &Vec<gltf::buffer::Data>,
    ) -> BufferSlice {
        let buffer_view = accessor
            .view()
            .expect("sparse accessor currently not supported");
        if is_vertex
            && buffer_view
                .stride()
                .is_some_and(|stride| stride != accessor.size())
        {
            panic!("only dense attributes buffer are currently supported");
        };
        self.init_buffer(&buffer_view, is_vertex, device, buffers);
        let start = accessor.offset() as u64;
        let end = start + (accessor.count() * accessor.size()) as u64;
        BufferSlice {
            buffer: buffer_view.index(),
            start,
            end,
        }
    }

    fn init_buffer(
        &mut self,
        buffer_view: &gltf::buffer::View,
        is_vertex: bool,
        device: &wgpu::Device,
        gltf_buffers: &Vec<gltf::buffer::Data>,
    ) {
        let index = buffer_view.index();
        let buffers = if is_vertex {
            &mut self.vertices
        } else {
            &mut self.indices
        };
        buffers.entry(index).or_insert_with(|| {
            let offset = buffer_view.offset();
            let usage = if is_vertex {
                wgpu::BufferUsages::VERTEX
            } else {
                wgpu::BufferUsages::INDEX
            };
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("bufferViews[{}] buffer", index)),
                contents: &gltf_buffers[buffer_view.buffer().index()].0
                    [offset..offset + buffer_view.length()],
                usage,
            })
        });
    }

    fn init_material(&mut self, gltf_material: &gltf::Material, device: &wgpu::Device) {
        let index = gltf_material.index();
        self.materials.entry(index).or_insert_with(|| {
            let name = if let Some(index) = index {
                &format!("materials[{index}]")
            } else {
                "default material"
            };

            let pbr_metallic_roughness = gltf_material.pbr_metallic_roughness();

            let base_color_factor = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{name} base color factor buffer")),
                contents: bytemuck::cast_slice(&[pbr_metallic_roughness.base_color_factor()]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("{name} bind group")),
                layout: &self.material_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: base_color_factor.as_entire_binding(),
                }],
            })
        });
    }
}

#[derive(Debug)]
struct BufferSlice {
    buffer: usize,
    start: u64,
    end: u64,
}

trait VecBufferExt {
    fn slice(&self, slice: &BufferSlice) -> wgpu::BufferSlice<'_>;
}

impl VecBufferExt for HashMap<usize, wgpu::Buffer> {
    fn slice(&self, slice: &BufferSlice) -> wgpu::BufferSlice<'_> {
        self[&slice.buffer].slice(slice.start..slice.end)
    }
}

pub struct Primitive {
    vertices_count: u32,
    indices: Option<(BufferSlice, wgpu::IndexFormat)>,
    positions: BufferSlice,
    material: Option<usize>,
}

pub trait DrawPrimitives {
    fn draw_primitives(&mut self, manager: &PrimitiveManager, scene: &Scene, camera: &Camera);
}

impl<'a> DrawPrimitives for wgpu::RenderPass<'a> {
    fn draw_primitives(&mut self, manager: &PrimitiveManager, scene: &Scene, camera: &Camera) {
        self.set_pipeline(&manager.pipeline);
        self.set_bind_group(0, scene.nodes_bind_group(), &[]);
        self.set_bind_group(2, camera.bind_group(), &[]);
        for mesh in manager.primitives.values() {
            for primitive in mesh.iter() {
                self.set_bind_group(1, &manager.materials[&primitive.material], &[]);
                self.set_vertex_buffer(0, manager.vertices.slice(&primitive.positions));
                match &primitive.indices {
                    Some((indices, format)) => {
                        self.set_index_buffer(manager.indices.slice(&indices), *format);
                        self.draw_indexed(0..primitive.vertices_count, 0, 0..1);
                    }
                    None => {
                        self.draw(0..primitive.vertices_count, 0..1);
                    }
                }
            }
        }
    }
}
