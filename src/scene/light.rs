use glam::{vec3, Mat4, Quat};
use wgpu::util::DeviceExt;

pub struct LightManager {
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    light_pipeline: wgpu::RenderPipeline,
    positions_buffer: wgpu::Buffer,
    model_bind_group: wgpu::BindGroup,
}

impl LightManager {
    pub fn new(device: &wgpu::Device, camera_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let light_color = vec3(1.0, 1.0, 1.0);

        let light_color_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light color buffer"),
            contents: bytemuck::cast_slice(&[light_color]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Light bind group layout"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Light bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_color_buffer.as_entire_binding(),
            }],
        });

        let light_module = device.create_shader_module(wgpu::include_wgsl!("light.wgsl"));

        let model_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Light view projection bind group layout"),
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

        let light_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Light render pipeline layout"),
                bind_group_layouts: &[&model_bind_group_layout, &camera_bind_group_layout],
                push_constant_ranges: &[],
            });

        let light_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Light render pipeline"),
            layout: Some(&light_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &light_module,
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
                module: &light_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
            }),
            multiview: None,
            cache: None,
        });

        let positions = [
            vec3(-0.5, -0.5, -0.5),
            vec3(0.5, -0.5, -0.5),
            vec3(0.5, 0.5, -0.5),
            vec3(0.5, 0.5, -0.5),
            vec3(-0.5, 0.5, -0.5),
            vec3(-0.5, -0.5, -0.5),
            vec3(-0.5, -0.5, 0.5),
            vec3(0.5, -0.5, 0.5),
            vec3(0.5, 0.5, 0.5),
            vec3(0.5, 0.5, 0.5),
            vec3(-0.5, 0.5, 0.5),
            vec3(-0.5, -0.5, 0.5),
            vec3(-0.5, 0.5, 0.5),
            vec3(-0.5, 0.5, -0.5),
            vec3(-0.5, -0.5, -0.5),
            vec3(-0.5, -0.5, -0.5),
            vec3(-0.5, -0.5, 0.5),
            vec3(-0.5, 0.5, 0.5),
            vec3(0.5, 0.5, 0.5),
            vec3(0.5, 0.5, -0.5),
            vec3(0.5, -0.5, -0.5),
            vec3(0.5, -0.5, -0.5),
            vec3(0.5, -0.5, 0.5),
            vec3(0.5, 0.5, 0.5),
            vec3(-0.5, -0.5, -0.5),
            vec3(0.5, -0.5, -0.5),
            vec3(0.5, -0.5, 0.5),
            vec3(0.5, -0.5, 0.5),
            vec3(-0.5, -0.5, 0.5),
            vec3(-0.5, -0.5, -0.5),
            vec3(-0.5, 0.5, -0.5),
            vec3(0.5, 0.5, -0.5),
            vec3(0.5, 0.5, 0.5),
            vec3(0.5, 0.5, 0.5),
            vec3(-0.5, 0.5, 0.5),
            vec3(-0.5, 0.5, -0.5),
        ];

        let positions_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light positions buffer"),
            contents: bytemuck::cast_slice(&positions),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let model = Mat4::from_scale_rotation_translation(
            [0.2; 3].into(),
            Quat::IDENTITY,
            vec3(1.2, 1.0, -2.0),
        );

        let model_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light model buffer"),
            contents: bytemuck::cast_slice(&[model]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let model_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Light model bind group"),
            layout: &model_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: model_buffer.as_entire_binding(),
            }],
        });

        Self {
            bind_group,
            bind_group_layout,
            light_pipeline,
            positions_buffer,
            model_bind_group,
        }
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }
}

pub trait DrawLights {
    fn draw_lights(&mut self, lights: &LightManager, camera_bind_group: &wgpu::BindGroup);
}

impl<'a> DrawLights for wgpu::RenderPass<'a> {
    fn draw_lights(&mut self, lights: &LightManager, camera_bind_group: &wgpu::BindGroup) {
        self.set_pipeline(&lights.light_pipeline);
        self.set_bind_group(0, Some(&lights.model_bind_group), &[]);
        self.set_bind_group(1, Some(camera_bind_group), &[]);
        self.set_vertex_buffer(0, lights.positions_buffer.slice(..));
        self.draw(0..36, 0..1);
    }
}
