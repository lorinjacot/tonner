use std::sync::Arc;
use std::time::Instant;

use glam::vec3;
use wgpu::util::DeviceExt;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::window::Window;

use crate::asset::Asset;
use crate::camera::{Camera, CameraController};
use crate::scene::{DrawScene, Scene};

const EXPOSURE: f32 = 1.0;

pub struct Engine {
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    egui_ctx: egui::Context,
    egui_input: egui::RawInput,
    egui_renderer: egui_wgpu::Renderer,
    hdr_texture_view: wgpu::TextureView,
    depth_texture_view: wgpu::TextureView,
    exposure_buffer: wgpu::Buffer,
    hdr_bind_group: wgpu::BindGroup,
    hdr_bind_group_layout: wgpu::BindGroupLayout,
    hdr_sampler: wgpu::Sampler,
    hdr_pipeline: wgpu::RenderPipeline,
    last_frame: Instant,
    camera_controller: CameraController,
    scene: Scene,
}

impl Engine {
    pub async fn new(window: Window) -> Self {
        let window = Arc::new(window);

        let mut size = window.inner_size();
        size.width = size.width.max(1);
        size.height = size.height.max(1);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
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

        let egui_ctx = egui::Context::default();
        let egui_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_x_y_ranges(
                0.0..=config.width as f32,
                0.0..=config.height as f32,
            )),
            ..Default::default()
        };

        let egui_renderer = egui_wgpu::Renderer::new(&device, swapchain_format, None, 1, false);

        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };

        let hdr_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let hdr_texture_view = hdr_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_texture_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let hdr_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("HDR sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let exposure_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("HDR exposure buffer"),
            contents: bytemuck::cast_slice(&[EXPOSURE]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let hdr_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("HDR bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let hdr_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HDR bind group"),
            layout: &hdr_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&hdr_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&hdr_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: exposure_buffer.as_entire_binding(),
                },
            ],
        });

        let hdr_module = device.create_shader_module(wgpu::include_wgsl!("hdr.wgsl"));

        let hdr_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HDR render pipeline layout"),
            bind_group_layouts: &[&hdr_bind_group_layout],
            push_constant_ranges: &[],
        });

        let hdr_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HDR render pipeline"),
            layout: Some(&hdr_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &hdr_module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &hdr_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(swapchain_format.add_srgb_suffix().into())],
            }),
            multiview: None,
            cache: None,
        });

        let camera = Camera::new(
            vec3(0.0, 0.0, 10.0),
            config.width as f32 / config.height as f32,
            &device,
            &queue,
        );
        let camera_controller = CameraController::new();

        let last_frame = Instant::now();

        let (mut asset, document) = Asset::open("assets/CompareBaseColor.gltf").unwrap();

        let scene_id = 0;
        let mut scene = Scene::new(camera, &device, &queue);

        let gltf_scene = document.default_scene().unwrap();
        asset
            .create_scene(gltf_scene, scene_id, &mut scene, &device, &queue)
            .unwrap();

        Self {
            window,
            device,
            queue,
            surface,
            config,
            egui_ctx,
            egui_input,
            egui_renderer,
            hdr_texture_view,
            depth_texture_view,
            exposure_buffer,
            hdr_bind_group,
            hdr_bind_group_layout,
            hdr_sampler,
            hdr_pipeline,
            last_frame,
            camera_controller,
            scene,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn window_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::RedrawRequested => {
                self.window.request_redraw();
                self.update();
                self.draw();
                true
            }
            WindowEvent::Resized(new_size) => {
                self.config.width = new_size.width.max(1);
                self.config.height = new_size.height.max(1);
                self.surface.configure(&self.device, &self.config);

                self.egui_input.screen_rect = Some(egui::Rect::from_x_y_ranges(
                    0.0..=self.config.width as f32,
                    0.0..=self.config.height as f32,
                ));

                let size = wgpu::Extent3d {
                    width: self.config.width,
                    height: self.config.height,
                    depth_or_array_layers: 1,
                };

                let hdr_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("frame buffer texture"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                self.hdr_texture_view =
                    hdr_texture.create_view(&wgpu::TextureViewDescriptor::default());

                self.hdr_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("HDR bind group"),
                    layout: &self.hdr_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&self.hdr_texture_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.hdr_sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: self.exposure_buffer.as_entire_binding(),
                        },
                    ],
                });

                let depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Depth texture"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Depth24Plus,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                self.depth_texture_view =
                    depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

                self.scene
                    .camera
                    .set_aspect_ration(self.config.width as f32 / self.config.height as f32);

                self.window.request_redraw();
                true
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.camera_controller.keyboard_input(event)
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => self
                .camera_controller
                .mouse_input(*state == ElementState::Pressed),
            _ => false,
        }
    }

    pub fn device_event(&mut self, event: &DeviceEvent) -> bool {
        match event {
            DeviceEvent::MouseMotion { delta } => self
                .camera_controller
                .mouse_move(delta.0 as f32, delta.1 as f32),
            _ => false,
        }
    }

    fn update(&mut self) {
        let delta_time = self.last_frame.elapsed();
        self.last_frame = Instant::now();

        self.camera_controller
            .update(&mut self.scene.camera, delta_time, &self.queue);
    }

    fn draw(&mut self) {
        let frame = self
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let full_output = self.egui_ctx.run(self.egui_input.take(), |ctx| {
            egui::SidePanel::left("my_left_panel").show(ctx, |ui| {
                ui.label(format!("dbg: {:?}", self.egui_input.viewport().inner_rect));
            });
            egui::SidePanel::right("display_panel").show(ctx, |ui| {
                ui.label("Hello world!");
            });
        });
        // handle_platform_output(full_output.platform_output);

        let clipped_primitives = self
            .egui_ctx
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

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: -f64::ln(1.0 - 0.1) / EXPOSURE as f64,
                            g: -f64::ln(1.0 - 0.1) / EXPOSURE as f64,
                            b: -f64::ln(1.0 - 0.1) / EXPOSURE as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.draw_scene(&self.scene);
        }

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

            render_pass.set_pipeline(&self.hdr_pipeline);
            render_pass.set_bind_group(0, Some(&self.hdr_bind_group), &[]);
            render_pass.draw(0..3, 0..1);

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
