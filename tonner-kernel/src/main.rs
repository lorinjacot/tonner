use std::{
    sync::{Arc, Mutex},
    thread::spawn,
};

use pyo3::{
    prelude::*,
    types::{PyDict, PyList},
};
use winit::{application::ApplicationHandler, event_loop::EventLoop};

use crate::state::State;

mod state;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("starting kernel");

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
        let state = State::new(event_loop);

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
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        if let Some(state) = self.state.lock().unwrap().as_mut() {
            state.on_window_event(window_id, event);
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        match event {
            winit::event::DeviceEvent::MouseMotion { delta } => {
                if let Some(state) = self.state.lock().unwrap().as_mut() {
                    state.on_mouse_motion(delta);
                }
            }
            _ => (),
        }
    }
}
