use bitflags::bitflags;
#[cfg(feature = "egui")]
use storm::Scene;

#[cfg(feature = "orbit")]
pub mod orbit;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
pub enum Key {
    ArrowLeft,
    ArrowUp,
    ArrowRight,
    ArrowDown,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Modifiers: u8 {
        const CTRL  = 1 << 0;
        const SHIFT = 1 << 1;
        const META  = 1 << 2;
    }
}

/// A controls that can used with egui out of the box.
#[cfg(feature = "egui")]
pub trait EguiControls {
    /// Handle egui responses. To make the controls interactive, this function
    /// needs to be called each time the egui renders.
    fn handle_response(&mut self, response: egui::Response, ui: &egui::Ui, scene: &mut Scene);
}
