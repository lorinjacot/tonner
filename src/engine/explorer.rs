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
    if let Some(scene) = storm.active_scene() {
        ui.separator();
        ui.label("Nodes");
        for node in scene.root_nodes() {
            add_node(ui, *node, scene);
        }
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
