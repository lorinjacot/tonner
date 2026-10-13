use crate::camera::Camera;

pub mod camera;
pub mod scene_graph;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

#[derive(Debug, Clone)]
pub struct Renderer {
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    ctx: Context,
}

impl Renderer {
    pub fn new(width: u32, height: u32, format: wgpu::TextureFormat, ctx: Context) -> Self {
        Renderer {
            width,
            height,
            format,
            ctx,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    pub fn render(&mut self, scene: &Scene, camera: &Camera, target: &wgpu::TextureView) {
        assert!(&self.ctx == scene.context());

        todo!()
    }

    pub fn context(&self) -> &Context {
        &self.ctx
    }
}

pub struct Scene {
    ctx: Context,
}

impl Scene {
    pub fn new(ctx: Context) -> Self {
        Scene { ctx }
    }

    pub fn render(&self, camera: &Camera, renderer: &mut Renderer, target: &wgpu::TextureView) {
        renderer.render(self, camera, target);
    }

    pub fn context(&self) -> &Context {
        &self.ctx
    }
}
