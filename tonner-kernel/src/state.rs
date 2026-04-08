use std::{collections::HashMap, sync::Arc};

use tonner::Context;

pub struct State {
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    storm_ctx: Context,
    surface_format: wgpu::TextureFormat,
    windows: HashMap<winit::window::WindowId, Window>,
}

#[derive(Debug)]
struct Window {
    window: Arc<winit::window::Window>,
    size: winit::dpi::PhysicalSize<u32>,
    surface: wgpu::Surface<'static>,
}

impl Window {
    fn configure(
        &mut self,
        size: winit::dpi::PhysicalSize<u32>,
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) {
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            // Request compatibility with the sRGB-format texture view we‘re going to create later.
            view_formats: vec![surface_format.add_srgb_suffix()],
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            width: size.width,
            height: size.height,
            desired_maximum_frame_latency: 2,
            present_mode: wgpu::PresentMode::AutoVsync,
        };
        self.size = size;
        self.surface.configure(device, &surface_config);
    }
}

impl State {
    pub fn new(event_loop: &winit::event_loop::ActiveEventLoop) -> State {
        let window = event_loop
            .create_window(winit::window::Window::default_attributes().with_title("Tonner Kernel"))
            .expect("Failed to create a window");
        let window = Arc::new(window);

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: wgpu::InstanceFlags::from_env_or_default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::from_env_or_default(),
        });

        let surface = instance
            .create_surface(window.clone())
            .expect("Failed to create the window surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::None,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .expect("Failed to get GPU adapter");

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("Failed to get GPU handle");

        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx,
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(device.limits().max_texture_dimension_2d as usize),
        );

        let surface_format = surface.get_capabilities(&adapter).formats[0];
        let size = window.inner_size();
        let mut window = Window {
            size,
            window,
            surface,
        };
        window.configure(size, &device, surface_format);

        let egui_renderer = egui_wgpu::Renderer::new(
            &device,
            surface_format,
            egui_wgpu::RendererOptions::default(),
        );

        let storm_ctx = Context::from_device(device, queue);

        let mut windows = HashMap::with_capacity(1);
        windows.insert(window.window.id(), window);

        State {
            egui_state,
            egui_renderer,
            storm_ctx,
            surface_format,
            windows,
        }
    }

    pub fn on_mouse_motion(&mut self, delta: (f64, f64)) {
        self.egui_state.on_mouse_motion(delta);
    }

    pub fn on_window_event(
        &mut self,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let window = self.windows.get_mut(&window_id).unwrap();

        if let winit::event::WindowEvent::Resized(size) = event {
            window.configure(size, self.storm_ctx.device(), self.surface_format);
        }

        let response = self.egui_state.on_window_event(&window.window, &event);

        if response.repaint {
            let raw_input = self.egui_state.take_egui_input(&window.window);

            let full_output = self.egui_state.egui_ctx().run(raw_input, |ctx| {
                egui::CentralPanel::default().show(&ctx, |ui| {
                    ui.label("Hello world!");
                    if ui.button("Click me").clicked() {
                        // take some action here
                    }
                });
            });

            self.egui_state
                .handle_platform_output(&window.window, full_output.platform_output);

            let clipped_primitives = self
                .egui_state
                .egui_ctx()
                .tessellate(full_output.shapes, full_output.pixels_per_point);

            let mut encoder =
                self.storm_ctx
                    .device()
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Tonner Kernel render() command encoder"),
                    });

            let screne_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [window.size.width, window.size.height],
                pixels_per_point: self.egui_state.egui_ctx().pixels_per_point(),
            };

            let mut command_buffers = self.egui_renderer.update_buffers(
                self.storm_ctx.device(),
                self.storm_ctx.queue(),
                &mut encoder,
                &clipped_primitives,
                &screne_descriptor,
            );

            full_output.textures_delta.free.iter().for_each(|id| {
                self.egui_renderer.free_texture(id);
            });

            full_output
                .textures_delta
                .set
                .iter()
                .for_each(|(id, delta)| {
                    self.egui_renderer.update_texture(
                        self.storm_ctx.device(),
                        self.storm_ctx.queue(),
                        *id,
                        delta,
                    );
                });

            let texture = window.surface.get_current_texture().unwrap();
            let view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("Tonner window surface texture view"),
                ..Default::default()
            });

            let mut render_pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Tonner Kernel render() render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();

            self.egui_renderer
                .render(&mut render_pass, &clipped_primitives, &screne_descriptor);

            drop(render_pass);

            command_buffers.push(encoder.finish());

            self.storm_ctx.queue().submit(command_buffers);
            window.window.pre_present_notify();
            texture.present();
        }
    }
}
