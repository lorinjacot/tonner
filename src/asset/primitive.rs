use std::collections::HashMap;

use wgpu::util::DeviceExt;

use crate::camera::Camera;

pub struct PrimitiveManager {
    pipeline: wgpu::RenderPipeline,
    buffers_mapping: HashMap<usize, usize>,
    indices: Vec<wgpu::Buffer>,
    vertices: Vec<wgpu::Buffer>,
    primitives: Vec<Primitive>,
}

impl PrimitiveManager {
    pub fn new(device: &wgpu::Device, targets: &[Option<wgpu::ColorTargetState>]) -> Self {
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
            bind_group_layouts: &[&camera_bind_group_layout],
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
            buffers_mapping: HashMap::new(),
            indices: Vec::new(),
            vertices: Vec::new(),
            primitives: Vec::new(),
        }
    }

    pub fn load(
        &mut self,
        gltf_primitive: &gltf::Primitive,
        device: &wgpu::Device,
        buffers: &Vec<gltf::buffer::Data>,
    ) {
        assert_eq!(
            gltf_primitive.mode(),
            gltf::mesh::Mode::Triangles,
            "only mode 4 currently supported"
        );

        let indices = gltf_primitive
            .indices()
            .expect("only indexed primitive are currently supported");
        let index_format = match indices.data_type() {
            gltf::accessor::DataType::U16 => wgpu::IndexFormat::Uint16,
            gltf::accessor::DataType::U32 => wgpu::IndexFormat::Uint32,
            _ => panic!("unsupported index format"),
        };
        let vertices_count = indices.count() as u32;
        let indices = self.create_buffer_slice(&indices, false, device, buffers);

        let positions = gltf_primitive
            .get(&gltf::Semantic::Positions)
            .expect("primitive should have a POSITION attribute");
        let positions = self.create_buffer_slice(&positions, true, device, buffers);

        self.primitives.push(Primitive {
            indices,
            index_format,
            vertices: positions,
            vertices_count,
        });
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
        let buffer = self.init_buffer(&buffer_view, is_vertex, device, buffers);
        let start = accessor.offset() as u64;
        let end = start + (accessor.count() * accessor.size()) as u64;
        BufferSlice { buffer, start, end }
    }

    fn init_buffer(
        &mut self,
        buffer_view: &gltf::buffer::View,
        is_vertex: bool,
        device: &wgpu::Device,
        buffers: &Vec<gltf::buffer::Data>,
    ) -> usize {
        *self
            .buffers_mapping
            .entry(buffer_view.index())
            .or_insert_with(|| {
                let offset = buffer_view.offset();
                let (index, vec, usage) = if is_vertex {
                    (
                        self.vertices.len(),
                        &mut self.vertices,
                        wgpu::BufferUsages::VERTEX,
                    )
                } else {
                    (
                        self.indices.len(),
                        &mut self.indices,
                        wgpu::BufferUsages::INDEX,
                    )
                };
                vec.push(
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("bufferViews[{}]", buffer_view.index())),
                        contents: &buffers[buffer_view.buffer().index()].0
                            [offset..offset + buffer_view.length()],
                        usage,
                    }),
                );
                index
            })
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

impl VecBufferExt for Vec<wgpu::Buffer> {
    fn slice(&self, slice: &BufferSlice) -> wgpu::BufferSlice<'_> {
        self[slice.buffer].slice(slice.start..slice.end)
    }
}

struct Primitive {
    indices: BufferSlice,
    index_format: wgpu::IndexFormat,
    vertices: BufferSlice,
    vertices_count: u32,
}

pub trait DrawPrimitives {
    fn draw_primitives(&mut self, manager: &PrimitiveManager, camera: &Camera);
}

impl<'a> DrawPrimitives for wgpu::RenderPass<'a> {
    fn draw_primitives(&mut self, manager: &PrimitiveManager, camera: &Camera) {
        self.set_pipeline(&manager.pipeline);
        self.set_bind_group(0, camera.bind_group(), &[]);
        for primitive in manager.primitives.iter() {
            self.set_index_buffer(
                manager.indices.slice(&primitive.indices),
                primitive.index_format,
            );
            self.set_vertex_buffer(0, manager.vertices.slice(&primitive.vertices));
            self.draw_indexed(0..primitive.vertices_count, 0, 0..1);
        }
    }
}
