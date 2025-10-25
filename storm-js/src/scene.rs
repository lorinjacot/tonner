use wasm_bindgen::prelude::*;

use crate::{Engine, Surface};

/// A scene describes a world. A scene can be evolve over time and can be rendered to a screen or a texture.
/// 
/// A scene is made up of nodes. Nodes are organized in a parent-child hierachy, known as the node-hierarchy
/// or the scene graph. A node is called a root node when it doesn't have a parent. Each node defines a local
/// space. The local transform is used to get from the parent node local space (parent space for short) to the
/// local space. The global transform is used to get from the scene space (or global space) to the local space.
/// Both transforms are equal for root nodes. 
/// 
/// To add an object to the scene, attach it to a node. For example, each node can have a mesh. During rendering,
/// the attached mesh will be rendered at the local space origin.
#[wasm_bindgen]
pub struct Scene(storm::Scene);

#[wasm_bindgen]
impl Scene {
    /// Create an scene builder with default values.
    pub fn builder() -> SceneBuilder {
        SceneBuilder(storm::Scene::builder())
    }

    /// Render the scene to the surface.
    pub fn render(&self, surface: &Surface) {
        todo!()
    }
}

/// A builder for `Scene`.
#[wasm_bindgen]
pub struct SceneBuilder(storm::SceneBuilder);

#[wasm_bindgen]
impl SceneBuilder {
    /// Build the scene.
    pub async fn build(self, engine: &mut Engine) -> Scene {
        Scene(self.0.build(&mut engine.inner))
    }
}