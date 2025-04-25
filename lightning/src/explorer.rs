use egui::Ui;
use glam::Mat4;

use storm::{Id, Node, Scene, Storm};

pub struct Explorer {
    node_modal: Option<Id<Node>>,
}

impl Explorer {
    pub fn new() -> Self {
        Self { node_modal: None }
    }

    pub fn ui(&mut self, ui: &mut Ui, storm: &mut Storm) {
        ui.heading("Explorer");
        if storm.scenes().len() > 0 {
            ui.horizontal(|ui| {
                ui.label("Scene");
                egui::ComboBox::from_id_salt("Scene")
                    .selected_text(storm.active_scene().map_or("", |scene| &scene.label))
                    .show_ui(ui, |ui| {
                        let mut active_scene = storm.active_scene;
                        for (id, scene) in storm.scenes() {
                            if ui
                                .selectable_value(&mut active_scene, Some(*id), &scene.label)
                                .clicked()
                            {
                                self.node_modal = None;
                            };
                        }
                        storm.active_scene = active_scene;
                    })
            });
        }

        if let Some(scene) = storm.active_scene() {
            ui.separator();
            ui.label("Nodes");
            for node in scene.root_nodes() {
                self.node_ui(ui, *node, scene);
            }

            ui.separator();
            ui.label("Camera");
            let mut active_camera = scene.active_camera;
            egui::ComboBox::from_id_salt("Camera")
                .selected_text(
                    scene
                        .active_camera
                        .map_or("", |camera| &scene.camera(camera).unwrap().name.0),
                )
                .show_ui(ui, |ui| {
                    for (id, camera) in scene.cameras.iter() {
                        ui.selectable_value(&mut active_camera, Some(*id), &camera.name.0);
                    }
                });

            ui.separator();
            ui.label("Environment");
            let mut active_environment_map = scene.environment_map;
            egui::ComboBox::from_id_salt("Environment map")
                .selected_text(
                    active_environment_map
                        .map_or("", |id| &storm.environment_map(id).unwrap().name.0),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut active_environment_map, None, "");
                    for (id, environment_map) in storm.environment_maps() {
                        ui.selectable_value(
                            &mut active_environment_map,
                            Some(*id),
                            &environment_map.name.0,
                        );
                    }
                });

            let scene = storm.active_scene_mut().unwrap();
            scene.active_camera = active_camera;
            scene.environment_map = active_environment_map;

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
        ui.collapsing(&node.name.0, |ui| {
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
            ui.text_edit_singleline(&mut node.name.0)
        });

        ui.collapsing("Local transform", |ui| {
            self.transform_ui(ui, node.local_matrix())
        });

        ui.collapsing("Global transform", |ui| {
            self.transform_ui(ui, node.local_matrix())
        });
    }

    fn transform_ui(&self, ui: &mut Ui, transform: Mat4) {
        ui.label(format!("{transform:#?}"));
        egui::Grid::new("Transform").show(ui, |ui| {
            let (scale, rotation, translation) = transform.to_scale_rotation_translation();

            ui.label("Scale");
            ui.code(scale.to_string());
            ui.end_row();

            ui.label("Rotation quaternion");
            ui.code(rotation.to_string());
            ui.end_row();

            let (axis, angle) = rotation.to_axis_angle();
            ui.label("Rotation axis");
            ui.code(axis.to_string());
            ui.end_row();

            ui.label("Rotation angle");
            ui.code(angle.to_string());
            ui.end_row();

            ui.label("Translation");
            ui.code(translation.to_string());
            ui.end_row();
        });
    }
}
