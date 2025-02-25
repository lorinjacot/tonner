use glam::Vec3;

use crate::texture::{Texture2dSampler, TextureCubeSampler, TextureManager, CUBE_INDICES};

const SKYBOX_SIZE: u32 = 512;
const IRRADIANCE_MAP_SIZE: u32 = 32;

const VERTEX_BUFFERS_LAYOUT: &[wgpu::VertexBufferLayout] = &[wgpu::VertexBufferLayout {
    array_stride: size_of::<Vec3>() as u64,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &[wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0,
    }],
}];

pub struct Environment {
    skybox_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    cubemap_bind_group_layout: wgpu::BindGroupLayout,
    skybox_bind_group: wgpu::BindGroup,
    irradiance_map_bind_group: wgpu::BindGroup,
}

impl Environment {
    pub fn from_equirectangular(
        equirectangular: &Texture2dSampler,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        textures: &mut TextureManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Self {
        let module = device.create_shader_module(wgpu::include_wgsl!("environment.wgsl"));

        let cubemap_bind_group_layout = create_cubemap_bind_group_layout(device);

        let skybox_pipeline = create_skybox_pipeline(
            camera_bind_group_layout,
            &cubemap_bind_group_layout,
            &module,
            device,
        );

        let cubemap_sampler = create_cubemap_sampler(device);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Environment::from_equirectangular command encoder"),
        });

        let skybox_texture = textures.create_cube_from_equirectangular(
            Some("Skybox texture"),
            SKYBOX_SIZE,
            SKYBOX_SIZE,
            wgpu::TextureFormat::Rgba16Float,
            equirectangular,
            &mut encoder,
        );
        let skybox = TextureCubeSampler {
            texture: skybox_texture,
            sampler: cubemap_sampler.clone(),
        };

        let skybox_bind_group =
            create_skybox_bind_group(&skybox, &cubemap_bind_group_layout, device);

        let irradiance_map_bind_group = create_irradiance_map_bind_group(
            &skybox_bind_group,
            &cubemap_bind_group_layout,
            cubemap_sampler,
            textures,
            &module,
            device,
            &mut encoder,
        );
        queue.submit([encoder.finish()]);

        Self {
            skybox_pipeline,
            vertex_buffer: textures.cube_vertex_buffer().clone(),
            index_buffer: textures.cube_index_buffer().clone(),
            cubemap_bind_group_layout,
            skybox_bind_group,
            irradiance_map_bind_group,
        }
    }

    pub fn from_faces(
        faces: &[image::DynamicImage; 6],
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        textures: &mut TextureManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Self, ()> {
        let module = device.create_shader_module(wgpu::include_wgsl!("environment.wgsl"));

        let cubemap_bind_group_layout = create_cubemap_bind_group_layout(device);

        let skybox_pipeline = create_skybox_pipeline(
            camera_bind_group_layout,
            &cubemap_bind_group_layout,
            &module,
            device,
        );

        let width = faces[0].width();
        let height = faces[0].height();
        let bytes_count = 6 * width as usize * height as usize * 4;
        let mut bytes = Vec::with_capacity(bytes_count);
        for face in faces {
            if face.width() == width && face.height() == height {
                bytes.extend_from_slice(face.as_bytes());
            } else {
                return Err(());
            }
        }

        let skybox_texture = textures
            .create_cube_from_faces(
                Some("Skybox texture"),
                faces,
                true,
                wgpu::TextureUsages::TEXTURE_BINDING,
            )
            .map_err(|_| ())?;
        let cubemap_sampler = create_cubemap_sampler(device);
        let skybox = TextureCubeSampler {
            texture: skybox_texture,
            sampler: cubemap_sampler.clone(),
        };

        let skybox_bind_group =
            create_skybox_bind_group(&skybox, &cubemap_bind_group_layout, device);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Environment command encoder"),
        });
        let irradiance_map_bind_group = create_irradiance_map_bind_group(
            &skybox_bind_group,
            &cubemap_bind_group_layout,
            cubemap_sampler,
            textures,
            &module,
            device,
            &mut encoder,
        );

        queue.submit([encoder.finish()]);

        Ok(Self {
            skybox_pipeline,
            vertex_buffer: textures.cube_vertex_buffer().clone(),
            index_buffer: textures.cube_index_buffer().clone(),
            cubemap_bind_group_layout,
            skybox_bind_group,
            irradiance_map_bind_group,
        })
    }

    pub fn irradiance_map_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.cubemap_bind_group_layout
    }

    pub fn irradiance_map_bind_group(&self) -> &wgpu::BindGroup {
        &self.irradiance_map_bind_group
    }
}

