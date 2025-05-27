use std::{
    f32::{INFINITY, consts::PI},
    ops::RangeInclusive,
};

use storm::{Id, math::{Plane, Ray}};
use glam::{Mat4, Vec2, Vec3};

use crate::scene::{Node, Scene, camera::Projection};

pub trait Controls {
    fn take_input(
        &mut self,
        inputs: &mut egui::InputState,
        viewport_size: egui::Vec2,
        scene: &Scene,
    );

    fn update(&mut self, viewport_aspect_ratio: f32, scene: &mut Scene);
}

/// Adapted from three.js OrbitControls
pub struct OrbitControls {
    scene: usize,

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

    /// How near/far you can dolly out ( `Projection::Perspective` only ). Default is `0.0..=INFINITY`.
    pub distance: RangeInclusive<f32>,

    /// How far you can orbit vertically. Max range is `0.0..PI`, and is the default.
    pub polar_angle: RangeInclusive<f32>,

    /// How far you can zoom out ( `Projection::Orthographic` only ). Default is `0.0..=INFINITY`.
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

    ///  Speed of zooming / dollying. Default is `1`.
    pub zoom_speed: f32,

    /// Setting this property to true allows to zoom to the cursor's position. Default is `false`.
    pub zoom_to_cursor: bool,

    delta_theta: f32,
    delta_phi: f32,
    pan_offset: Vec3,
    perform_cursor_zoom: bool,
    scale: f32,
    zoom_direction: Vec3,
    mouse: Vec2,
}

impl OrbitControls {
    pub fn new(
        scene_index: usize,
        scene: &Scene,
        target: Id<Node>,
        cursor: Id<Node>,
        camera: Id<Node>,
    ) -> Self {
        assert_eq!(
            scene[target].parent(),
            scene[cursor].parent(),
            "The target, cursor and camera must share the same parent"
        );
        assert_eq!(
            scene[target].parent(),
            scene[camera].parent(),
            "The target, cursor and camera must share the same parent"
        );
        OrbitControls {
            scene: scene_index,
            target,
            camera,
            cursor,
            damping_factor: Some(0.05),
            enable_pan: true,
            enable_rotate: true,
            enable_zoom: true,
            azimuth_angle: None,
            distance: 0.0..=INFINITY,
            polar_angle: 0.0..=PI,
            zoom: 0.0..=INFINITY,
            target_radius: 0.0..=INFINITY,
            pan_speed: 1.0,
            rotate_speed: 1.0,
            screen_space_panning: true,
            zoom_speed: 1.0,
            zoom_to_cursor: false,
            delta_theta: 0.0,
            delta_phi: 0.0,
            pan_offset: Vec3::ZERO,
            perform_cursor_zoom: false,
            scale: 1.0,
            zoom_direction: Vec3::Z,
            mouse: Vec2::ZERO,
        }
    }

    pub fn scene(&self) -> usize {
        self.scene
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

    fn zoom_scale(&self, delta: f32) -> f32 {
        let normalized_delta = (delta * 0.01).abs();
        f32::powf(0.95, self.zoom_speed * normalized_delta)
    }
}

impl Controls for OrbitControls {
    fn take_input(
        &mut self,
        inputs: &mut egui::InputState,
        viewport_size: egui::Vec2,
        scene: &Scene,
    ) {
        if self.enable_rotate && inputs.pointer.primary_down() {
            let delta = 2.0 * PI * inputs.pointer.delta() * self.rotate_speed / viewport_size.y;
            self.delta_theta -= delta.x;
            self.delta_phi -= delta.y;
        } else if self.enable_pan && inputs.pointer.secondary_down() {
            let delta = self.pan_speed * inputs.pointer.delta() / viewport_size.y;
            let camera = &scene[self.camera];
            let matrix = camera.local_transform().matrix();
            match scene.camera(self.camera).unwrap().projection {
                Projection::Perspective { y_fov, .. } => {
                    // perspective
                    let position = camera.local_position();
                    let target = scene[self.target].local_position();
                    let offset = position - target;
                    let mut target_distance = offset.length();

                    // half of the fov is center to top of screen
                    target_distance *= (y_fov / 2.0).tan();

                    // we use only viewport_size.y here so aspect ratio does not distort speed
                    self.pan_left(2.0 * delta.x * target_distance, matrix);
                    self.pan_up(2.0 * delta.y * target_distance, matrix);
                }
                Projection::Orthographic { x_mag, y_mag, .. } => {
                    self.pan_left(delta.x * 2.0 * x_mag, matrix);
                    self.pan_up(delta.y * 2.0 * y_mag, matrix);
                }
            }
        } else if self.enable_zoom {
            let delta = inputs.smooth_scroll_delta.y;
            if delta > 0.0 {
                self.scale *= self.zoom_scale(delta);
            } else if delta < 0.0 {
                self.scale /= self.zoom_scale(delta);
            }
        }
    }

