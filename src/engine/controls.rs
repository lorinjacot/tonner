pub trait Controls: Sync + Send {
    fn handle_inputs(&mut self, inputs: &mut egui::InputState);
}

pub struct OrbitControls {}

impl OrbitControls {
    pub fn new() -> Self {
        Self {  }
    }
}

impl Controls for OrbitControls {
    fn handle_inputs(&mut self, inputs: &mut egui::InputState) {
        todo!()
    }
}
