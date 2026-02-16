use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;

use pollster::block_on;
use pyo3::prelude::*;
use pyo3_ffi::c_str;
use storm::Context;
use storm::environment::{Environment, EnvironmentBuilder};
use storm::geometry::skin::SkinManager;
use storm::mesh::{MeshInstance, MeshInstanceId};
use storm::renderer::Renderer;
use storm::renderer::camera::Camera;
use storm::renderer::light::LightManager;
use storm::scene_graph::{NodeBuilder, SceneGraph};
use wgpu::Instance;
use winit::dpi::PhysicalSize;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

struct State {
    ctx: Context,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    size: PhysicalSize<u32>,
    renderer: Renderer,
    camera: Camera,
    scene_graph: SceneGraph,
    mesh_instances: HashMap<MeshInstanceId, MeshInstance>,
    skin_manager: SkinManager,
    light_manager: LightManager,
    environment: Environment,
    py_update: Py<PyAny>,
}

impl State {
    async fn new(window: Arc<Window>) -> Self {
        let instance = Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: wgpu::InstanceFlags::from_env_or_default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::from_env_or_default(),
        });

        let surface = instance
            .create_surface(window.clone())
            .expect("failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptionsBase {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("failed to get gpu adapter");

        let size = window.inner_size();
        let capabilities = surface.get_capabilities(&adapter);
        let surface_format = dbg!(capabilities.formats[0]);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .expect("failed to get gpu device");

        let ctx = Context::from_device(device, queue);
        let renderer = Renderer::new(
            size.width,
            size.height,
            surface_format.add_srgb_suffix(),
            &ctx,
        );
        let mut scene_graph = SceneGraph::new(&ctx);
        let camera_node = NodeBuilder::default()
            .name("Camera node")
            .build(&mut scene_graph)
            .unwrap();
        let camera = Camera::new(camera_node);

        let mut encoder = ctx
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Startup command encoder"),
            });
        let radiance_image = image::ImageReader::with_format(
            Cursor::new(include_bytes!("billiard_hall_1k.hdr")),
            image::ImageFormat::Hdr,
        )
        .decode()
        .unwrap();
        let environment = EnvironmentBuilder::default()
            .equirectangular_map(radiance_image)
            .build(&ctx, &mut encoder);

        let py_update = Python::attach(|py| -> PyResult<Py<PyAny>> {
            Ok(PyModule::from_code(
                py,
                c_str!(include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/scripts/update.py"
                ))),
                c"update.py",
                c"",
            )?
            .getattr("update")?
            .into())
        })
        .unwrap();

        ctx.queue().submit([encoder.finish()]);

        let state = State {
            window,
            surface,
            surface_format,
            size,
            renderer,
            camera,
            scene_graph,
            mesh_instances: HashMap::new(),
            skin_manager: SkinManager::new(&ctx),
            light_manager: LightManager::new(&ctx),
            environment,
            ctx: ctx,
            py_update,
        };

        state.configure_surface();

        state
    }

    fn configure_surface(&self) {
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            view_formats: vec![self.surface_format.add_srgb_suffix()],
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            width: self.size.width,
            height: self.size.height,
            desired_maximum_frame_latency: 2,
            present_mode: wgpu::PresentMode::AutoVsync,
        };
        self.surface.configure(self.ctx.device(), &surface_config);
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.configure_surface();
    }

    fn render(&mut self) {
        let surface_texture = self
            .surface
            .get_current_texture()
            .expect("failed to get next swapchain texture");
        let texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("surface texture view"),
                format: Some(self.surface_format.add_srgb_suffix()),
                ..Default::default()
            });

        let mut encoder =
            self.ctx
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render command encoder"),
                });

        Python::attach(|py| -> PyResult<()> {
            self.py_update.call0(py)?;
            Ok(())
        })
        .unwrap();

        self.renderer
            .render(
                &self.camera,
                &texture_view,
                &self.scene_graph,
                &mut self.skin_manager,
                self.mesh_instances.values(),
                &mut self.light_manager,
                &self.environment,
                &self.ctx,
                &mut encoder,
            )
            .expect("failed to render");

        self.ctx.queue().submit([encoder.finish()]);
        self.window.pre_present_notify();
        surface_texture.present();
    }
}

#[derive(Default)]
struct App {
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .expect("failed to create windown"),
        );

        self.state = Some(block_on(State::new(window)));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = self.state.as_mut().unwrap();
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                state.render();
                state.window.request_redraw();
            }
            WindowEvent::Resized(size) => {
                state.resize(size);
            }
            _ => (),
        }
    }
}

fn main() -> Result<(), winit::error::EventLoopError> {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    return event_loop.run_app(&mut app);
}
