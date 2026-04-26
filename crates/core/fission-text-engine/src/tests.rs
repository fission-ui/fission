//! Comprehensive tests for fission-text-engine.

use crate::buffer::TextBuffer;
use crate::coordinate::{CoordinateMapper, LspPosition};
use crate::edit::{EditHistory, EditTransaction, TextEdit};
use crate::line_index::{LineCol, LineIndex};

// =========================================================================
// TextBuffer — basic operations
// =========================================================================

#[test]
fn empty_buffer() {
    let buf = TextBuffer::new();
    assert_eq!(buf.len_bytes(), 0);
    assert_eq!(buf.len_chars(), 0);
    assert_eq!(buf.len_lines(), 1); // ropey counts one line even when empty
    assert_eq!(buf.revision(), 0);
    assert!(buf.is_empty());
    assert_eq!(buf.to_string(), "");
}

#[test]
fn from_str_basic() {
    let buf = TextBuffer::from_str("hello\nworld\n");
    assert_eq!(buf.len_bytes(), 12);
    assert_eq!(buf.len_chars(), 12);
    assert_eq!(buf.len_lines(), 3); // "hello\n", "world\n", ""
    assert_eq!(buf.revision(), 0);
    assert!(!buf.is_empty());
}

#[test]
fn insert_at_start() {
    let mut buf = TextBuffer::from_str("world");
    buf.insert(0, "hello ");
    assert_eq!(buf.to_string(), "hello world");
    assert_eq!(buf.revision(), 1);
}

#[test]
fn insert_at_end() {
    let mut buf = TextBuffer::from_str("hello");
    buf.insert(5, " world");
    assert_eq!(buf.to_string(), "hello world");
    assert_eq!(buf.revision(), 1);
}

#[test]
fn insert_in_middle() {
    let mut buf = TextBuffer::from_str("helo");
    buf.insert(3, "l");
    assert_eq!(buf.to_string(), "hello");
}

#[test]
fn delete_range() {
    let mut buf = TextBuffer::from_str("hello world");
    buf.delete(5..11); // remove " world"
    assert_eq!(buf.to_string(), "hello");
    assert_eq!(buf.revision(), 1);
}

#[test]
fn delete_all() {
    let mut buf = TextBuffer::from_str("abc");
    buf.delete(0..3);
    assert_eq!(buf.to_string(), "");
    assert!(buf.is_empty());
}

#[test]
fn replace_same_length() {
    let mut buf = TextBuffer::from_str("hello world");
    buf.replace(6..11, "earth");
    assert_eq!(buf.to_string(), "hello earth");
    assert_eq!(buf.revision(), 1);
}

#[test]
fn replace_shorter() {
    let mut buf = TextBuffer::from_str("hello world");
    buf.replace(5..11, "!");
    assert_eq!(buf.to_string(), "hello!");
}

#[test]
fn replace_longer() {
    let mut buf = TextBuffer::from_str("hello!");
    buf.replace(5..6, " beautiful world!");
    assert_eq!(buf.to_string(), "hello beautiful world!");
}

#[test]
fn line_contents() {
    let buf = TextBuffer::from_str("aaa\nbbb\nccc");
    assert_eq!(buf.line(0).to_string(), "aaa\n");
    assert_eq!(buf.line(1).to_string(), "bbb\n");
    assert_eq!(buf.line(2).to_string(), "ccc");
}

#[test]
fn slice_range() {
    let buf = TextBuffer::from_str("hello world");
    let s = buf.slice(6..11);
    assert_eq!(s.to_string(), "world");
}

#[test]
fn revision_increments() {
    let mut buf = TextBuffer::new();
    assert_eq!(buf.revision(), 0);
    buf.insert(0, "a");
    assert_eq!(buf.revision(), 1);
    buf.insert(1, "b");
    assert_eq!(buf.revision(), 2);
    buf.delete(0..1);
    assert_eq!(buf.revision(), 3);
    buf.replace(0..1, "x");
    assert_eq!(buf.revision(), 4);
}

// =========================================================================
// TextBuffer — UTF-8 multi-byte
// =========================================================================

