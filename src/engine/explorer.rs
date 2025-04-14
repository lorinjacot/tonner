use egui::Ui;

use crate::storm::{Id, Node, Scene, Storm};

pub fn add_contents(ui: &mut Ui, storm: &mut Storm) {
    ui.heading("Explorer");
    if storm.scenes.len() > 0 {
        ui.horizontal(|ui| {
            ui.label("Scene");
            egui::ComboBox::from_id_salt("Scene")
                .selected_text(storm.active_scene().map_or("", |scene| &scene.label))
                .show_ui(ui, |ui| {
                    for (id, scene) in storm.scenes.iter() {
                        ui.selectable_value(&mut storm.active_scene, Some(*id), &scene.label);
                    }
                })
        });
    }
    if let Some(scene) = storm.active_scene {
        let scene = &mut storm.scenes[scene];
        ui.separator();
        ui.label("Nodes");
        for node in scene.root_nodes() {
            add_node(ui, *node, scene);
        }

        ui.separator();
        ui.label("Camera");
        egui::ComboBox::from_id_salt("Camera")
            .selected_text(
                scene
                    .active_camera
                    .map_or("", |camera| &scene.camera(camera).unwrap().label),
            )
            .show_ui(ui, |ui| {
                for (id, camera) in scene.cameras.iter() {
                    ui.selectable_value(&mut scene.active_camera, Some(*id), &camera.label);
                }
            });
    }
}

fn add_node(ui: &mut Ui, node: Id<Node>, scene: &Scene) {
    let node = &scene[node];
    ui.collapsing(&node.label, |ui| {
        for node in node.children() {
            add_node(ui, *node, scene);
        }
    });
}
