use std::time::Duration;

use super::scene::Node;

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
    fn update(&mut self, delta_time: Duration, camera: &mut dyn Camera, node: &mut Node) {
        
    }
}
