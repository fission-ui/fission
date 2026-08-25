//! Canonical, platform-neutral text editing values and edit transactions.
//!
//! All offsets in this module are validated UTF-8 byte offsets. Platform
//! adapters must use the named UTF-16 conversion functions rather than mixing
//! offset units at their boundaries.

use fission_ir::semantics::{InputFormatter, MaxLengthEnforcement};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, sync::Arc};
use unicode_segmentation::UnicodeSegmentation;

/// A validated UTF-8 byte position in a particular text value.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct TextPosition(usize);

impl TextPosition {
    pub const START: Self = Self(0);

    /// Validates a canonical UTF-8 byte offset for `text`.
    pub fn from_utf8(text: &str, offset: usize) -> Result<Self, TextOffsetError> {
        if offset > text.len() {
            return Err(TextOffsetError::out_of_bounds(
                offset,
                text.len(),
                OffsetUnit::Utf8,
            ));
        }
        if !text.is_char_boundary(offset) {
            return Err(TextOffsetError::not_boundary(
                offset,
                text.len(),
                OffsetUnit::Utf8,
            ));
        }
        Ok(Self(offset))
    }

    /// Converts and validates a platform UTF-16 code-unit offset.
    pub fn from_utf16(text: &str, offset: usize) -> Result<Self, TextOffsetError> {
        let utf16_len = text.encode_utf16().count();
        if offset > utf16_len {
            return Err(TextOffsetError::out_of_bounds(
                offset,
                utf16_len,
                OffsetUnit::Utf16,
            ));
        }
        if offset == utf16_len {
            return Ok(Self(text.len()));
        }
        let mut utf16 = 0;
        for (byte, ch) in text.char_indices() {
            if utf16 == offset {
                return Ok(Self(byte));
            }
            utf16 += ch.len_utf16();
            if utf16 > offset {
                return Err(TextOffsetError::not_boundary(
                    offset,
                    utf16_len,
                    OffsetUnit::Utf16,
                ));
            }
        }
        Ok(Self(text.len()))
    }

    /// Converts a Unicode scalar-value index used by accessibility APIs.
    pub fn from_scalar_offset(text: &str, offset: usize) -> Result<Self, TextOffsetError> {
        let scalar_len = text.chars().count();
        if offset > scalar_len {
            return Err(TextOffsetError::out_of_bounds(
                offset,
                scalar_len,
                OffsetUnit::Scalar,
            ));
        }
        Ok(Self(
            text.char_indices()
                .nth(offset)
                .map(|(byte, _)| byte)
                .unwrap_or(text.len()),
        ))
    }

    pub fn at_end(text: &str) -> Self {
        Self(text.len())
    }

    pub const fn utf8_offset(self) -> usize {
        self.0
    }

    pub fn utf16_offset(self, text: &str) -> Result<usize, TextOffsetError> {
        Self::from_utf8(text, self.0)?;
        Ok(text[..self.0].encode_utf16().count())
    }

    pub fn scalar_offset(self, text: &str) -> Result<usize, TextOffsetError> {
        Self::from_utf8(text, self.0)?;
        Ok(text[..self.0].chars().count())
    }

    pub fn is_grapheme_boundary(self, text: &str) -> bool {
        Self::from_utf8(text, self.0).is_ok()
            && (self.0 == text.len() || text.grapheme_indices(true).any(|(i, _)| i == self.0))
    }

