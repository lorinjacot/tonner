use std::iter::once;
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::spawn;
use std::time::{Duration, Instant};

use pollster::block_on;
use wgpu::Instance;
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, MouseScrollDelta};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use crate::game::Game;
use crate::ui::{Action, Ui};

mod arrow;
mod ball;
mod game;
mod python;
mod table;
mod ui;

type PhysicsEngine = Arc<Mutex<tonner::Engine>>;

struct State {
    instance: wgpu::Instance,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    device: wgpu::Device,
    queue: wgpu::Queue,
    size: PhysicalSize<u32>,
    game_receiver: Receiver<Game>,
    game: Option<Game>,
    ui: Ui,
    last_action: Action,
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

        let ui = Ui::new(&window, &device, surface_format.remove_srgb_suffix());

        let (sender, game_receiver) = sync_channel(1);
        let game_device = device.clone();
        let game_queue = queue.clone();

        spawn(move || {
            let game = Game::new(
                game_device,
                game_queue,
                size.width,
                size.height,
                surface_format.add_srgb_suffix(),
            );
            sender.send(game).unwrap();
        });

        let state = State {
            instance,
            window,
            surface,
            surface_format,
            device,
            queue,
            size,
            game_receiver,
            game: None,
            ui,
            last_action: Action::None,
            last_render: Instant::now(),
        };

        state.configure_surface();
        state.queue.submit([]);

        state
    }

    fn configure_surface(&self) {
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            view_formats: vec![
                self.surface_format.add_srgb_suffix(),
                self.surface_format.remove_srgb_suffix(),
            ],
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            width: self.size.width,
            height: self.size.height,
            desired_maximum_frame_latency: 2,
            present_mode: wgpu::PresentMode::AutoVsync,
        };
        self.surface.configure(&self.device, &surface_config);
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

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render command encoder"),
            });

        if let Some(game) = self.game.as_mut() {
            let srgb_texture_view =
                surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor {
                        label: Some("surface texture view"),
                        format: Some(self.surface_format.add_srgb_suffix()),
                        ..Default::default()
                    });

            game.render(
                delta_time,
                &mut self.ui.state,
                &self.last_action,
                &srgb_texture_view,
                &mut encoder,
            );
        } else {
            match self.game_receiver.try_recv() {
                Ok(game) => {
                    self.game = Some(game);
                    self.ui.state = ui::UiState::MainMenu {};
                }
                Err(TryRecvError::Disconnected) => {
                    panic!("Game failed to initialize");
                }
                Err(TryRecvError::Empty) => (),
            }
        }

        let gamma_texture_view =
            surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("surface texture view"),
                    format: Some(self.surface_format.remove_srgb_suffix()),
                    ..Default::default()
                });

        let (action, command_buffers) = self.ui.render(
            &self.window,
            &self.device,
            &self.queue,
            &mut encoder,
            &gamma_texture_view,
        );
        self.last_action = action;

        self.queue
            .submit(command_buffers.into_iter().chain(once(encoder.finish())));
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
        let response = state.ui.on_window_event(&state.window, &event);
        if response.repaint {
            state.window.request_redraw();
        }
        if response.consumed {
            return;
        }
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
                if let Some(game) = state.game.as_mut() {
                    game.on_mouse_wheel(x as f64 * LINE_HEIGHT, y as f64 * LINE_HEIGHT);
                }
            }
            WindowEvent::MouseWheel {
                delta: MouseScrollDelta::PixelDelta(delta),
                ..
            } => {
                if let Some(game) = state.game.as_mut() {
                    game.on_mouse_wheel(delta.x, delta.y);
                }
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
                if let Some(game) = state.game.as_mut() {
                    game.on_mouse_input(button, elt_state);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(game) = state.game.as_mut() {
                    let w = state.size.width as f64;
                    let h = state.size.height as f64;
                    game.on_mouse_moved(
                        2.0 * position.x / w - 1.0,
                        1.0 - 2.0 * position.y / h,
                        (w / h) as f32,
                    );
                }
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
            DeviceEvent::MouseMotion { delta } => {
                state.ui.on_mouse_motion(delta);
                if let Some(game) = state.game.as_mut() {
                    game.on_mouse_motion(delta.0, delta.1);
                }
            }
            _ => (),
        }
    }
}

fn main() -> Result<(), winit::error::EventLoopError> {
    env_logger::init();
    python::PyScripts::init_python();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    return event_loop.run_app(&mut app);
}
