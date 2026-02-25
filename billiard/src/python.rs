use std::{
    ffi::CString,
    fs,
    path::Path,
    sync::mpsc::{Receiver, channel},
};

use log::{error, info};
use notify::Watcher;
use pyo3::{prelude::*, types::PyList};
use storm::scene_graph::{PyNode, SceneGraph};

const SCRIPTS_DIR: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts");

#[derive(Debug)]
pub struct PyScripts {
    #[allow(dead_code)] // need to keep watcher in order to continues watching for changes
    watcher: Option<notify::INotifyWatcher>,
    watcher_receiver: Option<Receiver<Result<notify::Event, notify::Error>>>,
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
        let path = Path::new(SCRIPTS_DIR);
        let (tx, rx) = channel();
        let watcher = notify::recommended_watcher(tx)
            .and_then(|mut watcher| {
                watcher
                    .watch(path, notify::RecursiveMode::Recursive)
                    .map(|()| watcher)
            })
            .inspect_err(|e| {
                error!(
                    "Failed to start watcher: {e}.\nScript hot reloading will not be available."
                );
            })
            .ok();
        let watcher_receiver = if watcher.is_some() { Some(rx) } else { None };

        PyScripts::with_watcher(watcher, watcher_receiver)
    }

    fn with_watcher(
        watcher: Option<notify::INotifyWatcher>,
        watcher_receiver: Option<Receiver<Result<notify::Event, notify::Error>>>,
    ) -> PyScripts {
        let mut scripts = PyScripts {
            watcher,
            watcher_receiver,
            update: None,
            mouse_input: None,
            mouse_motion: None,
            mouse_wheel: None,
        };

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
        &mut self,
        py: Python,
        delta_time: f32,
        scene_graph: &Py<SceneGraph>,
        camera_node: &Py<PyNode>,
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
                info!("File change detected. Reloading scripts.");
                *self = PyScripts::with_watcher(self.watcher.take(), self.watcher_receiver.take());
            }
        }
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
