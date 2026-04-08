use std::{
    sync::{Arc, Mutex},
    thread::spawn,
};

use pyo3::{
    prelude::*,
    types::{PyDict, PyList},
};
use tonner::Context;
use winit::{application::ApplicationHandler, event_loop::EventLoop, window::Window};

use crate::state::State;

mod state;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::with_user_event().build().unwrap();
    let state = Arc::new(Mutex::new(None));

    let args: Vec<String> = std::env::args().collect();
    let event_loop_proxy = event_loop.create_proxy();
    spawn(move || {
        let result = Python::attach(|py| -> PyResult<()> {
            let sys = py.import("sys")?;
            let py_args = PyList::new(py, &args)?;
            sys.setattr("argv", py_args)?;

            let locals = PyDict::new(py);
            py.run(
                cr#"
from ipykernel.ipkernel import IPythonKernel
from ipykernel.kernelapp import IPKernelApp

IPKernelApp.launch_instance(kernel_class=IPythonKernel)
"#,
                None,
                Some(&locals),
            )?;

            Ok(())
        });

        if let Err(err) = result {
            eprintln!("{err}");
        }

        event_loop_proxy.send_event(Event::ShutDown).unwrap();
    });

    let mut app = App { state };
    event_loop.run_app(&mut app)?;

    Ok(())
}

#[derive(Debug)]
enum Event {
    ShutDown,
}

struct App {
    state: Arc<Mutex<Option<State>>>,
}

impl ApplicationHandler<Event> for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes().with_title("Tonner Kernel"))
            .expect("Failed to create a window");

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: wgpu::InstanceFlags::from_env_or_default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::from_env_or_default(),
        });

        let surface = instance
            .create_surface(window)
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

        let ctx = Context::from_device(device, queue);
        let state = State::new(ctx, surface);

        *self.state.lock().unwrap() = Some(state);
    }

    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, event: Event) {
        match event {
            Event::ShutDown => event_loop.exit(),
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) {
    }
}
