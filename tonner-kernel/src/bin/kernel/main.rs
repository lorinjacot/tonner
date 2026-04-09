use std::{
    sync::{Arc, OnceLock},
    thread::spawn,
};

use pyo3::{
    prelude::*,
    types::{PyDict, PyList},
};
use tonner::{Context, py_tonner, world::TonnerWorldHandle};
use tonner_kernel::{Event, PyState, py_tonner_kernel};
use uuid::Uuid;
use winit::{application::ApplicationHandler, event_loop::EventLoop};

use crate::state::State;

mod state;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::with_user_event().build().unwrap();
    let root_window_id = Uuid::new_v4();
    let init = Arc::new(OnceLock::new());

    let mut app = App {
        state: None,
        init: init.clone(),
    };

    let args: Vec<String> = std::env::args().collect();
    let event_loop_proxy = event_loop.create_proxy();
    spawn(move || {
        let (context, default_world) = init.wait().clone();

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

            let py_state = PyState::new(
                py,
                root_window_id,
                default_world,
                context,
                event_loop_proxy.clone(),
            );

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
    init: Arc<OnceLock<(Context, TonnerWorldHandle)>>,
}

impl ApplicationHandler<Event> for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let state = State::new(event_loop);

        if self
            .init
            .set((state.context().clone(), state.root_world().clone()))
            .is_err()
        {
            panic!("Tonner kernel context already set");
        }

        self.state = Some(state);
    }

    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, event: Event) {
        let state = self.state.as_mut().unwrap();
        match event {
            Event::ShutDown => event_loop.exit(),
            Event::NewWindow { id, world } => {
                state.add_window(event_loop, id, world);
            }
            Event::CloseWindow { id } => {
                state.close_window(id);
            }
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
