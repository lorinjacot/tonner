use thiserror::Error;
use wasm_bindgen::prelude::*;

use crate::{asset::mesh::Mesh, scene::Scene};

/// A unique id for a mesh attached to a node. A mesh instance will always have the same id.
#[wasm_bindgen]
pub struct MeshInstanceId;

/// A mesh instance builder. This is used to add a mesh to a scene.
#[wasm_bindgen]
pub struct MeshInstanceBuilder(storm::mesh_instance::MeshInstanceBuilder);

#[wasm_bindgen]
impl MeshInstanceBuilder {
    /// New mesh instance builder with default paremeters.
    #[wasm_bindgen(constructor)]
    pub fn new(mesh: Mesh) -> Self {
        Self(storm::mesh_instance::MeshInstanceBuilder::new(mesh))
    }

    /// Give a name to the mesh instance. Useful for GUI and debugging.
    pub fn name(self, name: String) -> Self {
        Self(self.0.name(name))
    }

    /// Add the mesh to the scene.
    pub fn build(self, scene: &mut Scene) -> Result<MeshInstanceId, MeshInstanceBuilderError> {
        self.0.build(&mut scene.0)?;
        Ok(MeshInstanceId)
    }
}

/// Error when {@link MeshInstanceBuilder.build()} fails.
#[wasm_bindgen]
#[derive(Debug, Error)]
#[error(transparent)]
pub struct MeshInstanceBuilderError(#[from] storm::mesh_instance::MeshInstanceBuilderError);
