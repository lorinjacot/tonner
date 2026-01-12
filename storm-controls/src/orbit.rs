// /**
//  * Fires when the camera has been transformed by the controls.
//  *
//  * @event OrbitControls#change
//  * @type {Object}
//  */
// const _changeEvent = { type: 'change' };

// /**
//  * Fires when an interaction was initiated.
//  *
//  * @event OrbitControls#start
//  * @type {Object}
//  */
// const _startEvent = { type: 'start' };

// /**
//  * Fires when an interaction has finished.
//  *
//  * @event OrbitControls#end
//  * @type {Object}
//  */
// const _endEvent = { type: 'end' };

// const _ray = new Ray();
// const _plane = new Plane();

const TILT_LIMIT: f32 = 0.34202; // 70.0f32.to_radians().cos();

// const _v = new Vector3();
// const _twoPI = 2 * Math.PI;

const EPS: f32 = 0.000001;

use std::{
    f32::{INFINITY, consts::PI},
    ops::RangeInclusive,
    time::Duration,
};

use glam::{Mat4, Quat, Vec2, Vec3, vec3};
use log::warn;
use storm::{
    Scene,
    camera::Camera,
    math::{Plane, Ray, Spherical},
};

use crate::{Key, Modifiers, MouseButton};

/// Orbit controls allow the camera to orbit around a target.
///
/// OrbitControls performs orbiting, dollying (zooming), and panning. Unlike {@link TrackballControls},
/// it maintains the "up" direction `object.up` (+Y by default).
///
/// - Orbit: Left mouse / touch: one-finger move.
/// - Zoom: Middle mouse, or mousewheel / touch: two-finger spread or squish.
/// - Pan: Right mouse, or left mouse + ctrl/meta/shiftKey, or arrow keys / touch: two-finger move.
///
/// ```js
/// const controls = new OrbitControls( camera, renderer.domElement );
///
/// // controls[.Self::update() ]must be called after any manual changes to the camera's transform
/// camera.position.set( 0, 20, 100 );
/// controls[.Self::update();]
///
/// function animate() {
///
/// 	// required if controls.enableDamping or controls.autoRotate are set to true
/// 	controls[.Self::update();]
///
/// 	renderer.render( scene, camera );
///
/// }
/// ```
///
/// Based on [three.js OrbitControl][https://threejs.org/docs/#OrbitControls].
pub struct OrbitControls {
    /// The node that is managed by the controls.
    pub camera: Camera,

    /// The focus point of the controls, the `object` orbits around this.
    /// It can be updated manually at any point to change the focus of the controls.
    pub target: Vec3,

    /// The focus point of the `minTargetRadius` and `maxTargetRadius` limits.
    /// It can be updated manually at any point to change the center of interest
    /// for the `target`. Defaults to [`Vec3::ZERO`].
    pub cursor: Vec3,

    /// How far you can dolly in/out (perspective camera only). Defaults to `0.0..=INFINITY`,
    pub distance_range: RangeInclusive<f32>,

    /// How far you can zoom in/out (orthographic camera only). Defaults to `0.0..=INFINITY`,
    pub zoom_range: RangeInclusive<f32>,

    /// How close/far you can move the target to/from the 3D [`Self::cursor`]. Defaults to `0.0..=INFINITY`.
    pub target_radius_range: RangeInclusive<f32>,

    /// How far you can orbit vertically. Range is `0..<=PI` radians. Defaults to `0.0..=PI`,
    pub polar_angle_range: RangeInclusive<f32>,

    /// How far you can orbit horizontally If set, the interval `min..=max`
    /// must be a sub-interval of `-2.0*PI..=2.0*PI`, with `max - min < 2.0 * PI`.
    pub azimuth_angle_range: Option<RangeInclusive<f32>>,

    /// Set to `true` to enable damping (inertia), which can be used to give a sense of weight
    /// to the controls. Defaults to `false`.
    ///
    /// Note that if this is enabled, you must call [`Self::update()`] in your animation loop.
    pub enable_damping: bool,

    /// The damping inertia used if `enableDamping` is set to `true`. Defaults to `0.05`.
    ///
    /// Note that for this to work, you must call [`Self::update()`] in your animation loop.
    pub damping_factor: f32,

    /// Enable or disable zooming (dollying) of the camera. Defaults to `true`.
    pub enable_zoom: bool,

    /// Speed of zooming / dollying. Defaults to `1.0`.
    pub zoom_speed: f32,