#[test]
fn utf8_multibyte_insert() {
    let mut buf = TextBuffer::from_str("cafe");
    // Insert an accent e (U+00E9, 2 bytes in UTF-8) replacing the 'e'
    buf.replace(3..4, "\u{00E9}");
    assert_eq!(buf.to_string(), "caf\u{00E9}");
    assert_eq!(buf.len_bytes(), 5); // c(1) a(1) f(1) e-acute(2)
    assert_eq!(buf.len_chars(), 4);
}

#[test]
fn utf8_emoji() {
    let buf = TextBuffer::from_str("hi \u{1F600}!"); // hi [grinning face]!
    // U+1F600 is 4 bytes in UTF-8.
    assert_eq!(buf.len_bytes(), 8); // h(1) i(1) (1) grin(4) !(1)
    assert_eq!(buf.len_chars(), 5);
}

#[test]
fn utf8_cjk() {
    // CJK characters are 3 bytes each in UTF-8.
    let buf = TextBuffer::from_str("\u{4F60}\u{597D}"); // nihao
    assert_eq!(buf.len_bytes(), 6);
    assert_eq!(buf.len_chars(), 2);
}

// =========================================================================
// LineIndex
// =========================================================================

#[test]
fn line_index_single_line() {
    let idx = LineIndex::build_from_str("hello");
    assert_eq!(idx.line_count(), 1);
    assert_eq!(idx.line_start_byte(0), Some(0));
    assert_eq!(idx.line_end_byte(0), Some(5));
}

#[test]
fn line_index_multiple_lines() {
    let idx = LineIndex::build_from_str("aaa\nbbb\nccc\n");
    assert_eq!(idx.line_count(), 4); // last \n introduces a 4th (empty) line
    assert_eq!(idx.line_start_byte(0), Some(0));
    assert_eq!(idx.line_start_byte(1), Some(4));
    assert_eq!(idx.line_start_byte(2), Some(8));
    assert_eq!(idx.line_start_byte(3), Some(12));
}

#[test]
fn line_index_round_trip() {
    let text = "fn main() {\n    println!(\"hi\");\n}\n";
    let idx = LineIndex::build_from_str(text);
    for byte_offset in 0..text.len() {
        if !text.is_char_boundary(byte_offset) {
            continue;
        }
        let lc = idx.byte_to_line_col(byte_offset).unwrap();
        let back = idx.line_col_to_byte(lc).unwrap();
        assert_eq!(back, byte_offset, "round-trip failed at byte {byte_offset}");
    }
}

#[test]
fn line_index_empty() {
    let idx = LineIndex::build_from_str("");
    assert_eq!(idx.line_count(), 1);
    assert_eq!(idx.line_start_byte(0), Some(0));
    assert_eq!(idx.line_end_byte(0), Some(0));
    assert_eq!(
        idx.byte_to_line_col(0),
        Some(LineCol { line: 0, col: 0 })
    );
}

#[test]
fn line_index_out_of_bounds() {
    let idx = LineIndex::build_from_str("abc");
    assert_eq!(idx.line_start_byte(1), None);
    assert_eq!(idx.line_col_to_byte(LineCol { line: 5, col: 0 }), None);
    assert_eq!(idx.byte_to_line_col(100), None);
}

// =========================================================================
// UTF-16 mapping (LSP compat)
// =========================================================================

#[test]
fn utf16_ascii_identity() {
    let idx = LineIndex::build_from_str("hello\nworld");
    // ASCII chars are 1 UTF-16 code unit each, so column == byte col.
    assert_eq!(idx.utf16_col_to_byte(0, 3), Some(3));
    assert_eq!(idx.utf16_col_to_byte(1, 2), Some(8)); // 'r' in "world"
}

#[test]
fn utf16_bmp_char() {
    // U+00E9 (e-acute) is 2 bytes UTF-8 but 1 UTF-16 code unit.
    let idx = LineIndex::build_from_str("caf\u{00E9}!");
    // utf16 col 4 => the '!' which is at byte 5 (c=0, a=1, f=2, e-acute=3..4, !=5)
    assert_eq!(idx.utf16_col_to_byte(0, 4), Some(5));
}

