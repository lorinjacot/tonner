use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use eframe::egui_wgpu;
use storm::{Context, camera::Camera};
use storm_controls::{EguiControls, orbit::OrbitControls};

use crate::Scene;

pub struct SceneView {
    scene: Arc<RwLock<Scene>>,
    controls: OrbitControls,
    texture_view: TextureView,
    sized_texture: egui::load::SizedTexture,
    egui_renderer: Arc<egui::mutex::RwLock<egui_wgpu::Renderer>>,
    storm_renderer: storm::renderer::Renderer,
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
            &texture_view.srgb,
            wgpu::FilterMode::Linear,
        );
        let sized_texture = egui::load::SizedTexture::new(id, [width as f32, height as f32]);

        let storm_renderer =
            storm::renderer::Renderer::new(width, height, wgpu::TextureFormat::Rgba8UnormSrgb, ctx);

        let controls = OrbitControls::new(camera);

        Self {
            scene,
            controls,
            texture_view,
            sized_texture,
            egui_renderer: renderer,
            storm_renderer,
        }
    }

    pub fn update(&mut self, delta_time: Duration) {
        self.controls.update(
            &mut self.scene.write().unwrap().storm_scene.scene_graph,
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

        let texture = self.texture_view.srgb.texture();
        if texture.width() != width || texture.height() != height {
            let mut renderer = self.egui_renderer.write();
            renderer.free_texture(&self.sized_texture.id);

            self.texture_view = Self::create_texture_view(width, height, ctx.device());
            let id = renderer.register_native_texture(
                ctx.device(),
                &self.texture_view.rgb,
                wgpu::FilterMode::Linear,
            );
            self.sized_texture = egui::load::SizedTexture::new(id, [width as f32, height as f32]);
        }

        let scene = self.scene.read().unwrap();
        let storm_scene = &scene.storm_scene;
        self.storm_renderer
            .render(
                &self.controls.camera,
                &self.texture_view.srgb,
                &storm_scene.scene_graph,
                &storm_scene.skin_manager(),
                &storm_scene.mesh_manager(),
                &storm_scene.light_manager(),
                &storm_scene.environment(),
                ctx,
                encoder,
            )
            .unwrap();
        drop(scene);

        let response = ui.image(self.sized_texture).interact(egui::Sense::drag());
        self.controls.handle_response(
            response,
            ui,
            &mut self.scene.write().unwrap().storm_scene.scene_graph,
        );
    }

    fn create_texture_view(width: u32, height: u32, device: &wgpu::Device) -> TextureView {
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
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
        });
        let rgb = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("SceneView texture view"),
            format: Some(wgpu::TextureFormat::Rgba8Unorm),
            ..Default::default()
        });
        let srgb = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("SceneView texture view"),
            format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            ..Default::default()
        });
        TextureView { rgb, srgb }
    }
}

struct TextureView {
    rgb: wgpu::TextureView,
    srgb: wgpu::TextureView,
}

impl Drop for SceneView {
    fn drop(&mut self) {
        self.egui_renderer
            .write()
            .free_texture(&self.sized_texture.id);
    }
}
