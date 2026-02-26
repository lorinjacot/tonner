use std::io::Cursor;
use std::sync::Arc;
use std::time::Instant;

use glam::{Vec3, vec3};
use pollster::block_on;
use pyo3::prelude::*;
use storm::Context;
use storm::environment::{Environment, EnvironmentBuilder};
use storm::geometry::skin::SkinManager;
use storm::geometry::{GeometryBuilder, SphereBuilder};
use storm::mesh::material::MaterialBuilder;
use storm::mesh::{MeshBuilder, MeshInstance};
use storm::renderer::Renderer;
use storm::renderer::camera::Camera;
use storm::renderer::light::LightManager;
use storm::scene_graph::{NodeBuilder, PyNode, SceneGraph};
use wgpu::Instance;
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, MouseScrollDelta};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use crate::ball::Ball;

mod ball;
mod python;

const MAX_DELTA_TIME: f32 = 1.0 / 60.0;
const MIN_DELTA_TIME: f32 = 0.001;

struct State {
    ctx: Context,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    size: PhysicalSize<u32>,
    renderer: Renderer,
    camera: Camera,
    scene_graph: Py<SceneGraph>,
    skin_manager: SkinManager,
    light_manager: LightManager,
    environment: Environment,
    scripts: python::PyScripts,
    camera_node: Py<PyNode>,
    balls: Vec<Py<Ball>>,
    mesh_instances: Vec<MeshInstance>,
    last_render: Instant,
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
        let surface_format = capabilities.formats[0];

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
            .local_translation(Vec3::X)
            .build(&mut scene_graph)
            .unwrap();
        let camera = Camera::new(camera_node);

        let mut balls = Vec::new();
        let mut mesh_instances = Vec::new();
        let ball = SphereBuilder::default()
            .name("Ball")
            .radius(0.025)
            .build(&ctx);

        let table_node = NodeBuilder::default()
            .name("Table")
            .build(&mut scene_graph)
            .unwrap();

        #[rustfmt::skip]
        let table = GeometryBuilder::new(8, 0)
            .name("Table")
            .positions([
                vec3(-0.65, 0.0, -1.25),
                vec3(-0.65, 0.0, 1.25),
                vec3(0.65, 0.0, 1.25),
                vec3(0.65, 0.0, -1.25),
                vec3(-0.65, -0.1, -1.25),
                vec3(-0.65, -0.1, 1.25),
                vec3(0.65, -0.1, 1.25),
                vec3(0.65, -0.1, -1.25),
            ])
            .unwrap()
            .indices_u16([
                // top face
                0, 1, 2,
                2, 3, 0,
                // large side 1
                1, 0, 4,
                4, 5, 1,
                // large side 2
                3, 2, 6,
                6, 7, 3,
                // small side 1
                2, 1, 5, 
                5, 6, 2, 
                // small side 2
                0, 3, 7, 
                7, 4, 0, 
                // bottom face
                4, 6, 5, 
                6, 4, 7,
            ])
            .build(&ctx)
            .unwrap();
        let table_material = MaterialBuilder::default()
            .name("Table")
            .base_color_factor([1.0, 0.0, 0.0, 1.0])
            .metallic_factor(1.0)
            .roughness_factor(0.2)
            .double_sided(false)
            .build(&ctx);
        let table = MeshBuilder::default()
            .name("Table")
            .primitive(table, table_material)
            .build(&ctx)
            .unwrap()
            .new_instance(table_node);
        mesh_instances.push(table);

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

        let (scene_graph, camera_node) =
            Python::attach(|py| -> PyResult<(Py<SceneGraph>, Py<PyNode>)> {
                let scene_graph: Py<SceneGraph> = Bound::new(py, scene_graph)?.into();
                let camera_node =
                    Bound::new(py, PyNode::new(camera_node, scene_graph.clone_ref(py)))?.into();

                Ball::NUMBER_NAME_COLOR_POSITION_VELOCITY.iter().for_each(
                    |(number, name, color, position, velocity)| {
                        let (ball, mesh_instance) = Ball::new(
                            py,
                            *number,
                            ball.clone(),
                            name.to_string(),
                            *color,
                            *position,
                            *velocity,
                            scene_graph.clone_ref(py),
                            &ctx,
                        );
                        balls.push(ball.into());
                        mesh_instances.push(mesh_instance);
                    },
                );

                Ok((scene_graph, camera_node))
            })
            .unwrap();

        let scripts = python::PyScripts::new();

        ctx.queue().submit([encoder.finish()]);

        let state = State {
            window,
            surface,
            surface_format,
            size,
            renderer,
            camera,
            scene_graph,
            skin_manager: SkinManager::new(&ctx),
            light_manager: LightManager::new(&ctx),
            environment,
            ctx: ctx,
            scripts,
            camera_node,
            balls,
            mesh_instances,
            last_render: Instant::now(),
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
        let now = Instant::now();
        let delta_time = (now - self.last_render)
            .as_secs_f32()
            .clamp(MIN_DELTA_TIME, MAX_DELTA_TIME);
        self.last_render = now;

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
            self.scripts.update(
                py,
                delta_time,
                &self.scene_graph,
                &self.camera_node,
                &self.balls,
            );

            self.renderer
                .render(
                    &self.camera,
                    &texture_view,
                    &self.scene_graph.borrow(py),
                    &mut self.skin_manager,
                    &self.mesh_instances,
                    &mut self.light_manager,
                    &self.environment,
                    &self.ctx,
                    &mut encoder,
                )
                .expect("failed to render");

            Ok(())
        })
        .expect("failed to run python");

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
                .create_window(Window::default_attributes().with_title("Billiard"))
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
            WindowEvent::MouseWheel {
                delta: MouseScrollDelta::LineDelta(x, y),
                ..
            } => {
                state.scripts.mouse_wheel(x as f64, y as f64);
            }
            WindowEvent::MouseWheel {
                delta: MouseScrollDelta::PixelDelta(delta),
                ..
            } => {
                state.scripts.mouse_wheel(delta.x, delta.y);
            }
            WindowEvent::MouseInput {
                button,
                state: elt_state,
                ..
            } => {
                let button = match button {
                    winit::event::MouseButton::Left => "Left",
                    winit::event::MouseButton::Right => "Right",
                    winit::event::MouseButton::Middle => "Middle",
                    _ => return,
                };
                let elt_state = match elt_state {
                    winit::event::ElementState::Pressed => "Pressed",
                    winit::event::ElementState::Released => "Released",
                };
                state.scripts.mouse_input(button, elt_state);
            }
            _ => (),
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        let state = self.state.as_mut().unwrap();
        match event {
            DeviceEvent::MouseMotion { delta: (x, y) } => {
                state.scripts.mouse_motion(x, y);
            }
            _ => (),
        }
    }
}

fn main() -> Result<(), winit::error::EventLoopError> {
    env_logger::init();
    python::PyScripts::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    return event_loop.run_app(&mut app);
}
