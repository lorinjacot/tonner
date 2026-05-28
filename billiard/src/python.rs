use std::{
    ffi::CString,
    fs,
    path::Path,
    sync::mpsc::{Receiver, channel},
};

use glam::{Mat4, vec3};
use log::{error, info};
use notify::Watcher;
use numpy::{AllowTypeChange, PyArray2, PyArrayLike2, ndarray::aview2};
use pyo3::{prelude::*, types::PyList};
use tonner::{ecs::EntityId, scene_graph::NodeHandle};

use crate::{
    arrow::Arrow,
    ball::Ball,
    physics::{Constraint, Force},
};

const SCRIPTS_DIR: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts");

#[derive(Debug)]
pub struct PyScripts {
    #[allow(dead_code)] // need to keep watcher in order to continues watching for changes
    watcher: Option<notify::RecommendedWatcher>,
    watcher_receiver: Option<Receiver<Result<notify::Event, notify::Error>>>,
    update: Option<Py<PyAny>>,
    mouse_input: Option<Py<PyAny>>,
    mouse_moved: Option<Py<PyAny>>,
    mouse_motion: Option<Py<PyAny>>,
    mouse_wheel: Option<Py<PyAny>>,
}

fn to_pyarray<'py>(py: Python<'py>, rust: &[glam::Vec3]) -> Bound<'py, PyArray2<f32>> {
    let array: Vec<_> = rust.iter().map(|v| v.to_array()).collect();
    PyArray2::from_array(py, &aview2(&array))
}

#[pyclass]
pub struct ForceManager {
    forces: Vec<Box<dyn Force>>,
}

impl ForceManager {
    pub fn new() -> ForceManager {
        ForceManager { forces: Vec::new() }
    }

    pub fn forces(&self) -> &[Box<dyn Force>] {
        &self.forces
    }
}

#[pymethods]
impl ForceManager {
    fn clear(&mut self) {
        self.forces.clear();
    }

    fn push(&mut self, name: String, entities: Vec<EntityId>, value: Py<PyAny>) {
        self.forces.push(Box::new(PyForce {
            name,
            entities,
            value,
        }));
    }

    pub fn is_empty(&self) -> bool {
        self.forces.is_empty()
    }
}

struct PyForce {
    name: String,
    entities: Vec<EntityId>,
    value: Py<PyAny>,
}

impl Force for PyForce {
    fn entities(&self) -> &[EntityId] {
        &self.entities
    }

    fn value(&self, positions: &[glam::Vec3], velocities: &[glam::Vec3]) -> Vec<glam::Vec3> {
        let dim = (positions.len(), 3);
        match Python::attach(|py| {
            let positions = to_pyarray(py, positions);
            let velocities = to_pyarray(py, velocities);

            let force: PyArrayLike2<'_, f32, AllowTypeChange> =
                self.value.call1(py, (positions, velocities))?.extract(py)?;
            let force = force.as_array();
            if force.dim() != dim {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "expected a force vector with shape ({},3)",
                    dim.0
                )));
            }
            let mut result = Vec::with_capacity(dim.0);
            for i in 0..dim.0 {
                result.push(vec3(force[(i, 0)], force[(i, 1)], force[(i, 2)]));
            }
            Ok(result)
        }) {
            Ok(v) => v,
            Err(e) => {
                error!("Failed evaluate constraint {}: {e}.", self.name);
                vec![glam::Vec3::ZERO; positions.len()]
            }
        }
    }
}

#[pyclass]
pub struct ConstraintManager {
    constraints: Vec<Box<dyn Constraint>>,
}

impl ConstraintManager {
    pub fn new() -> ConstraintManager {
        ConstraintManager {
            constraints: Vec::new(),
        }
    }

    pub fn constraints(&self) -> &[Box<dyn Constraint>] {
        &self.constraints
    }
}

#[pymethods]
impl ConstraintManager {
    fn clear(&mut self) {
        self.constraints.clear();
    }