    /// Enable or disable horizontal and vertical rotation of the camera. Defaults to `true`.
    ///
    /// Note that it is possible to disable a single axis by setting the min and max of the
    /// [`Self::polar_angle_range`] or [`Self::azimuth_angle_range`] to the same value, which will cause the vertical
    /// or horizontal rotation to be fixed at that value.
    pub enable_rotate: bool,

    /// Speed of rotation. Defaults to 1.0,
    pub rotate_speed: f32,

    /// How fast to rotate the camera when the keyboard is used. Defaults to `1.0`.
    pub key_rotate_speed: f32,

    /// Enable or disable camera panning. Defaults to `true`.
    pub enable_pan: bool,

    /// Speed of panning. Defaults to `1.0`.
    pub pan_speed: f32,

    /// Defines how the camera's position is translated when panning. If `true`, the camera pans
    /// in screen space. Otherwise, the camera pans in the plane orthogonal to the camera's up
    /// direction. Defaults to `true`.
    pub screen_space_panning: bool,

    /// How fast to pan the camera when the keyboard is used in
    /// pixels per keypress. Defaults to `7.0`.
    pub key_pan_speed: f32,

    /// Setting this property to `true` allows to zoom to the cursor's position. Defaults to `false`.
    pub zoom_to_cursor: bool,

    /// Set to true to automatically rotate around the target. Defaults to `false`.
    ///
    /// Note that if this is enabled, you must call [`Self::update()`] in your animation loop.
    pub auto_rotate: bool,

    /// How fast to rotate around the target if [`Self::auto_rotate`] is `true`. Defaults to `2.0`. This
    /// equates to 30 seconds per orbit.
    ///
    /// Note that if [`Self::auto_rotate`] is enabled, you must call [`Self::update()`] in your animation loop.
    pub auto_rotate_speed: f32,

    /// Keyboard key used for leftward camera panning. Defaults to [`Key::ArrowLeft`].
    pub left_key: Key,

    /// Keyboard key used for upward camera panning. Defaults to [`Key::ArrowUp`].
    pub up_key: Key,

    /// Keyboard key used for rightward camera panning. Defaults to [`Key::ArrowRight`].
    pub right_key: Key,

    /// Keyboard key used for downward camera panning. Defaults to [`Key::ArrowDown`].
    pub bottom_key: Key,

    /// Mouse button used for dolly. Defaults to [`MouseButton::Middle`].
    pub dolly_botton: MouseButton,

    /// Mouse button used for rotate. Defaults to [`MouseButton::Left`].
    pub rotate_button: MouseButton,

    // Mouse button used for pan. Defaults to [`MouseButton::Right`].
    pub pan_button: MouseButton,

    last_position: Vec3,
    last_quaternion: Quat,
    last_target_position: Vec3,

    // so camera.up is the orbit axis
    quat: Quat,
    quat_inverse: Quat,

    spherical: Spherical,
    spherical_delta: Spherical,

    scale: f32,
    pan_offset: Vec3,

    dolly_direction: Vec3,
    mouse: Vec2,
    perform_cursor_zoom: bool,

    state: State,
}

impl Default for OrbitControls {
    fn default() -> Self {
        Self {
            camera: todo!(),
            target: Vec3::ZERO,
            cursor: Vec3::ZERO,
            distance_range: 0.0..=INFINITY,
            zoom_range: 0.0..=INFINITY,
            target_radius_range: 0.0..=INFINITY,
            polar_angle_range: 0.0..=PI,
            azimuth_angle_range: None,
            enable_damping: false,
            damping_factor: 0.05,
            enable_zoom: true,
            zoom_speed: 1.0,
            enable_rotate: true,
            rotate_speed: 1.0,
            key_rotate_speed: 1.0,
            enable_pan: true,
            pan_speed: 1.0,
            screen_space_panning: true,
            key_pan_speed: 7.0,
            zoom_to_cursor: false,
            auto_rotate: false,
            auto_rotate_speed: 2.0,
            left_key: Key::ArrowLeft,
            up_key: Key::ArrowUp,
            right_key: Key::ArrowRight,
            bottom_key: Key::ArrowDown,
            dolly_botton: MouseButton::Middle,
            rotate_button: MouseButton::Left,
            pan_button: MouseButton::Left,
            last_position: Vec3::ZERO,
            last_quaternion: Quat::IDENTITY,
            last_target_position: Vec3::ZERO,
            quat: Quat::IDENTITY,
            quat_inverse: Quat::IDENTITY,
            spherical: Spherical::ZERO,
            spherical_delta: Spherical::ZERO,
            scale: 1.0,
            pan_offset: Vec3::ZERO,
            dolly_direction: Vec3::ZERO,
            mouse: Vec2::ZERO,
            perform_cursor_zoom: false,
            state: State::None,
        }
    }
}