    pub fn floor(text: &str, offset: usize) -> Self {
        let mut offset = offset.min(text.len());
        while offset > 0 && !text.is_char_boundary(offset) {
            offset -= 1;
        }
        Self(offset)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OffsetUnit {
    Utf8,
    Utf16,
    Scalar,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextOffsetError {
    offset: usize,
    length: usize,
    unit: OffsetUnit,
    boundary: bool,
}

impl TextOffsetError {
    fn out_of_bounds(offset: usize, length: usize, unit: OffsetUnit) -> Self {
        Self {
            offset,
            length,
            unit,
            boundary: false,
        }
    }

    fn not_boundary(offset: usize, length: usize, unit: OffsetUnit) -> Self {
        Self {
            offset,
            length,
            unit,
            boundary: true,
        }
    }
}

impl fmt::Display for TextOffsetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let unit = match self.unit {
            OffsetUnit::Utf8 => "UTF-8 bytes",
            OffsetUnit::Utf16 => "UTF-16 code units",
            OffsetUnit::Scalar => "Unicode scalar values",
        };
        if self.boundary {
            write!(
                f,
                "text offset {} is not a character boundary in {} (length {})",
                self.offset, unit, self.length
            )
        } else {
            write!(
                f,
                "text offset {} is outside {} length {}",
                self.offset, unit, self.length
            )
        }
    }
}

impl Error for TextOffsetError {}

/// Direction used to resolve an ambiguous position at a soft line break.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextAffinity {
    Upstream,
    #[default]
    Downstream,
}

/// A validated half-open text range.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextRange {
    pub start: TextPosition,
    pub end: TextPosition,
}

impl TextRange {
    pub fn new(text: &str, start: usize, end: usize) -> Result<Self, TextOffsetError> {
        let start = TextPosition::from_utf8(text, start)?;
        let end = TextPosition::from_utf8(text, end)?;
        Ok(Self::from_positions(start, end))
    }

    pub const fn collapsed(at: TextPosition) -> Self {
        Self { start: at, end: at }
    }

    pub fn from_positions(a: TextPosition, b: TextPosition) -> Self {
        Self {
            start: a.min(b),
            end: a.max(b),
        }
    }

    pub fn validate(self, text: &str) -> Result<Self, TextOffsetError> {
        Self::new(text, self.start.0, self.end.0)
    }

    pub const fn is_collapsed(self) -> bool {
        self.start.0 == self.end.0
    }
    pub const fn len_bytes(self) -> usize {
        self.end.0 - self.start.0
    }
}

/// A directional selection. `base` is the fixed anchor and `extent` is the caret.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextSelection {
    pub base: TextPosition,
    pub extent: TextPosition,
    pub affinity: TextAffinity,
}

impl TextSelection {
    pub const fn collapsed(at: TextPosition) -> Self {
        Self {
            base: at,
            extent: at,
            affinity: TextAffinity::Downstream,
        }
    }

    pub fn new(
        text: &str,
        base: usize,
        extent: usize,
        affinity: TextAffinity,
    ) -> Result<Self, TextOffsetError> {
        Ok(Self {
            base: TextPosition::from_utf8(text, base)?,
            extent: TextPosition::from_utf8(text, extent)?,
            affinity,
        })
    }

    pub fn validate(self, text: &str) -> Result<Self, TextOffsetError> {
        Self::new(text, self.base.0, self.extent.0, self.affinity)
    }

    pub fn range(self) -> TextRange {
        TextRange::from_positions(self.base, self.extent)
    }
    pub const fn is_collapsed(self) -> bool {
        self.base.0 == self.extent.0
    }
}

/// The sole value required to synchronize an editable text session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEditingValue {
    pub text: String,
    pub selection: TextSelection,
    pub composing: Option<TextRange>,
}

impl Default for TextEditingValue {
    fn default() -> Self {
        Self::empty()
    }
}

impl TextEditingValue {
    pub fn empty() -> Self {
        Self {
            text: String::new(),
            selection: TextSelection::collapsed(TextPosition::START),
            composing: None,
        }
    }

