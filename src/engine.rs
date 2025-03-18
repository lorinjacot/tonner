use glam::Mat4;
use std::sync::Arc;
use std::time::Instant;
use winit::event::{DeviceEvent, WindowEvent};
use winit::window::Window;

use crate::storm::{Controls, MeshManager, NodeId, OrbitControls, PerspectiveCamera, Scene, Storm};

pub struct DisplaySettings {
    pub exposure: f32,
    pub background_blur: bool,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            exposure: 1.0,
            background_blur: true,
        }
    }
}

pub struct Engine {
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    last_frame: Instant,
    scene: Scene,
    camera: NodeId,
    controls: OrbitControls,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
}

impl Engine {
    pub async fn new(window: Window) -> Self {
        let window = Arc::new(window);

        let mut size = window.inner_size();
        size.width = size.width.max(1);
        size.height = size.height.max(1);

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptionsBase {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults()
                        .using_resolution(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];
        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();
        surface.configure(&device, &config);

        let mut storm = Storm::new(&device);
        let mut meshes = MeshManager::new(&device);
        let mut scene = Scene::new();

        let camera = scene.create_node(None, Mat4::IDENTITY);
        scene.add_camera(
            PerspectiveCamera::new(
                f32::to_radians(45.0),
                size.width as f32 / size.height as f32,
                0.1,
                100.0,
            ),
            camera,
        );
        let controls = OrbitControls::new();

        let egui_ctx = egui::Context::default();
        let viewport_id = egui_ctx.viewport_id();
        let egui_state = egui_winit::State::new(egui_ctx, viewport_id, &window, None, None, None);
        let egui_renderer = egui_wgpu::Renderer::new(&device, swapchain_format, None, 1, false);

        // let display_settings = DisplaySettings::default();

        // let size = wgpu::Extent3d {
        //     width: config.width,
        //     height: config.height,
        //     depth_or_array_layers: 1,
        // };

        // let hdr_texture = device.create_texture(&wgpu::TextureDescriptor {
        //     label: Some("HDR texture"),
        //     size,
        //     mip_level_count: 1,
        //     sample_count: 1,
        //     dimension: wgpu::TextureDimension::D2,
        //     format: wgpu::TextureFormat::Rgba16Float,
        //     usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        //     view_formats: &[],
        // });
        // let hdr_texture_view = hdr_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        //     label: Some("Depth texture"),
        //     size,
        //     mip_level_count: 1,
        //     sample_count: 1,
        //     dimension: wgpu::TextureDimension::D2,
        //     format: wgpu::TextureFormat::Depth24Plus,
        //     usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        //     view_formats: &[],
        // });
        // let depth_texture_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // let hdr_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        //     label: Some("HDR sampler"),
        //     address_mode_u: wgpu::AddressMode::ClampToEdge,
        //     address_mode_v: wgpu::AddressMode::ClampToEdge,
        //     address_mode_w: wgpu::AddressMode::ClampToEdge,
        //     mag_filter: wgpu::FilterMode::Linear,
        //     min_filter: wgpu::FilterMode::Nearest,
        //     mipmap_filter: wgpu::FilterMode::Linear,
        //     ..Default::default()
        // });

        // let exposure_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        //     label: Some("HDR exposure buffer"),
        //     contents: bytemuck::cast_slice(&[display_settings.exposure]),
        //     usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        // });

        // let hdr_bind_group_layout =
        //     device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        //         label: Some("HDR bind group layout"),
        //         entries: &[
        //             wgpu::BindGroupLayoutEntry {
        //                 binding: 0,
        //                 visibility: wgpu::ShaderStages::FRAGMENT,
        //                 ty: wgpu::BindingType::Texture {
        //                     sample_type: wgpu::TextureSampleType::Float { filterable: true },
        //                     view_dimension: wgpu::TextureViewDimension::D2,
        //                     multisampled: false,
        //                 },
        //                 count: None,
        //             },
        //             wgpu::BindGroupLayoutEntry {
        //                 binding: 1,
        //                 visibility: wgpu::ShaderStages::FRAGMENT,
        //                 ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        //                 count: None,
        //             },
        //             wgpu::BindGroupLayoutEntry {
        //                 binding: 2,
        //                 visibility: wgpu::ShaderStages::FRAGMENT,
        //                 ty: wgpu::BindingType::Buffer {
        //                     ty: wgpu::BufferBindingType::Uniform,
        //                     has_dynamic_offset: false,
        //                     min_binding_size: None,
        //                 },
        //                 count: None,
        //             },
        //         ],
        //     });

        // let hdr_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        //     label: Some("HDR bind group"),
        //     layout: &hdr_bind_group_layout,
        //     entries: &[
        //         wgpu::BindGroupEntry {
        //             binding: 0,
        //             resource: wgpu::BindingResource::TextureView(&hdr_texture_view),
        //         },
        //         wgpu::BindGroupEntry {
        //             binding: 1,
        //             resource: wgpu::BindingResource::Sampler(&hdr_sampler),
        //         },
        //         wgpu::BindGroupEntry {
        //             binding: 2,
        //             resource: exposure_buffer.as_entire_binding(),
        //         },
        //     ],
        // });

        // let hdr_module = device.create_shader_module(wgpu::include_wgsl!("hdr.wgsl"));

        // let hdr_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        //     label: Some("HDR render pipeline layout"),
        //     bind_group_layouts: &[&hdr_bind_group_layout],
        //     push_constant_ranges: &[],
        // });

        // let hdr_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        //     label: Some("HDR render pipeline"),
        //     layout: Some(&hdr_pipeline_layout),
        //     vertex: wgpu::VertexState {
        //         module: &hdr_module,
        //         entry_point: Some("vs_main"),
        //         compilation_options: wgpu::PipelineCompilationOptions::default(),
        //         buffers: &[],
        //     },
        //     primitive: wgpu::PrimitiveState {
        //         topology: wgpu::PrimitiveTopology::TriangleList,
        //         strip_index_format: None,
        //         front_face: wgpu::FrontFace::Ccw,
        //         cull_mode: None,
        //         unclipped_depth: false,
        //         polygon_mode: wgpu::PolygonMode::Fill,
        //         conservative: false,
        //     },
        //     depth_stencil: None,
        //     multisample: wgpu::MultisampleState {
        //         count: 1,
        //         mask: !0,
        //         alpha_to_coverage_enabled: false,
        //     },
        //     fragment: Some(wgpu::FragmentState {
        //         module: &hdr_module,
        //         entry_point: Some("fs_main"),
        //         compilation_options: wgpu::PipelineCompilationOptions::default(),
        //         targets: &[Some(swapchain_format.add_srgb_suffix().into())],
        //     }),
        //     multiview: None,
        //     cache: None,
        // });

        // let camera = Camera::new(
        //     vec3(0.0, 0.0, 10.0),
        //     config.width as f32 / config.height as f32,
        //     &device,
        //     &queue,
        // );
        // let camera_controller = CameraController::new();

        // let last_frame = Instant::now();

        // let (mut asset, document) = Asset::open("assets/EnvironmentTest.gltf").unwrap();

        // let scene_id = 0;
        // let mut scene = Scene::new(camera, &device, &queue);

        // let gltf_scene = document.default_scene().unwrap();
        // asset
        //     .create_scene(gltf_scene, scene_id, &mut scene, &device, &queue)
        //     .unwrap();

        device.stop_capture();
        let last_frame = Instant::now();

        Self {
            window,
            device,
            queue,
            surface,
            config,
            last_frame,
            scene,
            camera,
            controls,
            egui_state,
            egui_renderer,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn window_event(&mut self, event: &WindowEvent) -> bool {
        if self
            .egui_state
            .on_window_event(&self.window, event)
            .consumed
        {
            return true;
        }
        match event {
            WindowEvent::RedrawRequested => {
                self.draw();
                self.window.request_redraw();
                profiling::finish_frame!();
                true
            }
            WindowEvent::Resized(new_size) => {
                self.resize(new_size);
                true
            }
            WindowEvent::KeyboardInput { event, .. } => self.controls.keyboard_input(event),
            WindowEvent::MouseInput { state, button, .. } => {
                self.controls.mouse_input(state, button)
            }
            _ => false,
        }
    }

    pub fn device_event(&mut self, event: &DeviceEvent) -> bool {
        match event {
            DeviceEvent::MouseMotion { delta } => self.controls.mouse_motion(delta),
            _ => false,
        }
    }

    fn resize(&mut self, new_size: &winit::dpi::PhysicalSize<u32>) {
        self.config.width = new_size.width.max(1);
        self.config.height = new_size.height.max(1);
        self.surface.configure(&self.device, &self.config);

        // let size = wgpu::Extent3d {
        //     width: self.config.width,
        //     height: self.config.height,
        //     depth_or_array_layers: 1,
        // };

        // let hdr_texture = self.device.create_texture(&wgpu::TextureDescriptor {
        //     label: Some("frame buffer texture"),
        //     size,
        //     mip_level_count: 1,
        //     sample_count: 1,
        //     dimension: wgpu::TextureDimension::D2,
        //     format: wgpu::TextureFormat::Rgba16Float,
        //     usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        //     view_formats: &[],
        // });
        // self.hdr_texture_view = hdr_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // self.hdr_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
        //     label: Some("HDR bind group"),
        //     layout: &self.hdr_bind_group_layout,
        //     entries: &[
        //         wgpu::BindGroupEntry {
        //             binding: 0,
        //             resource: wgpu::BindingResource::TextureView(&self.hdr_texture_view),
        //         },
        //         wgpu::BindGroupEntry {
        //             binding: 1,
        //             resource: wgpu::BindingResource::Sampler(&self.hdr_sampler),
        //         },
        //         wgpu::BindGroupEntry {
        //             binding: 2,
        //             resource: self.exposure_buffer.as_entire_binding(),
        //         },
        //     ],
        // });

        // let depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
        //     label: Some("Depth texture"),
        //     size,
        //     mip_level_count: 1,
        //     sample_count: 1,
        //     dimension: wgpu::TextureDimension::D2,
        //     format: wgpu::TextureFormat::Depth24Plus,
        //     usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        //     view_formats: &[],
        // });
        // self.depth_texture_view =
        //     depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // self.scene
        //     .camera
        //     .set_aspect_ration(self.config.width as f32 / self.config.height as f32);

        self.window.request_redraw();
    }

    fn draw(&mut self) {
        let delta_time = self.last_frame.elapsed();
        self.last_frame = Instant::now();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        // handle inputs
        let new_input = self.egui_state.take_egui_input(&self.window);

        let camera = self.scene.camera_mut(self.camera).unwrap();
        self.controls.update(delta_time, camera.0, camera.1);

        // updates engine components
        self.scene.update();

        let full_output = self.egui_state.egui_ctx().run(new_input, |ctx| {
            // egui::SidePanel::left("display_panel").show(ctx, |ui| {
            //     ui.heading(egui::RichText::new("Display").size(32.0));
            //     ui.heading("Lighting");
            //     ui.label("Exposure");
            //     if ui
            //         .add(
            //             egui::Slider::new(&mut self.display_settings.exposure, 0.0..=64.0)
            //                 .logarithmic(true),
            //         )
            //         .changed()
            //     {
            //         self.queue.write_buffer(
            //             &self.exposure_buffer,
            //             0,
            //             bytemuck::cast_slice(&[self.display_settings.exposure]),
            //         );
            //     };

            //     ui.heading("Background");
            //     ui.checkbox(&mut self.display_settings.background_blur, "Blur");
            // });
        });
        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);

        let clipped_primitives = self
            .egui_state
            .egui_ctx()
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: full_output.pixels_per_point,
        };

        for (id, image_delta) in full_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, id, &image_delta);
        }
        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &clipped_primitives,
            &screen_descriptor,
        );

        // render

        // {
        //     let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        //         label: None,
        //         color_attachments: &[Some(wgpu::RenderPassColorAttachment {
        //             view: &self.hdr_texture_view,
        //             resolve_target: None,
        //             ops: wgpu::Operations {
        //                 load: wgpu::LoadOp::Clear(wgpu::Color {
        //                     r: -f64::ln(1.0 - 0.1) / self.display_settings.exposure as f64,
        //                     g: -f64::ln(1.0 - 0.1) / self.display_settings.exposure as f64,
        //                     b: -f64::ln(1.0 - 0.1) / self.display_settings.exposure as f64,
        //                     a: 1.0,
        //                 }),
        //                 store: wgpu::StoreOp::Store,
        //             },
        //         })],
        //         depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
        //             view: &self.depth_texture_view,
        //             depth_ops: Some(wgpu::Operations {
        //                 load: wgpu::LoadOp::Clear(1.0),
        //                 store: wgpu::StoreOp::Store,
        //             }),
        //             stencil_ops: None,
        //         }),
        //         timestamp_writes: None,
        //         occlusion_query_set: None,
        //     });

        //     render_pass.draw_scene(&self.scene, &self.display_settings);
        // }

        let frame = self
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");

        {
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.scene.render(self.camera, &mut render_pass);

            self.egui_renderer.render(
                &mut render_pass.forget_lifetime(),
                &clipped_primitives,
                &screen_descriptor,
            );
        }

        self.queue.submit([encoder.finish()]);
        frame.present();
    }
}
