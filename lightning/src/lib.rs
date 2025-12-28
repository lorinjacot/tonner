use std::{
    sync::{Arc, RwLock, atomic::AtomicBool},
    time::Duration,
};

use egui::containers::menu::SubMenuButton;
pub use scene_view::SceneView;
use storm::{Context, Scene, SceneBuilder, camera::CameraBuilder, gltf::GltfAsset};

use crate::{mesh_explorer::MeshExplorer, new_scene::NewSceneModal};

mod mesh_explorer;
mod new_scene;
mod scene_view;
mod shortcut;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
struct State {
    // Example stuff:
    label: String,

    #[serde(skip)] // This how you opt-out of serialization of a field
    value: f32,

    show_mesh_explorer: Arc<AtomicBool>,
    #[cfg_attr(target_arch = "wasm32", serde(skip))]
    detached_mesh_explorer: Arc<AtomicBool>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "Hello World!".to_owned(),
            value: 2.7,
            show_mesh_explorer: Arc::new(AtomicBool::new(false)),
            detached_mesh_explorer: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub struct App {
    state: State,
    storm_ctx: Context,
    mesh_explorer: MeshExplorer,
    scenes: Vec<Arc<RwLock<Scene>>>,
    main_scene: Arc<RwLock<Scene>>,
    main_scene_view: SceneView,
    new_scene_modal: NewSceneModal,
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let state: State = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        let wgpu_state = cc.wgpu_render_state.as_ref().unwrap();
        let storm_ctx = Context::from_device(wgpu_state.device.clone(), wgpu_state.queue.clone());

        let mut scene = SceneBuilder::default().build(&storm_ctx);
        let camera = CameraBuilder::default().build(&mut scene);

        let scene = Arc::new(RwLock::new(scene));
        let main_scene_view = SceneView::new(
            scene.clone(),
            camera,
            300,
            300,
            &mut wgpu_state.renderer.write(),
            &storm_ctx,
        );

        let mesh_explorer = MeshExplorer::new(
            state.show_mesh_explorer.clone(),
            state.detached_mesh_explorer.clone(),
        );

        Self {
            state,
            storm_ctx,
            mesh_explorer,
            scenes: vec![scene.clone()],
            main_scene: scene,
            main_scene_view,
            new_scene_modal: NewSceneModal::default(),
        }
    }

    fn open_file(&mut self) {
        let ctx = self.storm_ctx.clone();
        let meshes = Arc::clone(self.mesh_explorer.meshes());
        run(async move {
            if let Some(path) = rfd::AsyncFileDialog::new()
                .add_filter("glTF", &["gltf", "glb"])
                .pick_file()
                .await
            {
                let mut asset = GltfAsset::open(path.path()).unwrap();
                dbg!(&asset);

                let mut encoder =
                    ctx.device()
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("lighting::App:open_file command encoder"),
                        });

                asset
                    .load_meshes(&ctx, &mut encoder)
                    .unwrap()
                    .into_iter()
                    .for_each(|mesh| {
                        meshes.insert(mesh);
                    });

                // let scenes = asset.create_all_scenes(&mut self.engine);

                ctx.queue().submit([encoder.finish()]);
            }
        });
    }
}

impl eframe::App for App {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let mut encoder =
            self.storm_ctx
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("App::update command encoder"),
                });
        let duration = Duration::from_secs_f32(ctx.input(|input_state| input_state.stable_dt));

        ctx.input_mut(|input_state| {
            if input_state.consume_shortcut(&shortcut::NEW_SCENE) {
                self.new_scene_modal.open = true;
            }

            if input_state.consume_shortcut(&shortcut::OPEN_FILE) {
                self.open_file();
            }

            if input_state.consume_shortcut(&shortcut::MESH_EXPLORER) {
                self.mesh_explorer.toggle_show();
            }
        });

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
                ui.menu_button("File", |ui| {
                    if ui
                        .add(
                            egui::Button::new("New Scene")
                                .shortcut_text(ctx.format_shortcut(&shortcut::NEW_SCENE)),
                        )
                        .clicked()
                    {
                        self.new_scene_modal.open = true;
                    }

                    if ui
                        .add(
                            egui::Button::new("Open File")
                                .shortcut_text(ctx.format_shortcut(&shortcut::OPEN_FILE)),
                        )
                        .clicked()
                    {
                        self.open_file();
                    }

                    // NOTE: no File->Quit on web pages!
                    let is_web = cfg!(target_arch = "wasm32");
                    if !is_web {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui
                        .add(
                            egui::Button::new("Mesh Explorer")
                                .shortcut_text(ctx.format_shortcut(&shortcut::MESH_EXPLORER)),
                        )
                        .clicked()
                    {
                        self.mesh_explorer.toggle_show();
                    }

                    if cfg!(not(target_arch = "wasm32")) {
                        ui.separator();

                        SubMenuButton::from_button(
                            egui::Button::new("Detached Windows")
                                .right_text(SubMenuButton::RIGHT_ARROW),
                        )
                        .ui(ui, |ui| {
                            let mut detached = self.mesh_explorer.detached();
                            ui.checkbox(&mut detached, "Mesh Explorer");
                            self.mesh_explorer.set_detached(detached);
                        });
                    }
                });

                ui.add_space(16.0);

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
                &self.storm_ctx,
                &mut encoder,
            );

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            });
        });

        if let Some(mut scene) = self.new_scene_modal.ui(ctx, &self.storm_ctx) {
            let camera = CameraBuilder::default().build(&mut scene);
            let scene = Arc::new(RwLock::new(scene));
            self.scenes.push(scene.clone());
            let mut renderer = frame.wgpu_render_state().unwrap().renderer.write();
            self.main_scene_view = SceneView::new(
                scene.clone(),
                camera,
                300,
                300,
                &mut renderer,
                &self.storm_ctx,
            );
            self.main_scene = scene;
        }

        self.mesh_explorer.ui(ctx);

        self.storm_ctx.queue().submit([encoder.finish()]);
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

#[cfg(not(target_arch = "wasm32"))]
fn run<F: IntoFuture<Output = ()> + Send + 'static>(future: F) {
    std::thread::spawn(|| {
        pollster::block_on(future);
    });
}

#[cfg(target_arch = "wasm32")]
fn run<F: IntoFuture<Output = ()> + 'static>(future: F) {
    wasm_bindgen_futures::spawn_local(future.into_future());
}
