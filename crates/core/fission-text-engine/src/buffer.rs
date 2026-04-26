//! Rope-backed text buffer with revision tracking.

use ropey::Rope;
use std::fmt;
use std::ops::Range;

/// A text buffer backed by a [`ropey::Rope`].
///
/// Every mutating operation increments an internal **revision** counter so that
/// downstream systems (layout caches, syntax highlights, diagnostics, etc.) can
/// cheaply detect stale data.
#[derive(Clone)]
pub struct TextBuffer {
    rope: Rope,
    revision: u64,
}

// ── Constructors ────────────────────────────────────────────────────────────

impl TextBuffer {
    /// Create an empty buffer.
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            revision: 0,
        }
    }

    /// Create a buffer pre-populated with `text`.
    pub fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            revision: 0,
        }
    }
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Read-only queries ───────────────────────────────────────────────────────

impl TextBuffer {
    /// Return a reference to the underlying rope.
    pub fn text(&self) -> &Rope {
        &self.rope
    }

    /// Total length in bytes (UTF-8).
    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    /// Total length in Unicode characters (grapheme-unaware; counts `char`s).
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Number of lines.  A trailing `\n` implies an additional empty final
    /// line, matching the convention used by most editors.
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// Return the contents of `line_idx` (0-based) as a `ropey::RopeSlice`,
    /// including the line terminator if present.
    ///
    /// # Panics
    ///
    /// Panics if `line_idx >= self.len_lines()`.
    pub fn line(&self, line_idx: usize) -> ropey::RopeSlice<'_> {
        self.rope.line(line_idx)
    }

    /// Return an arbitrary byte-offset range as a `RopeSlice`.
    ///
    /// Both bounds are byte offsets and must lie on `char` boundaries.
    ///
    /// # Panics
    ///
    /// Panics if the range is out of bounds or not on char boundaries.
    pub fn slice(&self, byte_range: Range<usize>) -> ropey::RopeSlice<'_> {
        let start_char = self.rope.byte_to_char(byte_range.start);
        let end_char = self.rope.byte_to_char(byte_range.end);
        self.rope.slice(start_char..end_char)
    }

    /// Monotonically increasing revision counter.  Incremented on every
    /// mutation (`insert`, `delete`, `replace`).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// `true` when the buffer contains no characters.
    pub fn is_empty(&self) -> bool {
        self.len_chars() == 0
    }
}

// ── Mutations ───────────────────────────────────────────────────────────────

impl TextBuffer {
    /// Insert `text` at the given **byte offset**.
    ///
    /// # Panics
    ///
    /// Panics if `byte_offset` is out of bounds or not on a char boundary.
    pub fn insert(&mut self, byte_offset: usize, text: &str) {
        let char_idx = self.rope.byte_to_char(byte_offset);
        self.rope.insert(char_idx, text);
        self.revision += 1;
    }

    /// Delete the byte range `start..end`.
    ///
    /// # Panics
    ///
    /// Panics if the range is out of bounds or not on char boundaries.
    pub fn delete(&mut self, byte_range: Range<usize>) {
        let start_char = self.rope.byte_to_char(byte_range.start);
        let end_char = self.rope.byte_to_char(byte_range.end);
        self.rope.remove(start_char..end_char);
        self.revision += 1;
    }

    /// Replace the byte range `start..end` with `text`.
    ///
    /// Equivalent to a `delete` followed by an `insert` but only bumps the
    /// revision once.
    pub fn replace(&mut self, byte_range: Range<usize>, text: &str) {
        let start_char = self.rope.byte_to_char(byte_range.start);
        let end_char = self.rope.byte_to_char(byte_range.end);
        self.rope.remove(start_char..end_char);
        self.rope.insert(start_char, text);
        self.revision += 1;
    }
}

// ── Display / Debug ─────────────────────────────────────────────────────────

impl fmt::Display for TextBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for chunk in self.rope.chunks() {
            f.write_str(chunk)?;
        }
        Ok(())
    }
}

impl fmt::Debug for TextBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextBuffer")
            .field("len_bytes", &self.len_bytes())
            .field("len_chars", &self.len_chars())
            .field("len_lines", &self.len_lines())
            .field("revision", &self.revision)
            .finish()
    }
}
