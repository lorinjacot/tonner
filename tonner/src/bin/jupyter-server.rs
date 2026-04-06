use std::{
    sync::{Arc, OnceLock},
    thread::spawn,
};

use pyo3::prelude::*;
use tonner::{Context, world::World};
use winit::{
    application::ApplicationHandler,
    event_loop::EventLoop,
    window::{Window, WindowAttributes},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop = EventLoop::with_user_event().build().unwrap();
    let event_loop_proxy = event_loop.create_proxy();

    let context = Arc::new(OnceLock::new());
    let py_context = context.clone();

    spawn(move || {
        let context = py_context.wait();
        loop {
            let result = Python::attach(|py| -> PyResult<()> {
                let kernelapp = py.import("ipykernel.kernelapp")?;
                let class = kernelapp.getattr("IPKernelApp")?;

                // let main = kernelapp.getattr("main")?;
                // main.call0()?;

                // kernelapp.call_method0("launch_new_instance")?;

                let app = class.call_method0("instance")?;
                app.call_method1("initialize", (Vec::<String>::new(),))?;
                let connection_file: String = app.getattr("abs_connection_file")?.extract()?;

                println!("Kernel connection file is located at: {}", connection_file);

                app.call_method0("start")?;

                Ok(())
            });

            if let Err(e) = result {
                eprintln!("Error in Python thread: {e}");
            } else {
                println!("Python thread stopped. Restarting...");
            }
        }
    });

    let mut app = App {
        state: None,
        context,
    };
    event_loop.run_app(&mut app)?;

    Ok(())
}

enum Event {
    NewWorld(Py<World>),
}

struct State {
    context: Arc<OnceLock<Context>>,
    window: Arc<Window>,
    world: Option<Py<World>>,
}

struct App {
    state: Option<State>,
    context: Arc<OnceLock<Context>>,
}

impl ApplicationHandler<Event> for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(WindowAttributes::default())
                .unwrap(),
        );

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: wgpu::InstanceFlags::from_env_or_default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::from_env_or_default(),
        });

        let surface = instance
            .create_surface(window.clone())
            .expect("Failed to create GPU surface");

        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptionsBase {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            }))
            .expect("Failed to get GPU adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .expect("Failed to get GPU handle");

        let context = Context::from_device(device, queue);

        self.context.set(context).unwrap();

        self.state = Some(State {
            context: self.context.clone(),
            window,
            world: None,
        });
    }

    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, event: Event) {
        match event {
            Event::NewWorld(world) => {
                if let Some(state) = &mut self.state {
                    state.world = Some(world);
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            winit::event::WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            _ => {}
        }
    }
}