    pub fn from_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let end = TextPosition::at_end(&text);
        Self {
            text,
            selection: TextSelection::collapsed(end),
            composing: None,
        }
    }

    pub fn new(
        text: impl Into<String>,
        selection: TextSelection,
        composing: Option<TextRange>,
    ) -> Result<Self, TextOffsetError> {
        let text = text.into();
        let selection = selection.validate(&text)?;
        let composing = composing.map(|range| range.validate(&text)).transpose()?;
        Ok(Self {
            text,
            selection,
            composing,
        })
    }

    pub fn validate(&self) -> Result<(), TextOffsetError> {
        self.selection.validate(&self.text)?;
        if let Some(range) = self.composing {
            range.validate(&self.text)?;
        }
        Ok(())
    }

    pub fn selection_range(&self) -> TextRange {
        self.selection.range()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextEditSource {
    #[default]
    Programmatic,
    Keyboard,
    Pointer,
    Ime,
    Clipboard,
    Accessibility,
    Autocorrect,
    Autofill,
    Handwriting,
    Dictation,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextEditPhase {
    #[default]
    Committed,
    Selection,
    CompositionStarted,
    CompositionUpdated,
    CompositionCancelled,
    CompositionCommitted,
    Submitted,
    EditingCompleted,
    Focused,
    Blurred,
    Validated,
    TapOutside,
}

/// A complete proposed edit. Every mutation source is normalized to this model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextEditCommand {
    Replace {
        range: TextRange,
        text: String,
        source: TextEditSource,
    },
    SetSelection {
        selection: TextSelection,
        source: TextEditSource,
    },
    SetComposing {
        range: TextRange,
    },
    CancelComposition,
    CommitComposition {
        text: String,
        source: TextEditSource,
    },
    SetValue {
        value: TextEditingValue,
        source: TextEditSource,
        phase: TextValuePhase,
    },
    Delete {
        direction: TextEditDirection,
        boundary: TextEditBoundary,
        source: TextEditSource,
    },
    MoveSelection {
        direction: TextEditDirection,
        boundary: TextEditBoundary,
        extend: bool,
        source: TextEditSource,
    },
    Submit,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextEditDirection {
    Backward,
    Forward,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextEditBoundary {
    Grapheme,
    Word,
    Line,
    Paragraph,
    Document,
}

/// Lifecycle meaning of a complete value supplied by a platform adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextValuePhase {
    #[default]
    Committed,
    CompositionStarted,
    CompositionUpdated,
    CompositionCommitted,
}

impl From<TextValuePhase> for TextEditPhase {
    fn from(value: TextValuePhase) -> Self {
        match value {
            TextValuePhase::Committed => Self::Committed,
            TextValuePhase::CompositionStarted => Self::CompositionStarted,
            TextValuePhase::CompositionUpdated => Self::CompositionUpdated,
            TextValuePhase::CompositionCommitted => Self::CompositionCommitted,
        }
    }
}

impl TextEditCommand {
    pub fn source(&self) -> TextEditSource {
        match self {
            Self::Replace { source, .. }
            | Self::CommitComposition { source, .. }
            | Self::SetValue { source, .. } => *source,
            Self::Delete { source, .. } | Self::MoveSelection { source, .. } => *source,
            Self::SetSelection { source, .. } => *source,
            Self::SetComposing { .. } | Self::CancelComposition => TextEditSource::Ime,
            Self::Submit | Self::Complete => TextEditSource::Keyboard,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEditResult {
    pub old_value: TextEditingValue,
    pub new_value: TextEditingValue,
    pub source: TextEditSource,
    pub phase: TextEditPhase,
}

/// Application formatter extension point. Formatters receive complete values,
/// including selection and composing state, and must return another valid value.
pub trait CompleteTextInputFormatter: Send + Sync + fmt::Debug {
    fn format(
        &self,
        old_value: &TextEditingValue,
        proposed_value: TextEditingValue,
    ) -> TextEditingValue;
}

pub type SharedTextInputFormatter = Arc<dyn CompleteTextInputFormatter>;

#[derive(Clone, Default)]
pub struct TextEditPipeline {
    pub formatters: Vec<InputFormatter>,
    pub custom_formatters: Vec<SharedTextInputFormatter>,
    pub max_length: Option<usize>,
    pub max_length_enforcement: MaxLengthEnforcement,
}

impl TextEditPipeline {
    pub fn apply(
        &self,
        old: &TextEditingValue,
        command: TextEditCommand,
    ) -> Result<TextEditResult, TextOffsetError> {
        old.validate()?;
        let source = command.source();
        let (mut proposed, phase) = apply_command(old, command)?;
        if matches!(
            phase,
            TextEditPhase::Committed | TextEditPhase::CompositionCommitted
        ) {
            for formatter in &self.formatters {
                proposed = apply_builtin_formatter(formatter, proposed);
            }
            for formatter in &self.custom_formatters {
                proposed = formatter.format(old, proposed);
                proposed.validate()?;
            }
        }
        let mutates_text = matches!(
            phase,
            TextEditPhase::Committed
                | TextEditPhase::CompositionStarted
                | TextEditPhase::CompositionUpdated
                | TextEditPhase::CompositionCommitted
        );
        let enforce_length = mutates_text
            && match self.max_length_enforcement {
                MaxLengthEnforcement::None => false,
                MaxLengthEnforcement::Enforced => true,
                MaxLengthEnforcement::AfterComposition => !matches!(
                    phase,
                    TextEditPhase::CompositionStarted | TextEditPhase::CompositionUpdated
                ),
            };
        if enforce_length {
            if let Some(max) = self.max_length {
                proposed = enforce_grapheme_limit(proposed, max);
            }
        }
        proposed.validate()?;
        Ok(TextEditResult {
            old_value: old.clone(),
            new_value: proposed,
            source,
            phase,
        })
    }
}

fn apply_command(
    old: &TextEditingValue,
    command: TextEditCommand,
) -> Result<(TextEditingValue, TextEditPhase), TextOffsetError> {
    match command {
        TextEditCommand::Replace { range, text, .. } => {
            let range = range.validate(&old.text)?;
            let start = range.start.0;
            let mut next = old.text.clone();
            next.replace_range(start..range.end.0, &text);
            let caret = TextPosition::from_utf8(&next, start + text.len())?;
            Ok((
                TextEditingValue {
                    text: next,
                    selection: TextSelection::collapsed(caret),
                    composing: None,
                },
                TextEditPhase::Committed,
            ))
        }
        TextEditCommand::SetSelection { selection, .. } => {
            let selection = selection.validate(&old.text)?;
            Ok((
                TextEditingValue {
                    selection,
                    ..old.clone()
                },
                TextEditPhase::Selection,
            ))
        }
        TextEditCommand::SetComposing { range } => {
            let range = range.validate(&old.text)?;
            let phase = if old.composing.is_some() {
                TextEditPhase::CompositionUpdated
            } else {
                TextEditPhase::CompositionStarted
            };
            Ok((
                TextEditingValue {
                    composing: Some(range),
                    ..old.clone()
                },
                phase,
            ))
        }
        TextEditCommand::CancelComposition => Ok((
            TextEditingValue {
                composing: None,
                ..old.clone()
            },
            TextEditPhase::CompositionCancelled,
        )),
        TextEditCommand::CommitComposition { text, .. } => {
            let range = old.composing.unwrap_or_else(|| old.selection_range());
            let (mut value, _) = apply_command(
                old,
                TextEditCommand::Replace {
                    range,
                    text,
                    source: TextEditSource::Ime,
                },
            )?;
            value.composing = None;
            Ok((value, TextEditPhase::CompositionCommitted))
        }
        TextEditCommand::SetValue { value, phase, .. } => {
            value.validate()?;
            Ok((value, phase.into()))
        }
        TextEditCommand::Delete {
            direction,
            boundary,
            source,
        } => {
            let selected = old.selection_range();
            let range = if !selected.is_collapsed() {
                selected
            } else {
                deletion_range(old, direction, boundary)
            };
            apply_command(
                old,
                TextEditCommand::Replace {
                    range,
                    text: String::new(),
                    source,
                },
            )
        }
        TextEditCommand::MoveSelection {
            direction,
            boundary,
            extend,
            ..
        } => {
            let target = movement_position(old, direction, boundary);
            let selection = if extend {
                TextSelection {
                    base: old.selection.base,
                    extent: target,
                    affinity: old.selection.affinity,
                }
            } else {
                TextSelection::collapsed(target)
            };
            Ok((
                TextEditingValue {
                    selection,
                    ..old.clone()
                },
                TextEditPhase::Selection,
            ))
        }
        TextEditCommand::Submit => Ok((old.clone(), TextEditPhase::Submitted)),
        TextEditCommand::Complete => Ok((old.clone(), TextEditPhase::EditingCompleted)),
    }
}

fn deletion_range(
    value: &TextEditingValue,
    direction: TextEditDirection,
    boundary: TextEditBoundary,
) -> TextRange {
    let caret = value.selection.extent.0;
    let target = boundary_position(&value.text, caret, direction, boundary);
    TextRange::from_positions(TextPosition(caret), TextPosition(target))
}

fn movement_position(
    value: &TextEditingValue,
    direction: TextEditDirection,
    boundary: TextEditBoundary,
) -> TextPosition {
    TextPosition(boundary_position(
        &value.text,
        value.selection.extent.0,
        direction,
        boundary,
    ))
}

fn boundary_position(
    text: &str,
    caret: usize,
    direction: TextEditDirection,
    boundary: TextEditBoundary,
) -> usize {
    match (direction, boundary) {
        (TextEditDirection::Backward, TextEditBoundary::Grapheme) => text[..caret]
            .grapheme_indices(true)
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0),
        (TextEditDirection::Forward, TextEditBoundary::Grapheme) => text[caret..]
            .grapheme_indices(true)
            .nth(1)
            .map(|(i, _)| caret + i)
            .unwrap_or(text.len()),
        (TextEditDirection::Backward, TextEditBoundary::Word) => text[..caret]
            .split_word_bound_indices()
            .filter(|(_, part)| part.chars().any(char::is_alphanumeric))
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0),
        (TextEditDirection::Forward, TextEditBoundary::Word) => text[caret..]
            .split_word_bound_indices()
            .find(|(_, part)| part.chars().any(char::is_alphanumeric))
            .map(|(i, part)| caret + i + part.len())
            .unwrap_or(text.len()),
        (TextEditDirection::Backward, TextEditBoundary::Line | TextEditBoundary::Paragraph) => {
            text[..caret].rfind('\n').map(|i| i + 1).unwrap_or(0)
        }
        (TextEditDirection::Forward, TextEditBoundary::Line | TextEditBoundary::Paragraph) => text
            [caret..]
            .find('\n')
            .map(|i| caret + i)
            .unwrap_or(text.len()),
        (TextEditDirection::Backward, TextEditBoundary::Document) => 0,
        (TextEditDirection::Forward, TextEditBoundary::Document) => text.len(),
    }
}

fn apply_builtin_formatter(
    formatter: &InputFormatter,
    value: TextEditingValue,
) -> TextEditingValue {
    let transform = |input: &str| -> String {
        match formatter {
            InputFormatter::DigitsOnly => input.chars().filter(|ch| ch.is_ascii_digit()).collect(),
            InputFormatter::AsciiOnly => input.chars().filter(|ch| ch.is_ascii()).collect(),
            InputFormatter::InternalLowercase => input.to_lowercase(),
            InputFormatter::Uppercase => input.to_uppercase(),
            InputFormatter::TrimWhitespace => input.trim().to_string(),
            InputFormatter::SingleLine => input.replace(['\r', '\n'], ""),
        }
    };
    let map = |position: TextPosition| TextPosition(transform(&value.text[..position.0]).len());
    let selection = TextSelection {
        base: map(value.selection.base),
        extent: map(value.selection.extent),
        affinity: value.selection.affinity,
    };
    let composing = value
        .composing
        .map(|range| TextRange::from_positions(map(range.start), map(range.end)));
    TextEditingValue {
        text: transform(&value.text),
        selection,
        composing,
    }
}

fn enforce_grapheme_limit(mut value: TextEditingValue, max: usize) -> TextEditingValue {
    let end = value
        .text
        .grapheme_indices(true)
        .nth(max)
        .map(|(offset, _)| offset)
        .unwrap_or(value.text.len());
    if end == value.text.len() {
        return value;
    }
    value.text.truncate(end);
    let clamp = |position: TextPosition| TextPosition(position.0.min(end));
    value.selection.base = clamp(value.selection.base);
    value.selection.extent = clamp(value.selection.extent);
    value.composing = value
        .composing
        .map(|range| TextRange::from_positions(clamp(range.start), clamp(range.end)));
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_conversion_rejects_surrogate_interior_and_round_trips() {
        let text = "a😀e\u{301}";
        assert!(TextPosition::from_utf16(text, 2).is_err());
        for offset in [0, 1, 5, 6, text.len()] {
            let position = TextPosition::from_utf8(text, offset).unwrap();
            assert_eq!(
                TextPosition::from_utf16(text, position.utf16_offset(text).unwrap()).unwrap(),
                position
            );
        }
    }

    #[test]
    fn scalar_conversion_handles_non_bmp_and_combining_scalars() {
        let text = "a😀e\u{301}";
        let offsets = [0, 1, 5, 6, text.len()];
        for (scalar, byte) in offsets.into_iter().enumerate() {
            let position = TextPosition::from_scalar_offset(text, scalar).unwrap();
            assert_eq!(position.utf8_offset(), byte);
            assert_eq!(position.scalar_offset(text).unwrap(), scalar);
        }
        assert!(TextPosition::from_scalar_offset(text, 5).is_err());
    }

    #[test]
    fn directional_selection_has_ordered_range() {
        let selection = TextSelection::new("hello", 5, 1, TextAffinity::Upstream).unwrap();
        assert_eq!(selection.base.utf8_offset(), 5);
        assert_eq!(selection.extent.utf8_offset(), 1);
        assert_eq!(selection.range(), TextRange::new("hello", 1, 5).unwrap());
    }

    #[test]
    fn replacement_is_atomic_and_preserves_source() {
        let old = TextEditingValue::from_text("hello world");
        let result = TextEditPipeline::default()
            .apply(
                &old,
                TextEditCommand::Replace {
                    range: TextRange::new(&old.text, 6, 11).unwrap(),
                    text: "Fission".into(),
                    source: TextEditSource::Accessibility,
                },
            )
            .unwrap();
        assert_eq!(result.old_value, old);
        assert_eq!(result.new_value.text, "hello Fission");
        assert_eq!(result.new_value.selection.extent.utf8_offset(), 13);
        assert_eq!(result.source, TextEditSource::Accessibility);
    }

    #[test]
    fn max_length_counts_user_perceived_graphemes() {
        let old = TextEditingValue::empty();
        let pipeline = TextEditPipeline {
            max_length: Some(2),
            max_length_enforcement: MaxLengthEnforcement::Enforced,
            ..Default::default()
        };
        let result = pipeline
            .apply(
                &old,
                TextEditCommand::Replace {
                    range: TextRange::collapsed(TextPosition::START),
                    text: "e\u{301}👨‍👩‍👧‍👦x".into(),
                    source: TextEditSource::Keyboard,
                },
            )
            .unwrap();
        assert_eq!(result.new_value.text, "e\u{301}👨‍👩‍👧‍👦");
        assert_eq!(
            result.new_value.selection.extent.utf8_offset(),
            result.new_value.text.len()
        );
    }

    #[test]
    fn formatters_transform_complete_value_and_selection() {
        let old = TextEditingValue::empty();
        let pipeline = TextEditPipeline {
            formatters: vec![InputFormatter::DigitsOnly],
            ..Default::default()
        };
        let result = pipeline
            .apply(
                &old,
                TextEditCommand::Replace {
                    range: TextRange::collapsed(TextPosition::START),
                    text: "a1b2".into(),
                    source: TextEditSource::Keyboard,
                },
            )
            .unwrap();
        assert_eq!(result.new_value.text, "12");
        assert_eq!(result.new_value.selection.extent.utf8_offset(), 2);
    }

    #[derive(Debug)]
    struct PrefixFormatter;
    impl CompleteTextInputFormatter for PrefixFormatter {
        fn format(
            &self,
            _old: &TextEditingValue,
            mut proposed: TextEditingValue,
        ) -> TextEditingValue {
            proposed.text.insert(0, '#');
            let end = TextPosition::at_end(&proposed.text);
            proposed.selection = TextSelection::collapsed(end);
            proposed
        }
    }

    #[test]
    fn application_formatter_receives_complete_value() {
        let pipeline = TextEditPipeline {
            custom_formatters: vec![Arc::new(PrefixFormatter)],
            ..Default::default()
        };
        let result = pipeline
            .apply(
                &TextEditingValue::empty(),
                TextEditCommand::Replace {
                    range: TextRange::collapsed(TextPosition::START),
                    text: "tag".into(),
                    source: TextEditSource::Keyboard,
                },
            )
            .unwrap();
        assert_eq!(result.new_value.text, "#tag");
        assert_eq!(result.new_value.selection.extent.utf8_offset(), 4);
    }

    #[test]
    fn active_composition_is_not_truncated_until_commit() {
        let old = TextEditingValue::new(
            "abcd",
            TextSelection::collapsed(TextPosition::at_end("abcd")),
            Some(TextRange::new("abcd", 2, 4).unwrap()),
        )
        .unwrap();
        let pipeline = TextEditPipeline {
            max_length: Some(2),
            max_length_enforcement: MaxLengthEnforcement::AfterComposition,
            ..Default::default()
        };
        let result = pipeline
            .apply(
                &old,
                TextEditCommand::CommitComposition {
                    text: "😀".into(),
                    source: TextEditSource::Ime,
                },
            )
            .unwrap();
        assert_eq!(result.new_value.text, "ab");
        assert!(result.new_value.composing.is_none());
    }

    #[test]
    fn complete_platform_values_report_composition_lifecycle() {
        let pipeline = TextEditPipeline::default();
        let composing = TextEditingValue::new(
            "世",
            TextSelection::collapsed(TextPosition::at_end("世")),
            Some(TextRange::new("世", 0, "世".len()).unwrap()),
        )
        .unwrap();
        let started = pipeline
            .apply(
                &TextEditingValue::empty(),
                TextEditCommand::SetValue {
                    value: composing.clone(),
                    source: TextEditSource::Ime,
                    phase: TextValuePhase::CompositionStarted,
                },
            )
            .unwrap();
        assert_eq!(started.phase, TextEditPhase::CompositionStarted);

        let committed = pipeline
            .apply(
                &composing,
                TextEditCommand::SetValue {
                    value: TextEditingValue::from_text("世"),
                    source: TextEditSource::Ime,
                    phase: TextValuePhase::CompositionCommitted,
                },
            )
            .unwrap();
        assert_eq!(committed.phase, TextEditPhase::CompositionCommitted);
    }

    #[test]
    fn delete_and_move_commands_share_unicode_boundaries() {
        let value = TextEditingValue::from_text("a👨‍👩‍👧‍👦 café");
        let pipeline = TextEditPipeline::default();
        let deleted = pipeline
            .apply(
                &value,
                TextEditCommand::Delete {
                    direction: TextEditDirection::Backward,
                    boundary: TextEditBoundary::Word,
                    source: TextEditSource::Keyboard,
                },
            )
            .unwrap();
        assert_eq!(deleted.new_value.text, "a👨‍👩‍👧‍👦 ");
        let moved = pipeline
            .apply(
                &TextEditingValue::from_text("a👨‍👩‍👧‍👦"),
                TextEditCommand::MoveSelection {
                    direction: TextEditDirection::Backward,
                    boundary: TextEditBoundary::Grapheme,
                    extend: false,
                    source: TextEditSource::Keyboard,
                },
            )
            .unwrap();
        assert_eq!(moved.new_value.selection.extent.utf8_offset(), 1);
    }
}
