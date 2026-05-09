//! Input events consumed by the [`Runtime`](crate::Runtime).
//!
//! Platform shells convert native OS events into the types defined here and
//! pass them to [`Runtime::handle_input`](crate::Runtime::handle_input).

use fission_layout::{LayoutPoint, LayoutSize};
use serde::{Deserialize, Serialize};

/// Identifies which mouse button or touch produced a pointer event.
///
/// # Variants
///
/// - `Primary` -- left mouse button or primary touch contact.
/// - `Secondary` -- right mouse button.
/// - `Middle` -- middle mouse button (scroll wheel click).
/// - `Other(u8)` -- auxiliary buttons (back, forward, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PointerButton {
    /// Left mouse button or primary touch.
    Primary,
    /// Right mouse button.
    Secondary,
    /// Middle mouse button.
    Middle,
    /// Auxiliary buttons identified by index.
    Other(u8),
}

/// A pointer (mouse / touch / stylus) event in layout coordinates.
///
/// # Example
///
/// ```rust,ignore
/// let event = InputEvent::Pointer(PointerEvent::Down {
///     point: LayoutPoint::new(100.0, 200.0),
///     button: PointerButton::Primary,
/// });
/// runtime.handle_input(event, &ir, &layout)?;
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PointerEvent {
    /// A button was pressed at the given point.
    Down {
        point: LayoutPoint,
        button: PointerButton,
        /// Modifier bitmask (Shift=1, Alt=2, Ctrl=4, Super=8).
        modifiers: u8,
    },
    /// A button was released at the given point.
    Up {
        point: LayoutPoint,
        button: PointerButton,
        /// Modifier bitmask (Shift=1, Alt=2, Ctrl=4, Super=8).
        modifiers: u8,
    },
    /// The pointer moved (no button state change).
    Move {
        point: LayoutPoint,
        /// Modifier bitmask (Shift=1, Alt=2, Ctrl=4, Super=8).
        modifiers: u8,
    },
    /// A scroll (mouse wheel or trackpad) gesture.
    Scroll {
        point: LayoutPoint,
        /// Scroll delta in layout units (positive = scroll down / right).
        delta: LayoutPoint,
        /// Modifier bitmask (Shift=1, Alt=2, Ctrl=4, Super=8).
        modifiers: u8,
    },
}

/// Platform-independent key code for keyboard events.
///
/// Named keys map directly to their function. Printable characters use
/// `Char(char)`.
///
/// # Example
///
/// ```rust,ignore
/// let event = InputEvent::Keyboard(KeyEvent::Down {
///     key_code: KeyCode::Enter,
///     modifiers: 0,
/// });
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum KeyCode {
    Space,
    Enter,
    Escape,
    Backspace,
    Delete,
    Tab,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    /// A printable character.
    Char(char),
}

/// Shift modifier bit.
pub const MOD_SHIFT: u8 = 1;
/// Alt/Option modifier bit.
pub const MOD_ALT: u8 = 2;
/// Control modifier bit.
pub const MOD_CTRL: u8 = 4;
/// Super/Meta/Command modifier bit.
pub const MOD_SUPER: u8 = 8;

/// A keyboard key press or release event.
///
/// The `modifiers` field is a bitmask: bit 0 = Shift, bit 1 = Alt,
/// bit 2 = Ctrl, bit 3 = Super/Meta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum KeyEvent {
    /// A key was pressed.
    Down {
        key_code: KeyCode,
        /// Modifier bitmask (Shift=1, Alt=2, Ctrl=4, Super=8).
        modifiers: u8,
    },
    /// A key was released.
    Up { key_code: KeyCode, modifiers: u8 },
}

/// Application lifecycle events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LifecycleEvent {
    /// The application has finished initialisation.
    Init,
    /// The application returned to the foreground.
    Resume,
    /// The application moved to the background.
    Pause,
    /// The application is about to terminate.
    Terminate,
    /// The viewport was resized.
    Resize { size: LayoutSize },
}

/// High-level gesture events recognised by the platform.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GestureEvent {
    /// A single tap (pointer down + up within threshold).
    Tap { point: LayoutPoint },
    /// Two taps in quick succession.
    DoubleTap { point: LayoutPoint },
    /// A pan/drag gesture began.
    PanStart { point: LayoutPoint },
    /// A pan/drag gesture updated.
    PanUpdate {
        point: LayoutPoint,
        delta: LayoutPoint,
    },
    /// A pan/drag gesture ended.
    PanEnd { point: LayoutPoint },
    /// The pointer was held down for longer than the long-press threshold.
    LongPress { point: LayoutPoint },
}

/// The top-level input event type consumed by
/// [`Runtime::handle_input`](crate::Runtime::handle_input).
///
/// Platform shells convert native OS events into `InputEvent` values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    /// Mouse, touch, or stylus events.
    Pointer(PointerEvent),
    /// Keyboard key events.
    Keyboard(KeyEvent),
    /// Input Method Editor (IME) events for CJK and composed text.
    Ime(ImeEvent),
    /// High-level gesture events.
    Gesture(GestureEvent),
    /// Application lifecycle transitions.
    Lifecycle(LifecycleEvent),
}

/// Input Method Editor events for composed text input (CJK, emoji, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ImeEvent {
    /// The IME is composing text (shown as a preview before the user confirms).
    Preedit { text: String },
    /// The user confirmed the composed text.
    Commit { text: String },
}
