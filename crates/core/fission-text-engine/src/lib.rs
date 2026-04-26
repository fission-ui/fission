//! # fission-text-engine
//!
//! Production-quality text buffer for a code editor.
//!
//! This crate provides the **text storage and manipulation layer** — it is
//! deliberately not an editor.  It owns:
//!
//! * A rope-backed [`TextBuffer`] with O(log n) edits,
//! * A [`LineIndex`] that maps between byte offsets, `(line, col)` pairs, and
//!   UTF-16 code-unit columns (the encoding used by the Language Server
//!   Protocol),
//! * An [`EditHistory`] with bounded undo / redo stacks built on
//!   [`EditTransaction`]s,
//! * A [`CoordinateMapper`] that translates freely between the three
//!   coordinate systems (byte offset, line/col, LSP position).

pub mod buffer;
pub mod coordinate;
pub mod edit;
pub mod line_index;

pub use buffer::TextBuffer;
pub use coordinate::{CoordinateMapper, LspPosition};
pub use edit::{EditHistory, EditTransaction, TextEdit};
pub use line_index::{LineCol, LineIndex};

#[cfg(test)]
mod tests;
