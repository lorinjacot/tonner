use core::f32;

use bytemuck::{Pod, Zeroable};
use glam::{vec3, Mat4, Vec3};
use winit::{
    event::{ElementState, KeyEvent},
    keyboard::{KeyCode, PhysicalKey},
};

pub struct Camera {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    position: Vec3,
    front: Vec3,
    up: Vec3,
    right: Vec3,
    world_up: Vec3,
    yaw: f32,
    pitch: f32,
    fov: f32,
    translation_speed: f32,
    rotation_speed: f32,
    aspect_ration: f32,
    z_near: f32,
    z_far: f32,
    mouse_pressed: bool,
}

impl Camera {
    pub fn new(
        position: Vec3,
        front: Vec3,
        aspect_ration: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera uniform buffer"),
            size: 4 * 4 * 4,
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

        let mut camera = Self {
            buffer,
            bind_group,
            position,
            fov: f32::to_radians(45.0),
            aspect_ration,
            z_near: 0.1,
            z_far: 100.0,
            front,
            up: Vec3::Y,
            right: Vec3::X,
            world_up: Vec3::Y,
            yaw: -f32::consts::FRAC_PI_2,
            pitch: 0.0,
            translation_speed: 0.25,
            rotation_speed: 0.01,
            mouse_pressed: false,
        };

        camera.update_vectors();
        camera.update_buffer(queue);

        camera
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn process_keyboard_event(&mut self, event: &KeyEvent, queue: &wgpu::Queue) -> bool {
        if let KeyEvent {
            physical_key: PhysicalKey::Code(code),
            state: ElementState::Pressed,
            ..
        } = event
        {
            match code {
                KeyCode::KeyW => {
                    self.position += self.front * self.translation_speed;
                    self.update_buffer(queue);
                    true
                }
                KeyCode::KeyS => {
                    self.position -= self.front * self.translation_speed;
                    self.update_buffer(queue);
                    true
                }
                KeyCode::KeyA => {
                    self.position -= self.right * self.translation_speed;
                    self.update_buffer(queue);
                    true
                }
                KeyCode::KeyD => {
                    self.position += self.right * self.translation_speed;
                    self.update_buffer(queue);
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }

    pub fn set_mouse_pressed(&mut self, pressed: bool) {
        self.mouse_pressed = pressed;
    }

    pub fn process_mouse_event(&mut self, delta_x: f32, delta_y: f32, queue: &wgpu::Queue) {
        if self.mouse_pressed {
            self.yaw -= self.rotation_speed * delta_x;
            self.pitch -= self.rotation_speed * delta_y;

            if self.pitch > f32::consts::FRAC_PI_2 - 0.1 {
                self.pitch = f32::consts::FRAC_PI_2 - 0.1;
            }
            if self.pitch < -f32::consts::FRAC_PI_2 + 0.1 {
                self.pitch = -f32::consts::FRAC_PI_2 + 0.1;
            }

            self.update_vectors();
            self.update_buffer(queue);
        };
    }

    fn update_vectors(&mut self) {
        let front = vec3(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        );
        self.front = front.normalize();
        self.right = self.front.cross(self.world_up);
        self.up = self.right.cross(self.front);
    }

    fn update_buffer(&self, queue: &wgpu::Queue) {
        let view = Mat4::look_at_rh(self.position, self.position + self.front, self.up);
        let projection =
            Mat4::perspective_rh(self.fov, self.aspect_ration, self.z_near, self.z_far);
        let uniform = CameraUniform {
            view_projection: projection * view,
        };
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[uniform]));
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    view_projection: Mat4,
}
