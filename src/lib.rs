use std::path::PathBuf;

use engine::Engine;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

mod asset;
mod camera;
mod engine;
mod scene;
mod storage;
mod storm;
mod texture;

struct App {
    engine: Option<Engine>,
    load_asset: Option<PathBuf>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes().with_maximized(true))
            .unwrap();
        self.engine = Some(pollster::block_on(Engine::new(
            window,
            self.load_asset.take(),
        )));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(engine) = self.engine.as_mut() {
            if engine.window().id() != window_id || engine.window_event(&event) {
                return;
            }
            match event {
                WindowEvent::CloseRequested => {
                    event_loop.exit();
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
        if let Some(engine) = self.engine.as_mut() {
            engine.device_event(&event);
        }
    }
}

pub fn run(load_asset: Option<PathBuf>) {
    let event_loop = EventLoop::new().unwrap();

    // ControlFlow::Poll continuously runs the event loop, even if the OS hasn't
    // dispatched any events. This is ideal for games and similar applications.
    event_loop.set_control_flow(ControlFlow::Poll);

    // ControlFlow::Wait pauses the event loop if no events are available to process.
    // This is ideal for non-game applications that only update in response to user
    // input, and uses significantly less power/CPU time than ControlFlow::Poll.
    // event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App {
        engine: None,
        load_asset,
    };
    event_loop.run_app(&mut app).unwrap();
}