impl OrbitControls {
    pub fn update(&mut self, scene: &mut Scene, delta_time: Duration, viewport_aspect_ratio: f32) {
        let v = scene.local_position(self.camera.node).unwrap() - self.target;

        // rotate offset to "y-axis-is-up" space
        let v = self.quat.mul_vec3(v);

        // angle from z-axis around y-axis
        self.spherical = Spherical::from_vec3(v);

        if self.auto_rotate && self.state == State::None {
            self.rotate_left(self.get_auto_rotation_angle(delta_time));
        }

        if self.enable_damping {
            self.spherical.theta += self.spherical_delta.theta * self.damping_factor;
            self.spherical.phi += self.spherical_delta.phi * self.damping_factor;
        } else {
            self.spherical.theta += self.spherical_delta.theta;
            self.spherical.phi += self.spherical_delta.phi;
        }

        // restrict theta to be between desired limits
        if let Some(azimuth_angle_range) = &self.azimuth_angle_range {
            let mut min = *azimuth_angle_range.start();
            let mut max = *azimuth_angle_range.end();

            if min < -PI {
                min += 2.0 * PI;
            } else if min > PI {
                min -= 2.0 * PI;
            }

            if max < -PI {
                max += 2.0 * PI;
            } else if max > PI {
                max -= 2.0 * PI;
            }

            if min <= max {
                self.spherical.theta = self.spherical.theta.clamp(min, max);
            } else {
                self.spherical.theta = if self.spherical.theta > (min + max) / 2.0 {
                    min.max(self.spherical.theta)
                } else {
                    max.min(self.spherical.theta)
                }
            }
        }

        // restrict phi to be between desired limits
        self.spherical.phi = self.spherical.phi.clamp(
            *self.polar_angle_range.start(),
            *self.polar_angle_range.end(),
        );
        self.spherical = self.spherical.safe();

        // move target to panned location
        if self.enable_damping {
            self.target += self.pan_offset * self.damping_factor;
        } else {
            self.target += self.pan_offset;
        }

        // Limit the target distance from the cursor to create a sphere around the center of interest
        self.target -= self.cursor;
        self.target = self.target.clamp_length(
            *self.target_radius_range.start(),
            *self.target_radius_range.end(),
        );
        self.target += self.cursor;

        let mut zoom_changed = false;
        // adjust the camera position based on zoom only if we're not zooming to the cursor or if it's an ortho camera
        // we adjust zoom later in these cases
        if self.zoom_to_cursor && self.perform_cursor_zoom || self.camera.is_orthographic() {
            self.spherical.radius = self.clamp_distance(self.spherical.radius);
        } else {
            let previus_radius = self.spherical.radius;
            self.spherical.radius = self.clamp_distance(self.spherical.radius * self.scale);
            zoom_changed = previus_radius != self.spherical.radius;
        }

        let v = self.spherical.to_vec3();

        // rotate offset back to "camera-up-vector-is-up" space
        let v = self.quat_inverse.mul_vec3(v);

        scene
            .set_local_position(self.camera.node, self.target + v)
            .unwrap();
        scene.look_at(self.camera.node, self.target).unwrap();

        if self.enable_damping {
            self.spherical_delta.theta *= 1.0 - self.damping_factor;
            self.spherical_delta.phi *= 1.0 - self.damping_factor;
            self.pan_offset += 1.0 - self.damping_factor
        } else {
            self.spherical_delta = Spherical::ZERO;
            self.pan_offset = Vec3::ZERO;
        }

        // adjust camera position
        if zoom_changed && self.perform_cursor_zoom {
            let mut new_radius = None;
            if self.camera.is_perspective() {
                // move the camera down the pointer ray
                // this method avoids floating point error
                let previous_radius = v.length();
                new_radius = Some(self.clamp_distance(previous_radius * self.scale));

                let radius_delta = previous_radius - new_radius.unwrap();
                scene
                    .set_local_position(
                        self.camera.node,
                        scene.local_position(self.camera.node).unwrap()
                            + self.dolly_direction * radius_delta,
                    )
                    .unwrap();

                zoom_changed = radius_delta != 0.0;
            } else if self.camera.is_orthographic() {
                // adjust the ortho camera position based on zoom changes
                let mouse_before = vec3(self.mouse.x, self.mouse.y, 0.0);
                let mouse_before =
                    self.camera
                        .unproject(mouse_before, viewport_aspect_ratio, scene);

                let previous_zoom = self.camera.zoom().unwrap();
                let new_zoom =
                    previous_zoom.clamp(*self.zoom_range.start(), *self.zoom_range.end());
                self.camera.set_zoom(new_zoom).unwrap();

                zoom_changed = previous_zoom != new_zoom;

                let mouse_after = vec3(self.mouse.x, self.mouse.y, 0.0);
                let mouse_after = self
                    .camera
                    .unproject(mouse_after, viewport_aspect_ratio, scene);

                scene
                    .set_local_position(
                        self.camera.node,
                        scene.local_position(self.camera.node).unwrap() - mouse_after
                            + mouse_before,
                    )
                    .unwrap();
            } else {
                warn!("Unknown camera type - zoom to cursor disabled.");
                self.zoom_to_cursor = false;
            }

            // handle the placement of the target
            if let Some(new_radius) = new_radius {
                if self.screen_space_panning {
                    // position the orbit target in front of the new camera position
                    self.target = scene
                        .local_matrix(self.camera.node)
                        .unwrap()
                        .transform_vector3(vec3(0.0, 0.0, -1.0))
                        * new_radius
                        + scene.local_position(self.camera.node).unwrap();
                } else {
                    // get the ray and translation plane to compute target
                    let ray = Ray {
                        origin: scene.local_position(self.camera.node).unwrap(),
                        direction: scene
                            .local_matrix(self.camera.node)
                            .unwrap()
                            .transform_vector3(vec3(0.0, 0.0, -1.0)),
                    };

                    // if the camera is 20 degrees above the horizon then don't adjust the focus target to avoid
                    // extremely large values
                    if Vec3::Y.dot(ray.direction).abs() < TILT_LIMIT {
                        scene.look_at(self.camera.node, self.target).unwrap();
                    } else {
                        let plane = Plane::from_normal_and_coplanar_point(Vec3::Y, self.target);
                        self.target = ray.intersect_plane(plane).unwrap();
                    }
                }
            }
        } else if self.camera.is_orthographic() {
            let previous_zoom = self.camera.zoom().unwrap();
            let new_zoom = (previous_zoom / self.scale)
                .clamp(*self.zoom_range.start(), *self.zoom_range.end());
            self.camera.set_zoom(new_zoom).unwrap();

            if previous_zoom != new_zoom {
                zoom_changed = true;
            }
        }

        self.scale = 1.0;
        self.perform_cursor_zoom = false;

        // update condition is:
        // min(camera displacement, camera rotation in radians)^2 > EPS
        // using small-angle approximation cos(x/2) = 1 - x^2 / 8
        if zoom_changed
            || self
                .last_position
                .distance_squared(scene.local_position(self.camera.node).unwrap())
                > EPS
            || 8.0
                * (1.0
                    - self
                        .last_quaternion
                        .dot(scene.local_rotation(self.camera.node).unwrap()))
                > EPS
            || self.last_target_position.distance_squared(self.target) > EPS
        {
            self.last_position = scene.local_position(self.camera.node).unwrap();
            self.last_quaternion = scene.local_rotation(self.camera.node).unwrap();
            self.last_target_position = self.target;
        }
    }

