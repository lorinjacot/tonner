use std::{
    f32::{consts::PI, INFINITY},
    ops::RangeInclusive,
};

use glam::{Mat4, Vec3};

use crate::storm::{
    storage::{SparseMap, SparseSet},
    Id,
};

use super::{Camera, Node, Projection};

pub struct Controls(pub Box<dyn ControlsTrait>);

pub trait ControlsTrait {
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
    /// The focus point of the controls, the camera orbits around this.
    /// It can be updated at any point to change the focus of the controls.
    target: Id<Node>,

    /// The focus point of the `target_radius` limits.
    /// It can be updated at any point to change the center of interest for the `target`.
    cursor: Id<Node>,

    /// The camera to be controlled. The camera must not be a child of another node,
    /// unless target and cursor are also children of this same parent node.
    camera: Id<Node>,

    /// Optional damping inertia. Default is `Some(0.05)`
    pub damping_factor: Option<f32>,

    /// Enable or disable camera panning. Default is `true`.
    pub enable_pan: bool,

    /// Enable or disable horizontal and vertical rotation of the camera. Default is `true`.
    ///
    /// Note that it is possible to disable a single axis by setting the start and end of the `polar_angle`
    /// or `azimuth_angle` ranges to the same value, which will cause the vertical or horizontal rotation
    /// to be fixed at that value.
    pub enable_rotate: bool,

    /// Enable or disable zooming of the camera.
    pub enable_zoom: bool,

    /// How far you can orbit horizontally. If set, the interval must be a sub-interval of `-2.0*PI..=2.0*PI`,
    /// with `end - start < 2.0*PI`. Default is `None`.
    pub azimuth_angle: Option<RangeInclusive<f32>>,

    /// How far you can orbit vertically. Max range is `0.0..PI`, and is the default.
    pub polar_angle: RangeInclusive<f32>,

    /// How far you can zoom out Default is `0.0..=INFINITY`.
    pub zoom: RangeInclusive<f32>,

    /// How close/far you can get the target to/from the cursor. Default is `0.0..=INFINITY`
    pub target_radius: RangeInclusive<f32>,

    /// Speed of panning. Default is `1.0`.
    pub pan_speed: f32,

    /// Speed of rotation. Default is `1.0`.
    pub rotate_speed: f32,

    /// Defines how the camera's position is translated when panning. If `true`, the camera pans
    /// in screen space. Otherwise, the camera pans in the plane orthogonal to the camera's up direction.
    /// Default is `true`.
    pub screen_space_panning: bool,

    delta_theta: f32,
    delta_phi: f32,
    pan_offset: Vec3,
}

impl OrbitControls {
    pub fn new(
        target: Id<Node>,
        cursor: Id<Node>,
        camera: Id<Node>,
        nodes: &SparseSet<Node>,
    ) -> Self {
        assert_eq!(
            nodes[target].parent, nodes[cursor].parent,
            "The target, cursor and camera must share the same parent"
        );
        assert_eq!(
            nodes[target].parent, nodes[camera].parent,
            "The target, cursor and camera must share the same parent"
        );
        OrbitControls {
            target,
            camera,
            cursor,
            damping_factor: Some(0.05),
            enable_pan: true,
            enable_rotate: true,
            enable_zoom: true,
            azimuth_angle: None,
            polar_angle: 0.0..=PI,
            zoom: 0.0..=INFINITY,
            target_radius: 0.0..=INFINITY,
            pan_speed: 1.0,
            rotate_speed: 1.0,
            screen_space_panning: true,
            delta_theta: 0.0,
            delta_phi: 0.0,
            pan_offset: Vec3::ZERO,
        }
    }

    fn pan_left(&mut self, distance: f32, camera_local_matrix: Mat4) {
        self.pan_offset -= distance * camera_local_matrix.x_axis.truncate();
    }

    fn pan_up(&mut self, distance: f32, camera_local_matrix: Mat4) {
        self.pan_offset += distance
            * if self.screen_space_panning {
                camera_local_matrix.y_axis.truncate()
            } else {
                Vec3::Y.cross(camera_local_matrix.x_axis.truncate())
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
        if self.enable_rotate && inputs.pointer.primary_down() {
            let delta = 2.0 * PI * inputs.pointer.delta() * self.rotate_speed / viewport_size.y;
            self.delta_theta -= delta.x;
            self.delta_phi -= delta.y;
        } else if self.enable_pan && inputs.pointer.secondary_down() {
            match cameras[self.camera].projection {
                Projection::Perspective { y_fov, .. } => {
                    // perspective
                    let camera = &nodes[self.camera];
                    let position = camera.local_position();
                    let target = nodes[self.target].local_position();
                    let offset = position - target;
                    let mut target_distance = offset.length();

                    // half of the fov is center to top of screen
                    target_distance *= (y_fov / 2.0).tan();

                    // we use only viewport_size.y here so aspect ratio does not distort speed
                    let factor = 2.0 * target_distance / viewport_size.y;
                    let delta = self.pan_speed * inputs.pointer.delta();
                    let matrix = camera.local_matrix();
                    self.pan_left(factor * delta.x, matrix);
                    self.pan_up(factor * delta.y, matrix);
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

        // Limit the target distance from the cursor to create a sphere around the center of interest
        let cursor = nodes[self.cursor].local_position();
        target -= cursor;
        target = target.clamp_length(*self.target_radius.start(), *self.target_radius.end());
        target += cursor;

        offset.z = radius * phi.sin() * theta.cos();
        offset.x = radius * phi.sin() * theta.sin();
        offset.y = radius * phi.cos();

        let camera = target + offset;

        nodes.set_local_position(self.target, target);
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
