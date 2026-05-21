//! Terminal shell for Fission applications.
//!
//! The terminal shell runs the normal Fission build/lower/layout pipeline and
//! renders the resulting Core IR into a terminal cell frame. It is intentionally
//! not a separate terminal UI framework: unsupported terminal output is detected
//! from Core IR and semantics rather than from widget names.

mod app;
mod frame;
mod render;
mod screenshot;
mod text;
mod verify;

pub use app::{TerminalApp, TerminalRunOptions};
pub use frame::{TerminalCell, TerminalColor, TerminalFrame, TerminalStyle};
pub use render::TerminalRenderer;
pub use screenshot::{write_frame_png, ScreenshotOptions};
pub use text::TerminalTextMeasurer;
pub use verify::{verify_terminal_ir, TerminalSupportError};
