use std::f32::consts::{FRAC_PI_2, PI};
use std::iter::once;
use std::sync::Arc;
use std::time::Instant;

use glam::{Quat, Vec3, vec3};
use image::DynamicImage;
use image::codecs::hdr::HdrDecoder;
use storm_controls::EguiControls;
use storm_controls::orbit::OrbitControls;
use tonner::Context;
use tonner::entity_component::EntityManager;
use tonner::entity_component::component::sparse_array::SparseArray;
use tonner::environment::{Environment, EnvironmentBuilder};
use tonner::geometry::skin::SkinManager;
use tonner::geometry::{ArrowBuilder, GeometryBuilder, SphereBuilder};
use tonner::mesh::material::{AlphaMode, Material, MaterialBuilder};
use tonner::mesh::{Mesh, MeshBuilder, MeshInstance};
use tonner::renderer::Renderer;
use tonner::renderer::camera::Camera;
use tonner::renderer::light::LightManager;
use tonner::scene_graph::SceneGraph;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle};
use winit::window::{Window, WindowId};

use crate::epa::{EpaEngine, EpaState};
use crate::gjk::gjk_tetrahedron;
use crate::shape::{AxisAlignedBox, Ball};

mod epa;
mod gjk;
mod shape;

const DEFAULT_STEPS: usize = 0;

fn create_shapes() -> (AxisAlignedBox, Ball) {
    let aab = AxisAlignedBox::from_center_dimension(Vec3::ZERO, 2.0, 2.0, 2.0);

    let mut ball = Ball {
        center: Vec3::ZERO,
        radius: 1.0,
    };
    ball.center = vec3(1.99, 0.0, 0.0);

    (aab, ball)
}

fn create_points(
    epa_state: &EpaState,
    entity_manager: &mut EntityManager,
    scene_graph: &mut SceneGraph,
    point: &Mesh,
) -> SparseArray<MeshInstance> {
    epa_state
        .vertices
        .iter()
        .map(|v| {
            let entity = entity_manager.new_entity();
            scene_graph.add_with_transform(entity, None, v.difference, Quat::IDENTITY, Vec3::ONE);
            (entity, point.new_instance(entity))
        })
        .collect()
}

fn create_faces(
    epa_state: &EpaState,
    entity_manager: &mut EntityManager,
    scene_graph: &mut SceneGraph,
    context: &Context,
    face_material: &Material,
) -> SparseArray<MeshInstance> {
    epa_state
        .faces
        .iter()
        .enumerate()
        .filter(|(_, face)| !face.obsolete)
        .map(|(i, face)| {
            let entity = entity_manager.new_entity();
            scene_graph.add(entity, None);
            let face = GeometryBuilder::new(3, 0)
                .positions(
                    face.vertex_indices
                        .map(|i| epa_state.vertices[i].difference),
                )
                .unwrap()
                .build(&context)
                .unwrap();
            let face = MeshBuilder::default()
                .name(format!("Face {i}"))
                .primitive(face, face_material.clone())
                .build(&context)
                .unwrap();
            (entity, face.new_instance(entity))
        })
        .collect()
}

fn create_normals(
    epa_state: &EpaState,
    entity_manager: &mut EntityManager,
    scene_graph: &mut SceneGraph,
    normal_mesh: &Mesh,
) -> SparseArray<MeshInstance> {
    epa_state
        .faces
        .iter()
        .filter(|face| !face.obsolete)
        .map(|face| {
            let entity = entity_manager.new_entity();
            let origin = face.closest;
            let dir = face.closest.normalize();
            let mut rotation = Quat::look_to_rh(dir, Vec3::Y);
            if rotation.is_nan() {
                rotation = Quat::look_to_rh(dir, Vec3::X);
            }
            scene_graph.add_with_transform(entity, None, origin, rotation.inverse(), Vec3::ONE);
            (entity, normal_mesh.new_instance(entity))
        })
        .collect()
}

