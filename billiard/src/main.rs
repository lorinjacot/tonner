use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use glam::{Quat, Vec3};
use pollster::block_on;
use pyo3::prelude::*;
use tonner::Context;
use tonner::ecs::EntityRegistry;
use tonner::environment::{Environment, EnvironmentBuilder};
use tonner::geometry::SphereBuilder;
use tonner::geometry::skin::SkinManager;
use tonner::mesh::MeshInstance;
use tonner::renderer::Renderer;
use tonner::renderer::camera::Camera;
use tonner::renderer::light::LightManager;
use tonner::scene_graph::{NodeHandle, SceneGraph};
use wgpu::Instance;
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, MouseScrollDelta};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use crate::arrow::Arrow;
use crate::ball::Ball;
use crate::python::{ConstraintManager, ForceManager};
use crate::table::table;

mod arrow;
mod ball;
mod physics;
mod python;
mod table;

struct State {
    instance: wgpu::Instance,
    ctx: Context,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    size: PhysicalSize<u32>,
    renderer: Renderer,
    camera: Camera,
    scene_graph: Arc<Mutex<SceneGraph>>,
    skin_manager: SkinManager,
    light_manager: LightManager,
    environment: Environment,
    scripts: python::PyScripts,
    camera_node: Py<NodeHandle>,
    balls: Vec<Py<Ball>>,
    force_manager: Py<ForceManager>,
    constraint_manager: Py<ConstraintManager>,
    arrow: Py<Arrow>,
    mesh_instances: Vec<MeshInstance>,
    last_render: Instant,
}

impl State {
    async fn new(window: Arc<Window>) -> Self {
        let instance = Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
            window.clone(),
        )));

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
        let mut entity_registry = EntityRegistry::new();
        let mut scene_graph = SceneGraph::new(&ctx);

        let camera_entity = entity_registry.create();
        scene_graph.add_with_transform(camera_entity, None, Vec3::X, Quat::IDENTITY, Vec3::ONE);
        let camera = Camera::new(camera_entity);

        let mut balls = Vec::new();
        let mut mesh_instances = Vec::new();
        let ball = SphereBuilder::default()
            .name("Ball")
            .radius(0.025)
            .build(&ctx);

        mesh_instances.push(table(&mut entity_registry, &mut scene_graph, &ctx));

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

        let scene_graph = Arc::new(Mutex::new(scene_graph));

        let (camera_node, force_manager, constraint_manager, arrow) = Python::attach(
            |py| -> PyResult<(
                Py<NodeHandle>,
                Py<ForceManager>,
                Py<ConstraintManager>,
                Py<Arrow>,
            )> {
                let camera_node = Py::new(py, NodeHandle::new(camera_entity, scene_graph.clone()))?;

                Ball::settings()
                    .iter()
                    .for_each(|(number, name, color, position, velocity)| {
                        let ball = Ball::new(
                            py,
                            *number,
                            ball.clone(),
                            name.to_string(),
                            *color,
                            *position,
                            *velocity,
                            &mut entity_registry,
                            scene_graph.clone(),
                            &ctx,
                        );
                        balls.push(ball.into());
                    });

                let force_manager = Py::new(py, ForceManager::new())?;

                let constraint_manager = Py::new(py, ConstraintManager::new())?;

                let arrow = Py::new(
                    py,
                    Arrow::new(py, &mut entity_registry, scene_graph.clone(), &ctx),
                )?;

                Ok((camera_node, force_manager, constraint_manager, arrow))
            },
        )
        .unwrap();

        let scripts = python::PyScripts::new();

        ctx.queue().submit([encoder.finish()]);

        let state = State {
            instance,
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
            force_manager,
            constraint_manager,
            arrow,
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
        let min_delta_time = Duration::from_secs_f32(1.0 / 60.0);
        let max_delta_time = Duration::from_secs_f32(1.0 / 60.0);

        let now = Instant::now();
        let delta_time = (now - self.last_render).clamp(min_delta_time, max_delta_time);
        self.last_render = now;

        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Occluded | wgpu::CurrentSurfaceTexture::Timeout => return,
            wgpu::CurrentSurfaceTexture::Suboptimal(_) | wgpu::CurrentSurfaceTexture::Outdated => {
                self.configure_surface();
                return;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                unreachable!("No error scope registered, so validation errors will panic")
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self.instance.create_surface(self.window.clone()).unwrap();
                self.configure_surface();
                return;
            }
        };

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
                delta_time.as_secs_f32(),
                &self.camera_node,
                &self.balls,
                &self.force_manager,
                &self.constraint_manager,
            );
            let mut balls: Vec<_> = self
                .balls
                .iter()
                .map(|ball| ball.borrow_mut(py))
                .filter(|ball| !ball.out)
                .collect();

            let mut scene_graph = self.scene_graph.lock().unwrap();

            physics::update(
                py,
                delta_time,
                &mut scene_graph,
                balls.iter_mut().map(|ball| &mut **ball),
                self.force_manager.borrow(py).forces(),
                self.constraint_manager.borrow(py).constraints(),
            );

            self.renderer
                .render(
                    &self.camera,
                    &texture_view,
                    &mut scene_graph,
                    &mut self.skin_manager,
                    self.mesh_instances
                        .iter()
                        .chain(balls.iter().map(|ball| ball.mesh_instance()))
                        .chain(self.arrow.borrow(py).mesh_instances()),
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
                const LINE_HEIGHT: f64 = 24.0;
                state
                    .scripts
                    .mouse_wheel(x as f64 * LINE_HEIGHT, y as f64 * LINE_HEIGHT);
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
                state.scripts.mouse_input(button, elt_state, &state.arrow);
            }
            WindowEvent::CursorMoved { position, .. } => {
                let w = state.size.width as f64;
                let h = state.size.height as f64;
                state.scripts.mouse_moved(
                    2.0 * position.x / w - 1.0,
                    1.0 - 2.0 * position.y / h,
                    &state.camera_node,
                    state.camera.projection_matrix((w / h) as f32),
                    &state.balls,
                    &state.arrow,
                );
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
