use bytemuck::{Pod, Zeroable};
use egui::Pos2;
use glam::Mat4;

#[non_exhaustive]
#[derive(Debug)]
pub struct BillboardLabel {
    pub text: String,
}

impl BillboardLabel {
    pub fn new(text: impl Into<String>) -> Self {
        BillboardLabel { text: text.into() }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(super) struct CameraUniform {
    pub view: Mat4,
    pub projection: Mat4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(super) struct OutputUniform {
    pub screen_position: Pos2,
    pub view_z: f32,
    pub visible: u32,
}
