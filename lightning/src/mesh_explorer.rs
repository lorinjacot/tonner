use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use dashmap::DashSet;
use storm::mesh::Mesh;

#[derive(Debug, Clone)]
pub(super) struct MeshExplorer {
    show: Arc<AtomicBool>,
    detached: Arc<AtomicBool>,
    meshes: Arc<DashSet<Mesh>>,
}

impl MeshExplorer {
    pub(super) fn new(show: Arc<AtomicBool>, detached: Arc<AtomicBool>) -> Self {
        Self {
            show,
            detached,
            meshes: Arc::new(DashSet::new()),
        }
    }

    pub(super) fn meshes(&self) -> &Arc<DashSet<Mesh>> {
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
            .show(egui_ctx, |ui| {
                self.content(ui);
            });
        self.show.store(open, Ordering::Relaxed);
    }

    fn content(&self, ui: &mut egui::Ui) {
        ui.label("Mesh explorer content..");
    }
}
