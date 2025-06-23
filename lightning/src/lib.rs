use std::path::PathBuf;
use std::sync::{Arc, Mutex};
// use std::thread;
use std::time::Instant;

use controls::{Controls, OrbitControls};
use egui::ViewportBuilder;
use egui_wgpu::ScreenDescriptor;
use egui_winit::create_window;
use explorer::Explorer;
use glam::Vec3;
use storm::{DenseEntry, Resources, Scene, open_gltf};
use winit::application::ApplicationHandler;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;

mod controls;
mod explorer;

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

const RENDER_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

struct App {
    wgpu_instance: wgpu::Instance,
    engine: Option<Engine>,
    load_asset: Option<PathBuf>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.engine.is_none() {
            self.engine = Some(pollster::block_on(Engine::new(
                self.load_asset.take(),
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
    data: Arc<EngineData>,
    last_frame: Instant,
    explorer: Explorer,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    render_texture: wgpu::Texture,
    render_texture_view: wgpu::TextureView,
    render_texture_id: egui::TextureId,
}

struct EngineData {
    resources: Mutex<Resources>,
    scenes: Mutex<Scenes>,
    controls: Mutex<Vec<OrbitControls>>,
}

struct Scenes {
    all: Vec<Scene>,
    active: Option<usize>,
}

impl Engine {
    async fn new(
        load_asset: Option<PathBuf>,
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
                    required_limits: wgpu::Limits::default(),
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

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Engine::new command encoder"),
        });
        let resources = Mutex::new(Resources::new(
            RENDER_TEXTURE_FORMAT,
            device.clone(),
            queue.clone(),
            &mut encoder,
        ));
        queue.submit([encoder.finish()]);
        let scenes = Mutex::new(Scenes {
            all: Vec::new(),
            active: None,
        });
        let controls = Mutex::new(Vec::new());

        let data = Arc::new(EngineData {
            resources,
            scenes,
            controls,
        });

        // background scene loading
        let thread_data = data.clone();
        let thread_device = device.clone();
        let thread_queue = queue.clone();
        // thread::spawn(move || {
        let mut encoder = thread_device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Engine::new background thread command encoder"),
        });

        let mut resources = thread_data.resources.lock().unwrap();

        let radiance_image = include_bytes!("../../asset/environments/newport_loft.hdr");
        // let radiance_image = include_bytes!("../../asset/environments/Cannon_Exterior.hdr");
        let radiance_image = std::io::Cursor::new(radiance_image);
        let radiance_image = image::codecs::hdr::HdrDecoder::new(radiance_image).unwrap();
        let radiance_image = image::DynamicImage::from_decoder(radiance_image).unwrap();
        let environment = resources
            .environment_builder()
            .name("newport_loft".to_string())
            .from_equirectangular_map(&radiance_image)
            .build(&mut encoder)
            .id();

        let (mut scenes, active_scene) = load_asset.map_or_else(
            || (Vec::new(), None),
            |path| match open_gltf(path, &mut resources, &mut encoder, size.width, size.height) {
                Ok(value) => value,
                Err(err) => {
                    eprintln!("{err}");
                    panic!()
                }
            },
        );

        let controls = scenes.iter_mut().enumerate().map(|(index, scene)| {
            scene.set_environment(environment, &resources);

            let target = scene
                .node_builder()
                .name("Orbit camera target".to_string())
                .build(&resources)
                .id();
            let cursor = scene
                .node_builder()
                .name("Orbit camera cursor".to_string())
                .build(&resources)
                .id();
            let camera = scene
                .node_builder()
                .name("Orbit camera node".to_string())
                .local_position(1.5 * Vec3::Z)
                .camera(
                    storm::camera::CameraDescriptor {
                        name: Some("Orbit camera".to_string()),
                        projection: storm::camera::Projection::Perspective {
                            aspect_ratio: None,
                            y_fov: f32::to_radians(65.0),
                            z_far: None,
                            z_near: 0.01,
                        },
                    }
                    .into(),
                )
                .build(&resources)
                .id();
            scene.set_active_camera(camera.into());

            OrbitControls::new(index, scene, target, cursor, camera)
        });

        thread_data.controls.lock().unwrap().extend(controls);
        let mut engine_scenes = thread_data.scenes.lock().unwrap();
        engine_scenes.all.extend(scenes);
        engine_scenes.active = active_scene;

        thread_queue.submit([encoder.finish()]);
        // });

        let explorer = Explorer::new();

        let egui_state = egui_winit::State::new(
            egui_ctx,
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(device.limits().max_texture_dimension_2d as usize),
        );

        let mut egui_renderer = egui_wgpu::Renderer::new(&device, swapchain_format, None, 1, true);

        let (render_texture, render_texture_view, render_texture_id) =
            create_render_texture(size.width, size.height, &mut egui_renderer, &device);

        Self {
            device,
            queue,
            window,
            surface,
            surface_config,
            data,
            last_frame: Instant::now(),
            explorer,
            egui_state,
            egui_renderer,
            render_texture,
            render_texture_view,
            render_texture_id,
        }
    }

