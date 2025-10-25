use thiserror::Error;
use wasm_bindgen::prelude::*;

#[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
use web_sys::{HtmlCanvasElement, OffscreenCanvas};

mod scene;

#[wasm_bindgen(start)]
fn start() {
    use log::Level;

    console_error_panic_hook::set_once();
    console_log::init_with_level(Level::Debug).expect("error initializing logger");
}

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, storm-js!");
}

/// This is the entry point of the package. To get started, create a new Engine using {@link EngineBuilder}.
/// Once created, an engine can be used to create a {@link Scene}. The engine is also responsible to manage
/// the resources shared between scenes.
#[wasm_bindgen]
pub struct Engine {
    inner: storm::Engine,
    #[allow(dead_code)]
    instance: wgpu::Instance,
}

#[wasm_bindgen]
impl Engine {
    /// Create an engine builder with default values.
    pub fn builder() -> EngineBuilder {
        EngineBuilder(storm::Engine::builder())
    }

    /// Create a surface. A surface is needed to render a {@link Scene}.
    #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
    #[wasm_bindgen(js_name = createSurfaceFromCanvasElement)]
    pub fn create_surface_from_canvas_element(
        &self,
        canvas: HtmlCanvasElement,
    ) -> Result<Surface, CreateSurfaceError> {
        Ok(Surface(
            self.instance
                .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
                .or(Err(CreateSurfaceError))?,
        ))
    }

    /// Create a surface. A surface is needed to render a {@link Scene}.
    #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
    #[wasm_bindgen(js_name = createSurfaceFromOffscreenCanvas)]
    pub fn create_surface_from_offscreen_canvas(
        &self,
        canvas: OffscreenCanvas,
    ) -> Result<Surface, CreateSurfaceError> {
        Ok(Surface(
            self.instance
                .create_surface(wgpu::SurfaceTarget::OffscreenCanvas(canvas))
                .or(Err(CreateSurfaceError))?,
        ))
    }
}

/// A builder for {@link Engine}.
#[wasm_bindgen]
pub struct EngineBuilder(storm::EngineBuilder<'static>);

#[wasm_bindgen]
impl EngineBuilder {
    /// Build the engine.
    pub async fn build(self) -> Result<Engine, BuildEngineError> {
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
            .or(Err(BuildEngineError::Adapter))?;

        let (device, queue) = adapter
            .request_device(&wgpu::wgt::DeviceDescriptor {
                label: Some("storm-js engine device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::defaults(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .or(Err(BuildEngineError::Device))?;

        let inner = self.0.device(device, queue).build().await;

        Ok(Engine { inner, instance })
    }
}

/// Error when {@link EngineBuilder.build()} fails.
#[wasm_bindgen]
#[derive(Debug, Error)]
pub enum BuildEngineError {
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
pub struct Surface(wgpu::Surface<'static>);

/// Error when {@link} Engine.createSurfaceFromCanvasElement() or Engine.createSurfaceFromOffscreenCanvas() fail.
#[wasm_bindgen]
#[derive(Error, Debug)]
#[error("failed to create surface")]
pub struct CreateSurfaceError;
