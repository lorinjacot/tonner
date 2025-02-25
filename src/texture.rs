use std::f32::consts::FRAC_PI_2;

use glam::{vec3, Mat4, Vec3};
use image::EncodableLayout;
use thiserror::Error;
use wgpu::util::DeviceExt;

const VERTICES: &[Vec3] = &[
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

const INDICES: &[u16] = &[
    0, 1, 2, 2, 1, 3, // front
    4, 5, 6, 6, 5, 7, // right
    8, 9, 10, 10, 9, 11, // back
    12, 13, 14, 14, 13, 15, // left
    16, 17, 18, 18, 17, 19, // bottom
    20, 21, 22, 22, 21, 23, // top
];

pub struct TextureManager {
    device: wgpu::Device,
    queue: wgpu::Queue,
    view_projection_bind_group_layout: wgpu::BindGroupLayout,
    view_projection_bind_groups: [wgpu::BindGroup; 6],
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

impl TextureManager {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let projection = Mat4::perspective_rh(FRAC_PI_2, 1.0, 0.1, 10.0);
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
        let view_projection_bind_groups = [
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, Vec3::X, Vec3::Y)),
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, -Vec3::X, Vec3::Y)),
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, Vec3::Y, Vec3::Z)),
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, -Vec3::Y, -Vec3::Z)),
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, -Vec3::Z, Vec3::Y)), // the z-axis of wgpu is our -z
            create_bind_group(Mat4::look_to_rh(Vec3::ZERO, Vec3::Z, Vec3::Y)),
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Texture manager vertex buffer"),
            contents: bytemuck::cast_slice(&VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Texture manager index buffer"),
            contents: bytemuck::cast_slice(&INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            device,
            queue,
            view_projection_bind_group_layout,
            view_projection_bind_groups,
            vertex_buffer,
            index_buffer,
        }
    }

    pub fn view_projection_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.view_projection_bind_group_layout
    }

    #[profiling::function]
    pub fn create_texture_cube_from_faces(
        &mut self,
        label: Option<&str>,
        faces: &[image::DynamicImage; 6],
        is_srgb: bool,
        usage: wgpu::TextureUsages,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
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

        let texture = device.create_texture_with_data(
            queue,
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

    #[profiling::function]
    pub fn create_cubemap_with_pipeline(
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
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..INDICES.len() as u32, 0, 0..1);
        }

        TextureCube {
            view: texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            }),
        }
    }
}

// pub struct Texture2d {
//     texture: wgpu::Texture,
//     view: wgpu::TextureView,
// }

pub struct TextureCube {
    view: wgpu::TextureView,
}

impl TextureCube {
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TextureCreationError {
    #[error("image color type {0:?} is not supported")]
    UnsupportedColorType(image::ColorType),
    #[error("invalid source: {0}")]
    InvalidSource(String),
}
