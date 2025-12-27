use storm::{Engine, Scene, SceneBuilder};

#[derive(Debug, Default)]
pub(super) struct NewSceneModal {
    pub(super) open: bool,
    name: String,
}

impl NewSceneModal {
    pub(super) fn ui(&mut self, ctx: &egui::Context, engine: &mut Engine) -> Option<Scene> {
        if !self.open {
            return None;
        }

        let modal = egui::Modal::new(egui::Id::new("New Scene Modal")).show(ctx, |ui| {
            ui.heading("New Scene");

            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut self.name);
            });

            egui::Sides::new()
                .show(
                    ui,
                    |_ui| {},
                    |ui| {
                        let scene = if ui.button("Create").clicked() {
                            ui.close();
                            Some(
                                SceneBuilder::default()
                                    .name(self.name.clone())
                                    .build(engine),
                            )
                        } else {
                            None
                        };
                        if ui.button("Cancel").clicked() {
                            ui.close();
                        }
                        scene
                    },
                )
                .1
        });

        if modal.should_close() {
            self.open = false;
        }

        modal.inner
    }
}
