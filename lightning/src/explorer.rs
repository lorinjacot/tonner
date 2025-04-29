use egui::Ui;
use storm::{DenseEntry, Id, Node, Scene, Transform};

pub struct Explorer {
    node_modal: Option<Id<Node>>,
}

impl Explorer {
    pub fn new() -> Self {
        Self { node_modal: None }
    }

    pub fn ui(&mut self, ui: &mut Ui, scenes: &mut [Scene], active_scene: &mut Option<usize>) {
        ui.heading("Explorer");
        if scenes.len() > 0 {
            ui.horizontal(|ui| {
                ui.label("Scene");
                egui::ComboBox::from_id_salt("Scene")
                    .selected_text(active_scene.map_or("", |index| &scenes[index].name))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(active_scene, None, "");
                        for (index, scene) in scenes.iter().enumerate() {
                            if ui
                                .selectable_value(active_scene, Some(index), &scene.name)
                                .clicked()
                            {
                                self.node_modal = None;
                            };
                        }
                    })
            });
        }

        if let Some(active_scene) = active_scene {
            let scene = &scenes[*active_scene];
            ui.separator();
            ui.label("Nodes");
            for node in scene.root_nodes() {
                self.node_ui(ui, *node, scene);
            }

            ui.separator();
            ui.label("Camera");
            let mut active_camera = scene.active_camera();
            egui::ComboBox::from_id_salt("Camera")
                .selected_text(active_camera.map_or("", |id| &scene.camera(id).unwrap().name))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut active_camera, None, "");
                    for camera in scene.cameras() {
                        ui.selectable_value(&mut active_camera, Some(camera.id()), &camera.name);
                    }
                });

            //     ui.separator();
            //     ui.label("Environment");
            //     let mut active_environment_map = scene.environment_map;
            //     egui::ComboBox::from_id_salt("Environment map")
            //         .selected_text(
            //             active_environment_map
            //                 .map_or("", |id| &storm.environment_map(id).unwrap().name.0),
            //         )
            //         .show_ui(ui, |ui| {
            //             ui.selectable_value(&mut active_environment_map, None, "");
            //             for (id, environment_map) in storm.environment_maps() {
            //                 ui.selectable_value(
            //                     &mut active_environment_map,
            //                     Some(id),
            //                     &environment_map.name.0,
            //                 );
            //             }
            //         });

            //     let scene = storm.active_scene_mut().unwrap();

            let scene = &mut scenes[*active_scene];
            scene.set_active_camera(active_camera);
            // scene.environment_map = active_environment_map;

            if let Some(node) = self.node_modal {
                let modal = egui::Modal::new(egui::Id::new(format!("{node} properties")))
                    .show(ui.ctx(), |ui| self.node_modal_ui(ui, node, scene));

                if modal.should_close() {
                    self.node_modal = None;
                }
            }
        }
    }

    fn node_ui(&mut self, ui: &mut Ui, id: Id<Node>, scene: &Scene) {
        let node = &scene[id];
        ui.collapsing(&node.name, |ui| {
            for node in node.children() {
                self.node_ui(ui, *node, scene);
            }
        })
        .header_response
        .context_menu(|ui| {
            if ui.button("Properties").clicked() {
                self.node_modal = Some(id);
            }
        });
    }

    fn node_modal_ui(&self, ui: &mut Ui, id: Id<Node>, scene: &mut Scene) {
        let node = &mut scene[id];
        ui.heading(format!("{}'s properties", node.name));

        ui.horizontal(|ui| {
            ui.label("Id");
            ui.code(format!("{id}"));
        });
        ui.horizontal(|ui| {
            ui.label("Label");
            ui.text_edit_singleline(&mut node.name)
        });

        ui.collapsing("Local transform", |ui| {
            self.transform_ui(ui, node.local_transform())
        });

        ui.collapsing("Global transform", |ui| {
            self.transform_ui(ui, node.local_transform())
        });
    }

    fn transform_ui(&self, ui: &mut Ui, transform: &Transform) {
        egui::Grid::new("Transform").show(ui, |ui| {
            ui.label("Scale");
            ui.code(transform.scale().to_string());
            ui.end_row();

            ui.label("Rotation (quaternion)");
            ui.code(transform.rotation().to_string());
            ui.end_row();

            let (axis, angle) = transform.rotation().to_axis_angle();
            ui.label("Rotation axis");
            ui.code(axis.to_string());
            ui.end_row();

            ui.label("Rotation angle");
            ui.code(angle.to_string());
            ui.end_row();

            ui.label("Translation");
            ui.code(transform.translation().to_string());
            ui.end_row();
        });
    }
}
