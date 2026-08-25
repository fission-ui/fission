//! Input events consumed by the [`Runtime`](crate::Runtime).
//!
//! Platform shells convert native OS events into the types defined here and
//! pass them to [`Runtime::handle_input`](crate::Runtime::handle_input).

use fission_layout::{LayoutPoint, LayoutSize};
use serde::{Deserialize, Serialize};

/// Stable identity for one active pointer contact.
///
/// Pointer ids are only required to be unique among simultaneously active
/// pointers. Mouse input uses [`PointerId::MOUSE`]; touch ids come from the
/// platform shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PointerId(pub u128);

impl PointerId {
    pub const MOUSE: Self = Self(0);

    /// Namespaces a platform contact id away from the singleton mouse id.
    pub const fn contact(platform_id: u64) -> Self {
        Self(platform_id as u128 + 1)
    }
}

impl Default for PointerId {
    fn default() -> Self {
        Self::MOUSE
    }
}

/// Physical input source which produced a pointer event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointerKind {
    Mouse,
    Touch,
    Stylus,
    Unknown,
}

impl Default for PointerKind {
    fn default() -> Self {
        Self::Mouse
    }
}

/// Lifecycle phase shared by continuous pointer signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointerPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

impl Default for PointerPhase {
    fn default() -> Self {
        Self::Moved
    }
}

/// Units reported by a scroll input source.
///
/// This deliberately describes the reported delta rather than guessing whether
/// a high-resolution device is a wheel or trackpad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScrollDeltaMode {
    Line,
    Pixel,
}

impl Default for ScrollDeltaMode {
    fn default() -> Self {
        Self::Pixel
    }
}

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
///     pointer_id: PointerId::MOUSE,
///     kind: PointerKind::Mouse,
///     point: LayoutPoint::new(100.0, 200.0),
///     button: PointerButton::Primary,
///     modifiers: 0,
/// });
/// runtime.handle_input(event, &ir, &layout)?;
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PointerEvent {
    /// A button was pressed at the given point.
    Down {
        pointer_id: PointerId,
        kind: PointerKind,
        point: LayoutPoint,
        button: PointerButton,
        /// Modifier bitmask (Shift=1, Alt=2, Ctrl=4, Super=8).
        modifiers: u8,
    },
    /// A button was released at the given point.
    Up {
        pointer_id: PointerId,
        kind: PointerKind,
        point: LayoutPoint,
        button: PointerButton,
        /// Modifier bitmask (Shift=1, Alt=2, Ctrl=4, Super=8).
        modifiers: u8,
    },
    /// The pointer moved (no button state change).
    Move {
        pointer_id: PointerId,
        kind: PointerKind,
        point: LayoutPoint,
        /// Modifier bitmask (Shift=1, Alt=2, Ctrl=4, Super=8).
        modifiers: u8,
    },
    /// An active pointer sequence was cancelled by the platform.
    Cancel {
        pointer_id: PointerId,
        kind: PointerKind,
        point: LayoutPoint,
        /// Modifier bitmask (Shift=1, Alt=2, Ctrl=4, Super=8).
        modifiers: u8,
    },
    /// A scroll (mouse wheel or trackpad) gesture.
    Scroll {
        point: LayoutPoint,
        /// Scroll delta in layout units (positive = scroll down / right).
        delta: LayoutPoint,
        /// Whether the platform supplied line or pixel deltas.
        delta_mode: ScrollDeltaMode,
        /// Lifecycle phase supplied by the platform.
        phase: PointerPhase,
        /// Modifier bitmask (Shift=1, Alt=2, Ctrl=4, Super=8).
        modifiers: u8,
    },
    /// A platform-recognized magnification gesture, such as a trackpad pinch.
    Magnify {
        /// Gesture focal point in layout coordinates.
        point: LayoutPoint,
        /// Multiplicative scale factor for this update (`1.0` means unchanged).
        scale_factor: f32,
        phase: PointerPhase,
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
    /// A key press carrying the complete text produced by the platform.
    ///
    /// `key_code` remains the logical shortcut/navigation identity. `text` is
    /// inserted as one edit and may contain several scalars or graphemes.
    DownWithText {
        key_code: KeyCode,
        modifiers: u8,
        text: String,
    },
    /// A key was released.
    Up { key_code: KeyCode, modifiers: u8 },
}

/// Semantic text-editing commands produced by a platform shell.
///
/// Browser shells should translate trusted `copy`, `cut`, and `paste` events
/// into these commands instead of synthesizing platform-specific key chords.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditingCommand {
    Copy,
    Cut,
    Paste(String),
    SelectAll,
    Undo,
    Redo,
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

/// File drag-and-drop events delivered by desktop shells.
///
/// These events model OS-level drags such as files dragged from Finder,
/// Explorer, or a Linux file manager into a Fission window. Internal widget
/// drags use normal pointer events plus the widget drag payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExternalDragEvent {
    /// One or more external files are hovering over the window.
    Hover {
        point: LayoutPoint,
        paths: Vec<String>,
        /// Modifier bitmask (Shift=1, Alt=2, Ctrl=4, Super=8).
        modifiers: u8,
    },
    /// The external drag left the window or was cancelled by the platform.
    Cancel,
    /// One or more external files were dropped at the current pointer point.
    Drop {
        point: LayoutPoint,
        paths: Vec<String>,
        /// Modifier bitmask (Shift=1, Alt=2, Ctrl=4, Super=8).
        modifiers: u8,
    },
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
    /// A platform-native semantic text-editing command.
    Editing(EditingCommand),
    /// A complete platform-neutral value, selection, composition, or range edit.
    TextEdit(crate::TextEditCommand),
    /// A platform requested Fission's contextual action at this position.
    ContextMenuRequested {
        point: LayoutPoint,
        /// Modifier bitmask (Shift=1, Alt=2, Ctrl=4, Super=8).
        modifiers: u8,
    },
    /// High-level gesture events.
    Gesture(GestureEvent),
    /// Desktop shell drag-and-drop events from outside the app.
    ExternalDrag(ExternalDragEvent),
    /// Application lifecycle transitions.
    Lifecycle(LifecycleEvent),
}