    fn push(
        &mut self,
        name: String,
        entities: Vec<EntityId>,
        value: Py<PyAny>,
        gradient: Py<PyAny>,
        alpha: f32,
    ) {
        self.constraints.push(Box::new(PyConstraint {
            name,
            entities,
            value,
            gradient,
            alpha,
        }));
    }

    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }
}

struct PyConstraint {
    name: String,
    entities: Vec<EntityId>,
    value: Py<PyAny>,
    gradient: Py<PyAny>,
    alpha: f32,
}

impl Constraint for PyConstraint {
    fn entities(&self) -> &[EntityId] {
        &self.entities
    }

    fn value(&self, positions: &[glam::Vec3]) -> f32 {
        match Python::attach(|py| {
            let positions = to_pyarray(py, positions);
            self.value.call1(py, (positions,))?.extract(py)
        }) {
            Ok(v) => v,
            Err(e) => {
                error!("Failed evaluate constraint {}: {e}.", self.name);
                0.0
            }
        }
    }

    fn gradient(&self, positions: &[glam::Vec3]) -> Vec<glam::Vec3> {
        let dim = (positions.len(), 3);
        match Python::attach(|py| {
            let positions = to_pyarray(py, positions);

            let grad: PyArrayLike2<'_, f32, AllowTypeChange> =
                self.gradient.call1(py, (positions,))?.extract(py)?;
            let grad = grad.as_array();
            if grad.dim() != dim {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "expected a gradient with shape ({},3)",
                    dim.0
                )));
            }
            let mut gradient = Vec::with_capacity(dim.0);
            for i in 0..dim.0 {
                gradient.push(vec3(grad[(i, 0)], grad[(i, 1)], grad[(i, 2)]));
            }
            Ok(gradient)
        }) {
            Ok(v) => v,
            Err(e) => {
                error!("Failed evaluate constraint {}: {e}.", self.name);
                vec![glam::Vec3::ZERO; positions.len()]
            }
        }
    }

    fn alpha(&self) -> f32 {
        self.alpha
    }
}

impl PyScripts {
    /// Adds the `scripts` folder to python import path. This allow any python file in `scripts` to
    /// import other modules located in `scripts`.
    pub fn init() {
        let path = Path::new(SCRIPTS_DIR);
        Python::attach(|py| {
            let syspath = py
                .import("sys")
                .unwrap()
                .getattr("path")
                .unwrap()
                .cast_into::<PyList>()
                .unwrap();
            syspath.insert(0, path.to_str()).unwrap();
        });
    }

    pub fn new() -> PyScripts {
        let path = Path::new(SCRIPTS_DIR);
        let (tx, rx) = channel();
        let watcher = notify::recommended_watcher(tx)
            .and_then(|mut watcher| {
                watcher
                    .watch(path, notify::RecursiveMode::Recursive)
                    .map(|()| watcher)
            })
            .inspect_err(|e| {
                error!("Failed to start watcher: {e}.");
            })
            .ok();
        let watcher_receiver = if watcher.is_some() { Some(rx) } else { None };

        PyScripts::with_watcher(watcher, watcher_receiver)
    }