struct Scene {
    context: Context,
    entity_manager: EntityManager,
    scene_graph: SceneGraph,
    point: Mesh,
    axis: MeshInstance,
    skin_manager: SkinManager,
    light_manager: LightManager,
    environment: Environment,
    renderer: Renderer,
    controls: OrbitControls,
    last_frame: Instant,
    steps: usize,
    rendered_steps: usize,
    yellow: Material,
    normal_mesh: Mesh,
    points: SparseArray<MeshInstance>,
    faces: SparseArray<MeshInstance>,
    normals: SparseArray<MeshInstance>,
    epa_engine: EpaEngine,
}

impl Scene {
    fn new(context: Context, width: u32, height: u32) -> Scene {
        let mut encoder =
            context
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("App::resumed command encoder"),
                });

        let mut entity_manager = EntityManager::new();
        let mut scene_graph = SceneGraph::new(&context);

        let camera = entity_manager.new_entity();
        scene_graph.add_with_transform(camera, None, 2.0 * Vec3::Z, Quat::IDENTITY, Vec3::ONE);
        let camera = Camera::new(camera);

        let skin_manager = SkinManager::new(&context);
        let light_manager = LightManager::new(&context);
        let radiance_image = include_bytes!("../../assets/newport_loft.hdr");
        let radiance_image = std::io::Cursor::new(radiance_image);
        let radiance_image = HdrDecoder::new(radiance_image).unwrap();
        let radiance_image = DynamicImage::from_decoder(radiance_image).unwrap();
        let environment = EnvironmentBuilder::default()
            .equirectangular_map(radiance_image)
            .build(&context, &mut encoder);
        let renderer = Renderer::new(width, height, wgpu::TextureFormat::Rgba8UnormSrgb, &context);

        let controls = OrbitControls::new(camera);

        let red = MaterialBuilder::default()
            .base_color_factor([1.0, 0.0, 0.0, 0.9])
            .alpha_mode(AlphaMode::Blend)
            .build(&context);

        let green = MaterialBuilder::default()
            .base_color_factor([0.0, 1.0, 0.0, 0.9])
            .alpha_mode(AlphaMode::Blend)
            .build(&context);

        let blue = MaterialBuilder::default()
            .base_color_factor([0.0, 0.0, 1.0, 0.9])
            .alpha_mode(AlphaMode::Blend)
            .build(&context);

        let x_axis = ArrowBuilder::default()
            .name("X Axis")
            .rotate(Quat::from_rotation_y(-FRAC_PI_2))
            .build(&context);

        let y_axis = ArrowBuilder::default()
            .name("Y Axis")
            .rotate(Quat::from_rotation_x(FRAC_PI_2))
            .build(&context);

        let z_axis = ArrowBuilder::default()
            .name("Z Axis")
            .rotate(Quat::from_rotation_x(PI))
            .build(&context);

        let axis = MeshBuilder::default()
            .primitive(x_axis.head, red.clone())
            .primitive(x_axis.body, red)
            .primitive(y_axis.head, green.clone())
            .primitive(y_axis.body, green)
            .primitive(z_axis.head, blue.clone())
            .primitive(z_axis.body, blue)
            .build(&context)
            .unwrap();

        let axis_entity = entity_manager.new_entity();
        scene_graph.add(axis_entity, None);
        let axis = axis.new_instance(axis_entity);

        let black = MaterialBuilder::default()
            .base_color_factor([0.0, 0.0, 0.0, 1.0])
            .build(&context);

        let point = SphereBuilder::default().radius(0.02).build(&context);
        let point = MeshBuilder::default()
            .primitive(point, black.clone())
            .build(&context)
            .unwrap();

        let mut epa_engine = EpaEngine::default();

        let (aab, ball) = create_shapes();

        let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        let epa_result =
            epa_engine.penetration_depth_details(&aab, &ball, tetrahedron, DEFAULT_STEPS);

        let yellow = MaterialBuilder::default()
            .base_color_factor([1.0, 1.0, 0.0, 0.8])
            .alpha_mode(AlphaMode::Opaque)
            .double_sided(false)
            .build(&context);

        let normal_parts = ArrowBuilder::default().build(&context);
        let normal_mesh = MeshBuilder::default()
            .primitive(normal_parts.head, black.clone())
            .primitive(normal_parts.body, black)
            .build(&context)
            .unwrap();

        let points = create_points(&epa_result, &mut entity_manager, &mut scene_graph, &point);
        let faces = create_faces(
            &epa_result,
            &mut entity_manager,
            &mut scene_graph,
            &context,
            &yellow,
        );
        let normals = create_normals(
            &epa_result,
            &mut entity_manager,
            &mut scene_graph,
            &normal_mesh,
        );

        context.queue().submit([encoder.finish()]);

        Scene {
            context,
            entity_manager,
            scene_graph,
            point,
            axis,
            skin_manager,
            light_manager,
            environment,
            renderer,
            controls,
            last_frame: Instant::now(),
            steps: DEFAULT_STEPS,
            rendered_steps: DEFAULT_STEPS,
            yellow,
            normal_mesh,
            points,
            faces,
            normals,
            epa_engine,
        }
    }

    fn render(&mut self, texture_view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        if self.steps != self.rendered_steps {
            self.points.drain().for_each(|(entity, _)| {
                self.scene_graph.remove(entity);
                self.entity_manager.delete_entity(entity);
            });
            self.faces.drain().for_each(|(entity, _)| {
                self.scene_graph.remove(entity);
                self.entity_manager.delete_entity(entity);
            });
            self.normals.drain().for_each(|(entity, _)| {
                self.scene_graph.remove(entity);
                self.entity_manager.delete_entity(entity);
            });

            let (aab, ball) = create_shapes();
            let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
            let epa_result =
                self.epa_engine
                    .penetration_depth_details(&aab, &ball, tetrahedron, self.steps);

            self.points = create_points(
                &epa_result,
                &mut self.entity_manager,
                &mut self.scene_graph,
                &self.point,
            );

            self.faces = create_faces(
                &epa_result,
                &mut self.entity_manager,
                &mut self.scene_graph,
                &self.context,
                &self.yellow,
            );

            self.normals = create_normals(
                &epa_result,
                &mut self.entity_manager,
                &mut self.scene_graph,
                &self.normal_mesh,
            );

            self.rendered_steps = self.steps;
        }

        self.renderer
            .render(
                &self.controls.camera,
                &texture_view,
                &mut self.scene_graph,
                &mut self.skin_manager,
                self.points
                    .values()
                    .chain(self.faces.values())
                    .chain(self.normals.values())
                    .chain(once(&self.axis)),
                &mut self.light_manager,
                &self.environment,
                &self.context,
                encoder,
            )
            .unwrap();
    }
}

