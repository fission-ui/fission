//! Coordinate mapper: byte offset <-> line/col <-> LSP position (UTF-16).

use crate::line_index::{LineCol, LineIndex};

/// An LSP-compatible position (0-based line, 0-based UTF-16 code-unit column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LspPosition {
    /// 0-based line number.
    pub line: usize,
    /// 0-based column measured in UTF-16 code units.
    pub character: usize,
}

impl LspPosition {
    pub fn new(line: usize, character: usize) -> Self {
        Self { line, character }
    }
}

/// Stateless mapper that converts freely between three coordinate systems:
///
/// 1. **Byte offset** — absolute position in the UTF-8 buffer.
/// 2. **Line / Col** ([`LineCol`]) — 0-based line and byte-column.
/// 3. **LSP position** ([`LspPosition`]) — 0-based line and UTF-16 column.
///
/// All conversions go through a [`LineIndex`] that must be kept in sync with
/// the buffer text (rebuild after each edit batch).
pub struct CoordinateMapper<'a> {
    index: &'a LineIndex,
}

impl<'a> CoordinateMapper<'a> {
    /// Create a mapper backed by the given line index.
    pub fn new(index: &'a LineIndex) -> Self {
        Self { index }
    }

    // ── Byte <-> LineCol ────────────────────────────────────────────────

    /// Byte offset -> `LineCol`.
    pub fn byte_to_line_col(&self, byte_offset: usize) -> Option<LineCol> {
        self.index.byte_to_line_col(byte_offset)
    }

    /// `LineCol` -> byte offset.
    pub fn line_col_to_byte(&self, lc: LineCol) -> Option<usize> {
        self.index.line_col_to_byte(lc)
    }

    // ── Byte <-> LspPosition ───────────────────────────────────────────

    /// Byte offset -> `LspPosition`.
    pub fn byte_to_lsp(&self, byte_offset: usize) -> Option<LspPosition> {
        let (line, character) = self.index.byte_to_utf16_col(byte_offset)?;
        Some(LspPosition { line, character })
    }

    /// `LspPosition` -> byte offset.
    pub fn lsp_to_byte(&self, pos: LspPosition) -> Option<usize> {
        self.index.utf16_col_to_byte(pos.line, pos.character)
    }

    // ── LineCol <-> LspPosition ────────────────────────────────────────

    /// `LineCol` -> `LspPosition`.
    pub fn line_col_to_lsp(&self, lc: LineCol) -> Option<LspPosition> {
        let byte = self.index.line_col_to_byte(lc)?;
        self.byte_to_lsp(byte)
    }

    /// `LspPosition` -> `LineCol`.
    pub fn lsp_to_line_col(&self, pos: LspPosition) -> Option<LineCol> {
        let byte = self.lsp_to_byte(pos)?;
        self.byte_to_line_col(byte)
    }
}
