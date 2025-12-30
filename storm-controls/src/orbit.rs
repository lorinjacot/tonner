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
    math::{Plane, Ray, Spherical},
    node::NodeId,
};

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
    pub node: NodeId,

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

    rotate_start: Vec2,
    rotate_end: Vec2,
    rotate_delta: Vec2,

    pan_start: Vec2,
    pan_end: Vec2,
    pan_delta: Vec2,

    dolly_start: Vec2,
    dolly_end: Vec2,
    dolly_delta: Vec2,

    dolly_direction: Vec3,
    mouse: Vec2,
    perform_cursor_zoom: bool,

    control_active: bool,
    state: State,
}

impl Default for OrbitControls {
    fn default() -> Self {
        Self {
            node: todo!(),
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
            last_position: Vec3::ZERO,
            last_quaternion: Quat::IDENTITY,
            last_target_position: Vec3::ZERO,
            quat: Quat::IDENTITY,
            quat_inverse: Quat::IDENTITY,
            spherical: Spherical::ZERO,
            spherical_delta: Spherical::ZERO,
            scale: 1.0,
            pan_offset: Vec3::ZERO,
            rotate_start: Vec2::ZERO,
            rotate_end: Vec2::ZERO,
            rotate_delta: Vec2::ZERO,
            pan_start: Vec2::ZERO,
            pan_end: Vec2::ZERO,
            pan_delta: Vec2::ZERO,
            dolly_start: Vec2::ZERO,
            dolly_end: Vec2::ZERO,
            dolly_delta: Vec2::ZERO,
            dolly_direction: Vec3::ZERO,
            mouse: Vec2::ZERO,
            perform_cursor_zoom: false,
            control_active: false,
            state: State::None,
        }
    }
}

