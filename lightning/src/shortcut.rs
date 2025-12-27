use egui::{Key, KeyboardShortcut, Modifiers};

pub (super) const NEW_SCENE: KeyboardShortcut = KeyboardShortcut {
    modifiers: Modifiers::CTRL,
    logical_key: Key::N,
};

pub(super) const OPEN_FILE: KeyboardShortcut = KeyboardShortcut {
    modifiers: Modifiers::CTRL,
    logical_key: Key::O,
};