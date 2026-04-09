use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tonner::{
    Context,
    world::{TonnerWorld, TonnerWorldHandle},
};
use uuid::Uuid;

pub struct State {
    wgpu_instance: wgpu::Instance,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    storm_ctx: Context,
    surface_format: wgpu::TextureFormat,
    root_window: winit::window::WindowId,
    windows: HashMap<winit::window::WindowId, Window>,
    windows_by_uuid: HashMap<Uuid, winit::window::WindowId>,
}

#[derive(Debug)]
struct Window {
    window: Arc<winit::window::Window>,
    size: winit::dpi::PhysicalSize<u32>,
    surface: wgpu::Surface<'static>,
    world: TonnerWorldHandle,
}

impl Window {
    fn configure(&self, device: &wgpu::Device, surface_format: wgpu::TextureFormat) {
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            view_formats: vec![surface_format.add_srgb_suffix()],
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            width: self.size.width,
            height: self.size.height,
            desired_maximum_frame_latency: 2,
            present_mode: wgpu::PresentMode::AutoVsync,
        };
        self.surface.configure(device, &surface_config);
    }
}

impl State {
    pub fn new(event_loop: &winit::event_loop::ActiveEventLoop) -> State {
        let window = event_loop
            .create_window(winit::window::Window::default_attributes().with_title("Tonner Kernel"))
            .expect("Failed to create a window");
        let window = Arc::new(window);

        let wgpu_instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                Box::new(event_loop.owned_display_handle()),
            ));

        let surface = wgpu_instance
            .create_surface(window.clone())
            .expect("Failed to create the window surface");

        let adapter =
            pollster::block_on(wgpu_instance.request_adapter(&wgpu::RequestAdapterOptions {
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

        let storm_ctx = Context::from_device(device, queue);
        let world = TonnerWorld::new(storm_ctx.clone());
        let window = Window {
            size,
            window,
            surface,
            world: TonnerWorldHandle {
                world: Arc::new(Mutex::new(world)),
            },
        };
        window.configure(storm_ctx.device(), surface_format);

        let egui_renderer = egui_wgpu::Renderer::new(
            storm_ctx.device(),
            surface_format,
            egui_wgpu::RendererOptions::default(),
        );

        let root_window = window.window.id();
        let root_window_id = Uuid::new_v4();

        let mut windows = HashMap::with_capacity(1);
        windows.insert(window.window.id(), window);

        let mut windows_by_uuid = HashMap::with_capacity(1);
        windows_by_uuid.insert(root_window_id, root_window);

        State {
            wgpu_instance,
            egui_state,
            egui_renderer,
            storm_ctx,
            surface_format,
            windows,
            root_window,
            windows_by_uuid,
        }
    }

    pub fn context(&self) -> &Context {
        &self.storm_ctx
    }

    pub fn root_world(&self) -> &TonnerWorldHandle {
        &self.windows[&self.root_window].world
    }

    pub fn add_window(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        id: Uuid,
        world: TonnerWorldHandle,
    ) {
        let window = event_loop
            .create_window(
                winit::window::Window::default_attributes()
                    .with_title(format!("Tonner Kernel ({id})")),
            )
            .expect("Failed to create a window");

        let window = Arc::new(window);
        let surface = self
            .wgpu_instance
            .create_surface(window.clone())
            .expect("Failed to create the window surface");

        let size = window.inner_size();

        let window = Window {
            window,
            size,
            surface,
            world,
        };
        window.configure(self.context().device(), self.surface_format);

        self.windows_by_uuid.insert(id, window.window.id());
        self.windows.insert(window.window.id(), window);
    }

    pub fn close_window(&mut self, id: Uuid) {
        if let Some(window) = self.windows_by_uuid.remove(&id) {
            if window != self.root_window {
                self.windows.remove(&window);
            }
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
        let Some(window) = self.windows.get_mut(&window_id) else {
            return;
        };

        if let winit::event::WindowEvent::Resized(size) = event {
            window.size = size;
            window.configure(self.storm_ctx.device(), self.surface_format);
        }

        let response = self.egui_state.on_window_event(&window.window, &event);

        if response.repaint {
            let raw_input = self.egui_state.take_egui_input(&window.window);

            let full_output = self.egui_state.egui_ctx().run_ui(raw_input, |_ui| ());

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

            let surface_texture = match window.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(texture) => texture,
                wgpu::CurrentSurfaceTexture::Occluded | wgpu::CurrentSurfaceTexture::Timeout => {
                    return;
                }
                wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                    drop(texture);
                    window.configure(self.storm_ctx.device(), self.surface_format);
                    return;
                }
                wgpu::CurrentSurfaceTexture::Outdated => {
                    window.configure(self.storm_ctx.device(), self.surface_format);
                    return;
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    unreachable!("No error scope registered, so validation errors will panic")
                }
                wgpu::CurrentSurfaceTexture::Lost => {
                    window.surface = self
                        .wgpu_instance
                        .create_surface(window.window.clone())
                        .unwrap();
                    window.configure(self.storm_ctx.device(), self.surface_format);
                    return;
                }
            };
            let texture_view = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    format: Some(self.surface_format.add_srgb_suffix()),
                    ..Default::default()
                });

            let mut render_pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Tonner Kernel render() render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &texture_view,
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
                    multiview_mask: None,
                })
                .forget_lifetime();

            self.egui_renderer
                .render(&mut render_pass, &clipped_primitives, &screne_descriptor);

            drop(render_pass);

            command_buffers.push(encoder.finish());

            self.storm_ctx.queue().submit(command_buffers);
            window.window.pre_present_notify();
            surface_texture.present();
        }
    }
}
