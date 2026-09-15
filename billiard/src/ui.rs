use std::fmt::Display;

use winit::{event::WindowEvent, window::Window};

pub struct Ui {
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    state: UiState,
}

impl Ui {
    /// Creates a new `Ui` instance with the given window, device, and surface format.
    ///
    /// The surface format should be a non-sRGB format, as the Ui will handle sRGB conversion internally.
    pub fn new(window: &Window, device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Ui {
        assert!(
            !surface_format.is_srgb(),
            "Surface format must be non-sRGB, but got: {:?}",
            surface_format
        );

        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx,
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(device.limits().max_texture_dimension_2d as usize),
        );
        let egui_renderer = egui_wgpu::Renderer::new(
            device,
            surface_format,
            egui_wgpu::RendererOptions::default(),
        );

        Ui {
            egui_state,
            egui_renderer,
            state: UiState::Startup,
        }
    }

    /// Should be called on every winit's `WindowEvent` to update the internal state of the UI.
    /// Returns an `egui_winit::EventResponse` indicating whether the event was consumed by the UI or not and whether a repaint is requested.
    #[must_use]
    pub fn on_window_event(
        &mut self,
        window: &Window,
        event: &WindowEvent,
    ) -> egui_winit::EventResponse {
        self.egui_state.on_window_event(window, event)
    }

    /// Should be called on every mouse motion event to update the internal state of the UI.
    /// Returns `true` if the mouse motion event was consumed by the UI, otherwise returns `false`.
    pub fn on_mouse_motion(&mut self, delta: (f64, f64)) -> bool {
        self.egui_state.on_mouse_motion(delta)
    }

    /// Renders the current UI state to the given render target using the provided device, queue, and command encoder.
    /// Returns a vector of command buffers that should be submitted to the GPU for rendering the UI.
    ///
    /// The `render_target` format should match the `surface_format` used when creating the `Ui` instance.
    pub fn render(
        &mut self,
        window: &Window,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
    ) -> (Action, Vec<wgpu::CommandBuffer>) {
        let raw_input = self.egui_state.take_egui_input(window);

        let mut action = Action::None;
        let full_output = self.egui_state.egui_ctx().run_ui(raw_input, |ui| {
            action = self.state.render(ui);
        });

        self.egui_state
            .handle_platform_output(window, full_output.platform_output);

        let clipped_primitives = self
            .egui_state
            .egui_ctx()
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let window_size = window.inner_size();
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [window_size.width, window_size.height],
            pixels_per_point: full_output.pixels_per_point,
        };

        let command_buffers = self.egui_renderer.update_buffers(
            device,
            queue,
            encoder,
            &clipped_primitives,
            &screen_descriptor,
        );

        for (id, delta) in full_output.textures_delta.set {
            self.egui_renderer.update_texture(device, queue, id, &delta);
        }
        for id in full_output.textures_delta.free {
            self.egui_renderer.free_texture(&id);
        }

        {
            let mut egui_render_pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui render RenderPassDescriptor"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: render_target,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();

            self.egui_renderer.render(
                &mut egui_render_pass,
                &clipped_primitives,
                &screen_descriptor,
            );
        }

        (action, command_buffers)
    }
}

#[derive(Debug, Clone)]
pub enum UiState {
    Startup,
    MainMenu,
    InGame(GameState),
    GameOver { winner: Player },
}

#[derive(Debug, Clone)]
pub enum GameState {
    Playing { turn: Player },
    Watching { last: Player },
}

#[derive(Debug, Clone)]
pub enum Player {
    Solid,
    Stripe,
}

impl Display for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Player::Solid => write!(f, "Solid"),
            Player::Stripe => write!(f, "Stripde"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    None,
    NewGame { first_turn: Player },
    MainMenu,
}

impl UiState {
    fn render(&self, ui: &mut egui::Ui) -> Action {
        match self {
            UiState::Startup => Self::startup(ui),
            UiState::MainMenu => Self::main_menu(ui),
            UiState::InGame(game_state) => Self::in_game(game_state, ui),
            UiState::GameOver { winner } => Self::game_over(winner, ui),
        }
    }

    fn startup(ui: &mut egui::Ui) -> Action {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.label("Loading assets and scripts ...");
            });
        });
        Action::None
    }

    fn main_menu(ui: &mut egui::Ui) -> Action {
        egui::CentralPanel::default()
            .show_inside(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    if ui.button("New game").clicked() {
                        Action::NewGame {
                            first_turn: Player::Solid,
                        }
                    } else {
                        Action::None
                    }
                })
                .inner
            })
            .inner
    }

    fn in_game(game_state: &GameState, ui: &mut egui::Ui) -> Action {
        egui::Panel::top("Status bar").show_inside(ui, |ui| match game_state {
            GameState::Playing { turn } => {
                ui.label(format!("Now playing: {turn}"));
            }
            GameState::Watching { last } => {
                ui.label(format!("Now playing: {last}"));
            }
        });
        Action::None
    }

    fn game_over(winner: &Player, ui: &mut egui::Ui) -> Action {
        egui::Panel::top("Status bar")
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{winner} won!"));
                    if ui.button("Exit to main menu").clicked() {
                        Action::MainMenu
                    } else {
                        Action::None
                    }
                })
                .inner
            })
            .inner
    }
}
