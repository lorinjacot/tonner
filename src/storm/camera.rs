use glam::Mat4;

use super::{
    storage::{Id, SparseSet},
    Asset,
};

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

    pub fn create_camera(&mut self, camera: gltf::Camera) -> Id<Camera> {
        let projection = match camera.projection() {
            gltf::camera::Projection::Orthographic(projection) => {
                let f = projection.zfar();
                let n = projection.znear();

                Mat4::from_cols_array_2d(&[
                    [1.0 / projection.xmag(), 0.0, 0.0, 0.0],
                    [0.0, 1.0 / projection.ymag(), 0.0, 0.0],
                    [0.0, 0.0, 2.0 / (n - f), 0.0],
                    [0.0, 0.0, (f + n) / (n - f), 0.0],
                ])
            }
            gltf::camera::Projection::Perspective(projection) => {
                let a = projection
                    .aspect_ratio()
                    .unwrap_or(self.viewport_aspect_ratio);
                let tan_y = (0.5 * projection.yfov()).tan();
                let n = projection.znear();
                let (zz, zw) = match projection.zfar() {
                    Some(f) => ((f + n) / (n - f), (2.0 * f * n) / (n - f)),
                    None => (-1.0, -2.0 * n),
                };
                Mat4::from_cols_array_2d(&[
                    [1.0 / (a * tan_y), 0.0, 0.0, 0.0],
                    [0.0, 1.0 / a, 0.0, 0.0],
                    [0.0, 0.0, zz, -1.0],
                    [0.0, 0.0, zw, 0.0],
                ])
            }
        };

        self.cameras.push(Camera { projection })
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }
}

pub struct Camera {
    projection: Mat4,
}

// pub trait Camera {}

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
