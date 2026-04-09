use pyo3::{PyTraverseError, PyVisit, exceptions::PyRuntimeError, prelude::*};
use tonner::{Context, world::TonnerWorldHandle};
use uuid::Uuid;
use winit::event_loop::EventLoopProxy;

use crate::window::PyWindow;

mod window;

#[derive(Debug)]
pub enum Event {
    ShutDown,
    NewWindow { id: Uuid, world: TonnerWorldHandle },
    CloseWindow { id: Uuid },
}

#[pyclass(frozen)]
pub struct PyState {
    context: Context,
    root_window: Py<PyWindow>,
    event_loop_proxy: EventLoopProxy<Event>,
}

impl PyState {
    pub fn new(
        py: Python<'_>,
        root_window_id: Uuid,
        default_world: TonnerWorldHandle,
        context: Context,
        event_loop_proxy: EventLoopProxy<Event>,
    ) -> PyState {
        let root_window = PyWindow::new(root_window_id, default_world, event_loop_proxy.clone());

        PyState {
            context,
            root_window: Py::new(py, root_window).unwrap(),
            event_loop_proxy,
        }
    }
}

#[pymethods]
impl PyState {
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.root_window)?;
        Ok(())
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

    #[pymodule_export]
    use super::window::{PyWindow, root_window};
}
