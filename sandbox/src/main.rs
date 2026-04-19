use std::f32::consts::FRAC_PI_2;
use std::iter::once;
use std::sync::Arc;
use std::time::Instant;

use glam::{Quat, Vec3, vec3};
use storm_controls::EguiControls;
use storm_controls::orbit::OrbitControls;
use tonner::Context;
use tonner::entity_component::EntityManager;
use tonner::environment::{Environment, EnvironmentBuilder};
use tonner::geometry::skin::SkinManager;
use tonner::geometry::{ArrowBuilder, SphereBuilder};
use tonner::mesh::material::MaterialBuilder;
use tonner::mesh::{MeshBuilder, MeshInstance};
use tonner::renderer::Renderer;
use tonner::renderer::camera::Camera;
use tonner::renderer::light::LightManager;
use tonner::scene_graph::SceneGraph;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle};
use winit::window::{Window, WindowId};

use crate::epa::epa_dbg;
use crate::gjk::gjk_tetrahedron;
use crate::shape::{AxisAlignedBox, Ball};

mod epa;
mod gjk;
mod shape;

struct Scene {
    context: Context,
    scene_graph: SceneGraph,
    points: Vec<MeshInstance>,
    axis: MeshInstance,
    skin_manager: SkinManager,
    light_manager: LightManager,
    environment: Environment,
    renderer: Renderer,
    controls: OrbitControls,
    last_frame: Instant,
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
        let environment = EnvironmentBuilder::default().build(&context, &mut encoder);
        let renderer = Renderer::new(width, height, wgpu::TextureFormat::Rgba8UnormSrgb, &context);

        let controls = OrbitControls::new(camera);

        let red = MaterialBuilder::default()
            .base_color_factor([1.0, 0.0, 0.0, 1.0])
            .build(&context);

        let green = MaterialBuilder::default()
            .base_color_factor([0.0, 1.0, 0.0, 1.0])
            .build(&context);

        let blue = MaterialBuilder::default()
            .base_color_factor([0.0, 0.0, 1.0, 1.0])
            .build(&context);

        let x_axis = ArrowBuilder::default()
            .name("X Axis")
            .rotate(Quat::from_rotation_z(-FRAC_PI_2))
            .build(&context);

        let y_axis = ArrowBuilder::default()
            .name("Y Axis")
            .length(1.0)
            .build(&context);

        let z_axis = ArrowBuilder::default()
            .name("Z Axis")
            .length(1.0)
            .rotate(Quat::from_rotation_x(FRAC_PI_2))
            .build(&context);

        let axis = MeshBuilder::default()
            .primitive(x_axis.head, red.clone())
            .primitive(x_axis.body, red.clone())
            .primitive(y_axis.head, green.clone())
            .primitive(y_axis.body, green)
            .primitive(z_axis.head, blue.clone())
            .primitive(z_axis.body, blue)
            .build(&context)
            .unwrap();

        let axis_entity = entity_manager.new_entity();
        scene_graph.add(axis_entity, None);
        let axis = axis.new_instance(axis_entity);

        let point = SphereBuilder::default().radius(0.1).build(&context);
        let point = MeshBuilder::default()
            .primitive(point, red)
            .build(&context)
            .unwrap();

        let aab = AxisAlignedBox::from_center_dimension(Vec3::ZERO, 2.0, 2.0, 2.0);

        let mut ball = Ball {
            center: Vec3::ZERO,
            radius: 1.0,
        };
        ball.center = vec3(1.57, 1.57, 1.57);
        let tetrahedron = gjk_tetrahedron(&aab, &ball).unwrap();
        let polyhedron = epa_dbg(&aab, &ball, tetrahedron, 0);

        let points = polyhedron
            .vertices
            .iter()
            .map(|v| {
                let entity = entity_manager.new_entity();
                scene_graph.add_with_transform(
                    entity,
                    None,
                    v.difference,
                    Quat::IDENTITY,
                    Vec3::ONE,
                );
                point.new_instance(entity)
            })
            .collect();

        context.queue().submit([encoder.finish()]);

        Scene {
            context,
            scene_graph,
            points,
            axis,
            skin_manager,
            light_manager,
            environment,
            renderer,
            controls,
            last_frame: Instant::now(),
        }
    }

    fn render(&mut self, texture_view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        self.renderer
            .render(
                &self.controls.camera,
                &texture_view,
                &mut self.scene_graph,
                &mut self.skin_manager,
                self.points.iter().chain(once(&self.axis)),
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
            let response = ui.interact(ui.clip_rect(), egui::Id::new("scene"), egui::Sense::drag());
            self.scene
                .controls
                .handle_response(response, ui, &mut self.scene.scene_graph);
            // egui::Panel::right("right panel").show_inside(ui, |ui| {
            //     ui.label("Hello world!");
            //     if ui.button("Click me").clicked() {
            //         println!("Clicked");
            //     }
            // });
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
    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