    fn get_auto_rotation_angle(&self, delta_time: Duration) -> f32 {
        2.0 * PI / 60.0 * self.auto_rotate_speed * delta_time.as_secs_f32()
    }

    fn get_zoom_scale(&self, delta: f32) -> f32 {
        let normalized_delta = (delta * 0.01).abs();
        0.95f32.powf(self.zoom_speed * normalized_delta)
    }

    fn rotate_left(&mut self, angle: f32) {
        self.spherical_delta.theta -= angle;
    }

    fn rotate_up(&mut self, angle: f32) {
        self.spherical_delta.phi -= angle;
    }

    fn pan_left(&mut self, distance: f32, node_local_transform: Mat4) {
        self.pan_offset += node_local_transform.x_axis.truncate() * -distance;
    }

    fn pan_up(&mut self, distance: f32, node_local_transform: Mat4) {
        let v = if self.screen_space_panning {
            node_local_transform.y_axis.truncate()
        } else {
            Vec3::Y.cross(node_local_transform.x_axis.truncate())
        };
        self.pan_offset += v * distance;
    }

    // delta_x and delta_y are in pixels; right and down are positive
    fn pan(&mut self, scene: &Scene, delta_x: f32, delta_y: f32, view_height: f32) {
        if let Some(projection) = self.camera.perspective_projection() {
            let v = scene.local_position(self.camera.node).unwrap() - self.target;
            let mut target_distance = v.length();

            // half of the fov is center to top of screen
            target_distance *= (projection.y_fov / 2.0).to_radians().tan();

            // we use only clientHeight here so aspect ratio does not distort speed
            let node_local_transform = scene.local_matrix(self.camera.node).unwrap();
            self.pan_left(
                2.0 * delta_x * target_distance / view_height,
                node_local_transform,
            );
            self.pan_up(
                2.0 * delta_y * target_distance / view_height,
                node_local_transform,
            );
        } else if let Some(projection) = self.camera.orthographic_projection() {
            let node_local_transform = scene.local_matrix(self.camera.node).unwrap();
            let zoom = projection.zoom;
            let x_mag = projection.x_mag;
            let y_mag = projection.y_mag;
            self.pan_left(delta_x * x_mag / zoom / view_height, node_local_transform);
            self.pan_left(delta_y * y_mag / zoom / view_height, node_local_transform);
        } else {
            warn!("OrbitControls encountered an unknown camera type - pan disabled.");
            self.enable_pan = false;
        }
    }

