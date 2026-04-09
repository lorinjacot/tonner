use pyo3::{exceptions::PyRuntimeError, prelude::*};
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

fn state<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyState>> {
    let state = py.eval(c"_tonner_kernel_state", None, None).map_err(|_| {
        PyRuntimeError::new_err(
            "the tonner_kernel package should only be used with the Tonner juypter kernel",
        )
    })?;

    let state = state.cast()?;

    Ok(state.clone())
}

#[pyfunction]
pub fn context(py: Python<'_>) -> PyResult<Context> {
    Ok(state(py)?.get().context.clone())
}

#[pymodule(name = "tonner_kernel")]
pub mod py_tonner_kernel {
    #[pymodule_export]
    use super::{PyState, context};
}
