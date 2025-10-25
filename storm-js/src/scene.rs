use wasm_bindgen::prelude::*;

use crate::Engine;

#[wasm_bindgen]
pub struct Scene(storm::Scene);

#[wasm_bindgen]
impl Scene {
    pub fn builder() -> SceneBuilder {
        SceneBuilder(storm::Scene::builder())
    }
}

#[wasm_bindgen]
pub struct SceneBuilder(storm::SceneBuilder);

#[wasm_bindgen]
impl SceneBuilder {
    pub async fn build(self, engine: &mut Engine) -> Scene {
        Scene(self.0.build(&mut engine.0))
    }
}