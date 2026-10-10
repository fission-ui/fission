//! Desktop window operations requested through the normal host-effect boundary.

use crate::{CapabilityType, OperationCapability};
use serde::{Deserialize, Serialize};

/// A command for the desktop application's active window.
///
/// Emit with `ctx.effects.capability(WINDOW_CONTROL, command).dispatch()` from a
/// reducer. Moving the window requires a native pointer press and is provided by
/// [`crate::ui::WindowDragRegion`] instead of an asynchronous command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowCommand {
    Minimize,
    Maximize,
    /// Leaves maximization; restoring a minimized Wayland window belongs to its compositor.
    Restore,
    /// Uses the same close policy as the native title-bar button, including tray apps.
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowControlError {
    /// The current shell does not host a desktop window.
    Unsupported,
    /// The window's event loop has stopped.
    Unavailable,
}

pub struct WindowControlCapability;

impl OperationCapability for WindowControlCapability {
    type Request = WindowCommand;
    type Ok = ();
    type Err = WindowControlError;
}

pub const WINDOW_CONTROL: CapabilityType<WindowControlCapability> =
    CapabilityType::new("fission.ui.window_control");