/// Input Method Editor events for composed text input (CJK, emoji, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ImeEvent {
    /// The IME is composing text before the user confirms it.
    ///
    /// `cursor` is an optional byte range inside `text` reported by the
    /// platform IME. Shells can use it to render the active composition cursor
    /// or marked segment separately from the rest of the preedit text.
    Preedit {
        text: String,
        cursor: Option<(usize, usize)>,
    },
    /// The active composition was cancelled without committing text.
    Cancel,
    /// The user confirmed the composed text.
    Commit { text: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_contact_identity_round_trips() {
        let event = PointerEvent::Cancel {
            pointer_id: PointerId::contact(42),
            kind: PointerKind::Touch,
            point: LayoutPoint::new(12.0, 18.0),
            modifiers: MOD_SHIFT,
        };
        let encoded = serde_json::to_string(&event).unwrap();
        assert_eq!(
            serde_json::from_str::<PointerEvent>(&encoded).unwrap(),
            event
        );
    }

    #[test]
    fn detailed_pointer_signals_round_trip() {
        let scroll = PointerEvent::Scroll {
            point: LayoutPoint::new(3.0, 5.0),
            delta: LayoutPoint::new(-2.0, 7.0),
            delta_mode: ScrollDeltaMode::Line,
            phase: PointerPhase::Started,
            modifiers: MOD_CTRL,
        };
        let magnify = PointerEvent::Magnify {
            point: LayoutPoint::new(9.0, 11.0),
            scale_factor: 1.25,
            phase: PointerPhase::Moved,
            modifiers: 0,
        };
        for event in [scroll, magnify] {
            let encoded = serde_json::to_string(&event).unwrap();
            assert_eq!(
                serde_json::from_str::<PointerEvent>(&encoded).unwrap(),
                event
            );
        }
    }
}
