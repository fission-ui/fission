//! Text paragraph styles and the compact legacy paragraph codec.

use super::LayoutUnit;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub enum TextAlign {
    Left,
    Right,
    Center,
    Justify,
    #[default]
    Start,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub enum TextOverflow {
    Clip,
    Ellipsis,
    Fade,
    #[default]
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub enum TextDirection {
    #[default]
    Auto,
    Ltr,
    Rtl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub enum TextWidthBasis {
    #[default]
    Parent,
    LongestLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub enum MouseCursor {
    #[default]
    Basic,
    Pointer,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct TextHeightBehavior {
    pub apply_height_to_first_ascent: bool,
    pub apply_height_to_last_descent: bool,
}

impl Default for TextHeightBehavior {
    fn default() -> Self {
        Self {
            apply_height_to_first_ascent: true,
            apply_height_to_last_descent: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct TextParagraphStyle {
    pub text_align: TextAlign,
    pub max_lines: Option<usize>,
    pub overflow: TextOverflow,
    #[serde(default)]
    pub text_direction: TextDirection,
    #[serde(default)]
    pub text_width_basis: TextWidthBasis,
    #[serde(default)]
    pub strut_line_height: Option<LayoutUnit>,
    #[serde(default)]
    pub text_height_behavior: TextHeightBehavior,
}

/// Paragraph-local alignment and indentation.
///
/// Indentation is a logical leading inset: left for left-to-right paragraphs
/// and right for right-to-left paragraphs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TextParagraphLayoutStyle {
    #[serde(default)]
    pub indent: LayoutUnit,
    #[serde(default)]
    pub text_align: TextAlign,
    #[serde(default)]
    pub text_direction: TextDirection,
}

impl Default for TextParagraphLayoutStyle {
    fn default() -> Self {
        Self {
            indent: 0.0,
            text_align: TextAlign::Start,
            text_direction: TextDirection::Auto,
        }
    }
}

/// Paragraph-local layout style at a UTF-8 byte offset in a rich-text value.
/// A span applies from its offset until the next span; offsets must be at
/// grapheme boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TextParagraphStyleSpan {
    #[serde(default)]
    pub text_offset: usize,
    #[serde(flatten)]
    pub style: TextParagraphLayoutStyle,
}

impl PartialEq for TextParagraphStyle {
    fn eq(&self, other: &Self) -> bool {
        self.text_align == other.text_align
            && self.max_lines == other.max_lines
            && self.overflow == other.overflow
            && self.text_direction == other.text_direction
            && self.text_width_basis == other.text_width_basis
            && self.strut_line_height.map(f32::to_bits) == other.strut_line_height.map(f32::to_bits)
            && self.text_height_behavior == other.text_height_behavior
    }
}

impl Eq for TextParagraphStyle {}

impl std::hash::Hash for TextParagraphStyle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.text_align.hash(state);
        self.max_lines.hash(state);
        self.overflow.hash(state);
        self.text_direction.hash(state);
        self.text_width_basis.hash(state);
        self.strut_line_height.map(f32::to_bits).hash(state);
        self.text_height_behavior.hash(state);
    }
}

const TEXT_PARAGRAPH_ALIGN_BITS: u32 = 0b111;
const TEXT_PARAGRAPH_OVERFLOW_BITS: u32 = 0b111 << 3;
const TEXT_PARAGRAPH_MAX_LINES_SHIFT: u32 = 6;
const TEXT_PARAGRAPH_SENTINEL: u32 = 1;
pub(crate) const TEXT_PARAGRAPH_MAX_ENCODED_LINES: usize =
    ((1 << 24) - 1) >> TEXT_PARAGRAPH_MAX_LINES_SHIFT;

const fn text_align_code(align: TextAlign) -> u32 {
    match align {
        TextAlign::Start => 0,
        TextAlign::Left => 1,
        TextAlign::Center => 2,
        TextAlign::Right => 3,
        TextAlign::End => 4,
        TextAlign::Justify => 5,
    }
}

const fn text_overflow_code(overflow: TextOverflow) -> u32 {
    match overflow {
        TextOverflow::Visible => 0,
        TextOverflow::Clip => 1,
        TextOverflow::Ellipsis => 2,
        TextOverflow::Fade => 3,
    }
}

const fn decode_text_align(code: u32) -> TextAlign {
    match code {
        1 => TextAlign::Left,
        2 => TextAlign::Center,
        3 => TextAlign::Right,
        4 => TextAlign::End,
        5 => TextAlign::Justify,
        _ => TextAlign::Start,
    }
}

const fn decode_text_overflow(code: u32) -> TextOverflow {
    match code {
        1 => TextOverflow::Clip,
        2 => TextOverflow::Ellipsis,
        3 => TextOverflow::Fade,
        _ => TextOverflow::Visible,
    }
}

pub fn encode_text_paragraph_style(style: TextParagraphStyle) -> Option<LayoutUnit> {
    if style == TextParagraphStyle::default() {
        return None;
    }
    if style.text_direction != TextDirection::Auto
        || style.text_width_basis != TextWidthBasis::Parent
        || style.strut_line_height.is_some()
        || style.text_height_behavior != TextHeightBehavior::default()
    {
        return None;
    }

    let max_lines = style
        .max_lines
        .unwrap_or(0)
        .min(TEXT_PARAGRAPH_MAX_ENCODED_LINES) as u32;
    let encoded = TEXT_PARAGRAPH_SENTINEL
        + text_align_code(style.text_align)
        + (text_overflow_code(style.overflow) << 3)
        + (max_lines << TEXT_PARAGRAPH_MAX_LINES_SHIFT);

    Some(-(encoded as LayoutUnit))
}

pub fn decode_text_paragraph_style(
    encoded_width: Option<LayoutUnit>,
) -> Option<TextParagraphStyle> {
    let encoded_width = encoded_width?;
    if !encoded_width.is_finite() || encoded_width >= 0.0 {
        return None;
    }

    let raw = (-encoded_width).round();
    if raw < TEXT_PARAGRAPH_SENTINEL as f32 {
        return None;
    }

    let bits = raw as u32 - TEXT_PARAGRAPH_SENTINEL;
    let text_align = decode_text_align(bits & TEXT_PARAGRAPH_ALIGN_BITS);
    let overflow = decode_text_overflow((bits & TEXT_PARAGRAPH_OVERFLOW_BITS) >> 3);
    let max_lines = match bits >> TEXT_PARAGRAPH_MAX_LINES_SHIFT {
        0 => None,
        lines => Some(lines as usize),
    };

    Some(TextParagraphStyle {
        text_align,
        max_lines,
        overflow,
        text_direction: TextDirection::Auto,
        text_width_basis: TextWidthBasis::Parent,
        strut_line_height: None,
        text_height_behavior: TextHeightBehavior::default(),
    })
}
