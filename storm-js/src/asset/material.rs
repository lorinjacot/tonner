#![allow(non_snake_case)]

use glam_js::Vec4;
use wasm_bindgen::prelude::*;

use crate::Context;

/// A material describe how to render a 2D surface.
///
/// A material does not contain any information about the geometry. For that, see {@link Mesh}.
/// A `Mesh` wrap a `Material` with a {@link Material}.
#[wasm_bindgen]
pub struct Material(storm::material::Material);

impl From<storm::material::Material> for Material {
    fn from(value: storm::material::Material) -> Self {
        Self(value)
    }
}

impl From<Material> for storm::material::Material {
    fn from(value: Material) -> Self {
        value.0
    }
}

/// A builder for {@link Material}.
#[wasm_bindgen]
pub struct MaterialBuilder(storm::material::MaterialBuilder);

#[wasm_bindgen]
impl MaterialBuilder {
    /// Create a new material builder with default parameters.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self(storm::material::MaterialBuilder::default())
    }

    /// Give a name to the material. Default to no name.
    pub fn name(self, name: String) -> Self {
        Self(self.0.name(name))
    }

    /// The factors for the base color of the material. Default to {@link Vec4::ONE}.
    pub fn baseColorFactor(self, color: Vec4) -> Self {
        Self(self.0.base_color_factor(color))
    }

    /// Build the material.
    pub fn build(self, ctx: &Context) -> Material {
        Material(self.0.build(&ctx.inner))
    }
}