    fn dolly_out(&mut self, dolly_scale: f32) {
        if self.camera.is_perspective() || self.camera.is_orthographic() {
            self.scale /= dolly_scale;
        } else {
            warn!("OrbitControls encountered an unknown camera type - dolly/zoom disabled.");
            self.enable_zoom = false;
        }
    }

    fn dolly_in(&mut self, dolly_scale: f32) {
        if self.camera.is_perspective() || self.camera.is_orthographic() {
            self.scale *= dolly_scale;
        } else {
            warn!("OrbitControls encountered an unknown camera type - dolly/zoom disabled.");
            self.enable_zoom = false;
        }
    }

    fn update_zoom_parameters(
        &mut self,
        x: f32,
        y: f32,
        view_width: f32,
        view_height: f32,
        scene: &Scene,
    ) {
        if !self.zoom_to_cursor {
            return;
        }

        self.perform_cursor_zoom = true;

        self.mouse.x = (x / view_width) * 2.0 - 1.0;
        self.mouse.y = -(y / view_height) * 2.0 + 1.0;

        self.dolly_direction = self.camera.unproject(
            vec3(self.mouse.x, self.mouse.y, 1.0),
            view_width / view_height,
            scene,
        ) - scene.local_position(self.camera.node).unwrap().normalize();
    }

    fn clamp_distance(&self, distance: f32) -> f32 {
        distance.clamp(*self.distance_range.start(), *self.distance_range.end())
    }

    fn handle_mouse_move_rotate(&mut self, rotate_delta: Vec2, view_height: f32) {
        self.rotate_left(2.0 * PI * rotate_delta.x / view_height); // yes, height
        self.rotate_up(2.0 * PI * rotate_delta.y / view_height);
    }

    fn handle_mouse_move_dolly(&mut self, dolly_delta: Vec2) {
        if dolly_delta.y > 0.0 {
            self.dolly_out(self.get_zoom_scale(dolly_delta.y));
        } else if dolly_delta.y < 0.0 {
            self.dolly_in(self.get_zoom_scale(dolly_delta.y));
        }
    }

    fn handle_mouse_move_pan(&mut self, pan_delta: Vec2, scene: &mut Scene, view_height: f32) {
        self.pan(scene, pan_delta.x, pan_delta.y, view_height);
    }

    fn handle_mouse_wheel(
        &mut self,
        delta: Vec2,
        scene: &mut Scene,
        view_width: f32,
        view_height: f32,
    ) {
        self.update_zoom_parameters(delta.x, delta.y, view_width, view_height, scene);

        if delta.y < 0.0 {
            self.dolly_in(self.get_zoom_scale(delta.y));
        } else if delta.y > 0.0 {
            self.dolly_out(self.get_zoom_scale(delta.y));
        }
    }

