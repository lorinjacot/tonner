use std::{
    collections::HashMap,
    io::Cursor,
    sync::{Arc, Mutex},
    time::Duration,
};

use glam::{Quat, Vec3};
use pyo3::prelude::*;
use tempete::{
    ecs::EntityRegistry,
    environment::{Environment, EnvironmentBuilder},
    geometry::skin::SkinManager,
    mesh::MeshInstance,
    renderer::{Renderer, camera::Camera, light::LightManager},
    scene_graph::{NodeHandle, SceneGraph},
};

use crate::{
    PhysicsEngine,
    arrow::Arrow,
    ball::{self, Ball, BallColor, BallsAsset},
    python::{self, PyScripts},
    table::table,
    ui::{Action, GameState, UiState},
};

pub struct Game {
    tempete_ctx: tempete::Context,
    renderer: Renderer,
    camera: Camera,
    scene_graph: Arc<Mutex<SceneGraph>>,
    skin_manager: SkinManager,
    light_manager: LightManager,
    environment: Environment,
    scripts: PyScripts,
    camera_node: Py<NodeHandle>,
    balls: HashMap<BallColor, Py<Ball>>,
    arrow: Py<Arrow>,
    mesh_instances: Vec<MeshInstance>,
    physics_engine: PhysicsEngine,
}

impl Game {
    /// Creates a new `Game` instance with the given device, queue, width, height, and surface format.
    ///
    /// This function initializes the game state, including loading assets and scripts, setting up the scene graph, and preparing the physics engine.
    ///
    /// The surface format should be a sRGB format, as the Game will output color in linear space.
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        width: u32,
        height: u32,
        surface_format: wgpu::TextureFormat,
    ) -> Game {
        assert!(
            surface_format.is_srgb(),
            "Surface format must be sRGB, but got: {:?}",
            surface_format
        );

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Game::new command encoder"),
        });

        let tempete_ctx = tempete::Context::from_device(device, queue);
        let balls_asset = BallsAsset::load(&tempete_ctx, &mut encoder).unwrap();

        let radiance_image = image::ImageReader::with_format(
            Cursor::new(include_bytes!("billiard_hall_1k.hdr")),
            image::ImageFormat::Hdr,
        )
        .decode()
        .unwrap();
        let environment = EnvironmentBuilder::default()
            .equirectangular_map(radiance_image)
            .build(&tempete_ctx, &mut encoder);

        tempete_ctx.queue().submit([encoder.finish()]);

        let renderer = Renderer::new(width, height, surface_format, &tempete_ctx);
        let mut entity_registry = EntityRegistry::new();
        let mut scene_graph = SceneGraph::new(&tempete_ctx);

        let mut physics_engine = tonner::Engine::new();

        let camera_entity = entity_registry.create();
        scene_graph.add_with_transform(camera_entity, None, Vec3::X, Quat::IDENTITY, Vec3::ONE);
        let camera = Camera::new(camera_entity);

        let mut balls = HashMap::new();
        let mut mesh_instances = Vec::new();

        mesh_instances.push(table(
            &mut entity_registry,
            &mut scene_graph,
            &mut physics_engine,
            &tempete_ctx,
        ));

        let scene_graph = Arc::new(Mutex::new(scene_graph));
        let physics_engine = Arc::new(Mutex::new(physics_engine));

        let (camera_node, arrow) = Python::attach(|py| -> PyResult<(Py<NodeHandle>, Py<Arrow>)> {
            let camera_node = Py::new(py, NodeHandle::new(camera_entity, scene_graph.clone()))?;

            ball::settings()
                .iter()
                .for_each(|(color, position, velocity)| {
                    let ball = Ball::new(
                        py,
                        *color,
                        *position,
                        *velocity,
                        &mut entity_registry,
                        scene_graph.clone(),
                        physics_engine.clone(),
                        &balls_asset,
                    );
                    balls.insert(*color, ball.into());
                });

            let arrow = Py::new(
                py,
                Arrow::new(py, &mut entity_registry, scene_graph.clone(), &tempete_ctx),
            )?;

            Ok((camera_node, arrow))
        })
        .unwrap();

        let scripts = python::PyScripts::new(&balls);

        Game {
            renderer,
            camera,
            scene_graph,
            skin_manager: SkinManager::new(&tempete_ctx),
            light_manager: LightManager::new(&tempete_ctx),
            environment,
            scripts,
            camera_node,
            balls,
            arrow,
            mesh_instances,
            physics_engine,
            tempete_ctx,
        }
    }

    /// Renders the game scene to the given render target using the provided command encoder.
    ///
    /// `render_target` format should match the `surface_format` used when creating the `Game` instance.
    pub fn render(
        &mut self,
        delta_time: Duration,
        ui_state: &mut UiState,
        action: &Action,
        render_target: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        Python::attach(|py| -> PyResult<()> {
            match action {
                Action::None => (),
                Action::MainMenu => *ui_state = UiState::MainMenu {},
                Action::NewGame { first_turn } => {
                    *ui_state = UiState::InGame {
                        game_state: GameState::Playing {
                            turn: first_turn.clone(),
                        },
                    }
                }
            }

            self.scripts
                .update(py, delta_time.as_secs_f32(), &self.camera_node, ui_state);

            let mut physics_engine = self.physics_engine.lock().unwrap();
            physics_engine.simulate(delta_time);

            let mut scene_graph = self.scene_graph.lock().unwrap();
            let balls: Vec<_> = self
                .balls
                .values()
                .map(|ball| ball.borrow_mut(py))
                .filter(|ball| !ball.out)
                .collect();
            for ball in &balls {
                let position = physics_engine.position(ball.physics_id()).unwrap();
                scene_graph.set_local_transformation(
                    ball.entity_id(),
                    position.as_vec3(),
                    None,
                    None,
                );
            }
            drop(physics_engine);

            self.renderer
                .render(
                    &self.camera,
                    render_target,
                    &mut scene_graph,
                    &mut self.skin_manager,
                    self.mesh_instances
                        .iter()
                        .chain(balls.iter().map(|ball| ball.mesh_instance()))
                        .chain(self.arrow.borrow(py).mesh_instances()),
                    &mut self.light_manager,
                    &self.environment,
                    &self.tempete_ctx,
                    encoder,
                )
                .expect("failed to render");

            Ok(())
        })
        .expect("failed to run python");
    }

    /// Handles mouse wheel events.
    pub fn on_mouse_wheel(&mut self, delta_x: f64, delta_y: f64) {
        self.scripts.mouse_wheel(delta_x, delta_y);
    }

    /// Handles mouse input events.
    pub fn on_mouse_input(&mut self, button: &'static str, state: &'static str) {
        self.scripts.mouse_input(button, state, &self.arrow);
    }

    /// Handles mouse motion events.
    pub fn on_mouse_moved(&mut self, delta_x: f64, delta_y: f64, viewport_aspect_ratio: f32) {
        self.scripts.mouse_moved(
            delta_x,
            delta_y,
            &self.camera_node,
            self.camera.projection_matrix(viewport_aspect_ratio),
            &self.arrow,
        );
    }

    /// Handles mouse motion events.
    pub fn on_mouse_motion(&mut self, delta_x: f64, delta_y: f64) {
        self.scripts.mouse_motion(delta_x, delta_y);
    }
}
