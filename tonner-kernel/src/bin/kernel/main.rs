use std::{
    sync::{Arc, OnceLock},
    thread::spawn,
};

use pyo3::{
    prelude::*,
    types::{PyDict, PyList},
};
use tonner::{Context, py_tonner};
use tonner_kernel::{Event, PyState, py_tonner_kernel};
use winit::{application::ApplicationHandler, event_loop::EventLoop};

use crate::state::State;

mod state;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::with_user_event().build().unwrap();
    let context = Arc::new(OnceLock::new());

    let mut app = App {
        state: None,
        context: context.clone(),
    };

    let args: Vec<String> = std::env::args().collect();
    let event_loop_proxy = event_loop.create_proxy();
    spawn(move || {
        let context = context.wait().clone();
        let py_state = PyState::new(context, event_loop_proxy.clone());

        pyo3::append_to_inittab!(py_tonner);
        pyo3::append_to_inittab!(py_tonner_kernel);
        Python::initialize();

        let result = Python::attach(|py| -> PyResult<()> {
            let sys = py.import("sys")?;
            let py_args = PyList::new(py, &args)?;
            sys.setattr("argv", py_args)?;

            let kernelapp = py.import("ipykernel.kernelapp")?;
            let app_class = kernelapp.getattr("IPKernelApp")?;
            let app = app_class.call_method0("instance")?;

            let user_ns = PyDict::new(py);
            user_ns.set_item("_tonner_kernel_state", py_state)?;
            app.setattr("user_ns", user_ns)?;

            app.call_method0("initialize")?;
            app.call_method0("start")?;

            Ok(())
        });

        if let Err(err) = result {
            eprintln!("{err}");
        }

        event_loop_proxy.send_event(Event::ShutDown).unwrap();
    });

    event_loop.run_app(&mut app)?;

    Ok(())
}

struct App {
    state: Option<State>,
    context: Arc<OnceLock<Context>>,
}

impl ApplicationHandler<Event> for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let state = State::new(event_loop);

        if self.context.set(state.context().clone()).is_err() {
            panic!("Tonner kernel context already set");
        }

        self.state = Some(state);
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
        if let Some(state) = self.state.as_mut() {
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
                if let Some(state) = &mut self.state {
                    state.on_mouse_motion(delta);
                }
            }
            _ => (),
        }
    }
}
