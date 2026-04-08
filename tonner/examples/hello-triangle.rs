use std::sync::Arc;

use glam::{vec3, vec4};
use tonner::Context;
use tonner::entity_component::EntityManager;
use tonner::environment::{Environment, EnvironmentBuilder};
use tonner::geometry::GeometryBuilder;
use tonner::geometry::skin::SkinManager;
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

struct Scene {
    context: Context,
    scene_graph: SceneGraph,
    triangle: MeshInstance,
    camera: Camera,
    skin_manager: SkinManager,
    light_manager: LightManager,
    environment: Environment,
    renderer: Renderer,
}

impl Scene {
    fn render(&mut self, texture_view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        self.renderer
            .render(
                &self.camera,
                &texture_view,
                &mut self.scene_graph,
                &mut self.skin_manager,
                [&self.triangle],
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

        let context = Context::from_device(device, queue);
        let mut entity_manager = EntityManager::new();

        let mut encoder =
            context
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("App::resumed command encoder"),
                });

        let mut scene_graph = SceneGraph::new(&context);
        let renderer = Renderer::new(
            size.width,
            size.height,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            &context,
        );

        let triangle = GeometryBuilder::new(3, 0)
            .name("Triangle")
            .positions([
                vec3(0.0, -0.5, -5.0),
                vec3(0.5, 0.5, -5.0),
                vec3(-0.5, 0.5, -5.0),
            ])
            .unwrap()
            .build(&context)
            .unwrap();

        let red = MaterialBuilder::default()
            .name("red")
            .base_color_factor(vec4(1.0, 0.0, 0.0, 1.0))
            .build(&context);

        let red_triangle = MeshBuilder::default()
            .name("Triangle")
            .primitive(triangle, red)
            .build(&context)
            .unwrap();

        let triangle = entity_manager.new_entity();
        scene_graph.add(triangle, None);
        let triangle = red_triangle.new_instance(triangle);

        let camera = entity_manager.new_entity();
        scene_graph.add(camera, None);
        let camera = Camera::new(camera);

        let skin_manager = SkinManager::new(&context);
        let light_manager = LightManager::new(&context);
        let environment = EnvironmentBuilder::default().build(&context, &mut encoder);

        context.queue().submit([encoder.finish()]);

        let scene = Scene {
            context,
            scene_graph,
            triangle,
            camera,
            skin_manager,
            light_manager,
            environment,
            renderer,
        };

        let state = State {
            scene,
            window,
            size,
            instance,
            surface,
            surface_format,
        };

        state.configure_surface();

        state
    }

    fn configure_surface(&self) {
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            view_formats: vec![self.surface_format.add_srgb_suffix()],
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

    fn render(&mut self) {
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
        let texture_view = surface_texture
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

        self.scene.render(&texture_view, &mut encoder);

        self.scene.context.queue().submit([encoder.finish()]);
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
