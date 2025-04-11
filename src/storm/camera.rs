use std::ops::Index;

use bytemuck::{Pod, Zeroable};
use glam::Mat4;

use super::storage::{Id, SparseSet};

pub struct CameraManager {
    cameras: SparseSet<Camera>,
    bind_group_layout: wgpu::BindGroupLayout,
    viewport_aspect_ratio: f32,
}

impl CameraManager {
    pub fn new(viewport_aspect_ratio: f32, device: &wgpu::Device) -> Self {
        let cameras = SparseSet::new();

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        Self {
            cameras,
            bind_group_layout,
            viewport_aspect_ratio,
        }
    }

    pub fn create_camera(&mut self, camera: gltf::Camera, device: &wgpu::Device) -> Id<Camera> {
        let projection = Projection::from(camera.projection());
        let matrix = projection.matrix(self.viewport_aspect_ratio);

        self.cameras.push(Camera { projection, matrix })
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }
}

impl Index<Id<Camera>> for CameraManager {
    type Output = Camera;

    fn index(&self, index: Id<Camera>) -> &Self::Output {
        &self.cameras[index]
    }
}

pub struct Camera {
    projection: Projection,
    matrix: Mat4,
}

impl Camera {
    pub fn projection_matrix(&self) -> Mat4 {
        self.matrix
    }
}

enum Projection {
    Orthographic {
        xmag: f32,
        ymag: f32,
        zfar: f32,
        znear: f32,
    },
    Perspective {
        aspect_ration: Option<f32>,
        yfov: f32,
        zfar: Option<f32>,
        znear: f32,
    },
}

impl Projection {
    fn matrix(&self, viewport_aspect_ratio: f32) -> Mat4 {
        match self {
            Projection::Orthographic {
                xmag,
                ymag,
                zfar,
                znear,
            } => Mat4::from_cols_array_2d(&[
                [1.0 / xmag, 0.0, 0.0, 0.0],
                [0.0, 1.0 / ymag, 0.0, 0.0],
                [0.0, 0.0, 2.0 / (znear - zfar), 0.0],
                [0.0, 0.0, (zfar + znear) / (znear - zfar), 0.0],
            ]),
            Projection::Perspective {
                aspect_ration,
                yfov,
                zfar,
                znear,
            } => {
                let a = aspect_ration.unwrap_or(viewport_aspect_ratio);
                let tan_y = (0.5 * yfov).tan();
                let (zz, zw) = match zfar {
                    Some(zfar) => (
                        (zfar + znear) / (znear - zfar),
                        (2.0 * zfar * znear) / (znear - zfar),
                    ),
                    None => (-1.0, -2.0 * znear),
                };
                Mat4::from_cols_array_2d(&[
                    [1.0 / (a * tan_y), 0.0, 0.0, 0.0],
                    [0.0, 1.0 / a, 0.0, 0.0],
                    [0.0, 0.0, zz, -1.0],
                    [0.0, 0.0, zw, 0.0],
                ])
            }
        }
    }
}

impl<'a> From<gltf::camera::Projection<'a>> for Projection {
    fn from(value: gltf::camera::Projection) -> Self {
        match value {
            gltf::camera::Projection::Orthographic(ortho) => Self::Orthographic {
                xmag: ortho.xmag(),
                ymag: ortho.ymag(),
                zfar: ortho.zfar(),
                znear: ortho.znear(),
            },
            gltf::camera::Projection::Perspective(pers) => Self::Perspective {
                aspect_ration: pers.aspect_ratio(),
                yfov: pers.yfov(),
                zfar: pers.zfar(),
                znear: pers.znear(),
            },
        }
    }
}

// pub struct PerspectiveCamera {}

// impl PerspectiveCamera {
//     pub fn new(fov: f32, aspect: f32, near: f32, far: f32) -> Self {
//         PerspectiveCamera {}
//     }
// }

// impl Camera for PerspectiveCamera {}

// pub trait Controls {
//     fn keyboard_input(&mut self, _event: &winit::event::KeyEvent) -> bool {
//         false
//     }

//     fn mouse_input(
//         &mut self,
//         _state: &winit::event::ElementState,
//         _button: &winit::event::MouseButton,
//     ) -> bool {
//         false
//     }

//     fn mouse_motion(&mut self, _delta: &(f64, f64)) -> bool {
//         false
//     }

//     fn update(&mut self, delta_time: Duration, camera: &mut dyn Camera, node: &mut Node);
// }

// pub struct OrbitControls {}

// impl OrbitControls {
//     pub fn new() -> Self {
//         OrbitControls {}
//     }
// }

// impl Controls for OrbitControls {
//     fn update(&mut self, delta_time: Duration, camera: &mut dyn Camera, node: &mut Node) {}
// }
