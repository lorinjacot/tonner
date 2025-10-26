use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct NodeId;

#[wasm_bindgen]
pub struct NodeBuilder {
    parent: Option<NodeId>,
    
}