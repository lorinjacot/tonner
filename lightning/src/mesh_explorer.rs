use std::{
    hash::Hash,
    ops::DerefMut,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use dashmap::DashSet;
use storm::{
    Context, GpuCommandQueue, SceneBuilder, camera::CameraBuilder, mesh::Mesh,
    scene_graph::NodeBuilder,
};
use storm_animation::AnimationManager;

use crate::{Scene, SceneView};

#[derive(Clone)]
pub(super) struct MeshExplorer {
    show: Arc<AtomicBool>,
    detached: Arc<AtomicBool>,
    meshes: Arc<Mutex<Vec<Mesh>>>,
    gpu_command_queue: Arc<Mutex<GpuCommandQueue>>,
    renderer: Arc<egui::mutex::RwLock<eframe::egui_wgpu::Renderer>>,
    properties_windows: Arc<DashSet<PropertiesWindow>>,
}

struct PropertiesWindow {
    mesh: Mesh,
    scene: Arc<RwLock<Scene>>,
    preview: Mutex<SceneView>,
}

impl PartialEq for PropertiesWindow {
    fn eq(&self, other: &Self) -> bool {
        self.mesh == other.mesh
    }
}

impl Eq for PropertiesWindow {}

impl Hash for PropertiesWindow {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.mesh.hash(state);
    }
}

impl MeshExplorer {
    pub(super) fn new(
        storm_ctx: Context,
        renderer: Arc<egui::mutex::RwLock<eframe::egui_wgpu::Renderer>>,
        show: Arc<AtomicBool>,
        detached: Arc<AtomicBool>,
    ) -> Self {
        Self {
            show,
            detached,
            meshes: Arc::new(Mutex::new(Vec::new())),
            gpu_command_queue: Arc::new(Mutex::new(GpuCommandQueue::new(storm_ctx, 1e6 as u64))),
            renderer,
            properties_windows: Arc::new(DashSet::new()),
        }
    }

    pub(super) fn meshes(&self) -> &Arc<Mutex<Vec<Mesh>>> {
        &self.meshes
    }

    pub(super) fn toggle_show(&self) {
        let current = self.show.load(Ordering::Relaxed);
        self.show.store(!current, Ordering::Relaxed);
    }

    pub(super) fn detached(&self) -> bool {
        self.detached.load(Ordering::Relaxed)
    }

    pub(super) fn set_detached(&self, value: bool) {
        self.detached.store(value, Ordering::Relaxed);
    }

    pub(super) fn ui(&self, egui_ctx: &egui::Context) {
        if self.show.load(Ordering::Relaxed) {
            if self.detached.load(Ordering::Relaxed) {
                let this = self.clone();
                egui_ctx.show_viewport_deferred(
                    egui::ViewportId::from_hash_of("Mesh explorer"),
                    egui::ViewportBuilder::default().with_title("Mesh explorer"),
                    move |egui_ctx, class| {
                        if class == egui::ViewportClass::Embedded {
                            this.wrap_in_window(egui_ctx);
                        } else {
                            egui::CentralPanel::default().show(egui_ctx, |ui| {
                                this.content(ui);
                            });

                            if egui_ctx.input(|i| i.viewport().close_requested()) {
                                this.show.store(false, Ordering::Relaxed);
                            }
                        }
                    },
                )
            } else {
                self.wrap_in_window(egui_ctx);
            }
        }
    }

    fn wrap_in_window(&self, egui_ctx: &egui::Context) {
        let mut open = self.show.load(Ordering::Relaxed);
        egui::Window::new("Mesh explorer")
            .open(&mut open)
            .vscroll(true)
            .show(egui_ctx, |ui| {
                self.content(ui);
            });
        self.show.store(open, Ordering::Relaxed);
    }

    fn content(&self, ui: &mut egui::Ui) {
        let meshes = self.meshes.lock().unwrap();
        let mut gpu_command_queue = self.gpu_command_queue.lock().unwrap();

        if meshes.is_empty() {
            ui.label(egui::RichText::new("No meshes loaded").italics());
        } else {
            meshes.iter().for_each(|mesh| {
                let name = mesh.name();
                let label = if name.is_empty() {
                    egui::Label::new(egui::RichText::new("No name").italics())
                } else {
                    egui::Label::new(name.as_str())
                };
                if ui.add(label.sense(egui::Sense::click())).double_clicked() {
                    let mut storm_scene = SceneBuilder::default()
                        .name(format!("Preview of {:?}", mesh.id()))
                        .build(&mut gpu_command_queue);
                    let node = NodeBuilder::default()
                        .build(&mut storm_scene.scene_graph)
                        .unwrap();
                    let instance = mesh.new_instance(node);
                    storm_scene.mesh_instances.insert(instance.id(), instance);

                    let camera = CameraBuilder::default()
                        .node(
                            NodeBuilder::default()
                                .local_translation([0.0, 0.0, 2.0])
                                .build(&mut storm_scene.scene_graph)
                                .unwrap(),
                        )
                        .build(&mut storm_scene);

                    let animation_manager = AnimationManager::default();

                    let scene = Arc::new(RwLock::new(Scene {
                        storm_scene,
                        animation_manager,
                    }));

                    self.properties_windows.insert(PropertiesWindow {
                        mesh: mesh.clone(),
                        scene: scene.clone(),
                        preview: Mutex::new(SceneView::new(
                            scene,
                            camera,
                            400,
                            400,
                            self.renderer.clone(),
                            &mut gpu_command_queue,
                        )),
                    });
                }
            });
        }

        let duration = Duration::from_secs_f32(ui.input(|input_state| input_state.stable_dt));

        self.properties_windows.retain(|properties_window| {
            properties_window.preview.lock().unwrap().update(duration);

            properties_window
                .scene
                .write()
                .unwrap()
                .storm_scene
                .simulate(duration, &mut gpu_command_queue)
                .unwrap();

            let mut name = properties_window.mesh.name();
            let window = if name.is_empty() {
                egui::Window::new(egui::RichText::new("No name").italics())
            } else {
                egui::Window::new(name.as_str())
            };

            let mut open = true;
            window
                .id(egui::Id::new(properties_window.mesh.id()))
                .open(&mut open)
                .show(ui.ctx(), |ui| {
                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(
                            egui::Vec2 {
                                x: 150.0,
                                y: ui.available_height(),
                            },
                            egui::Layout::left_to_right(egui::Align::Min),
                            |ui| {
                                ui.label("Name");
                                ui.text_edit_singleline(name.deref_mut());
                            },
                        );
                        properties_window
                            .preview
                            .lock()
                            .unwrap()
                            .render(ui, &mut gpu_command_queue);
                    });
                });

            open
        });

        gpu_command_queue.submit();
        ui.ctx().request_repaint();
    }
}
