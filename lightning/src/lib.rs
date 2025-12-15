use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

pub use scene_view::SceneView;
use storm::{Engine, Scene, SceneBuilder, camera::CameraBuilder};

mod scene_view;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
struct State {
    // Example stuff:
    label: String,

    #[serde(skip)] // This how you opt-out of serialization of a field
    value: f32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "Hello World!".to_owned(),
            value: 2.7,
        }
    }
}

pub struct App {
    state: State,
    engine: Engine,
    main_scene: Arc<RwLock<Scene>>,
    main_scene_view: SceneView,
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let state = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        let wgpu_state = cc.wgpu_render_state.as_ref().unwrap();
        let mut engine = Engine::new(wgpu_state.device.clone(), wgpu_state.queue.clone());

        let mut scene = SceneBuilder::default().build(&mut engine);
        let camera = CameraBuilder::default().build(&mut scene);

        let scene = Arc::new(RwLock::new(scene));
        let main_scene_view = SceneView::new(
            scene.clone(),
            camera,
            300,
            300,
            &mut wgpu_state.renderer.write(),
            &mut engine,
        );

        Self {
            state,
            engine,
            main_scene: scene,
            main_scene_view,
        }
    }
}

impl eframe::App for App {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let mut encoder = self.engine.encoder(Some("App::update command encoder"));
        let duration = Duration::from_secs_f32(ctx.input(|input_state| input_state.stable_dt));
        self.main_scene
            .write()
            .unwrap()
            .simulate(duration, &mut encoder)
            .unwrap();

        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::MenuBar::new().ui(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            ui.heading("eframe template");

            ui.horizontal(|ui| {
                ui.label("Write something: ");
                ui.text_edit_singleline(&mut self.state.label);
            });

            ui.add(egui::Slider::new(&mut self.state.value, 0.0..=10.0).text("value"));
            if ui.button("Increment").clicked() {
                self.state.value += 1.0;
            }

            ui.separator();

            ui.add(egui::github_link_file!(
                "https://github.com/emilk/eframe_template/blob/main/",
                "Source code."
            ));

            self.main_scene_view.render(
                ui,
                &mut frame.wgpu_render_state().unwrap().renderer.write(),
                &mut self.engine,
                &mut encoder,
            );

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            });
        });

        self.engine.submit_commands(encoder.finish());
        ctx.request_repaint();
    }
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
}
