use cfg_if::cfg_if;
use dioxus::logger::tracing;
use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
use web_sys::{wasm_bindgen::JsCast, HtmlCanvasElement};

async fn create_scene() {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let document = web_sys::window().unwrap().document().unwrap();
            let canvas = document.get_element_by_id("scene-canvas").unwrap();
            let canvas: HtmlCanvasElement = canvas.dyn_into::<HtmlCanvasElement>().unwrap();

            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::default(),
                flags: wgpu::InstanceFlags::default(),
                memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
                backend_options: wgpu::BackendOptions::default(),
            });

            let surface_target = wgpu::SurfaceTarget::Canvas(canvas);

            tracing::info!("wasm32");
        } else {
            tracing::info!("not wasm32");
        }
    }
}

#[component]
pub fn Scene() -> Element {
    rsx! {
        canvas {
            id: "scene-canvas",
            class: "border-2 border-red-700 border-solid",
            onmounted: async |_| create_scene().await,
        }
    }
}
