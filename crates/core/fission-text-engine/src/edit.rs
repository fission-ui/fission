//! Edit primitives, transactions, and undo/redo history.

use crate::buffer::TextBuffer;
use std::ops::Range;

// ── Single edit ─────────────────────────────────────────────────────────────

/// A single atomic text edit expressed as a byte-range replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// Byte range in the document **before** this edit is applied.
    pub range: Range<usize>,
    /// The text that replaces `range`.
    pub new_text: String,
    /// The text that was present in `range` before the edit (stored so we can
    /// invert the operation for undo).
    pub old_text: String,
}

impl TextEdit {
    /// Create a new edit.
    pub fn new(range: Range<usize>, new_text: impl Into<String>, old_text: impl Into<String>) -> Self {
        Self {
            range,
            new_text: new_text.into(),
            old_text: old_text.into(),
        }
    }

    /// Return the inverse edit that undoes this one.
    ///
    /// The inverse replaces the `new_text` (at the position it was inserted)
    /// with the original `old_text`.
    pub fn inverse(&self) -> TextEdit {
        TextEdit {
            range: self.range.start..(self.range.start + self.new_text.len()),
            new_text: self.old_text.clone(),
            old_text: self.new_text.clone(),
        }
    }
}

// ── Transaction ─────────────────────────────────────────────────────────────

/// A group of [`TextEdit`]s that should be undone / redone as a unit.
///
/// Edits within a transaction are stored in **application order** (first edit
/// first).  When inverting for undo, the list is reversed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditTransaction {
    pub edits: Vec<TextEdit>,
}

impl EditTransaction {
    /// Create an empty transaction.
    pub fn new() -> Self {
        Self { edits: Vec::new() }
    }

    /// Push an edit onto the transaction.
    pub fn push(&mut self, edit: TextEdit) {
        self.edits.push(edit);
    }

    /// Return the inverse transaction (for undo).
    pub fn inverse(&self) -> EditTransaction {
        let edits: Vec<TextEdit> = self.edits.iter().rev().map(|e| e.inverse()).collect();
        EditTransaction { edits }
    }

    /// `true` when the transaction contains no edits.
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }
}

impl Default for EditTransaction {
    fn default() -> Self {
        Self::new()
    }
}

// ── History ─────────────────────────────────────────────────────────────────

/// Bounded undo / redo history.
///
/// The maximum number of entries on each stack is configurable via
/// [`EditHistory::with_max`].  When the undo stack exceeds the limit the
/// oldest entry is discarded (FIFO eviction).
#[derive(Debug, Clone)]
pub struct EditHistory {
    undo_stack: Vec<EditTransaction>,
    redo_stack: Vec<EditTransaction>,
    max_entries: usize,
}

/// Default maximum undo depth.
const DEFAULT_MAX_ENTRIES: usize = 1000;

impl EditHistory {
    /// Create a history with the default max depth (1 000 entries).
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_entries: DEFAULT_MAX_ENTRIES,
        }
    }

    /// Create a history with a custom maximum depth.
    pub fn with_max(max_entries: usize) -> Self {
        assert!(max_entries > 0, "max_entries must be > 0");
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_entries,
        }
    }

    /// Apply a pre-built [`EditTransaction`] to `buffer`, record it in the
    /// undo stack, and clear the redo stack (any previously undone work is
    /// forked away).
    pub fn apply(&mut self, txn: &EditTransaction, buffer: &mut TextBuffer) {
        for edit in &txn.edits {
            buffer.replace(edit.range.clone(), &edit.new_text);
        }
        self.undo_stack.push(txn.clone());
        if self.undo_stack.len() > self.max_entries {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Convenience: build a single-edit transaction, apply it, and record it.
    ///
    /// `old_text` is captured automatically from the buffer.
    pub fn apply_edit(
        &mut self,
        buffer: &mut TextBuffer,
        range: Range<usize>,
        new_text: &str,
    ) {
        let old_text = buffer.slice(range.clone()).to_string();
        let edit = TextEdit::new(range, new_text, old_text);
        let mut txn = EditTransaction::new();
        txn.push(edit);
        self.apply(&txn, buffer);
    }

    /// Undo the most recent transaction, returning `true` if an undo was
    /// performed.
    pub fn undo(&mut self, buffer: &mut TextBuffer) -> bool {
        let txn = match self.undo_stack.pop() {
            Some(t) => t,
            None => return false,
        };
        let inv = txn.inverse();
        for edit in &inv.edits {
            buffer.replace(edit.range.clone(), &edit.new_text);
        }
        self.redo_stack.push(txn);
        true
    }

    /// Redo the most recently undone transaction, returning `true` if a redo
    /// was performed.
    pub fn redo(&mut self, buffer: &mut TextBuffer) -> bool {
        let txn = match self.redo_stack.pop() {
            Some(t) => t,
            None => return false,
        };
        for edit in &txn.edits {
            buffer.replace(edit.range.clone(), &edit.new_text);
        }
        self.undo_stack.push(txn);
        true
    }

    /// Record a pre-built [`EditTransaction`] in the undo stack **without**
    /// applying it to the buffer.  This is useful when the caller has already
    /// mutated the buffer (or is about to) and just needs the history entry.
    ///
    /// Clears the redo stack and evicts the oldest entry when the stack
    /// exceeds `max_entries`.
    pub fn record(&mut self, txn: EditTransaction) {
        self.undo_stack.push(txn);
        if self.undo_stack.len() > self.max_entries {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Pop the most recent transaction from the undo stack.
    ///
    /// Returns `None` if the undo stack is empty.  The caller is responsible
    /// for applying the inverse and pushing a matching entry onto the redo
    /// stack via [`push_redo`](Self::push_redo).
    pub fn pop_undo(&mut self) -> Option<EditTransaction> {
        self.undo_stack.pop()
    }

    /// Pop the most recent transaction from the redo stack.
    ///
    /// Returns `None` if the redo stack is empty.  The caller is responsible
    /// for re-applying the edits and pushing a matching entry onto the undo
    /// stack via [`push_undo`](Self::push_undo).
    pub fn pop_redo(&mut self) -> Option<EditTransaction> {
        self.redo_stack.pop()
    }

    /// Push a transaction onto the redo stack (used by external undo logic).
    pub fn push_redo(&mut self, txn: EditTransaction) {
        self.redo_stack.push(txn);
    }

    /// Push a transaction onto the undo stack without clearing the redo stack
    /// (used by external redo logic that manually manages both stacks).
    pub fn push_undo(&mut self, txn: EditTransaction) {
        self.undo_stack.push(txn);
        if self.undo_stack.len() > self.max_entries {
            self.undo_stack.remove(0);
        }
    }

    /// Number of entries currently on the undo stack.
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    /// Number of entries currently on the redo stack.
    pub fn redo_depth(&self) -> usize {
        self.redo_stack.len()
    }

    /// Maximum entries per stack.
    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    /// Discard all history.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl Default for EditHistory {
    fn default() -> Self {
        Self::new()
    }
}
