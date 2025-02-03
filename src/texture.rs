use image::EncodableLayout;
use thiserror::Error;
use wgpu::util::DeviceExt;

pub struct TextureManager {}

impl TextureManager {
    pub fn new(_device: &wgpu::Device, _queue: &wgpu::Queue) -> Self {
        Self {}
    }

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
                image::ColorType::Rgb8 => bytes.extend_from_slice(face.to_rgba8().as_bytes()),
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
