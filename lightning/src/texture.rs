use std::{borrow::Borrow, f32::consts::FRAC_PI_2};

use bytemuck::cast_slice;
use glam::{vec2, vec3, Mat4, Vec2, Vec3};
use image::EncodableLayout;
use thiserror::Error;
use wgpu::util::DeviceExt;

#[rustfmt::skip]
pub const TRIANGLE_VERTICES: &[Vec2] = &[
    // positions           // texCoords
    vec2(-1.0,  3.0), vec2(0.0, 2.0),
    vec2(-1.0, -1.0), vec2(0.0, 0.0),
    vec2( 3.0, -1.0), vec2(2.0, 0.0),
];

pub const TRIANGLE_VERTEX_BUFFER_LAYOUT: &[wgpu::VertexBufferLayout] =
    &[wgpu::VertexBufferLayout {
        array_stride: 2 * size_of::<Vec2>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
        ],
    }];

pub const CUBE_VERTICES: &[Vec3] = &[
    // front face
    vec3(-1.0, 1.0, 1.0),
    vec3(-1.0, -1.0, 1.0),
    vec3(1.0, 1.0, 1.0),
    vec3(1.0, -1.0, 1.0),
    // right face
    vec3(1.0, 1.0, -1.0),
    vec3(1.0, 1.0, 1.0),
    vec3(1.0, -1.0, -1.0),
    vec3(1.0, -1.0, 1.0),
    // back face
    vec3(1.0, 1.0, -1.0),
    vec3(1.0, -1.0, -1.0),
    vec3(-1.0, 1.0, -1.0),
    vec3(-1.0, -1.0, -1.0),
    // left face
    vec3(-1.0, 1.0, 1.0),
    vec3(-1.0, 1.0, -1.0),
    vec3(-1.0, -1.0, 1.0),
    vec3(-1.0, -1.0, -1.0),
    // bottom face
    vec3(1.0, -1.0, 1.0),
    vec3(-1.0, -1.0, 1.0),
    vec3(1.0, -1.0, -1.0),
    vec3(-1.0, -1.0, -1.0),
    // top face
    vec3(-1.0, 1.0, 1.0),
    vec3(1.0, 1.0, 1.0),
    vec3(-1.0, 1.0, -1.0),
    vec3(1.0, 1.0, -1.0),
];

pub const CUBE_INDICES: &[u16] = &[
    0, 1, 2, 2, 1, 3, // front
    4, 5, 6, 6, 5, 7, // right
    8, 9, 10, 10, 9, 11, // back
    12, 13, 14, 14, 13, 15, // left
    16, 17, 18, 18, 17, 19, // bottom
    20, 21, 22, 22, 21, 23, // top
];

pub const CUBE_VERTEX_BUFFER_LAYOUT: &[wgpu::VertexBufferLayout] = &[wgpu::VertexBufferLayout {
    array_stride: size_of::<Vec3>() as u64,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &[wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0,
    }],
}];

pub struct TextureManager {
    device: wgpu::Device,
    queue: wgpu::Queue,
    shader_module: wgpu::ShaderModule,
    view_projection_bind_group_layout: wgpu::BindGroupLayout,
    equirectangular_bind_group_layout: wgpu::BindGroupLayout,
    equirectangular_to_cube_pipeline_layout: wgpu::PipelineLayout,
    triangle_vertex_buffer: wgpu::Buffer,
    cube_vertex_buffer: wgpu::Buffer,
    cube_index_buffer: wgpu::Buffer,
    view_projection_bind_groups: [wgpu::BindGroup; 6],
}

impl TextureManager {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let shader_module = device.create_shader_module(wgpu::include_wgsl!("texture.wgsl"));

        let view_projection_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture manager view projection bind group layout"),
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

