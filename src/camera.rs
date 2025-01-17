use std::{
    f32::consts::{FRAC_PI_2, FRAC_PI_4, PI},
    time::Duration,
};

use bytemuck::{Pod, Zeroable};
use glam::{vec3, Mat4, Vec3, Vec4};
use winit::{
    event::{ElementState, KeyEvent},
    keyboard::{KeyCode, PhysicalKey},
};

pub struct CameraController {
    forward: bool,
    backward: bool,
    right: bool,
    left: bool,
    up: bool,
    down: bool,
    turn_left: bool,
    turn_right: bool,
    turn_up: bool,
    turn_down: bool,
    mouvement_speed: f32,
    rotation_speed: f32,
    mouse_pressed: bool,
    mouse_sensitivity: f32,
    mouse_dx: f32,
    mouse_dy: f32,
}

impl CameraController {
    pub fn new() -> Self {
        Self {
            forward: false,
            backward: false,
            right: false,
            left: false,
            up: false,
            down: false,
            turn_left: false,
            turn_right: false,
            turn_up: false,
            turn_down: false,
            mouvement_speed: 2.5,
            rotation_speed: 1.0,
            mouse_pressed: false,
            mouse_sensitivity: 0.003,
            mouse_dx: 0.0,
            mouse_dy: 0.0,
        }
    }

    pub fn keyboard_input(&mut self, event: &KeyEvent) -> bool {
        if let PhysicalKey::Code(key_code) = event.physical_key {
            match key_code {
                KeyCode::KeyW => self.forward = event.state == ElementState::Pressed,
                KeyCode::KeyS => self.backward = event.state == ElementState::Pressed,
                KeyCode::KeyA => self.left = event.state == ElementState::Pressed,
                KeyCode::KeyD => self.right = event.state == ElementState::Pressed,
                KeyCode::KeyR => self.up = event.state == ElementState::Pressed,
                KeyCode::KeyF => self.down = event.state == ElementState::Pressed,
                KeyCode::ArrowLeft => self.turn_left = event.state == ElementState::Pressed,
                KeyCode::ArrowRight => self.turn_right = event.state == ElementState::Pressed,
                KeyCode::ArrowUp => self.turn_up = event.state == ElementState::Pressed,
                KeyCode::ArrowDown => self.turn_down = event.state == ElementState::Pressed,
                _ => {
                    return false;
                }
            }
            return true;
        }
        return false;
    }

    pub fn mouse_input(&mut self, pressed: bool) -> bool {
        self.mouse_pressed = pressed;
        true
    }

    pub fn mouse_move(&mut self, dx: f32, dy: f32) -> bool {
        if self.mouse_pressed {
            self.mouse_dx += dx;
            self.mouse_dy += dy;
        }
        return self.mouse_pressed;
    }

    pub fn update(&mut self, camera: &mut Camera, delta_time: Duration, queue: &wgpu::Queue) {
        let da = self.rotation_speed * delta_time.as_secs_f32();
        if self.turn_left {
            camera.yaw += da;
        } else if self.turn_right {
            camera.yaw -= da;
        } else {
            camera.yaw -= self.mouse_dx * self.mouse_sensitivity;
        }
        if self.turn_up {
            camera.pitch -= da;
        } else if self.turn_down {
            camera.pitch += da;
        } else {
            camera.pitch -= self.mouse_dy * self.mouse_sensitivity;
        }
        camera.pitch = f32::max(0.1, f32::min(PI - 0.1, camera.pitch));
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;

        camera.front = vec3(
            camera.pitch.sin() * camera.yaw.sin(),
            camera.pitch.cos(),
            camera.pitch.sin() * camera.yaw.cos(),
        );
        camera.left = Vec3::cross(Vec3::Y, camera.front).normalize();
        camera.up = Vec3::cross(camera.front, camera.left).normalize();

        let dx = self.mouvement_speed * delta_time.as_secs_f32();
        if self.forward {
            camera.position += dx * camera.front;
        } else if self.backward {
            camera.position -= dx * camera.front
        }
        if self.left {
            camera.position += dx * camera.left;
        } else if self.right {
            camera.position -= dx * camera.left;
        }
        if self.up {
            camera.position += dx * camera.up;
        } else if self.down {
            camera.position -= dx * camera.up;
        }

        camera.update_buffer(queue);
    }
}

pub struct Camera {
    position: Vec3,
    front: Vec3,
    left: Vec3,
    up: Vec3,
    aspect_ration: f32,
    fov: f32,
    z_near: f32,
    z_far: f32,
    yaw: f32,
    pitch: f32,
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl Camera {
    pub fn new(
        position: Vec3,
        aspect_ration: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Self {
        let front = -Vec3::Z;
        let left = -Vec3::X;
        let up = Vec3::Y;

        let pitch = FRAC_PI_2;
        let yaw = PI;
        let fov = FRAC_PI_4;
        let z_near = 0.1;
        let z_far = 100.0;

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera uniform buffer"),
            size: 4 * 4 * 4 + 4 * 4,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera uniform bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera uniform bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        let camera = Self {
            position,
            front,
            up,
            left,
            yaw,
            pitch,
            fov,
            aspect_ration,
            z_near,
            z_far,
            buffer,
            bind_group,
            bind_group_layout,
        };

        camera.update_buffer(queue);

        camera
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn set_aspect_ration(&mut self, aspect_ration: f32) {
        self.aspect_ration = aspect_ration;
    }

    fn update_buffer(&self, queue: &wgpu::Queue) {
        let view = Mat4::look_to_rh(self.position, self.front, self.up);
        let projection =
            Mat4::perspective_rh(self.fov, self.aspect_ration, self.z_near, self.z_far);
        let uniform = CameraUniform {
            view_projection: projection * view,
            world_position: self.position.extend(0.0),
        };
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[uniform]));
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    view_projection: Mat4,
    world_position: Vec4,
}
