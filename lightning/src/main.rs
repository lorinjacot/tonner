use dioxus::prelude::*;

#[cfg(feature = "web")]
mod web_renderer;

#[cfg(feature = "desktop")]
mod native_renderer;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut show_triangle = use_signal(|| true);

    use_effect(move || println!("{:?}", show_triangle));

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }

        div { id:"overlay",
            h2 { "Control Panel" },
            button {
                onclick: move |_| show_triangle.toggle(),
                if show_triangle() {
                    "Hide triangle"
                } else {
                    "Show triangle"
                }
            }
            br {}
            p { "This overlay demonstrates that the custom WGPU content can be rendered beneath layers of HTML content" }
        }
        div { id:"underlay",
            h2 { "Underlay" },
            p { "This underlay demonstrates that the custom WGPU content can be rendered above layers and blended with the content underneath" }
        }
        header {
            h1 { "Blitz WGPU Demo" }
        }
        if show_triangle() {
            SpinningTriangle {  }
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