    fn update(&mut self, viewport_aspect_ratio: f32, scene: &mut Scene) {
        let mut camera = scene[self.camera].local_transform().clone();
        let mut target = scene[self.target].local_transform().clone();
        let mut offset = camera.translation() - target.translation();

        let mut radius = offset.length();
        let mut theta = offset.x.atan2(offset.z);
        let mut phi = (offset.y / radius).acos();

        if let Some(damping_factor) = self.damping_factor {
            theta += self.delta_theta * damping_factor;
            phi += self.delta_phi * damping_factor;
            target.set_translation(target.translation() + self.pan_offset * damping_factor);

            self.delta_theta *= 1.0 - damping_factor;
            self.delta_phi *= 1.0 - damping_factor;
            self.pan_offset *= 1.0 - damping_factor;
        } else {
            theta += self.delta_theta;
            phi += self.delta_phi;
            target.set_translation(target.translation() + self.pan_offset);

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
        let cursor = scene[self.cursor].local_transform();
        let mut distance = target.translation() - cursor.translation();
        distance = distance.clamp_length(*self.target_radius.start(), *self.target_radius.end());
        target.set_translation(distance + cursor.translation());

        let camera_instance = scene.camera(self.camera).unwrap();
        let camera_projection = camera_instance.projection.matrix(viewport_aspect_ratio);
        // adjust the camera position based on zoom only if we're not zooming to the cursor or if it's an ortho camera
        // we adjust zoom later in these cases
        let is_orthographic_camera = match camera_instance.projection {
            Projection::Orthographic { .. } => true,
            Projection::Perspective { .. } => false,
        };
        if self.zoom_to_cursor && self.perform_cursor_zoom || is_orthographic_camera {
            radius = radius.clamp(*self.distance.start(), *self.distance.end());
        } else {
            radius = (radius * self.scale).clamp(*self.distance.start(), *self.distance.end());
        }

        offset.z = radius * phi.sin() * theta.cos();
        offset.x = radius * phi.sin() * theta.sin();
        offset.y = radius * phi.cos();

        camera.set_translation(target.translation() + offset);
        camera.look_at(target.translation());

        if self.zoom_to_cursor && self.perform_cursor_zoom {
            let new_radius = if !is_orthographic_camera {
                // move the camera down the pointer ray
                // this method avoids floating point error
                let previous_radius = offset.length();
                let new_radius = (previous_radius * self.scale)
                    .clamp(*self.distance.start(), *self.distance.end());

                let radius_delta = previous_radius - new_radius;
                camera.set_translation(camera.translation() + self.zoom_direction * radius_delta);

                new_radius
            } else {
                scene
                    .node_handle(self.camera)
                    .set_local_transform(camera.clone());
                let camera_world_matrix = scene[self.camera].world_matrix();

                // adjust the ortho camera position based on zoom changes
                let mouse_before = self.mouse.extend(0.0);
                let mouse_before = (camera_world_matrix * camera_projection.inverse())
                    .project_point3(mouse_before);

                let zoom = match scene.camera_mut(self.camera).unwrap().projection {
                    Projection::Orthographic { ref mut zoom, .. } => zoom,
                    Projection::Perspective { .. } => unreachable!(),
                };
                *zoom = (*zoom / self.scale).clamp(*self.zoom.start(), *self.zoom.end());

                let mouse_after = (camera_world_matrix
                    * scene
                        .camera(self.camera)
                        .unwrap()
                        .projection
                        .matrix(viewport_aspect_ratio)
                        .inverse())
                .project_point3(mouse_before);

                camera.set_translation(camera.translation() - mouse_after + mouse_before);

                offset.length()
            };

            // handle the placement of the target
            if self.screen_space_panning {
                // position the orbit target in front of the new camera position
                target.set_translation(
                    target
                        .matrix()
                        .transform_vector3(target.translation())
                        .normalize()
                        * new_radius
                        + scene[self.camera].local_position(),
                );
            } else {
                // get the ray and translation plane to compute target
                let ray = Ray {
                    origin: scene[self.camera].local_position(),
                    direction: scene[self.camera]
                        .local_transform()
                        .matrix()
                        .transform_vector3(-Vec3::Z)
                        .normalize(),
                };

                // if the camera is 20 degrees above the horizon then don't adjust the focus target to avoid
                // extremely large values
                let tilt_limit = f32::to_radians(70.0).cos();
                if Vec3::Y.dot(ray.direction).abs() < tilt_limit {
                    camera.look_at(target.translation());
                } else {
                    let plane =
                        Plane::from_normal_and_coplanar_point(Vec3::Y, target.translation());
                    if let Some(target_pos) = ray.intersect_plane(plane) {
                        target.set_translation(target_pos);
                    };
                }
            }
        } else if let Projection::Orthographic { ref mut zoom, .. } =
            scene.camera_mut(self.camera).unwrap().projection
        {
            *zoom = (*zoom / self.scale).clamp(*self.zoom.start(), *self.zoom.end());
        }

        self.scale = 1.0;
        self.perform_cursor_zoom = false;

        scene.node_handle(self.camera).set_local_transform(camera);
        scene.node_handle(self.target).set_local_transform(target);
    }
}