#[test]
fn utf16_supplementary_char() {
    // U+1F600 (grinning face) is 4 bytes UTF-8 and 2 UTF-16 code units.
    let idx = LineIndex::build_from_str("a\u{1F600}b");
    // 'a' => utf16 col 0, len_utf16 = 1
    // grinning => utf16 col 1..2, len_utf16 = 2 (surrogate pair)
    // 'b' => utf16 col 3
    assert_eq!(idx.utf16_col_to_byte(0, 0), Some(0)); // 'a'
    assert_eq!(idx.utf16_col_to_byte(0, 1), Some(1)); // start of emoji
    assert_eq!(idx.utf16_col_to_byte(0, 3), Some(5)); // 'b'
}

#[test]
fn utf16_round_trip() {
    let text = "a\u{00E9}\u{1F600}z";
    let idx = LineIndex::build_from_str(text);
    // Walk every char and ensure byte->utf16->byte round-trips.
    for (byte_idx, _ch) in text.char_indices() {
        let (line, u16col) = idx.byte_to_utf16_col(byte_idx).unwrap();
        let back = idx.utf16_col_to_byte(line, u16col).unwrap();
        assert_eq!(back, byte_idx, "utf16 round-trip failed at byte {byte_idx}");
    }
}

// =========================================================================
// CoordinateMapper
// =========================================================================

#[test]
fn coord_mapper_byte_to_lsp() {
    let text = "a\u{1F600}b\ncd";
    let idx = LineIndex::build_from_str(text);
    let m = CoordinateMapper::new(&idx);

    // 'a' at byte 0 => LSP (0, 0)
    assert_eq!(m.byte_to_lsp(0), Some(LspPosition::new(0, 0)));
    // emoji at byte 1 => LSP (0, 1)
    assert_eq!(m.byte_to_lsp(1), Some(LspPosition::new(0, 1)));
    // 'b' at byte 5 => LSP (0, 3) — emoji took 2 utf16 units
    assert_eq!(m.byte_to_lsp(5), Some(LspPosition::new(0, 3)));
    // 'c' at byte 7 => LSP (1, 0)
    assert_eq!(m.byte_to_lsp(7), Some(LspPosition::new(1, 0)));
    // 'd' at byte 8 => LSP (1, 1)
    assert_eq!(m.byte_to_lsp(8), Some(LspPosition::new(1, 1)));
}

#[test]
fn coord_mapper_lsp_to_byte() {
    let text = "a\u{1F600}b\ncd";
    let idx = LineIndex::build_from_str(text);
    let m = CoordinateMapper::new(&idx);

    assert_eq!(m.lsp_to_byte(LspPosition::new(0, 0)), Some(0));
    assert_eq!(m.lsp_to_byte(LspPosition::new(0, 3)), Some(5));
    assert_eq!(m.lsp_to_byte(LspPosition::new(1, 0)), Some(7));
    assert_eq!(m.lsp_to_byte(LspPosition::new(1, 1)), Some(8));
}

#[test]
fn coord_mapper_line_col_to_lsp_and_back() {
    let text = "hi\u{00E9}!\nok";
    let idx = LineIndex::build_from_str(text);
    let m = CoordinateMapper::new(&idx);

    let lc = LineCol { line: 0, col: 4 }; // '!' at byte 4 (h=0, i=1, e-acute=2..3, !=4)
    let lsp = m.line_col_to_lsp(lc).unwrap();
    assert_eq!(lsp, LspPosition::new(0, 3)); // utf16: h=0, i=1, e-acute=2, !=3
    let back = m.lsp_to_line_col(lsp).unwrap();
    assert_eq!(back, lc);
}

// =========================================================================
// EditHistory — undo / redo
// =========================================================================

#[test]
fn single_undo() {
    let mut buf = TextBuffer::from_str("hello");
    let mut hist = EditHistory::new();
    hist.apply_edit(&mut buf, 5..5, " world");
    assert_eq!(buf.to_string(), "hello world");

    assert!(hist.undo(&mut buf));
    assert_eq!(buf.to_string(), "hello");
}