        let equirectangular_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture manager equirectangular bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                ],
            });

        let equirectangular_to_cube_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Textures manager equirectangular to cube pipeline layout"),
                bind_group_layouts: &[
                    &view_projection_bind_group_layout,
                    &equirectangular_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let projection = Mat4::perspective_rh(FRAC_PI_2, 1.0, 0.1, 10.0);
        let create_bind_group = |view| {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Texture manager view projection buffer"),
                contents: bytemuck::cast_slice(&[projection * view]),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Texture manager view projection bind group"),
                layout: &view_projection_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            })
        };

        let triangle_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Texture manager triangle vertex buffer"),
            contents: cast_slice(&TRIANGLE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let cube_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Texture manager cube vertex buffer"),
            contents: bytemuck::cast_slice(&CUBE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let cube_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Texture manager cube index buffer"),
            contents: bytemuck::cast_slice(&CUBE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let view_projection_bind_groups = [
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, Vec3::X, Vec3::Y)),
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, -Vec3::X, Vec3::Y)),
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, Vec3::Y, Vec3::Z)),
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, -Vec3::Y, -Vec3::Z)),
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, -Vec3::Z, Vec3::Y)), // the z-axis of wgpu is our -z
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, Vec3::Z, Vec3::Y)),
        ];

        Self {
            device,
            queue,
            shader_module,
            view_projection_bind_group_layout,
            equirectangular_bind_group_layout,
            equirectangular_to_cube_pipeline_layout,
            triangle_vertex_buffer,
            cube_vertex_buffer,
            cube_index_buffer,
            view_projection_bind_groups,
        }
    }

    pub fn view_projection_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.view_projection_bind_group_layout
    }

    pub fn cube_vertex_buffer(&self) -> &wgpu::Buffer {
        &self.cube_vertex_buffer
    }

    pub fn cube_index_buffer(&self) -> &wgpu::Buffer {
        &self.cube_index_buffer
    }

    pub fn create_from_pixels(
        &self,
        label: Option<&str>,
        usage: wgpu::TextureUsages,
        width: u32,
        height: u32,
        pixels: &[u8],
        format: wgpu::TextureFormat,
    ) -> Texture2d {
        let texture = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::MipMajor,
            pixels,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Texture2d { view }
    }

    pub fn create_from_image(
        &self,
        label: Option<&str>,
        usage: wgpu::TextureUsages,
        image: &image::DynamicImage,
        is_srgb: bool,
    ) -> Result<Texture2d, TextureCreationError> {
        let create_texture = |pixels: &[u8], format: wgpu::TextureFormat| {
            self.create_from_pixels(
                label,
                usage,
                image.width(),
                image.height(),
                pixels,
                if is_srgb {
                    format.add_srgb_suffix()
                } else {
                    format
                },
            )
        };

        Ok(match image.color() {
            image::ColorType::Rgb8 => {
                // rgb => rgba conversion needed
                create_texture(image.to_rgba8().as_bytes(), wgpu::TextureFormat::Rgba8Unorm)
            }
            image::ColorType::Rgba8 => {
                create_texture(image.as_bytes(), wgpu::TextureFormat::Rgba8Unorm)
            }
            image::ColorType::Rgb32F => create_texture(
                image.to_rgba32f().as_bytes(),
                wgpu::TextureFormat::Rgba32Float,
            ),
            _ => return Err(TextureCreationError::UnsupportedColorType(image.color())),
        })
    }

    pub fn create_with_pipeline(
        &self,
        label: Option<&str>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        pipeline: &wgpu::RenderPipeline,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Texture2d {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let texture = Texture2d { view };

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: label
                .map(|label| format!("Create {label} render pass"))
                .as_deref(),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: texture.view(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rpass.set_pipeline(pipeline);
        rpass.set_vertex_buffer(0, self.triangle_vertex_buffer.slice(..));
        rpass.draw(0..3 as u32, 0..1);

        texture
    }

    pub fn create_cube_from_equirectangular(
        &mut self,
        label: Option<&str>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        equirectangular: &Texture2dSampler,
        encoder: &mut wgpu::CommandEncoder,
    ) -> TextureCube {
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Equirectangular to cube pipeline"),
                layout: Some(&self.equirectangular_to_cube_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.shader_module,
                    entry_point: Some("vs_cube"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: CUBE_VERTEX_BUFFER_LAYOUT,
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
                    module: &self.shader_module,
                    entry_point: Some("fs_equirectangular_to_cube"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(format.into())],
                }),
                multiview: None,
                cache: None,
            });

        let equirectangular_bind_group =
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("equirectangular bind group"),
                layout: &self.equirectangular_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            equirectangular.texture.view(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&equirectangular.sampler),
                    },
                ],
            });

        self.create_cube_with_pipeline(
            label,
            width,
            height,
            format,
            &pipeline,
            &equirectangular_bind_group,
            encoder,
        )
    }

    pub fn create_cube_from_faces(
        &mut self,
        label: Option<&str>,
        faces: &[image::DynamicImage; 6],
        is_srgb: bool,
        usage: wgpu::TextureUsages,
    ) -> Result<TextureCube, TextureCreationError> {
        let color_type = faces[0].color();

        let (mut format, channel_count) = match color_type {
            image::ColorType::Rgb8 => (wgpu::TextureFormat::Rgba8Unorm, 4), // rgb => rgba conversion needed
            image::ColorType::Rgba8 => (wgpu::TextureFormat::Rgba8Unorm, 4),
            _ => return Err(TextureCreationError::UnsupportedColorType(color_type)),
        };
        if is_srgb {
            format = format.add_srgb_suffix();
        }

        let width = faces[0].width();
        let height = faces[0].height();
        let mut bytes = Vec::with_capacity(6 * width as usize * height as usize * channel_count);
        for face in faces {
            if face.color() != color_type || face.width() != width || face.height() != height {
                return Err(TextureCreationError::InvalidSource(
                    "All faces must have same format and dimensions".to_string(),
                ));
            }
            match color_type {
                image::ColorType::Rgb8 => {
                    profiling::scope!("rgb to rgba");
                    bytes.extend_from_slice(face.to_rgba8().as_bytes())
                }
                _ => bytes.extend_from_slice(face.as_bytes()),
            };
        }

        let texture = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 6,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::MipMajor,
            &bytes,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label,
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        Ok(TextureCube { view })
    }

    pub fn create_cube_with_pipeline(
        &mut self,
        label: Option<&str>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        pipeline: &wgpu::RenderPipeline,
        source_bind_group: &wgpu::BindGroup,
        encoder: &mut wgpu::CommandEncoder,
    ) -> TextureCube {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        for base_array_layer in 0..6 {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("create cubemap pipeline"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture.create_view(&wgpu::TextureViewDescriptor {
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        base_array_layer,
                        array_layer_count: Some(1),
                        ..Default::default()
                    }),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(
                0,
                &self.view_projection_bind_groups[base_array_layer as usize],
                &[],
            );
            render_pass.set_bind_group(1, source_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.cube_vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(self.cube_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..CUBE_INDICES.len() as u32, 0, 0..1);
        }

        TextureCube {
            view: texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            }),
        }
    }

    pub fn create_cube_mip<A>(
        &mut self,
        label: Option<&str>,
        width: u32,
        height: u32,
        mip_level_count: u32,
        format: wgpu::TextureFormat,
        pipeline: &wgpu::RenderPipeline,
        source_bind_group: impl Fn(u32) -> A,
        encoder: &mut wgpu::CommandEncoder,
    ) -> TextureCube
    where
        A: Borrow<wgpu::BindGroup>,
    {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 6,
            },
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        for mip_level in 0..mip_level_count {
            for array_layer in 0..6 {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("create cubemap pipeline"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &texture.create_view(&wgpu::TextureViewDescriptor {
                            dimension: Some(wgpu::TextureViewDimension::D2),
                            base_array_layer: array_layer,
                            array_layer_count: Some(1),
                            base_mip_level: mip_level,
                            mip_level_count: Some(1),
                            ..Default::default()
                        }),
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                render_pass.set_pipeline(pipeline);
                render_pass.set_bind_group(
                    0,
                    &self.view_projection_bind_groups[array_layer as usize],
                    &[],
                );
                render_pass.set_bind_group(1, source_bind_group(mip_level).borrow(), &[]);
                render_pass.set_vertex_buffer(0, self.cube_vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(self.cube_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..CUBE_INDICES.len() as u32, 0, 0..1);
            }
        }

        TextureCube {
            view: texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            }),
        }
    }
}

pub struct Texture2d {
    view: wgpu::TextureView,
}

impl Texture2d {
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

pub struct Texture2dSampler {
    pub texture: Texture2d,
    pub sampler: wgpu::Sampler,
}

pub struct TextureCube {
    view: wgpu::TextureView,
}

impl TextureCube {
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

pub struct TextureCubeSampler {
    pub texture: TextureCube,
    pub sampler: wgpu::Sampler,
}

pub struct Texture2dDescriptor<'a> {
    pub label: Option<&'a str>,
    pub usage: wgpu::TextureUsages,
    pub source: Texture2dSource<'a>,
}

pub enum Texture2dSource<'a> {
    Pixel {
        width: u32,
        height: u32,
        pixels: &'a [u8],
        format: wgpu::TextureFormat,
    },
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TextureCreationError {
    #[error("image color type {0:?} is not supported")]
    UnsupportedColorType(image::ColorType),
    #[error("invalid source: {0}")]
    InvalidSource(String),
}
