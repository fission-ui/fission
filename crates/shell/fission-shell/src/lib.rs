use anyhow::Result;
use fission_ir::NodeId;
use serde::{Deserialize, Serialize};

pub use fission_render::{LayoutPoint, LayoutRect, LayoutSize}; // Re-export layout types

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PointerButton {
    Primary,   // Left mouse button, primary touch
    Secondary, // Right mouse button, secondary touch
    Middle,    // Middle mouse button
    Other(u8), // Other buttons
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PointerEvent {
    Down { point: LayoutPoint, button: PointerButton },
    Up { point: LayoutPoint, button: PointerButton },
    Move { point: LayoutPoint },
    // ... hover, scroll, etc.
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum KeyCode {
    // A simplified set of key codes for now
    Space,
    Enter,
    Escape,
    Backspace,
    Left,
    Right,
    Up,
    Down,
    Char(char),
    // ... many more
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum KeyEvent {
    Down { key_code: KeyCode, modifiers: u8 }, // Modifiers could be an enum/bitflags
    Up { key_code: KeyCode, modifiers: u8 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LifecycleEvent {
    Init,
    Resume,
    Pause,
    Terminate,
    Resize { size: LayoutSize },
    // ... low memory, etc.
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    Pointer(PointerEvent),
    Keyboard(KeyEvent),
    Lifecycle(LifecycleEvent),
    // ... other categories like Scroll, Text, Accessibility, System
}

// The Platform trait, implemented by concrete platform shells (desktop, mobile, web).
pub trait Platform {
    // Dispatches an input event to the Core Runtime. The Platform is responsible for
    // normalizing raw OS input into `InputEvent`.
    fn dispatch_event(&mut self, event: InputEvent) -> Result<()>;

    // A placeholder for getting the render surface, or rendering commands.
    // Actual rendering would happen via `fission-render` traits.
    fn present(&mut self, display_list_data: &[u8]) -> Result<()>; // placeholder for byte data

    // Placeholder for clipboard access, system dialogs, etc.
    // fn get_clipboard_text(&self) -> Result<String>;
    // fn set_clipboard_text(&mut self, text: &str) -> Result<()>;
}