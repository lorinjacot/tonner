use std::{
    f32::consts::PI,
    hash::Hash,
    ops::DerefMut,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use dashmap::DashSet;
use glam::Quat;
use storm::{
    Context, Scene, SceneBuilder, camera::CameraBuilder, mesh::Mesh,
    mesh_instance::MeshInstanceBuilder, scene_graph::NodeBuilder,
};

use crate::SceneView;

#[derive(Clone)]
pub(super) struct MeshExplorer {
    show: Arc<AtomicBool>,
    detached: Arc<AtomicBool>,
    meshes: Arc<Mutex<Vec<Mesh>>>,
    ctx: Context,
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
            ctx: storm_ctx,
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
                    let mut scene = SceneBuilder::default()
                        .name(format!("Preview of {:?}", mesh.id()))
                        .build(&self.ctx);
                    MeshInstanceBuilder::new(mesh.clone())
                        .build(&mut scene)
                        .unwrap();

                    let camera = CameraBuilder::default()
                        .node(
                            NodeBuilder::default()
                                .local_translation([0.0, 0.0, -5.0])
                                .local_rotation(Quat::from_rotation_y(PI))
                                .build(&mut scene.scene_graph)
                                .unwrap(),
                        )
                        .build(&mut scene);

                    let scene = Arc::new(RwLock::new(scene));

                    self.properties_windows.insert(PropertiesWindow {
                        mesh: mesh.clone(),
                        scene: scene.clone(),
                        preview: Mutex::new(SceneView::new(
                            scene,
                            camera,
                            400,
                            400,
                            self.renderer.clone(),
                            &self.ctx,
                        )),
                    });
                }
            });
        }

        let mut encoder =
            self.ctx
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Mesh preview command encoder"),
                });
        let duration = Duration::from_secs_f32(ui.input(|input_state| input_state.stable_dt));

        self.properties_windows.retain(|properties_window| {
            properties_window.preview.lock().unwrap().update(duration);

            properties_window
                .scene
                .write()
                .unwrap()
                .simulate(duration, &mut encoder)
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
                        properties_window.preview.lock().unwrap().render(
                            ui,
                            &self.ctx,
                            &mut encoder,
                        );
                    });
                });

            open
        });

        self.ctx.queue().submit([encoder.finish()]);
        ui.ctx().request_repaint();
    }
}