    fn with_watcher(
        watcher: Option<notify::RecommendedWatcher>,
        watcher_receiver: Option<Receiver<Result<notify::Event, notify::Error>>>,
    ) -> PyScripts {
        let mut scripts = PyScripts {
            watcher,
            watcher_receiver,
            update: None,
            mouse_input: None,
            mouse_moved: None,
            mouse_motion: None,
            mouse_wheel: None,
        };

        let path = Path::new(SCRIPTS_DIR);
        let main_content = match fs::read_to_string(path.join("main.py")) {
            Ok(content) => content,
            Err(e) => {
                error!("Failed to read main.py: {e}.");
                return scripts;
            }
        };
        let main_content = match CString::new(main_content) {
            Ok(content) => content,
            Err(e) => {
                error!("Failed to convert main.py to a C-string: {e}.");
                c"".to_owned()
            }
        };

        Python::attach(|py| {
            let main_module = match PyModule::from_code(py, &main_content, c"main.py", c"") {
                Ok(module) => module,
                Err(e) => {
                    error!("Failed to load main.py: {e}.");
                    return;
                }
            };

            scripts.update = load_function(&main_module, "update");
            scripts.mouse_input = load_function(&main_module, "mouse_input");
            scripts.mouse_moved = load_function(&main_module, "mouse_moved");
            scripts.mouse_motion = load_function(&main_module, "mouse_motion");
            scripts.mouse_wheel = load_function(&main_module, "mouse_wheel");
        });

        scripts
    }

    pub fn update(
        &mut self,
        py: Python,
        delta_time: f32,
        camera_node: &Py<NodeHandle>,
        balls: &[Py<Ball>],
        force_manager: &Py<ForceManager>,
        constraint_manager: &Py<ConstraintManager>,
    ) {
        if let Some(rx) = &self.watcher_receiver {
            use notify::EventKind::*;

            let mut need_reloading = false;
            for res in rx.try_iter() {
                match res {
                    Ok(event) => match event.kind {
                        Create(_) | Modify(_) | Remove(_) => need_reloading = true,
                        _ => (),
                    },
                    Err(e) => {
                        error!("File watching error: {e}.");
                    }
                }
            }
            if need_reloading {
                info!("File change detected. Reloading python scripts.");
                *self = PyScripts::with_watcher(self.watcher.take(), self.watcher_receiver.take());
            }
        }
        if let Some(func) = self.update.as_ref() {
            if let Err(e) = func.call1(
                py,
                (
                    delta_time,
                    camera_node,
                    balls,
                    force_manager,
                    constraint_manager,
                ),
            ) {
                error!("Failed to run update(): {e}.");
            }
        }
    }

    pub fn mouse_input(&self, button: &'static str, state: &'static str, arrow: &Py<Arrow>) {
        if let Some(func) = self.mouse_input.as_ref() {
            if let Err(e) = Python::attach(|py| func.call1(py, (button, state, arrow))) {
                error!("Failed to run mouse_input(): {e}.");
            }
        }
    }

    pub fn mouse_moved(
        &self,
        x: f64,
        y: f64,
        camera_node: &Py<NodeHandle>,
        projection_matrix: Mat4,
        balls: &[Py<Ball>],
        arrow: &Py<Arrow>,
    ) {
        if let Some(func) = self.mouse_moved.as_ref() {
            if let Err(e) = Python::attach(|py| {
                let projection_matrix = PyArray2::from_array(
                    py,
                    &aview2(&projection_matrix.transpose().to_cols_array_2d()),
                );
                func.call1(py, (x, y, camera_node, projection_matrix, balls, arrow))
            }) {
                error!("Failed to run mouse_motion(): {e}.");
            }
        }
    }

    pub fn mouse_motion(&self, dx: f64, dy: f64) {
        if let Some(func) = self.mouse_motion.as_ref() {
            if let Err(e) = Python::attach(|py| func.call1(py, (dx, dy))) {
                error!("Failed to run mouse_motion(): {e}.");
            }
        }
    }

    pub fn mouse_wheel(&self, x: f64, y: f64) {
        if let Some(func) = self.mouse_wheel.as_ref() {
            if let Err(e) = Python::attach(|py| func.call1(py, (x, y))) {
                error!("Failed to run mouse_wheel(): {e}.");
            }
        }
    }
}

fn load_function(main_module: &Bound<'_, PyModule>, name: &str) -> Option<Py<PyAny>> {
    match main_module.getattr(name) {
        Ok(function) => Some(function.into()),
        Err(e) => {
            error!("Failed to get {name} function: {e}.\nSkipping {name}().");
            None
        }
    }
}
