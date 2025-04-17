use std::f32::consts::PI;

use glam::{Mat4, Vec3};

use crate::storm::{
    storage::{SparseMap, SparseSet},
    Id,
};

use super::{Camera, Node};

pub struct Controls(Box<dyn ControlsTrait>);

impl Controls {
    pub(super) fn handle_inputs(
        &mut self,
        inputs: &egui::InputState,
        viewport_size: egui::Vec2,
        nodes: &mut SparseSet<Node>,
        cameras: &mut SparseMap<Node, Camera>,
    ) {
        self.0.handle_inputs(inputs, viewport_size, nodes, cameras);
    }
}

trait ControlsTrait {
    fn handle_inputs(
        &mut self,
        inputs: &egui::InputState,
        viewport_size: egui::Vec2,
        nodes: &mut SparseSet<Node>,
        cameras: &mut SparseMap<Node, Camera>,
    );
}

pub struct OrbitControls {
    target: Id<Node>,
    camera: Id<Node>,
    rotation_speed: f32,
}

impl OrbitControls {
    pub fn new(target: Id<Node>, camera: Id<Node>) -> Self {
        OrbitControls {
            target,
            camera,
            rotation_speed: 1.0,
        }
    }
}

impl ControlsTrait for OrbitControls {
    fn handle_inputs(
        &mut self,
        inputs: &egui::InputState,
        viewport_size: egui::Vec2,
        nodes: &mut SparseSet<Node>,
        _cameras: &mut SparseMap<Node, Camera>,
    ) {
        if inputs.pointer.primary_down() {
            let delta = -2.0 * PI * inputs.pointer.delta() * self.rotation_speed / viewport_size.y;

            let camera = nodes[self.camera]
                .global_transform
                .project_point3(Vec3::ZERO);
            let target = nodes[self.target]
                .global_transform
                .project_point3(Vec3::ZERO);
            let mut offset = camera - target;

            let radius = offset.length();
            let mut theta = offset.x.atan2(offset.z);
            let mut phi = (offset.y / radius).acos();

            theta += delta.x;
            phi += delta.y;

            offset.z = radius * phi.sin() * theta.cos();
            offset.x = radius * phi.sin() * theta.sin();
            offset.y = radius * phi.cos();

            let camera = target + offset;

            nodes.set_global_transform(
                self.camera,
                Mat4::look_at_rh(camera, target, Vec3::Y).inverse(),
            );
        }
    }
}

impl Into<Controls> for OrbitControls {
    fn into(self) -> Controls {
        Controls(Box::new(self))
    }
}
