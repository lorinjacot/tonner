use thiserror::Error;
use wasm_bindgen::prelude::*;

#[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
use web_sys::{HtmlCanvasElement, OffscreenCanvas};

mod asset;
mod scene;

#[wasm_bindgen(start)]
fn start() {
    use log::Level;

    console_error_panic_hook::set_once();
    console_log::init_with_level(Level::Debug).expect("error initializing logger");
}

/// This is the entry point of the package. To get started, create a new Context.
/// Once created, context can be used to create a {@link Scene}. The context is also responsible to manage
/// the resources shared between scenes.
#[wasm_bindgen]
pub struct Context {
    inner: storm::Context,
    #[allow(dead_code)]
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
}

#[wasm_bindgen]
impl Context {
    pub async fn new() -> Result<Self, ContextCreationError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            flags: wgpu::InstanceFlags::from_build_config(),
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptionsBase {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .or(Err(ContextCreationError::Adapter))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("storm-js engine device"),
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits::defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .or(Err(ContextCreationError::Device))?;

        let inner = storm::Context::from_device(device, queue);

        Ok(Self {
            inner,
            instance,
            adapter,
        })
    }

    /// Create a render target. A render target is needed to render a {@link Scene}.
    #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
    #[wasm_bindgen(js_name = createSurfaceFromCanvasElement)]
    pub fn create_render_target_from_canvas_element(
        &mut self,
        canvas: HtmlCanvasElement,
    ) -> Result<RenderTarget, CreateSurfaceError> {
        let width = canvas.width();
        let height = canvas.height();
        let surface = self
            .instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .or(Err(CreateSurfaceError))?;
        Ok(self.create_render_target(width, height, surface))
    }

    /// Create a surface. A surface is needed to render a {@link Scene}.
    #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
    #[wasm_bindgen(js_name = createSurfaceFromOffscreenCanvas)]
    pub fn create_render_target_from_offscreen_canvas(
        &mut self,
        canvas: OffscreenCanvas,
    ) -> Result<RenderTarget, CreateSurfaceError> {
        let width = canvas.width();
        let height = canvas.height();
        let surface = self
            .instance
            .create_surface(wgpu::SurfaceTarget::OffscreenCanvas(canvas))
            .or(Err(CreateSurfaceError))?;
        Ok(self.create_render_target(width, height, surface))
    }

    #[allow(dead_code)]
    fn create_render_target(
        &mut self,
        width: u32,
        height: u32,
        surface: wgpu::Surface<'static>,
    ) -> RenderTarget {
        let config = surface
            .get_default_config(&self.adapter, width, height)
            .unwrap();
        surface.configure(self.inner.device(), &config);
        let builder = storm::render_target::RenderTargetBuilder::new(
            width,
            height,
            config.format,
            &mut self.inner,
        );
        RenderTarget { surface, builder }
    }
}

/// Error when {@link Context.new()} fails.
#[wasm_bindgen]
#[derive(Debug, Error)]
pub enum ContextCreationError {
    /// failed to get a gpu adapter
    #[error("failed to get a gpu adapter")]
    Adapter,
    /// failed to get a logical gpu device
    #[error("failed to get a logical gpu device")]
    Device,
}

/// A surface is can be used as a render target.
/// To create one, use {@link Engine.createSurfaceFromCanvasElement()} with a
/// {@link HTMLCanvasElement} or {@link Engine.createSurfaceFromOffscreenCanvas()} with {@link OffscreenCanvas}.
#[wasm_bindgen]
pub struct RenderTarget {
    surface: wgpu::Surface<'static>,
    builder: storm::render_target::RenderTargetBuilder,
}

/// Error when {@link} Engine.createSurfaceFromCanvasElement() or Engine.createSurfaceFromOffscreenCanvas() fail.
#[wasm_bindgen]
#[derive(Error, Debug)]
#[error("failed to create surface")]
pub struct CreateSurfaceError;
