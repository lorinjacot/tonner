use dioxus::prelude::*;
use dioxus::logger::tracing;
use lightning::EngineProvider;

use lightning::components::menubar::*;

#[cfg(feature = "web")]
mod web_renderer;

#[cfg(feature = "desktop")]
mod native_renderer;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const DX_COMPONENT_THEME_CSS: Asset = asset!("/assets/dx-components-theme.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: DX_COMPONENT_THEME_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }

        EngineProvider {
            Demo { }
        }
    }
}

#[component]
pub fn Demo() -> Element {
    rsx! {
        div { class: "menubar-example",
            Menubar {
                MenubarMenu { index: 0usize,
                    MenubarTrigger { "File" }
                    MenubarContent {
                        MenubarItem {
                            index: 0usize,
                            value: "new".to_string(),
                            on_select: move |value| {
                                tracing::info!("Selected value: {}", value);
                            },
                            "New"
                        }
                        MenubarItem {
                            index: 1usize,
                            value: "open".to_string(),
                            on_select: move |value| {
                                tracing::info!("Selected value: {}", value);
                            },
                            "Open"
                        }
                        MenubarItem {
                            index: 2usize,
                            value: "save".to_string(),
                            on_select: move |value| {
                                tracing::info!("Selected value: {}", value);
                            },
                            "Save"
                        }
                    }
                }
                MenubarMenu { index: 1usize,
                    MenubarTrigger { "Edit" }
                    MenubarContent {
                        MenubarItem {
                            index: 0usize,
                            value: "cut".to_string(),
                            on_select: move |value| {
                                tracing::info!("Selected value: {}", value);
                            },
                            "Cut"
                        }
                        MenubarItem {
                            index: 1usize,
                            value: "copy".to_string(),
                            on_select: move |value| {
                                tracing::info!("Selected value: {}", value);
                            },
                            "Copy"
                        }
                        MenubarItem {
                            index: 2usize,
                            value: "paste".to_string(),
                            on_select: move |value| {
                                tracing::info!("Selected value: {}", value);
                            },
                            "Paste"
                        }
                    }
                }
            }
        }
    }
}

#[cfg(feature = "web")]
#[component]
fn SpinningTriangle() -> Element {
    use crate::web_renderer::State;
    use uuid::Uuid;
    use web_sys::wasm_bindgen::JsCast;

    let id = Uuid::new_v4().to_string();
    let canvas_id = id.clone();
    use_future(move || {
        let canvas_id = canvas_id.clone();
        async move {
            let canvas = web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .get_element_by_id(&canvas_id)
                .unwrap()
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .unwrap();

            let mut state = State::new(canvas).await;
            state.render();
        }
    });

    rsx!(
        div { id:"canvas-container",
            canvas {
                id
            }
        }
    )
}

#[cfg(feature = "desktop")]
#[component]
fn SpinningTriangle() -> Element {
    use crate::native_renderer::DemoPaintSource;
    use dioxus_native::use_wgpu;

    // Create custom paint source and register it with the renderer
    let paint_source = DemoPaintSource::new();
    let paint_source_id = use_wgpu(move || paint_source);

    rsx!(
        div { id:"canvas-container",
            canvas {
                id: "demo-canvas",
                "src": paint_source_id
            }
        }
    )
}
