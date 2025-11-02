#![allow(non_snake_case)]

use thiserror::Error;
use wasm_bindgen::prelude::*;

use crate::Engine;

/// A material describe how to render a 2D surface.
///
/// A material does not contain any information about the geometry. For that, see {@link Mesh}.
/// A `Mesh` wrap a `Material` with a {@link Material}.
#[wasm_bindgen]
pub struct Material();

/// A builder for {@link Material}.
#[wasm_bindgen]
pub struct MaterialBuilder;

#[wasm_bindgen]
impl MaterialBuilder {
    /// Create a new material builder with default parameters.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self
    }

    /// Build the material.
    pub fn build(self, _engine: &mut Engine) -> Result<Material, MaterialBuilderError> {
        todo!()
    }
}

/// Error when {@link MaterialBuilder.build()} fails.
#[wasm_bindgen]
#[derive(Debug, Error)]
#[error("build failed")]
pub struct MaterialBuilderError;