#[test]
fn single_redo() {
    let mut buf = TextBuffer::from_str("hello");
    let mut hist = EditHistory::new();
    hist.apply_edit(&mut buf, 5..5, " world");
    hist.undo(&mut buf);
    assert!(hist.redo(&mut buf));
    assert_eq!(buf.to_string(), "hello world");
}

#[test]
fn undo_redo_nothing() {
    let mut buf = TextBuffer::from_str("hello");
    let mut hist = EditHistory::new();
    assert!(!hist.undo(&mut buf));
    assert!(!hist.redo(&mut buf));
}

#[test]
fn multiple_undo_redo() {
    let mut buf = TextBuffer::from_str("");
    let mut hist = EditHistory::new();

    hist.apply_edit(&mut buf, 0..0, "a");
    hist.apply_edit(&mut buf, 1..1, "b");
    hist.apply_edit(&mut buf, 2..2, "c");
    assert_eq!(buf.to_string(), "abc");

    hist.undo(&mut buf);
    assert_eq!(buf.to_string(), "ab");
    hist.undo(&mut buf);
    assert_eq!(buf.to_string(), "a");
    hist.undo(&mut buf);
    assert_eq!(buf.to_string(), "");

    hist.redo(&mut buf);
    assert_eq!(buf.to_string(), "a");
    hist.redo(&mut buf);
    assert_eq!(buf.to_string(), "ab");
    hist.redo(&mut buf);
    assert_eq!(buf.to_string(), "abc");
}

#[test]
fn redo_cleared_on_new_edit() {
    let mut buf = TextBuffer::from_str("ab");
    let mut hist = EditHistory::new();

    hist.apply_edit(&mut buf, 2..2, "c");
    hist.undo(&mut buf);
    assert_eq!(hist.redo_depth(), 1);

    // New edit should clear redo stack.
    hist.apply_edit(&mut buf, 2..2, "d");
    assert_eq!(hist.redo_depth(), 0);
    assert_eq!(buf.to_string(), "abd");
}

#[test]
fn undo_replace() {
    let mut buf = TextBuffer::from_str("hello world");
    let mut hist = EditHistory::new();
    hist.apply_edit(&mut buf, 6..11, "earth");
    assert_eq!(buf.to_string(), "hello earth");

    hist.undo(&mut buf);
    assert_eq!(buf.to_string(), "hello world");
}

#[test]
fn undo_delete() {
    let mut buf = TextBuffer::from_str("abcdef");
    let mut hist = EditHistory::new();
    hist.apply_edit(&mut buf, 2..4, ""); // delete "cd"
    assert_eq!(buf.to_string(), "abef");

    hist.undo(&mut buf);
    assert_eq!(buf.to_string(), "abcdef");
}

#[test]
fn transaction_group() {
    let mut buf = TextBuffer::from_str("aaabbbccc");
    let mut hist = EditHistory::new();

    // Build a multi-edit transaction: replace "aaa" with "A" and "ccc" with "C".
    // We must apply from back to front so that byte offsets remain valid.
    let mut txn = EditTransaction::new();
    txn.push(TextEdit::new(6..9, "C", "ccc"));
    txn.push(TextEdit::new(0..3, "A", "aaa"));

    // Apply edits in order: first replaces "ccc" -> "C", then "aaa" -> "A".
    // After first edit: "aaabbbC"    (bytes: 0..7)
    // After second edit: "AbbbC"
    hist.apply(&txn, &mut buf);
    assert_eq!(buf.to_string(), "AbbbC");

    // Undo should revert both in one step.
    hist.undo(&mut buf);
    assert_eq!(buf.to_string(), "aaabbbccc");
}

#[test]
fn history_depth() {
    let mut buf = TextBuffer::from_str("");
    let mut hist = EditHistory::new();
    hist.apply_edit(&mut buf, 0..0, "a");
    hist.apply_edit(&mut buf, 1..1, "b");
    assert_eq!(hist.undo_depth(), 2);
    assert_eq!(hist.redo_depth(), 0);

    hist.undo(&mut buf);
    assert_eq!(hist.undo_depth(), 1);
    assert_eq!(hist.redo_depth(), 1);
}

