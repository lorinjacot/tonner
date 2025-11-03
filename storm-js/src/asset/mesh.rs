use thiserror::Error;
use wasm_bindgen::prelude::*;

use crate::{Engine, asset::{geometry::Geometry, material::Material}};

/// A mesh is a model of a 3D object. It wraps a {@link Geometry} and a {@link Material}.
#[wasm_bindgen]
pub struct Mesh();

/// A builder for {@link Mesh}.
#[wasm_bindgen]
#[derive(Default)]
pub struct MeshBuilder();

#[wasm_bindgen]
impl MeshBuilder {
    /// Create a mesh builder with default parameters.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn geometry(self, _geometry: Geometry) -> Self {
        todo!()
    }

    pub fn material(self, _material: Material) -> Self {
        todo!()
    }

    /// Build the mesh.
    pub fn build(self, _engine: &mut Engine) -> Result<Mesh, MeshBuilderError> {
        Ok(Mesh())
    }
}

/// Error when {@link MeshBuilder.build()} fails.
#[wasm_bindgen]
#[derive(Debug, Error)]
#[error("build failed")]
pub struct MeshBuilderError;