struct State {
    scene: Scene,
    window: Arc<Window>,
    size: winit::dpi::PhysicalSize<u32>,
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    next_shortcut: egui::KeyboardShortcut,
    previous_shortcut: egui::KeyboardShortcut,
}

impl State {
    async fn new(display: OwnedDisplayHandle, window: Arc<Window>) -> State {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
            Box::new(display),
        ));
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();

        let size = window.inner_size();

        let surface = instance.create_surface(window.clone()).unwrap();
        let cap = surface.get_capabilities(&adapter);
        let surface_format = cap.formats[0];

        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx,
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(device.limits().max_texture_dimension_2d as usize),
        );

        let next_shortcut =
            egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::ArrowRight);
        let previous_shortcut =
            egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::ArrowLeft);

        let egui_renderer = egui_wgpu::Renderer::new(
            &device,
            surface_format.remove_srgb_suffix(),
            egui_wgpu::RendererOptions::default(),
        );

        let context = Context::from_device(device, queue);
        let scene = Scene::new(context, size.width, size.height);

        let state = State {
            scene,
            window,
            size,
            instance,
            surface,
            surface_format,
            egui_state,
            egui_renderer,
            next_shortcut,
            previous_shortcut,
        };

        state.configure_surface();

        state
    }

    fn configure_surface(&self) {
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            view_formats: vec![
                self.surface_format.remove_srgb_suffix(),
                self.surface_format.add_srgb_suffix(),
            ],
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            width: self.size.width,
            height: self.size.height,
            desired_maximum_frame_latency: 2,
            present_mode: wgpu::PresentMode::AutoVsync,
        };
        self.surface
            .configure(self.scene.context.device(), &surface_config);
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.configure_surface();
    }

    fn on_window_event(&mut self, event: &winit::event::WindowEvent) -> bool {
        let response = self.egui_state.on_window_event(&self.window, event);
        if response.repaint {
            self.window.request_redraw();
        }
        response.consumed
    }

    fn on_mouse_motion(&mut self, delta: (f64, f64)) -> bool {
        self.egui_state.on_mouse_motion(delta)
    }

    fn render(&mut self) {
        let dt = self.scene.last_frame.elapsed();

        let raw_input = self.egui_state.take_egui_input(&self.window);

        let full_output = self.egui_state.egui_ctx().run_ui(raw_input, |ui| {
            ui.input_mut(|input_state| {
                if input_state.consume_shortcut(&self.next_shortcut) {
                    self.scene.steps += 1;
                }
                if input_state.consume_shortcut(&self.previous_shortcut) {
                    self.scene.steps = self.scene.steps.saturating_sub(1);
                }
            });

            let response = ui.interact(ui.clip_rect(), egui::Id::new("scene"), egui::Sense::drag());
            self.scene
                .controls
                .handle_response(response, ui, &mut self.scene.scene_graph);
            ui.label(format!("EPA steps: {}", self.scene.steps));
            ui.horizontal(|ui| {
                ui.button("Previous")
                    .clicked()
                    .then(|| self.scene.steps = self.scene.steps.saturating_sub(1));
                ui.button("Next").clicked().then(|| self.scene.steps += 1);
            });
            ui.button("Reset EPA")
                .clicked()
                .then(|| self.scene.steps = 0);
        });

        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);

        self.scene.controls.update(
            &mut self.scene.scene_graph,
            dt,
            self.size.width as f32 / self.size.height as f32,
        );

        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Occluded | wgpu::CurrentSurfaceTexture::Timeout => return,
            wgpu::CurrentSurfaceTexture::Suboptimal(_) | wgpu::CurrentSurfaceTexture::Outdated => {
                self.configure_surface();
                return;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                unreachable!("No error scope registered, so validation errors will panic")
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self.instance.create_surface(self.window.clone()).unwrap();
                self.configure_surface();
                return;
            }
        };
        let srgb_texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                format: Some(self.surface_format.add_srgb_suffix()),
                ..Default::default()
            });

        let mut encoder = self
            .scene
            .context
            .device()
            .create_command_encoder(&Default::default());

        self.scene.render(&srgb_texture_view, &mut encoder);

        let clipped_primitives = self
            .egui_state
            .egui_ctx()
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: full_output.pixels_per_point,
        };
        let command_buffers = self.egui_renderer.update_buffers(
            self.scene.context.device(),
            self.scene.context.queue(),
            &mut encoder,
            &clipped_primitives,
            &screen_descriptor,
        );
        full_output.textures_delta.free.iter().for_each(|id| {
            self.egui_renderer.free_texture(&id);
        });
        full_output
            .textures_delta
            .set
            .into_iter()
            .for_each(|(id, delta)| {
                self.egui_renderer.update_texture(
                    self.scene.context.device(),
                    self.scene.context.queue(),
                    id,
                    &delta,
                );
            });

        let rgb_texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                format: Some(self.surface_format.remove_srgb_suffix()),
                ..Default::default()
            });
        let mut render_pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("State::render() render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &rgb_texture_view,
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
        self.egui_renderer
            .render(&mut render_pass, &clipped_primitives, &screen_descriptor);
        drop(render_pass);

        self.scene
            .context
            .queue()
            .submit(command_buffers.into_iter().chain(once(encoder.finish())));
        self.window.pre_present_notify();
        surface_texture.present();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );

        let state = pollster::block_on(State::new(
            event_loop.owned_display_handle(),
            window.clone(),
        ));
        self.state = Some(state);

        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = self.state.as_mut().unwrap();
        let consumed = state.on_window_event(&event);
        if !consumed {
            match event {
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }
                WindowEvent::RedrawRequested => {
                    state.render();
                    state.window.request_redraw();
                }
                WindowEvent::Resized(size) => {
                    state.resize(size);
                }
                _ => (),
            }
        }
    }
    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        match event {
            winit::event::DeviceEvent::MouseMotion { delta } => {
                let state = self.state.as_mut().unwrap();
                state.on_mouse_motion(delta);
            }
            _ => (),
        }
    }
}

#[derive(Default)]
struct App {
    state: Option<State>,
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
