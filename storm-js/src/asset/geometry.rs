#![allow(non_snake_case)]

use glam_js::Vec3;
use thiserror::Error;
use wasm_bindgen::prelude::*;

use crate::Engine;

/// A geometry describe a 3D shape.
///
/// A 3D does not contain any information about the material. For that, see {@link Mesh}.
/// A `Mesh` wrap a `Geometry` with a {@link Material}.
#[wasm_bindgen]
pub struct Geometry();

/// A builder for {@link Geometry}.
#[wasm_bindgen]
pub struct GeometryBuilder(storm::geometry::GeometryBuilder);

#[wasm_bindgen]
impl GeometryBuilder {
    /// Create a new geometry builder with default parameters.
    #[wasm_bindgen(constructor)]
    pub fn new(vertexCount: usize, morphTargetCount: usize) -> Self {
        Self(storm::geometry::GeometryBuilder::new(
            vertexCount,
            morphTargetCount,
        ))
    }

    /// Set the `position` attribute of the geometry vertices. `positions.length` must be equal
    /// to the constructor argument `vertexCount`.
    pub fn positions(self, positions: Vec<Vec3>) -> Result<Self, InvalidAttributeIterLenError> {
        Ok(Self(
            self.0
                .positions(positions.into_iter().map(|v| v.into()))
                .map_err(|e| InvalidAttributeIterLenError { min: e.min })?,
        ))
    }

    /// Build the geometry.
    pub fn build(self, _engine: &mut Engine) -> Result<Geometry, GeometryBuilderError> {
        todo!()
    }
}

#[wasm_bindgen]
#[derive(Debug, Error)]
#[error("attribute must contain at least {min} elements")]
pub struct InvalidAttributeIterLenError {
    pub min: usize,
}

/// Error when {@link GeometryBuilder.build()} fails.
#[wasm_bindgen]
#[derive(Debug, Error)]
#[error("build failed")]
pub struct GeometryBuilderError;
