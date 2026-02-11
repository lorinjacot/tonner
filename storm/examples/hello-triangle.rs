use std::sync::Arc;
use std::time::Instant;

use glam::{vec3, vec4};
use pollster::block_on;
use storm::Context;
use storm::camera::Camera;
use storm::environment::{Environment, EnvironmentBuilder};
use storm::geometry::GeometryBuilder;
use storm::light::LightManager;
use storm::mesh::material::MaterialBuilder;
use storm::mesh::{MeshBuilder, MeshInstance};
use storm::renderer::Renderer;
use storm::scene_graph::{NodeBuilder, SceneGraph};
use storm::skin::SkinManager;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

#[derive(Default)]
struct App {
    scene: Option<Scene>,
    surface: Option<wgpu::Surface<'static>>,
    last_redraw: Option<Instant>,
    window: Option<Arc<Window>>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptionsBase {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .unwrap();
        let (device, queue) =
            block_on(adapter.request_device(&wgpu::wgt::DeviceDescriptor::default())).unwrap();
        let size = window.inner_size();
        surface.configure(
            &device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                width: size.width,
                height: size.height,
                present_mode: wgpu::PresentMode::AutoVsync,
                desired_maximum_frame_latency: 2,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
            },
        );

        let context = Context::from_device(device, queue);

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
                vec3(0.5, 0.5, -5.0),
                vec3(0.0, -0.5, -5.0),
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

        let triangle_node = NodeBuilder::default()
            .name("Triangle node")
            .build(&mut scene_graph)
            .unwrap();
        let instance = red_triangle.new_instance(triangle_node);

        let camera_node = NodeBuilder::default()
            .name("Camera node")
            .build(&mut scene_graph)
            .unwrap();
        let camera = Camera::new(camera_node);

        let skin_manager = SkinManager::new(&context);
        let light_manager = LightManager::new(&context);
        let environment = EnvironmentBuilder::default().build(&context, &mut encoder);

        context.queue().submit([encoder.finish()]);

        let scene = Scene {
            context,
            scene_graph,
            triangle: instance,
            camera,
            skin_manager,
            light_manager,
            environment,
            renderer,
        };

        self.scene = Some(scene);
        self.window = Some(window);
        self.surface = Some(surface);
        self.last_redraw = Some(Instant::now());
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.

                // Draw.
                let surface = self.surface.as_ref().unwrap();
                let surface_texture = surface.get_current_texture().unwrap();
                let surface_view = surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let now = Instant::now();
                let duration = now.duration_since(self.last_redraw.replace(now).unwrap());

                let scene = self.scene.as_mut().unwrap();
                let mut encoder = scene
                    .context
                    .device()
                    .create_command_encoder(&Default::default());

                scene
                    .renderer
                    .render(
                        &scene.camera,
                        &surface_view,
                        &mut scene.scene_graph,
                        &scene.skin_manager,
                        [&scene.triangle],
                        &scene.light_manager,
                        &scene.environment,
                        &scene.context,
                        &mut encoder,
                    )
                    .unwrap();
                scene.context.queue().submit([encoder.finish()]);
                surface_texture.present();

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw in
                // applications which do not always need to. Applications that redraw continuously
                // can render here instead.
                self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
    }
}

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

fn main() {
    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