#[test]
fn history_cap() {
    let mut buf = TextBuffer::from_str("");
    let mut hist = EditHistory::with_max(3);

    for i in 0..5 {
        let s = format!("{}", i);
        let len = buf.len_bytes();
        hist.apply_edit(&mut buf, len..len, &s);
    }
    assert_eq!(buf.to_string(), "01234");
    // Only last 3 should be on the undo stack.
    assert_eq!(hist.undo_depth(), 3);

    hist.undo(&mut buf);
    hist.undo(&mut buf);
    hist.undo(&mut buf);
    assert_eq!(buf.to_string(), "01");
    // Cannot undo further — oldest entries were evicted.
    assert!(!hist.undo(&mut buf));
}

#[test]
fn history_clear() {
    let mut buf = TextBuffer::from_str("");
    let mut hist = EditHistory::new();
    hist.apply_edit(&mut buf, 0..0, "a");
    hist.undo(&mut buf);
    assert_eq!(hist.undo_depth(), 0);
    assert_eq!(hist.redo_depth(), 1);

    hist.clear();
    assert_eq!(hist.undo_depth(), 0);
    assert_eq!(hist.redo_depth(), 0);
}

// =========================================================================
// Large file simulation
// =========================================================================

#[test]
fn large_file_insert_and_query() {
    // Build a ~100 000-line buffer.
    let line = "the quick brown fox jumps over the lazy dog\n";
    let line_count = 100_000;
    let text: String = line.repeat(line_count);
    let buf = TextBuffer::from_str(&text);

    assert_eq!(buf.len_lines(), line_count + 1); // trailing \n adds empty line

    // Spot-check a line in the middle.
    let mid = line_count / 2;
    let l = buf.line(mid);
    assert_eq!(l.to_string(), line);

    // Build index and query.
    let idx = LineIndex::build(buf.text());
    assert_eq!(idx.line_count(), line_count + 1);
    let start = idx.line_start_byte(mid).unwrap();
    let end = idx.line_end_byte(mid).unwrap();
    assert_eq!(end - start, line.len());
}

#[test]
fn large_file_edit_in_middle() {
    let line = "abcdefghij\n";
    let n = 10_000;
    let text: String = line.repeat(n);
    let mut buf = TextBuffer::from_str(&text);

    // Replace a line in the middle.
    let mid_byte = line.len() * (n / 2);
    buf.replace(mid_byte..mid_byte + line.len(), "REPLACED\n");

    let replaced_line = buf.line(n / 2);
    assert_eq!(replaced_line.to_string(), "REPLACED\n");
    // Total lines unchanged.
    assert_eq!(buf.len_lines(), n + 1);
}

// =========================================================================
// Edge cases
// =========================================================================

#[test]
fn buffer_default_is_empty() {
    let buf = TextBuffer::default();
    assert!(buf.is_empty());
}

#[test]
fn insert_into_empty() {
    let mut buf = TextBuffer::new();
    buf.insert(0, "hi");
    assert_eq!(buf.to_string(), "hi");
}

#[test]
fn delete_empty_range_is_noop_content() {
    let mut buf = TextBuffer::from_str("hello");
    buf.delete(2..2);
    assert_eq!(buf.to_string(), "hello");
    // Revision still increments (the operation was issued).
    assert_eq!(buf.revision(), 1);
}

#[test]
fn only_newlines() {
    let buf = TextBuffer::from_str("\n\n\n");
    assert_eq!(buf.len_lines(), 4);
    assert_eq!(buf.line(0).to_string(), "\n");
    assert_eq!(buf.line(3).to_string(), "");
}

#[test]
fn windows_line_endings() {
    let buf = TextBuffer::from_str("a\r\nb\r\n");
    // ropey counts \r\n as one line break (matching most editor conventions).
    assert_eq!(buf.len_lines(), 3);
}

#[test]
fn line_index_windows_crlf() {
    // LineIndex splits only on \n so \r stays as part of the line text.
    let idx = LineIndex::build_from_str("ab\r\ncd\r\n");
    assert_eq!(idx.line_count(), 3);
    assert_eq!(idx.line_start_byte(0), Some(0));
    assert_eq!(idx.line_start_byte(1), Some(4));
    assert_eq!(idx.line_start_byte(2), Some(8));
}
