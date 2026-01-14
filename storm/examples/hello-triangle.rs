use std::sync::Arc;
use std::time::Instant;

use glam::{vec3, vec4};
use pollster::block_on;
use storm::camera::{Camera, CameraBuilder};
use storm::geometry::GeometryBuilder;
use storm::material::MaterialBuilder;
use storm::mesh::MeshBuilder;
use storm::mesh_instance::MeshInstanceBuilder;
use storm::render_target::RenderTargetBuilder;
use storm::{Context, Scene, SceneBuilder};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

#[derive(Default)]
struct App {
    scene: Option<Scene>,
    surface: Option<wgpu::Surface<'static>>,
    render_target_builder: Option<RenderTargetBuilder>,
    camera: Option<Camera>,
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

        let ctx = Context::from_device(device, queue);

        let triangle = GeometryBuilder::new(3, 0)
            .name("Triangle")
            .positions([
                vec3(0.5, 0.5, -5.0),
                vec3(0.0, -0.5, -5.0),
                vec3(-0.5, 0.5, -5.0),
            ])
            .unwrap()
            .build(&ctx)
            .unwrap();

        let red = MaterialBuilder::default()
            .name("red")
            .base_color_factor(vec4(1.0, 0.0, 0.0, 1.0))
            .build(&ctx);

        let red_triangle = MeshBuilder::default()
            .name("Triangle")
            .primitive(triangle, red)
            .build(&ctx)
            .unwrap();

        let mut scene = SceneBuilder::default().build(&ctx);

        MeshInstanceBuilder::new(red_triangle)
            .name("first triangle")
            .build(&mut scene)
            .unwrap();

        let render_target_builder = RenderTargetBuilder::new(
            size.width,
            size.height,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            &ctx,
        );

        let camera = CameraBuilder::default().build(&mut scene);

        self.window = Some(window);
        self.scene = Some(scene);
        self.surface = Some(surface);
        self.render_target_builder = Some(render_target_builder);
        self.camera = Some(camera);
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
                let window = self.window.as_ref().unwrap();

                let now = Instant::now();
                let duration = now.duration_since(self.last_redraw.replace(now).unwrap());

                let scene = self.scene.as_mut().unwrap();
                let mut encoder = scene
                    .context()
                    .device()
                    .create_command_encoder(&Default::default());

                let render_target_builder = self.render_target_builder.clone().unwrap();
                let render_target = match render_target_builder.build(&surface_view) {
                    Ok(render_target) => render_target,
                    Err(_) => {
                        let size = window.inner_size();
                        let render_target_builder = RenderTargetBuilder::new(
                            size.width,
                            size.height,
                            wgpu::TextureFormat::Rgba8UnormSrgb,
                            scene.context(),
                        );
                        self.render_target_builder = Some(render_target_builder.clone());
                        render_target_builder.build(&surface_view).unwrap()
                    }
                };
                scene.simulate(duration, &mut encoder).unwrap();
                scene
                    .render(&render_target, self.camera.as_ref().unwrap(), &mut encoder)
                    .unwrap();
                scene.context().queue().submit([encoder.finish()]);
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

fn main() {
    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
