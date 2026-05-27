use std::{
    collections::HashMap,
    hash::Hash,
    ops::DerefMut,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use dashmap::DashSet;
use glam::{Quat, Vec3, vec3};
use storm_animation::AnimationManager;
use tonner::{
    Context,
    ecs::EntityRegistry,
    environment::Environment,
    geometry::skin::SkinManager,
    mesh::Mesh,
    renderer::{camera::Camera, light::LightManager},
    scene_graph::SceneGraph,
};

use crate::{Scene, SceneView};

#[derive(Clone)]
pub(super) struct MeshExplorer {
    show: Arc<AtomicBool>,
    detached: Arc<AtomicBool>,
    meshes: Arc<Mutex<Vec<Mesh>>>,
    ctx: Context,
    renderer: Arc<egui::mutex::RwLock<eframe::egui_wgpu::Renderer>>,
    properties_windows: Arc<DashSet<PropertiesWindow>>,
    environment: Environment,
}

struct PropertiesWindow {
    mesh: Mesh,
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
        environment: Environment,
    ) -> Self {
        Self {
            show,
            detached,
            meshes: Arc::new(Mutex::new(Vec::new())),
            ctx: storm_ctx,
            renderer,
            properties_windows: Arc::new(DashSet::new()),
            environment,
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

    pub(super) fn ui(&self, ui: &mut egui::Ui) {
        if self.show.load(Ordering::Relaxed) {
            if self.detached.load(Ordering::Relaxed) {
                let this = self.clone();
                ui.show_viewport_deferred(
                    egui::ViewportId::from_hash_of("Mesh explorer"),
                    egui::ViewportBuilder::default().with_title("Mesh explorer"),
                    move |ui, class| {
                        if class == egui::ViewportClass::EmbeddedWindow {
                            this.wrap_in_window(ui);
                        } else {
                            egui::CentralPanel::default().show_inside(ui, |ui| {
                                this.content(ui);
                            });

                            if ui.input(|i| i.viewport().close_requested()) {
                                this.show.store(false, Ordering::Relaxed);
                            }
                        }
                    },
                )
            } else {
                self.wrap_in_window(ui);
            }
        }
    }

    fn wrap_in_window(&self, ui: &mut egui::Ui) {
        let mut open = self.show.load(Ordering::Relaxed);
        egui::Window::new("Mesh explorer")
            .open(&mut open)
            .vscroll(true)
            .show(ui.ctx(), |ui| {
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
                    let mut scene = Scene {
                        entity_registry: EntityRegistry::new(),
                        name: mesh.name().to_string(),
                        scene_graph: SceneGraph::new(&self.ctx),
                        mesh_instances: HashMap::new(),
                        skin_manager: SkinManager::new(&self.ctx),
                        animation_manager: AnimationManager::default(),
                        light_manager: LightManager::new(&self.ctx),
                        environment: self.environment.clone(),
                    };
                    let mesh_entity = scene.entity_registry.new_entity();
                    scene.scene_graph.add(mesh_entity, None);
                    let instance = mesh.new_instance(mesh_entity);
                    scene.mesh_instances.insert(instance.id(), instance);

                    let camera_entity = scene.entity_registry.new_entity();
                    scene.scene_graph.add_with_transform(
                        camera_entity,
                        None,
                        vec3(0.0, 0.0, 2.0),
                        Quat::IDENTITY,
                        Vec3::ONE,
                    );
                    let camera = Camera::new(camera_entity);

                    let scene = Arc::new(Mutex::new(scene));

                    self.properties_windows.insert(PropertiesWindow {
                        mesh: mesh.clone(),
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