fn create_cubemap_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Environment cubemap sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    })
}

fn create_cubemap_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Environment cubemap bind group layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

fn create_skybox_pipeline(
    camera_bind_group_layout: &wgpu::BindGroupLayout,
    cubemap_bind_group_layout: &wgpu::BindGroupLayout,
    module: &wgpu::ShaderModule,
    device: &wgpu::Device,
) -> wgpu::RenderPipeline {
    let skybox_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Environment skybox pipeline layout"),
        bind_group_layouts: &[camera_bind_group_layout, cubemap_bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Environment skybox pipeline"),
        layout: Some(&skybox_pipeline_layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_cube_camera"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: VERTEX_BUFFERS_LAYOUT,
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
            module,
            entry_point: Some("fs_skybox"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
        }),
        multiview: None,
        cache: None,
    })
}

fn create_irradiance_pipeline(
    view_projection_bind_group_layout: &wgpu::BindGroupLayout,
    cubemap_bind_group_layout: &wgpu::BindGroupLayout,
    module: &wgpu::ShaderModule,
    device: &wgpu::Device,
) -> wgpu::RenderPipeline {
    let irradiance_pipeline_layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Environment irrandiance pipeline layout"),
            bind_group_layouts: &[view_projection_bind_group_layout, cubemap_bind_group_layout],
            push_constant_ranges: &[],
        });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Environment irradiance pipeline"),
        layout: Some(&irradiance_pipeline_layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_cube_view_projection"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: VERTEX_BUFFERS_LAYOUT,
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
            module,
            entry_point: Some("fs_irradiance"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
        }),
        multiview: None,
        cache: None,
    })
}

fn create_skybox_bind_group(
    skybox: &TextureCubeSampler,
    cubemap_bind_group_layout: &wgpu::BindGroupLayout,
    device: &wgpu::Device,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Skybox bind group"),
        layout: cubemap_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(skybox.texture.view()),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&skybox.sampler),
            },
        ],
    })
}

fn create_irradiance_map_bind_group(
    skybox_bind_group: &wgpu::BindGroup,
    cubemap_bind_group_layout: &wgpu::BindGroupLayout,
    cubemap_sampler: wgpu::Sampler,
    textures: &mut TextureManager,
    shader_module: &wgpu::ShaderModule,
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
) -> wgpu::BindGroup {
    let irradiance_pipeline = create_irradiance_pipeline(
        textures.view_projection_bind_group_layout(),
        &cubemap_bind_group_layout,
        shader_module,
        device,
    );
    let irradiance_map_texture = textures.create_cubemap_with_pipeline(
        Some("Irrandiance map texture"),
        IRRADIANCE_MAP_SIZE,
        IRRADIANCE_MAP_SIZE,
        wgpu::TextureFormat::Rgba16Float,
        &irradiance_pipeline,
        &skybox_bind_group,
        encoder,
    );

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Environment irradiance bind group"),
        layout: cubemap_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(irradiance_map_texture.view()),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&cubemap_sampler),
            },
        ],
    })
}

pub trait DrawEnvironment {
    fn draw_environment(
        &mut self,
        environment: &Environment,
        blur: bool,
        camera_bind_group: &wgpu::BindGroup,
    );
}

impl<'a> DrawEnvironment for wgpu::RenderPass<'a> {
    fn draw_environment(
        &mut self,
        environment: &Environment,
        blur: bool,
        camera_bind_group: &wgpu::BindGroup,
    ) {
        self.set_pipeline(&environment.skybox_pipeline);
        self.set_bind_group(0, Some(camera_bind_group), &[]);
        if blur {
            self.set_bind_group(1, Some(&environment.irradiance_map_bind_group), &[]);
        } else {
            self.set_bind_group(1, Some(&environment.skybox_bind_group), &[]);
        }
        self.set_vertex_buffer(0, environment.vertex_buffer.slice(..));
        self.set_index_buffer(
            environment.index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        self.draw_indexed(0..CUBE_INDICES.len() as u32, 0, 0..1);
    }
}
