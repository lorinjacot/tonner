use thiserror::Error;
use wasm_bindgen::prelude::*;

use crate::{
    Engine,
    asset::{geometry::Geometry, material::Material},
};

/// A mesh is a model of a 3D object. It wraps a {@link Geometry} and a {@link Material}.
#[wasm_bindgen]
pub struct Mesh(storm::mesh::Mesh);

impl From<storm::mesh::Mesh> for Mesh {
    fn from(value: storm::mesh::Mesh) -> Self {
        Self(value)
    }
}

impl From<Mesh> for storm::mesh::Mesh {
    fn from(value: Mesh) -> Self {
        value.0
    }
}

/// A builder for {@link Mesh}.
#[wasm_bindgen]
pub struct MeshBuilder(storm::mesh::MeshBuilder);

#[wasm_bindgen]
impl MeshBuilder {
    /// Create a mesh builder with default parameters.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self(storm::mesh::MeshBuilder::default())
    }

    /// Gives a name to the mesh. Used for GUI and debugging.
    pub fn name(self, name: String) -> Self {
        Self(self.0.name(name))
    }

    /// Add a new {@link Geometry}-{@link Material} pair to the mesh.
    /// This method must be called at least once.
    pub fn primitive(self, geometry: Geometry, material: Material) -> Self {
        Self(self.0.primitive(geometry, material))
    }

    /// Build the mesh.
    pub fn build(self, engine: &mut Engine) -> Result<Mesh, MeshBuilderError> {
        Ok(Mesh(self.0.build(&mut engine.inner)?))
    }
}

/// Error when {@link MeshBuilder.build()} fails.
#[wasm_bindgen]
#[derive(Debug, Error)]
#[error(transparent)]
pub struct MeshBuilderError(#[from] storm::mesh::MeshBuilderError);