impl OrbitControls {
    pub fn update(&mut self, scene: &mut Scene, delta_time: Duration, viewport_aspect_ratio: f32) {
        let v = scene.local_position(self.node).unwrap() - self.target;

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
        if self.zoom_to_cursor && self.perform_cursor_zoom
            || scene.is_orthographic_camera_node(self.node)
        {
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
            .set_local_position(self.node, self.target + v)
            .unwrap();
        scene.look_at(self.node, self.target).unwrap();

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
            if scene.is_perspective_camera_node(self.node) {
                // move the camera down the pointer ray
                // this method avoids floating point error
                let previous_radius = v.length();
                new_radius = Some(self.clamp_distance(previous_radius * self.scale));

                let radius_delta = previous_radius - new_radius.unwrap();
                scene
                    .set_local_position(
                        self.node,
                        scene.local_position(self.node).unwrap()
                            + self.dolly_direction * radius_delta,
                    )
                    .unwrap();

                zoom_changed = radius_delta != 0.0;
            } else if scene.is_orthographic_camera_node(self.node) {
                // adjust the ortho camera position based on zoom changes
                let mouse_before = vec3(self.mouse.x, self.mouse.y, 0.0);
                let mouse_before = scene
                    .unproject_node(self.node, mouse_before, viewport_aspect_ratio)
                    .unwrap();

                let previous_zoom = scene.camera_zoom_node(self.node).unwrap();
                let new_zoom =
                    previous_zoom.clamp(*self.zoom_range.start(), *self.zoom_range.end());
                scene.set_camera_zoom_node(self.node, new_zoom).unwrap();

                zoom_changed = previous_zoom != new_zoom;

                let mouse_after = vec3(self.mouse.x, self.mouse.y, 0.0);
                let mouse_after = scene
                    .unproject_node(self.node, mouse_after, viewport_aspect_ratio)
                    .unwrap();

                scene
                    .set_local_position(
                        self.node,
                        scene.local_position(self.node).unwrap() - mouse_after + mouse_before,
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
                        .local_matrix(self.node)
                        .unwrap()
                        .transform_vector3(vec3(0.0, 0.0, -1.0))
                        * new_radius
                        + scene.local_position(self.node).unwrap();
                } else {
                    // get the ray and translation plane to compute target
                    let ray = Ray {
                        origin: scene.local_position(self.node).unwrap(),
                        direction: scene
                            .local_matrix(self.node)
                            .unwrap()
                            .transform_vector3(vec3(0.0, 0.0, -1.0)),
                    };

                    // if the camera is 20 degrees above the horizon then don't adjust the focus target to avoid
                    // extremely large values
                    if Vec3::Y.dot(ray.direction).abs() < TILT_LIMIT {
                        scene.look_at(self.node, self.target).unwrap();
                    } else {
                        let plane = Plane::from_normal_and_coplanar_point(Vec3::Y, self.target);
                        self.target = ray.intersect_plane(plane).unwrap();
                    }
                }
            }
        } else if scene.is_orthographic_camera_node(self.node) {
            let previous_zoom = scene.camera_zoom_node(self.node).unwrap();
            let new_zoom = (previous_zoom / self.scale)
                .clamp(*self.zoom_range.start(), *self.zoom_range.end());
            scene.set_camera_zoom_node(self.node, new_zoom).unwrap();

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
                .distance_squared(scene.local_position(self.node).unwrap())
                > EPS
            || 8.0
                * (1.0
                    - self
                        .last_quaternion
                        .dot(scene.local_rotation(self.node).unwrap()))
                > EPS
            || self.last_target_position.distance_squared(self.target) > EPS
        {
            self.last_position = scene.local_position(self.node).unwrap();
            self.last_quaternion = scene.local_rotation(self.node).unwrap();
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
        if scene.is_perspective_camera_node(self.node) {
            let v = scene.local_position(self.node).unwrap() - self.target;
            let mut target_distance = v.length();

            // half of the fov is center to top of screen
            target_distance *= (scene.camera_y_fov_node(self.node).unwrap() / 2.0)
                .to_radians()
                .tan();

            // we use only clientHeight here so aspect ratio does not distort speed
            let node_local_transform = scene.local_matrix(self.node).unwrap();
            self.pan_left(
                2.0 * delta_x * target_distance / view_height,
                node_local_transform,
            );
            self.pan_up(
                2.0 * delta_y * target_distance / view_height,
                node_local_transform,
            );
        } else if scene.is_orthographic_camera_node(self.node) {
            let node_local_transform = scene.local_matrix(self.node).unwrap();
            let zoom = scene.camera_zoom_node(self.node).unwrap();
            self.pan_left(
                delta_x * scene.camera_x_mag_node(self.node).unwrap() / zoom / view_height,
                node_local_transform,
            );
            self.pan_left(
                delta_y * scene.camera_y_mag_node(self.node).unwrap() / zoom / view_height,
                node_local_transform,
            );
        } else {
            warn!("OrbitControls encountered an unknown camera type - pan disabled.");
            self.enable_pan = false;
        }
    }

    fn dolly_out(&mut self, dolly_scale: f32, scene: &Scene) {
        if scene.is_perspective_camera_node(self.node)
            || scene.is_orthographic_camera_node(self.node)
        {
            self.scale /= dolly_scale;
        } else {
            warn!("OrbitControls encountered an unknown camera type - dolly/zoom disabled.");
            self.enable_zoom = false;
        }
    }

    fn dolly_in(&mut self, dolly_scale: f32, scene: &Scene) {
        if scene.is_perspective_camera_node(self.node)
            || scene.is_orthographic_camera_node(self.node)
        {
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

        self.dolly_direction = scene
            .unproject_node(
                self.node,
                vec3(self.mouse.x, self.mouse.y, 1.0),
                view_width / view_height,
            )
            .unwrap()
            - scene.local_position(self.node).unwrap().normalize();
    }

    fn clamp_distance(&self, distance: f32) -> f32 {
        distance.clamp(*self.distance_range.start(), *self.distance_range.end())
    }

    //
    // event callbacks - update the object state
    //

    fn handle_mouse_down_rotate(&mut self, position: Vec2) {
        self.rotate_start = position;
    }

    fn handle_mouse_down_dolly(
        &mut self,
        position: Vec2,
        view_width: f32,
        view_height: f32,
        scene: &Scene,
    ) {
        self.update_zoom_parameters(position.x, position.y, view_width, view_height, scene);
        self.dolly_start = position;
    }

    fn handle_mouse_down_pan(&mut self, position: Vec2) {
        self.pan_start = position;
    }

    fn handle_mouse_move_rotate(&mut self, position: Vec2, view_height: f32) {
        self.rotate_end = position;
        self.rotate_delta = (self.rotate_end - self.rotate_start) * self.rotate_speed;

        self.rotate_left(2.0 * PI * self.rotate_delta.x / view_height); // yes, height
        self.rotate_up(2.0 * PI * self.rotate_delta.y / view_height);

        self.rotate_start = self.rotate_end;
        // self.update(scene, delta_time, viewport_aspect_ratio);
    }

    fn handle_mouse_move_dolly(&mut self, position: Vec2, scene: &Scene) {
        self.dolly_end = position;
        self.dolly_delta = self.dolly_end - self.dolly_start;

        if self.dolly_delta.y > 0.0 {
            self.dolly_out(self.get_zoom_scale(self.dolly_delta.y), scene);
        } else if (self.dolly_delta.y < 0.0) {
            self.dolly_in(self.get_zoom_scale(self.dolly_delta.y), scene);
        }

        self.dolly_start = self.dolly_end;
        // self.update(scene, delta_time, viewport_aspect_ratio);
    }

    fn handle_mouse_move_pan(&mut self, position: Vec2, scene: &Scene, view_height: f32) {
        self.pan_end = position;
        self.pan_delta = (self.pan_end - self.pan_start) * self.pan_speed;

        self.pan(scene, self.pan_delta.x, self.pan_delta.y, view_height);

        self.pan_start = self.pan_end;
        // self.update(scene, delta_time, viewport_aspect_ratio);
    }

    fn handle_mouse_wheel(
        &mut self,
        position: Vec2,
        scene: &Scene,
        view_width: f32,
        view_height: f32,
    ) {
        self.update_zoom_parameters(position.x, position.y, view_width, view_height, scene);

        if position.y < 0.0 {
            self.dolly_in(self.get_zoom_scale(position.y), scene);
        } else if position.y > 0.0 {
            self.dolly_out(self.get_zoom_scale(position.y), scene);
        }

        // self.update(scene, delta_time, viewport_aspect_ratio);
    }

    // 	_handleKeyDown( event ) {

    // 		let needsUpdate = false;

    // 		switch ( event.code ) {

    // 			case this.keys.UP:

    // 				if ( event.ctrlKey || event.metaKey || event.shiftKey ) {

    // 					if ( this.enableRotate ) {

    // 						this._rotateUp( _twoPI * this.keyRotateSpeed / this.domElement.clientHeight );

    // 					}

    // 				} else {

    // 					if ( this.enablePan ) {

    // 						this._pan( 0, this.keyPanSpeed );

    // 					}

    // 				}

    // 				needsUpdate = true;
    // 				break;

    // 			case this.keys.BOTTOM:

    // 				if ( event.ctrlKey || event.metaKey || event.shiftKey ) {

    // 					if ( this.enableRotate ) {

    // 						this._rotateUp( - _twoPI * this.keyRotateSpeed / this.domElement.clientHeight );

    // 					}

    // 				} else {

    // 					if ( this.enablePan ) {

    // 						this._pan( 0, - this.keyPanSpeed );

    // 					}

    // 				}

    // 				needsUpdate = true;
    // 				break;

    // 			case this.keys.LEFT:

    // 				if ( event.ctrlKey || event.metaKey || event.shiftKey ) {

    // 					if ( this.enableRotate ) {

    // 						this._rotateLeft( _twoPI * this.keyRotateSpeed / this.domElement.clientHeight );

    // 					}

    // 				} else {

    // 					if ( this.enablePan ) {

    // 						this._pan( this.keyPanSpeed, 0 );

    // 					}

    // 				}

    // 				needsUpdate = true;
    // 				break;

    // 			case this.keys.RIGHT:

    // 				if ( event.ctrlKey || event.metaKey || event.shiftKey ) {

    // 					if ( this.enableRotate ) {

    // 						this._rotateLeft( - _twoPI * this.keyRotateSpeed / this.domElement.clientHeight );

    // 					}

    // 				} else {

    // 					if ( this.enablePan ) {

    // 						this._pan( - this.keyPanSpeed, 0 );

    // 					}

    // 				}

    // 				needsUpdate = true;
    // 				break;

    // 		}

    // 		if ( needsUpdate ) {

    // 			// prevent the browser from scrolling on cursor keys
    // 			event.preventDefault();

    // 			this[.Self::update();]

    // 		}

    // 	}

    // 	_handleTouchStartRotate( event ) {

    // 		if ( this._pointers.length === 1 ) {

    // 			this._rotateStart.set( event.pageX, event.pageY );

    // 		} else {

    // 			const position = this._getSecondPointerPosition( event );

    // 			const x = 0.5 * ( event.pageX + position.x );
    // 			const y = 0.5 * ( event.pageY + position.y );

    // 			this._rotateStart.set( x, y );

    // 		}

    // 	}

    // 	_handleTouchStartPan( event ) {

    // 		if ( this._pointers.length === 1 ) {

    // 			this._panStart.set( event.pageX, event.pageY );

    // 		} else {

    // 			const position = this._getSecondPointerPosition( event );

    // 			const x = 0.5 * ( event.pageX + position.x );
    // 			const y = 0.5 * ( event.pageY + position.y );

    // 			this._panStart.set( x, y );

    // 		}

    // 	}

    // 	_handleTouchStartDolly( event ) {

    // 		const position = this._getSecondPointerPosition( event );

    // 		const dx = event.pageX - position.x;
    // 		const dy = event.pageY - position.y;

    // 		const distance = Math.sqrt( dx * dx + dy * dy );

    // 		this._dollyStart.set( 0, distance );

    // 	}

    // 	_handleTouchStartDollyPan( event ) {

    // 		if ( this.enableZoom ) this._handleTouchStartDolly( event );

    // 		if ( this.enablePan ) this._handleTouchStartPan( event );

    // 	}

    // 	_handleTouchStartDollyRotate( event ) {

    // 		if ( this.enableZoom ) this._handleTouchStartDolly( event );

    // 		if ( this.enableRotate ) this._handleTouchStartRotate( event );

    // 	}

    // 	_handleTouchMoveRotate( event ) {

    // 		if ( this._pointers.length == 1 ) {

    // 			this._rotateEnd.set( event.pageX, event.pageY );

    // 		} else {

    // 			const position = this._getSecondPointerPosition( event );

    // 			const x = 0.5 * ( event.pageX + position.x );
    // 			const y = 0.5 * ( event.pageY + position.y );

    // 			this._rotateEnd.set( x, y );

    // 		}

    // 		this._rotateDelta.subVectors( this._rotateEnd, this._rotateStart ).multiplyScalar( this.rotateSpeed );

    // 		const element = this.domElement;

    // 		this._rotateLeft( _twoPI * this._rotateDelta.x / element.clientHeight ); // yes, height

    // 		this._rotateUp( _twoPI * this._rotateDelta.y / element.clientHeight );

    // 		this._rotateStart.copy( this._rotateEnd );

    // 	}

    // 	_handleTouchMovePan( event ) {

    // 		if ( this._pointers.length === 1 ) {

    // 			this._panEnd.set( event.pageX, event.pageY );

    // 		} else {

    // 			const position = this._getSecondPointerPosition( event );

    // 			const x = 0.5 * ( event.pageX + position.x );
    // 			const y = 0.5 * ( event.pageY + position.y );

    // 			this._panEnd.set( x, y );

    // 		}

    // 		this._panDelta.subVectors( this._panEnd, this._panStart ).multiplyScalar( this.panSpeed );

    // 		this._pan( this._panDelta.x, this._panDelta.y );

    // 		this._panStart.copy( this._panEnd );

    // 	}

    // 	_handleTouchMoveDolly( event ) {

    // 		const position = this._getSecondPointerPosition( event );

    // 		const dx = event.pageX - position.x;
    // 		const dy = event.pageY - position.y;

    // 		const distance = Math.sqrt( dx * dx + dy * dy );

    // 		this._dollyEnd.set( 0, distance );

    // 		this._dollyDelta.set( 0, Math.pow( this._dollyEnd.y / this._dollyStart.y, this.zoomSpeed ) );

    // 		this._dollyOut( this._dollyDelta.y );

    // 		this._dollyStart.copy( this._dollyEnd );

    // 		const centerX = ( event.pageX + position.x ) * 0.5;
    // 		const centerY = ( event.pageY + position.y ) * 0.5;

    // 		this._updateZoomParameters( centerX, centerY );

    // 	}

    // 	_handleTouchMoveDollyPan( event ) {

    // 		if ( this.enableZoom ) this._handleTouchMoveDolly( event );

    // 		if ( this.enablePan ) this._handleTouchMovePan( event );

    // 	}

    // 	_handleTouchMoveDollyRotate( event ) {

    // 		if ( this.enableZoom ) this._handleTouchMoveDolly( event );

    // 		if ( this.enableRotate ) this._handleTouchMoveRotate( event );

    // 	}

    // 	// pointers

    // 	_addPointer( event ) {

    // 		this._pointers.push( event.pointerId );

    // 	}

    // 	_removePointer( event ) {

    // 		delete this._pointerPositions[ event.pointerId ];

    // 		for ( let i = 0; i < this._pointers.length; i ++ ) {

    // 			if ( this._pointers[ i ] == event.pointerId ) {

    // 				this._pointers.splice( i, 1 );
    // 				return;

    // 			}

    // 		}

    // 	}

    // 	_isTrackingPointer( event ) {

    // 		for ( let i = 0; i < this._pointers.length; i ++ ) {

    // 			if ( this._pointers[ i ] == event.pointerId ) return true;

    // 		}

    // 		return false;

    // 	}

    // 	_trackPointer( event ) {

    // 		let position = this._pointerPositions[ event.pointerId ];

    // 		if ( position === undefined ) {

    // 			position = new Vector2();
    // 			this._pointerPositions[ event.pointerId ] = position;

    // 		}

    // 		position.set( event.pageX, event.pageY );

    // 	}

    // 	_getSecondPointerPosition( event ) {

    // 		const pointerId = ( event.pointerId === this._pointers[ 0 ] ) ? this._pointers[ 1 ] : this._pointers[ 0 ];

    // 		return this._pointerPositions[ pointerId ];

    // 	}

    // 	//

    // 	_customWheelEvent( event ) {

    // 		const mode = event.deltaMode;

    // 		// minimal wheel event altered to meet delta-zoom demand
    // 		const newEvent = {
    // 			clientX: event.clientX,
    // 			clientY: event.clientY,
    // 			deltaY: event.deltaY,
    // 		};

    // 		switch ( mode ) {

    // 			case 1: // LINE_MODE
    // 				newEvent.deltaY *= 16;
    // 				break;

    // 			case 2: // PAGE_MODE
    // 				newEvent.deltaY *= 100;
    // 				break;

    // 		}

    // 		// detect if event was triggered by pinching
    // 		if ( event.ctrlKey && ! this._controlActive ) {

    // 			newEvent.deltaY *= 10;

    // 		}

    // 		return newEvent;

    // 	}

    // }
}

#[derive(Debug, PartialEq, Eq)]
enum State {
    None,
    Rotate,
    Dolly,
    Pan,
    TouchRotate,
    TouchPan,
    TouchDollyPan,
    TouchDollyRotate,
}

// function onPointerDown( event ) {

// 	if ( this.enabled === false ) return;

// 	if ( this._pointers.length === 0 ) {

// 		this.domElement.setPointerCapture( event.pointerId );

// 		this.domElement.ownerDocument.addEventListener( 'pointermove', this._onPointerMove );
// 		this.domElement.ownerDocument.addEventListener( 'pointerup', this._onPointerUp );

// 	}

// 	//

// 	if ( this._isTrackingPointer( event ) ) return;

// 	//

// 	this._addPointer( event );

// 	if ( event.pointerType === 'touch' ) {

// 		this._onTouchStart( event );

// 	} else {

// 		this._onMouseDown( event );

// 	}

// }

// function onPointerMove( event ) {

// 	if ( this.enabled === false ) return;

// 	if ( event.pointerType === 'touch' ) {

// 		this._onTouchMove( event );

// 	} else {

// 		this._onMouseMove( event );

// 	}

// }

// function onPointerUp( event ) {

// 	this._removePointer( event );

// 	switch ( this._pointers.length ) {

// 		case 0:

// 			this.domElement.releasePointerCapture( event.pointerId );

// 			this.domElement.ownerDocument.removeEventListener( 'pointermove', this._onPointerMove );
// 			this.domElement.ownerDocument.removeEventListener( 'pointerup', this._onPointerUp );

// 			this.dispatchEvent( _endEvent );

// 			this.state = _STATE.NONE;

// 			break;

// 		case 1:

// 			const pointerId = this._pointers[ 0 ];
// 			const position = this._pointerPositions[ pointerId ];

// 			// minimal placeholder event - allows state correction on pointer-up
// 			this._onTouchStart( { pointerId: pointerId, pageX: position.x, pageY: position.y } );

// 			break;

// 	}

// }

// function onMouseDown( event ) {

// 	let mouseAction;

// 	switch ( event.button ) {

// 		case 0:

// 			mouseAction = this.mouseButtons.LEFT;
// 			break;

// 		case 1:

// 			mouseAction = this.mouseButtons.MIDDLE;
// 			break;

// 		case 2:

// 			mouseAction = this.mouseButtons.RIGHT;
// 			break;

// 		default:

// 			mouseAction = - 1;

// 	}

// 	switch ( mouseAction ) {

// 		case MOUSE.DOLLY:

// 			if ( this.enableZoom === false ) return;

// 			this._handleMouseDownDolly( event );

// 			this.state = _STATE.DOLLY;

// 			break;

// 		case MOUSE.ROTATE:

// 			if ( event.ctrlKey || event.metaKey || event.shiftKey ) {

// 				if ( this.enablePan === false ) return;

// 				this._handleMouseDownPan( event );

// 				this.state = _STATE.PAN;

// 			} else {

// 				if ( this.enableRotate === false ) return;

// 				this._handleMouseDownRotate( event );

// 				this.state = _STATE.ROTATE;

// 			}

// 			break;

// 		case MOUSE.PAN:

// 			if ( event.ctrlKey || event.metaKey || event.shiftKey ) {

// 				if ( this.enableRotate === false ) return;

// 				this._handleMouseDownRotate( event );

// 				this.state = _STATE.ROTATE;

// 			} else {

// 				if ( this.enablePan === false ) return;

// 				this._handleMouseDownPan( event );

// 				this.state = _STATE.PAN;

// 			}

// 			break;

// 		default:

// 			this.state = _STATE.NONE;

// 	}

// 	if ( this.state !== _STATE.NONE ) {

// 		this.dispatchEvent( _startEvent );

// 	}

// }

// function onMouseMove( event ) {

// 	switch ( this.state ) {

// 		case _STATE.ROTATE:

// 			if ( this.enableRotate === false ) return;

// 			this._handleMouseMoveRotate( event );

// 			break;

// 		case _STATE.DOLLY:

// 			if ( this.enableZoom === false ) return;

// 			this._handleMouseMoveDolly( event );

// 			break;

// 		case _STATE.PAN:

// 			if ( this.enablePan === false ) return;

// 			this._handleMouseMovePan( event );

// 			break;

// 	}

// }

// function onMouseWheel( event ) {

// 	if ( this.enabled === false || this.enableZoom === false || this.state !== _STATE.NONE ) return;

// 	event.preventDefault();

// 	this.dispatchEvent( _startEvent );

// 	this._handleMouseWheel( this._customWheelEvent( event ) );

// 	this.dispatchEvent( _endEvent );

// }

// function onKeyDown( event ) {

// 	if ( this.enabled === false ) return;

// 	this._handleKeyDown( event );

// }

// function onTouchStart( event ) {

// 	this._trackPointer( event );

// 	switch ( this._pointers.length ) {

// 		case 1:

// 			switch ( this.touches.ONE ) {

// 				case TOUCH.ROTATE:

// 					if ( this.enableRotate === false ) return;

// 					this._handleTouchStartRotate( event );

// 					this.state = _STATE.TOUCH_ROTATE;

// 					break;

// 				case TOUCH.PAN:

// 					if ( this.enablePan === false ) return;

// 					this._handleTouchStartPan( event );

// 					this.state = _STATE.TOUCH_PAN;

// 					break;

// 				default:

// 					this.state = _STATE.NONE;

// 			}

// 			break;

// 		case 2:

// 			switch ( this.touches.TWO ) {

// 				case TOUCH.DOLLY_PAN:

// 					if ( this.enableZoom === false && this.enablePan === false ) return;

// 					this._handleTouchStartDollyPan( event );

// 					this.state = _STATE.TOUCH_DOLLY_PAN;

// 					break;

// 				case TOUCH.DOLLY_ROTATE:

// 					if ( this.enableZoom === false && this.enableRotate === false ) return;

// 					this._handleTouchStartDollyRotate( event );

// 					this.state = _STATE.TOUCH_DOLLY_ROTATE;

// 					break;

// 				default:

// 					this.state = _STATE.NONE;

// 			}

// 			break;

// 		default:

// 			this.state = _STATE.NONE;

// 	}

// 	if ( this.state !== _STATE.NONE ) {

// 		this.dispatchEvent( _startEvent );

// 	}

// }

// function onTouchMove( event ) {

// 	this._trackPointer( event );

// 	switch ( this.state ) {

// 		case _STATE.TOUCH_ROTATE:

// 			if ( this.enableRotate === false ) return;

// 			this._handleTouchMoveRotate( event );

// 			this[.Self::update();]

// 			break;

// 		case _STATE.TOUCH_PAN:

// 			if ( this.enablePan === false ) return;

// 			this._handleTouchMovePan( event );

// 			this[.Self::update();]

// 			break;

// 		case _STATE.TOUCH_DOLLY_PAN:

// 			if ( this.enableZoom === false && this.enablePan === false ) return;

// 			this._handleTouchMoveDollyPan( event );

// 			this[.Self::update();]

// 			break;

// 		case _STATE.TOUCH_DOLLY_ROTATE:

// 			if ( this.enableZoom === false && this.enableRotate === false ) return;

// 			this._handleTouchMoveDollyRotate( event );

// 			this[.Self::update();]

// 			break;

// 		default:

// 			this.state = _STATE.NONE;

// 	}

// }

// function onContextMenu( event ) {

// 	if ( this.enabled === false ) return;

// 	event.preventDefault();

// }

// function interceptControlDown( event ) {

// 	if ( event.key === 'Control' ) {

// 		this._controlActive = true;

// 		const document = this.domElement.getRootNode(); // offscreen canvas compatibility

// 		document.addEventListener( 'keyup', this._interceptControlUp, { passive: true, capture: true } );

// 	}

// }

// function interceptControlUp( event ) {

// 	if ( event.key === 'Control' ) {

// 		this._controlActive = false;

// 		const document = this.domElement.getRootNode(); // offscreen canvas compatibility

// 		document.removeEventListener( 'keyup', this._interceptControlUp, { passive: true, capture: true } );

// 	}

// }

// export { OrbitControls };
