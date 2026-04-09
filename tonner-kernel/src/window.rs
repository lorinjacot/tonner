use pyo3::prelude::*;
use tonner::world::TonnerWorldHandle;
use uuid::Uuid;
use winit::event_loop::EventLoopProxy;

use crate::{Event, state};

/// An OS window. A window can be used to:
/// - render a world
/// - render some GUI
#[pyclass(name = "Window", skip_from_py_object, frozen)]
pub struct PyWindow {
    #[pyo3(get)]
    id: Uuid,
    #[pyo3(get)]
    world: TonnerWorldHandle,
    event_loop_proxy: EventLoopProxy<Event>,
}

impl PyWindow {
    pub(super) fn new(
        id: Uuid,
        world: TonnerWorldHandle,
        event_loop_proxy: EventLoopProxy<Event>,
    ) -> PyWindow {
        PyWindow {
            id,
            world,
            event_loop_proxy,
        }
    }
}

#[pymethods]
impl PyWindow {
    /// Creates a new OS windows that renders the given world.
    #[new]
    pub fn py_new(py: Python<'_>, world: &TonnerWorldHandle) -> PyResult<PyWindow> {
        let state = state(py)?;

        let event_loop_proxy = state.get().event_loop_proxy.clone();

        let id = Uuid::new_v4();
        event_loop_proxy
            .send_event(crate::Event::NewWindow {
                id,
                world: world.clone(),
            })
            .unwrap();

        Ok(PyWindow {
            id,
            world: world.clone(),
            event_loop_proxy,
        })
    }

    fn __str__(&self) -> String {
        format!("Window({})", self.id)
    }
}

impl Drop for PyWindow {
    fn drop(&mut self) {
        self.event_loop_proxy
            .send_event(Event::CloseWindow { id: self.id })
            .unwrap();
    }
}

/// Returns the roots window. This is the window that is created at the kernel startup.
/// The root window cannot be closed.
#[pyfunction]
pub fn root_window(py: Python<'_>) -> PyResult<Py<PyWindow>> {
    let state = state(py)?;

    Ok(state.get().root_window.clone_ref(py))
}
