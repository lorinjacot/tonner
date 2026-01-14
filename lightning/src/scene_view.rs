use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use eframe::egui_wgpu;
use storm::{
    Context, Scene,
    camera::Camera,
    render_target::{RenderTarget, RenderTargetBuilder},
};
use storm_controls::{EguiControls, orbit::OrbitControls};

pub struct SceneView {
    scene: Arc<RwLock<Scene>>,
    controls: OrbitControls,
    sized_texture: egui::load::SizedTexture,
    render_target: RenderTarget<wgpu::TextureView>,
    renderer: Arc<egui::mutex::RwLock<egui_wgpu::Renderer>>,
}

impl SceneView {
    pub fn new(
        scene: Arc<RwLock<Scene>>,
        camera: Camera,
        width: u32,
        height: u32,
        renderer: Arc<egui::mutex::RwLock<egui_wgpu::Renderer>>,
        ctx: &Context,
    ) -> Self {
        let texture_view = Self::create_texture_view(width, height, ctx.device());

        let id = renderer.write().register_native_texture(
            ctx.device(),
            &texture_view,
            wgpu::FilterMode::Linear,
        );
        let sized_texture = egui::load::SizedTexture::new(id, [width as f32, height as f32]);

        let render_target =
            RenderTargetBuilder::new(width, height, wgpu::TextureFormat::Rgba8UnormSrgb, ctx)
                .build(texture_view)
                .unwrap();

        let controls = OrbitControls::new(camera);

        Self {
            scene,
            controls,
            sized_texture,
            render_target,
            renderer,
        }
    }

    pub fn update(&mut self, delta_time: Duration) {
        self.controls.update(
            &mut self.scene.write().unwrap(),
            delta_time,
            self.sized_texture.size.x / self.sized_texture.size.y,
        );
    }

    pub fn render(&mut self, ui: &mut egui::Ui, ctx: &Context, encoder: &mut wgpu::CommandEncoder) {
        let size = ui.available_size();
        let width = size.x as u32;
        let height = size.y as u32;

        if width == 0 || height == 0 {
            return;
        }

        if self.render_target.width() != width || self.render_target.height() != height {
            let mut renderer = self.renderer.write();
            renderer.free_texture(&self.sized_texture.id);

            let texture_view = Self::create_texture_view(width, height, ctx.device());
            let id = renderer.register_native_texture(
                ctx.device(),
                &texture_view,
                wgpu::FilterMode::Linear,
            );
            self.sized_texture = egui::load::SizedTexture::new(id, [width as f32, height as f32]);

            self.render_target =
                RenderTargetBuilder::new(width, height, wgpu::TextureFormat::Rgba8UnormSrgb, ctx)
                    .build(texture_view)
                    .unwrap();
        }

        self.scene
            .read()
            .unwrap()
            .render(&self.render_target, &self.controls.camera, encoder)
            .unwrap();

        let response = ui.image(self.sized_texture).interact(egui::Sense::drag());
        self.controls.handle_response(
            response,
            ui,
            size.x,
            size.y,
            &mut self.scene.write().unwrap(),
        );
    }

    fn create_texture_view(width: u32, height: u32, device: &wgpu::Device) -> wgpu::TextureView {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SceneView texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("SceneView texture view"),
            ..Default::default()
        })
    }
}

impl Drop for SceneView {
    fn drop(&mut self) {
        self.renderer.write().free_texture(&self.sized_texture.id);
    }
}
