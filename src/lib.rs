use std::sync::Arc;
use std::time::Instant;

use asset::primitive::{DrawPrimitives, PrimitiveManager};
use camera::Camera;
use glam::vec3;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

mod asset;
mod camera;

struct Engine {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    window: Arc<Window>,
    primitive_manager: PrimitiveManager,
    camera: Camera,
    last_frame: Instant,
}

impl Engine {
    async fn new(window: Arc<Window>) -> Self {
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
            vec3(0.0, 0.0, 10.0),
            vec3(0.0, 0.0, 1.0),
            config.width as f32 / config.height as f32,
            &device,
            &queue,
        );

        let last_frame = Instant::now();

        Self {
            device,
            queue,
            surface,
            config,
            window,
            primitive_manager,
            camera,
            last_frame,
        }
    }

    fn resized(&mut self, new_size: PhysicalSize<u32>) {
        self.config.width = new_size.width.max(1);
        self.config.height = new_size.height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.window.request_redraw();
    }

    fn window_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                self.camera.process_keyboard_event(event, &self.queue)
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.camera
                    .set_mouse_pressed(*state == ElementState::Pressed);
                true
            }
            _ => false,
        }
    }

    fn device_event(&mut self, event: &DeviceEvent) -> bool {
        match event {
            DeviceEvent::MouseMotion { delta } => {
                self.camera
                    .process_mouse_event(delta.0 as f32, delta.1 as f32, &self.queue);
                true
            }
            _ => false,
        }
    }

    fn update(&mut self) {
        let _delta_time = self.last_frame.elapsed();
        self.last_frame = Instant::now();
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

#[derive(Default)]
struct App {
    engine: Option<Engine>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );
        self.engine = Some(pollster::block_on(Engine::new(window)));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let engine = self
            .engine
            .as_mut()
            .expect("Engine should have been created");
        if engine.window.id() != id {
            panic!("the only window should be engine.window");
        }
        if !engine.window_event(&event) {
            match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key: PhysicalKey::Code(KeyCode::Escape),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    event_loop.exit();
                }
                WindowEvent::Resized(new_size) => {
                    engine.resized(new_size);
                    engine.window.request_redraw();
                }
                WindowEvent::RedrawRequested => {
                    // Redraw the application.
                    //
                    // It's preferable for applications that do not render continuously to render in
                    // this event rather than in AboutToWait, since rendering in here allows
                    // the program to gracefully handle redraws requested by the OS.

                    // Draw.
                    engine.update();
                    engine.draw();

                    // Queue a RedrawRequested event.
                    //
                    // You only need to call this if you've determined that you need to redraw in
                    // applications which do not always need to. Applications that redraw continuously
                    // can render here instead.
                    // self.window.as_ref().unwrap().request_redraw();
                    engine.window.request_redraw();
                }
                _ => (),
            }
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        let engine = self
            .engine
            .as_mut()
            .expect("Engine should have been created");
        engine.device_event(&event);
    }
}

pub fn run() {
    let event_loop = EventLoop::new().unwrap();

    // ControlFlow::Poll continuously runs the event loop, even if the OS hasn't
    // dispatched any events. This is ideal for games and similar applications.
    event_loop.set_control_flow(ControlFlow::Poll);

    // ControlFlow::Wait pauses the event loop if no events are available to process.
    // This is ideal for non-game applications that only update in response to user
    // input, and uses significantly less power/CPU time than ControlFlow::Poll.
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
