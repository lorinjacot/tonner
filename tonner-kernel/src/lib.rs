use pyo3::prelude::*;
use tonner::Context;
use winit::event_loop::EventLoopProxy;

#[derive(Debug)]
pub enum Event {
    ShutDown,
}

#[pyclass(frozen)]
pub struct PyState {
    context: Context,
    event_loop_proxy: EventLoopProxy<Event>,
}

impl PyState {
    pub fn new(context: Context, event_loop_proxy: EventLoopProxy<Event>) -> PyState {
        PyState {
            context,
            event_loop_proxy,
        }
    }
}
