use std::time::Duration;

use super::{
    scene::Node,
    storage::{SparseMap, SparseSet},
    Asset,
};

pub struct CameraManager {
    cameras: SparseSet<Box<dyn Camera>>,
    assets: SparseMap<Asset, Vec<Option<Box<dyn Camera>>>>,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl CameraManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let cameras = SparseSet::new();
        let assets = SparseMap::new();

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
            assets,
            bind_group_layout,
        }
    }

    

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }
}

pub trait Camera {}

pub struct PerspectiveCamera {}

impl PerspectiveCamera {
    pub fn new(fov: f32, aspect: f32, near: f32, far: f32) -> Self {
        PerspectiveCamera {}
    }
}

impl Camera for PerspectiveCamera {}

pub trait Controls {
    fn keyboard_input(&mut self, _event: &winit::event::KeyEvent) -> bool {
        false
    }

    fn mouse_input(
        &mut self,
        _state: &winit::event::ElementState,
        _button: &winit::event::MouseButton,
    ) -> bool {
        false
    }

    fn mouse_motion(&mut self, _delta: &(f64, f64)) -> bool {
        false
    }

    fn update(&mut self, delta_time: Duration, camera: &mut dyn Camera, node: &mut Node);
}

pub struct OrbitControls {}

impl OrbitControls {
    pub fn new() -> Self {
        OrbitControls {}
    }
}

impl Controls for OrbitControls {
    fn update(&mut self, delta_time: Duration, camera: &mut dyn Camera, node: &mut Node) {}
}
