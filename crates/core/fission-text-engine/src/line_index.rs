//! Efficient mapping between byte offsets and line/column positions.
//!
//! [`LineIndex`] is built once from a snapshot of the buffer text and then
//! queried many times.  Rebuilding is cheap (single linear scan) and should
//! be done after every edit batch.

use ropey::Rope;

/// A `(line, col)` pair — both 0-based, with `col` measured in **bytes**
/// relative to the start of the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineCol {
    /// 0-based line number.
    pub line: usize,
    /// 0-based column as a **byte** offset from the start of the line.
    pub col: usize,
}

/// Precomputed index that maps between byte offsets and `(line, col)` pairs.
///
/// It stores the byte offset of the start of every line so that both directions
/// of the mapping are O(log n) via binary search.
#[derive(Debug, Clone)]
pub struct LineIndex {
    /// `line_starts[i]` is the byte offset of the first byte of line `i`.
    line_starts: Vec<usize>,
    /// A copy of the source text used for UTF-16 column translation.  Storing
    /// the full text is intentional: the index is short-lived (rebuilt after
    /// each edit batch) and avoids lifetime coupling to the buffer.
    source: String,
}

impl LineIndex {
    /// Build a new index from a [`Rope`].
    pub fn build(rope: &Rope) -> Self {
        let source: String = rope.chunks().collect();
        let mut line_starts = vec![0usize];
        for (i, b) in source.as_bytes().iter().enumerate() {
            if *b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self {
            line_starts,
            source,
        }
    }

    /// Build from a plain `&str` (convenience for testing).
    pub fn build_from_str(text: &str) -> Self {
        let rope = Rope::from_str(text);
        Self::build(&rope)
    }

    // ── Queries ─────────────────────────────────────────────────────────

    /// Number of lines in the indexed text.
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Convert a `LineCol` to an absolute byte offset.
    ///
    /// Returns `None` if the position is out of range.
    pub fn line_col_to_byte(&self, lc: LineCol) -> Option<usize> {
        let start = *self.line_starts.get(lc.line)?;
        let offset = start + lc.col;
        if offset > self.source.len() {
            return None;
        }
        Some(offset)
    }

    /// Convert an absolute byte offset to a `LineCol`.
    ///
    /// Returns `None` if `byte_offset > source.len()`.
    pub fn byte_to_line_col(&self, byte_offset: usize) -> Option<LineCol> {
        if byte_offset > self.source.len() {
            return None;
        }
        // Binary search: find the last line whose start <= byte_offset.
        let line = match self.line_starts.binary_search(&byte_offset) {
            Ok(exact) => exact,
            Err(insert) => insert - 1,
        };
        let col = byte_offset - self.line_starts[line];
        Some(LineCol { line, col })
    }

    /// Byte offset of the first byte of `line` (0-based).
    ///
    /// Returns `None` if `line >= line_count()`.
    pub fn line_start_byte(&self, line: usize) -> Option<usize> {
        self.line_starts.get(line).copied()
    }

    /// Byte offset one past the last byte of `line` (exclusive end).
    ///
    /// For the last line this equals `source.len()`.  Returns `None` if
    /// `line >= line_count()`.
    pub fn line_end_byte(&self, line: usize) -> Option<usize> {
        if line >= self.line_starts.len() {
            return None;
        }
        if line + 1 < self.line_starts.len() {
            Some(self.line_starts[line + 1])
        } else {
            Some(self.source.len())
        }
    }

    /// Convert a **UTF-16 code-unit column** (as used by LSP) to a byte
    /// offset within the document.
    ///
    /// `line` is 0-based.  `utf16_col` is the number of UTF-16 code units
    /// from the start of the line.
    ///
    /// Returns `None` if the position cannot be mapped (line out of range or
    /// column past end of line).
    pub fn utf16_col_to_byte(&self, line: usize, utf16_col: usize) -> Option<usize> {
        let line_start = *self.line_starts.get(line)?;
        let line_end = self.line_end_byte(line)?;
        let line_text = &self.source[line_start..line_end];

        let mut utf16_units = 0usize;
        for (byte_idx, ch) in line_text.char_indices() {
            if utf16_units == utf16_col {
                return Some(line_start + byte_idx);
            }
            utf16_units += ch.len_utf16();
        }
        // Allow pointing one-past-end (cursor at EOL).
        if utf16_units == utf16_col {
            return Some(line_start + line_text.len());
        }
        None
    }

    /// Convert a byte offset to a **(line, utf16_col)** pair — the inverse of
    /// [`utf16_col_to_byte`](Self::utf16_col_to_byte).
    pub fn byte_to_utf16_col(&self, byte_offset: usize) -> Option<(usize, usize)> {
        let lc = self.byte_to_line_col(byte_offset)?;
        let line_start = self.line_starts[lc.line];
        let prefix = &self.source[line_start..byte_offset];
        let utf16_col: usize = prefix.chars().map(|c| c.len_utf16()).sum();
        Some((lc.line, utf16_col))
    }
}
