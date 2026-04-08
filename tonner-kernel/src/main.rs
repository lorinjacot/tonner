use std::thread::spawn;

use pyo3::{
    prelude::*,
    types::{PyDict, PyList},
};
use winit::{application::ApplicationHandler, event_loop::EventLoop};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::with_user_event().build().unwrap();

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

    let mut app = App {};
    event_loop.run_app(&mut app)?;

    Ok(())
}

#[derive(Debug)]
enum Event {
    ShutDown,
}

struct App {}

impl ApplicationHandler<Event> for App {
    #[allow(unused)]
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {}

    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, event: Event) {
        match event {
            Event::ShutDown => event_loop.exit(),
        }
    }

    #[allow(unused)]
    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
    }
}
