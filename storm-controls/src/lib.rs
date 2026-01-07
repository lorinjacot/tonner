use bitflags::bitflags;

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

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}