    fn handle_key_down(
        &mut self,
        key: Key,
        modifiers: Modifiers,
        scene: &mut Scene,
        view_height: f32,
    ) {
        if key == self.up_key {
            if modifiers.contains(Modifiers::CTRL)
                || modifiers.contains(Modifiers::META)
                || modifiers.contains(Modifiers::SHIFT)
            {
                if self.enable_rotate {
                    self.rotate_up(2.0 * PI * self.key_rotate_speed / view_height);
                }
            } else {
                if self.enable_pan {
                    self.pan(scene, 0.0, self.key_pan_speed, view_height);
                }
            }
        } else if key == self.bottom_key {
            if modifiers.contains(Modifiers::CTRL)
                || modifiers.contains(Modifiers::META)
                || modifiers.contains(Modifiers::SHIFT)
            {
                if self.enable_rotate {
                    self.rotate_up(-2.0 * PI * self.key_rotate_speed / view_height);
                }
            } else {
                if self.enable_pan {
                    self.pan(scene, 0.0, -self.key_pan_speed, view_height);
                }
            }
        } else if key == self.left_key {
            if modifiers.contains(Modifiers::CTRL)
                || modifiers.contains(Modifiers::META)
                || modifiers.contains(Modifiers::SHIFT)
            {
                if self.enable_rotate {
                    self.rotate_left(2.0 * PI * self.key_rotate_speed / view_height);
                }
            } else {
                if self.enable_pan {
                    self.pan(scene, self.key_pan_speed, 0.0, view_height);
                }
            }
        } else if key == self.right_key {
            if modifiers.contains(Modifiers::CTRL)
                || modifiers.contains(Modifiers::META)
                || modifiers.contains(Modifiers::SHIFT)
            {
                if self.enable_rotate {
                    self.rotate_left(-2.0 * PI * self.key_rotate_speed / view_height);
                }
            } else {
                if self.enable_pan {
                    self.pan(scene, -self.key_pan_speed, 0.0, view_height);
                }
            }
        }
    }

    pub fn on_mouse_down(
        &mut self,
        position: Vec2,
        mouse_button: MouseButton,
        modifiers: Modifiers,
        view_width: f32,
        view_height: f32,
        scene: &Scene,
    ) {
        if mouse_button == self.dolly_botton {
            if !self.enable_zoom {
                return;
            }
            self.update_zoom_parameters(position.x, position.y, view_width, view_height, scene);
            self.state = State::Dolly;
        } else if mouse_button == self.rotate_button {
            if modifiers.contains(Modifiers::CTRL)
                || modifiers.contains(Modifiers::META)
                || modifiers.contains(Modifiers::SHIFT)
            {
                if !self.enable_pan {
                    return;
                }
                self.state = State::Pan;
            } else {
                if !self.enable_rotate {
                    return;
                }
                self.state = State::Rotate;
            }
        } else if mouse_button == self.pan_button {
            if modifiers.contains(Modifiers::CTRL)
                || modifiers.contains(Modifiers::META)
                || modifiers.contains(Modifiers::SHIFT)
            {
                if !self.enable_rotate {
                    return;
                }
                self.state = State::Rotate;
            } else {
                if !self.enable_pan {
                    return;
                }
                self.state = State::Pan;
            }
        } else {
            self.state = State::None;
        }
    }

    pub fn on_mouse_move(&mut self, delta: Vec2, view_height: f32, scene: &mut Scene) {
        match self.state {
            State::Rotate if self.enable_rotate => {
                self.handle_mouse_move_rotate(delta, view_height)
            }
            State::Dolly if self.enable_zoom => {
                self.handle_mouse_move_dolly(delta);
            }
            State::Pan if self.enable_pan => {
                self.handle_mouse_move_pan(delta, scene, view_height);
            }
            _ => (),
        }
    }

    pub fn on_mouse_wheel(
        &mut self,
        delta: Vec2,
        view_width: f32,
        view_height: f32,
        scene: &mut Scene,
    ) {
        if self.enable_zoom && self.state == State::None {
            self.handle_mouse_wheel(delta, scene, view_width, view_height);
        }
    }

    pub fn on_key_down(
        &mut self,
        key: Key,
        modifiers: Modifiers,
        view_width: f32,
        scene: &mut Scene,
    ) {
        self.handle_key_down(key, modifiers, scene, view_width);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum State {
    None,
    Rotate,
    Dolly,
    Pan,
}
