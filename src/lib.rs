use std::path::PathBuf;
use std::sync::Arc;

use egui::ViewportBuilder;
use egui_wgpu::ScreenDescriptor;
use egui_winit::create_window;
use winit::application::ApplicationHandler;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;

mod asset;
mod camera;
mod engine;
mod scene;
mod storage;
mod storm;
mod texture;

pub fn run(load_asset: Option<PathBuf>) {
    let wgpu_instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        flags: wgpu::InstanceFlags::debugging(),
        backend_options: wgpu::BackendOptions::default(),
    });

    let mut app = App {
        wgpu_instance,
        engine: None,
        load_asset,
    };

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut app).unwrap();
}

struct App {
    wgpu_instance: wgpu::Instance,
    engine: Option<Engine>,
    load_asset: Option<PathBuf>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.engine.is_none() {
            self.engine = Some(pollster::block_on(Engine::new(
                event_loop,
                &self.wgpu_instance,
            )));
        }
    }

    fn suspended(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        self.engine = None;
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        if let Some(engine) = self.engine.as_mut() {
            assert_eq!(window_id, engine.window.id());
            let event_response = engine.egui_state.on_window_event(&engine.window, &event);
            if !event_response.consumed {
                match event {
                    winit::event::WindowEvent::CloseRequested => event_loop.exit(),
                    winit::event::WindowEvent::Resized(size) => {
                        engine.surface_config.width = size.width.max(1);
                        engine.surface_config.height = size.height.max(1);
                        engine
                            .surface
                            .configure(&engine.device, &engine.surface_config);
                    }
                    _ => (),
                }
            }
            if event_response.repaint {
                engine.draw();
                engine.window.request_redraw();
            }
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        if let Some(engine) = self.engine.as_mut() {
            match event {
                winit::event::DeviceEvent::MouseMotion { delta } => {
                    engine.egui_state.on_mouse_motion(delta)
                }
                _ => (),
            }
        }
    }
}

struct Engine {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
}

impl Engine {
    async fn new(
        event_loop: &winit::event_loop::ActiveEventLoop,
        wgpu_instance: &wgpu::Instance,
    ) -> Self {
        let egui_ctx = egui::Context::default();
        let window = create_window(
            &egui_ctx,
            event_loop,
            &ViewportBuilder::default().with_maximized(true),
        )
        .unwrap();
        let window = Arc::new(window);

        let surface = wgpu_instance
            .create_surface(window.clone())
            .expect("Failed to create surface");

        let adapter = wgpu_instance
            .request_adapter(&wgpu::RequestAdapterOptionsBase {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to get wgpu adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .expect("Failed to get wgpu device");

        let mut size = window.inner_size();
        size.width = size.width.max(1);
        size.height = size.height.max(1);

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];
        let surface_config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();
        surface.configure(&device, &surface_config);

        dbg!(
            device.limits().max_texture_dimension_2d,
            adapter.limits().max_texture_dimension_2d
        );
        let egui_state = egui_winit::State::new(
            egui_ctx,
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(device.limits().max_texture_dimension_2d as usize),
            // None,
        );

        let egui_renderer = egui_wgpu::Renderer::new(&device, swapchain_format, None, 1, true);

        Self {
            device,
            queue,
            window,
            surface,
            surface_config,
            egui_state,
            egui_renderer,
        }
    }

    fn draw(&mut self) {
        let raw_input = self.egui_state.take_egui_input(&self.window);

        let full_output = self.egui_state.egui_ctx().run(raw_input, |ctx| {
            egui::CentralPanel::default().show(&ctx, |ui| {
                ui.label("Hello world!");
                if ui.button("Click me").clicked() {
                    // take some action here
                }
            });
        });

        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);
        let clipped_primitives = self
            .egui_state
            .egui_ctx()
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let mut command_encoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Engine::draw command encoder"),
                });

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point: full_output.pixels_per_point,
        };
        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut command_encoder,
            &clipped_primitives,
            &screen_descriptor,
        );
        for (id, image_delta) in full_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, id, &image_delta);
        }
        for id in full_output.textures_delta.free {
            self.egui_renderer.free_texture(&id);
        }

        let frame = self
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");

        {
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let egui_render_pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.egui_renderer.render(
                &mut egui_render_pass.forget_lifetime(),
                &clipped_primitives,
                &screen_descriptor,
            );
        }

        self.queue.submit([command_encoder.finish()]);
        frame.present();
    }
}