    fn draw(&mut self) {
        let mut command_encoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Engine::draw command encoder"),
                });

        let scenes = &mut *self.data.scenes.lock().unwrap();
        let mut controls = self.data.controls.lock().unwrap();

        let delta_time = self.last_frame.elapsed();
        self.last_frame = Instant::now();
        let raw_input = self.egui_state.take_egui_input(&self.window);

        let full_output = self.egui_state.egui_ctx().run(raw_input, |ctx| {
            egui::SidePanel::left("explorer").show(ctx, |ui| {
                self.explorer.ui(
                    ui,
                    &mut scenes.all,
                    &mut scenes.active,
                    &self.data.resources,
                )
            });
            if let Some(scene_index) = scenes.active {
                egui::CentralPanel::default().show(&ctx, |ui| {
                    let scene = &mut scenes.all[scene_index];
                    let size = match scene.aspect_ratio() {
                        Some(aspect_ratio) => {
                            let width = ui.available_width();
                            let height = ui.available_height();
                            egui::vec2(
                                width.min(height * aspect_ratio),
                                height.min(width / aspect_ratio),
                            )
                        }
                        None => ui.available_size(),
                    };
                    let width = (size.x * ui.pixels_per_point()) as u32;
                    let height = (size.y * ui.pixels_per_point()) as u32;
                    if width != self.render_texture.width()
                        || height != self.render_texture.height()
                    {
                        (
                            self.render_texture,
                            self.render_texture_view,
                            self.render_texture_id,
                        ) = create_render_texture(
                            width,
                            height,
                            &mut self.egui_renderer,
                            &self.device,
                        );
                        scene.set_render_dimension(
                            width,
                            height,
                            &mut self.data.resources.lock().unwrap(),
                            &mut command_encoder,
                        );
                    }

                    ui.horizontal_centered(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.image((self.render_texture_id, size));
                            let response = ui.interact(
                                ui.clip_rect(),
                                egui::Id::new("render region"),
                                egui::Sense::all(),
                            );
                            if response.hovered() {
                                ui.input_mut(|inputs| {
                                    for controls in controls.iter_mut() {
                                        if controls.scene() == scene_index {
                                            controls.take_input(inputs, size, scene);
                                        }
                                    }
                                });
                            }
                        });
                    });
                });
            }
        });

        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);
        let clipped_primitives = self
            .egui_state
            .egui_ctx()
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        if let Some(scene_index) = scenes.active {
            let scene = &mut scenes.all[scene_index];
            let viewport_aspect_ratio =
                self.render_texture.width() as f32 / self.render_texture.height() as f32;
            for controls in controls.iter_mut() {
                if controls.scene() == scene_index {
                    controls.update(viewport_aspect_ratio, scene);
                }
            }
            scene.update(delta_time, viewport_aspect_ratio);
        }

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

        if let Some(scene_index) = scenes.active {
            let scene = &scenes.all[scene_index];
            scene.render(&self.render_texture_view, &mut command_encoder);
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

fn create_render_texture(
    width: u32,
    height: u32,
    egui_renderer: &mut egui_wgpu::Renderer,
    device: &wgpu::Device,
) -> (wgpu::Texture, wgpu::TextureView, egui::TextureId) {
    let render_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Render texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: RENDER_TEXTURE_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let render_texture_view = render_texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Render texture view"),
        ..Default::default()
    });

    let render_texture_id = egui_renderer.register_native_texture(
        device,
        &render_texture_view,
        wgpu::FilterMode::Nearest,
    );

    (render_texture, render_texture_view, render_texture_id)
}
