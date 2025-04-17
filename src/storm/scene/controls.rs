use std::{f32::consts::PI, ops::RangeInclusive};

use glam::{Mat4, Vec3};

use crate::storm::{
    storage::{SparseMap, SparseSet},
    Id,
};

use super::{Camera, Node, Projection};

pub struct Controls(pub(super) Box<dyn ControlsTrait>);

pub(super) trait ControlsTrait {
    fn take_input(
        &mut self,
        inputs: &mut egui::InputState,
        viewport_size: egui::Vec2,
        nodes: &SparseSet<Node>,
        cameras: &SparseMap<Node, Camera>,
    );

    fn update(&mut self, nodes: &mut SparseSet<Node>, cameras: &mut SparseMap<Node, Camera>);
}

/// Adapted from three.js OrbitControls
pub struct OrbitControls {
    target: Id<Node>,
    camera: Id<Node>,
    delta_theta: f32,
    delta_phi: f32,
    pan_offset: Vec3,
    polar_angle: RangeInclusive<f32>,
    azimuth_angle: Option<RangeInclusive<f32>>,
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
            polar_angle: 0.0..=PI,
            pan_offset: Vec3::ZERO,
            azimuth_angle: None,
            rotation_speed: 1.0,
            damping_factor: Some(0.05),
        }
    }
}

impl ControlsTrait for OrbitControls {
    fn take_input(
        &mut self,
        inputs: &mut egui::InputState,
        viewport_size: egui::Vec2,
        nodes: &SparseSet<Node>,
        cameras: &SparseMap<Node, Camera>,
    ) {
        if inputs.pointer.primary_down() {
            let delta = 2.0 * PI * inputs.pointer.delta() * self.rotation_speed / viewport_size.y;
            self.delta_theta -= delta.x;
            self.delta_phi -= delta.y;
        } else if inputs.pointer.secondary_down() {
            match cameras[self.camera].projection {
                Projection::Perspective { .. } => {
                    // let position = nodes[self.camera].local_position();
                    todo!()
                }
                Projection::Orthographic { .. } => {
                    todo!()
                }
            }
        }
    }

    fn update(&mut self, nodes: &mut SparseSet<Node>, _cameras: &mut SparseMap<Node, Camera>) {
        let camera = nodes[self.camera].local_position();
        let mut target = nodes[self.target].local_position();
        let mut offset = camera - target;

        let radius = offset.length();
        let mut theta = offset.x.atan2(offset.z);
        let mut phi = (offset.y / radius).acos();

        if let Some(damping_factor) = self.damping_factor {
            theta += self.delta_theta * damping_factor;
            phi += self.delta_phi * damping_factor;
            target += self.pan_offset * damping_factor;

            self.delta_theta *= 1.0 - damping_factor;
            self.delta_phi *= 1.0 - damping_factor;
            self.pan_offset *= 1.0 - damping_factor;
        } else {
            theta += self.delta_theta;
            phi += self.delta_phi;
            target += self.pan_offset;

            self.delta_theta = 0.0;
            self.delta_phi = 0.0;
            self.pan_offset = Vec3::ZERO;
        }

        if let Some(azimuth_angle) = &self.azimuth_angle {
            let mut min = *azimuth_angle.start();
            let mut max = *azimuth_angle.end();

            if min < -PI {
                min += 2.0 * PI
            } else if min > PI {
                min -= 2.0 * PI
            }

            if max < -PI {
                max += 2.0 * PI
            } else if max > PI {
                max -= 2.0 * PI
            }

            theta = if min <= max {
                theta.clamp(min, max)
            } else if theta > (min + max) / 2.0 {
                dbg!(theta, (min + max) / 2.0);
                min.max(theta)
            } else {
                dbg!(theta, (min + max) / 2.0);
                max.min(theta)
            }
        }

        phi = phi
            .clamp(*self.polar_angle.start(), *self.polar_angle.end())
            .clamp(0.000001, PI - 0.000001); // make safe

        offset.z = radius * phi.sin() * theta.cos();
        offset.x = radius * phi.sin() * theta.sin();
        offset.y = radius * phi.cos();

        let camera = target + offset;

        nodes.set_world_matrix(
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
