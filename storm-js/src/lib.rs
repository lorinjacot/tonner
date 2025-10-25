use wasm_bindgen::prelude::*;

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

/// This is the entry point of the package. To get started, create a new Engine using `EngineBuilder`.
/// Once created, an engine can be used to create a `Scene`. The engine is also responsible to manage
/// the resources shared between scenes.
#[wasm_bindgen]
pub struct Engine(storm::Engine);

#[wasm_bindgen]
impl Engine {
    /// Create an engine builder with default values.
    pub fn builder() -> EngineBuilder {
        EngineBuilder(storm::Engine::builder())
    }
}

/// A builder for `Engine`.
#[wasm_bindgen]
pub struct EngineBuilder(storm::EngineBuilder);

#[wasm_bindgen]
impl EngineBuilder {
    /// Build the engine.
    pub async fn build(self) -> Engine {
        Engine(self.0.build().await)
    }
}