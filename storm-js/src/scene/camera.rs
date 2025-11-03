#![allow(non_snake_case)]

use serde::Deserialize;
use wasm_bindgen::prelude::*;

/// A unique id for a camera. A camera will always have the same id.
#[wasm_bindgen]
pub struct CameraId;

#[wasm_bindgen]
pub struct CameraBuilder;

#[wasm_bindgen]
impl CameraBuilder {
    /// Create a builder with default parameters.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self
    }


}

#[derive(Deserialize)]
pub struct CameraPerspective {
    pub aspectRatio: Option<f32>,
    pub yfov: f32,
    pub zfar: Option<f32>,
    pub znear: f32,
}