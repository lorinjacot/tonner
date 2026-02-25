use std::{ffi::CString, fs, path::Path};

use log::error;
use pyo3::{prelude::*, types::PyList};
use storm::scene_graph::{PyNode, SceneGraph};

const SCRIPTS_DIR: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts");

#[derive(Debug, Default)]
pub struct PyScripts {
    update: Option<Py<PyAny>>,
    mouse_input: Option<Py<PyAny>>,
    mouse_motion: Option<Py<PyAny>>,
    mouse_wheel: Option<Py<PyAny>>,
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
            syspath.insert(0, path).unwrap();
        });
    }

    pub fn new() -> PyScripts {
        let mut scripts = PyScripts::default();

        let path = Path::new(SCRIPTS_DIR);
        let main_content = match fs::read_to_string(path.join("main.py")) {
            Ok(content) => content,
            Err(e) => {
                error!("Failed to read main.py: {e}.\nSkipping python...");
                return scripts;
            }
        };
        let main_content = match CString::new(main_content) {
            Ok(content) => content,
            Err(e) => {
                error!("Failed to convert main.py to a C-string: {e}.\nSkipping python...");
                c"".to_owned()
            }
        };

        Python::attach(|py| {
            let main_module = match PyModule::from_code(py, &main_content, c"main.py", c"") {
                Ok(module) => module,
                Err(e) => {
                    error!("Failed to load main.py: {e}.\nSkipping python...");
                    return;
                }
            };

            scripts.update = load_function(&main_module, "update");
            scripts.mouse_input = load_function(&main_module, "mouse_input");
            scripts.mouse_motion = load_function(&main_module, "mouse_motion");
            scripts.mouse_wheel = load_function(&main_module, "mouse_wheel");
        });

        scripts
    }

    pub fn update(
        &self,
        py: Python,
        delta_time: f32,
        scene_graph: &Py<SceneGraph>,
        camera_node: &Py<PyNode>,
    ) {
        if let Some(func) = self.update.as_ref() {
            if let Err(e) = func.call1(py, (delta_time, scene_graph, camera_node)) {
                error!("Failed to run update(): {e}.");
            }
        }
    }

    pub fn mouse_input(&self, button: &'static str, state: &'static str) {
        if let Some(func) = self.mouse_input.as_ref() {
            if let Err(e) = Python::attach(|py| func.call1(py, (button, state))) {
                error!("Failed to run mouse_input(): {e}.");
            }
        }
    }

    pub fn mouse_motion(&self, x: f64, y: f64) {
        if let Some(func) = self.mouse_motion.as_ref() {
            if let Err(e) = Python::attach(|py| func.call1(py, (x, y))) {
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
