use std::f32::consts::PI;

use glam::{Mat4, Vec3};

use crate::storm::{
    storage::{SparseMap, SparseSet},
    Id,
};

use super::{Camera, Node};

pub struct Controls(pub(super) Box<dyn ControlsTrait>);

pub(super) trait ControlsTrait {
    fn take_input(&mut self, inputs: &mut egui::InputState, viewport_size: egui::Vec2);

    fn update(&mut self, nodes: &mut SparseSet<Node>, cameras: &mut SparseMap<Node, Camera>);
}

pub struct OrbitControls {
    target: Id<Node>,
    camera: Id<Node>,
    delta_theta: f32,
    delta_phi: f32,
    rotation_speed: f32,
    damping_factor: Option<f32>,
}

impl OrbitControls {
    pub fn new(target: Id<Node>, camera: Id<Node>) -> Self {
        OrbitControls {
            target,
            camera,
            delta_theta: 0.0,
            delta_phi: 0.0,
            rotation_speed: 1.0,
            damping_factor: Some(0.05),
        }
    }
}

impl ControlsTrait for OrbitControls {
    fn take_input(&mut self, inputs: &mut egui::InputState, viewport_size: egui::Vec2) {
        if inputs.pointer.primary_down() {
            let delta = 2.0 * PI * inputs.pointer.delta() * self.rotation_speed / viewport_size.y;
            self.delta_theta -= delta.x;
            self.delta_phi -= delta.y;
        }
    }

    fn update(&mut self, nodes: &mut SparseSet<Node>, _cameras: &mut SparseMap<Node, Camera>) {
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

        if let Some(damping_factor) = self.damping_factor {
            theta += self.delta_theta * damping_factor;
            phi += self.delta_phi * damping_factor;

            self.delta_theta *= 1.0 - damping_factor;
            self.delta_phi *= 1.0 - damping_factor;
        } else {
            theta += self.delta_theta;
            phi += self.delta_phi;

            self.delta_theta = 0.0;
            self.delta_phi = 0.0;
        }

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

impl Into<Controls> for OrbitControls {
    fn into(self) -> Controls {
        Controls(Box::new(self))
    }
}
