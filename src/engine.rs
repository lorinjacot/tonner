use std::sync::Arc;
use std::time::Instant;

use glam::vec3;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::window::Window;

use crate::asset::primitive::{DrawPrimitives, PrimitiveManager};
use crate::camera::{Camera, CameraController};

pub struct Engine {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    window: Arc<Window>,
    last_frame: Instant,
    camera: Camera,
    camera_controller: CameraController,
    primitive_manager: PrimitiveManager,
}

impl Engine {
    pub async fn new(window: Window) -> Self {
        let window = Arc::new(window);

        let mut size = window.inner_size();
        size.width = size.width.max(1);
        size.height = size.height.max(1);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptionsBase {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults()
                        .using_resolution(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];
        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();
        surface.configure(&device, &config);

        // let asset = Asset::import("assets/Box.glb").expect("Failed to import Box.glb");
        // let mesh = asset.document.meshes().next().unwrap().primitives().next().unwrap();
        // let mesh = mesh::MeshPrimitive::from_gltf(
        //     &mesh, &asset, &device
        // );

        let (document, buffers, _images) = gltf::import("assets/Box.glb").unwrap();

        let mut primitive_manager =
            PrimitiveManager::new(&device, &[Some(swapchain_format.into())]);
        let primitive = document
            .meshes()
            .next()
            .unwrap()
            .primitives()
            .next()
            .unwrap();
        primitive_manager.load(&primitive, &device, &buffers);

        let camera = Camera::new(
            vec3(0.0, 0.0, -10.0),
            config.width as f32 / config.height as f32,
            &device,
            &queue,
        );
        let camera_controller = CameraController::new();

        let last_frame = Instant::now();

        Self {
            device,
            queue,
            surface,
            config,
            window,
            last_frame,
            camera,
            camera_controller,
            primitive_manager,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn window_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::RedrawRequested => {
                self.update();
                self.draw();
                self.window.request_redraw();
                true
            }
            WindowEvent::Resized(new_size) => {
                self.config.width = new_size.width.max(1);
                self.config.height = new_size.height.max(1);
                self.surface.configure(&self.device, &self.config);

                self.camera
                    .set_aspect_ration(self.config.width as f32 / self.config.height as f32);

                self.window.request_redraw();
                true
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.camera_controller.keyboard_input(event)
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => self
                .camera_controller
                .mouse_input(*state == ElementState::Pressed),
            _ => false,
        }
    }

    pub fn device_event(&mut self, event: &DeviceEvent) -> bool {
        match event {
            DeviceEvent::MouseMotion { delta } => self
                .camera_controller
                .mouse_move(delta.0 as f32, delta.1 as f32),
            _ => false,
        }
    }

    fn update(&mut self) {
        let delta_time = self.last_frame.elapsed();
        self.last_frame = Instant::now();

        self.camera_controller
            .update(&mut self.camera, delta_time, &self.queue);
    }

    fn draw(&mut self) {
        let frame = self
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.draw_primitives(&self.primitive_manager, &self.camera);
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}
