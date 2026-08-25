use crate::op::{Color, Fill, LayoutUnit};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

/// Slant applied while resolving a font face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
}

/// Lines that decorate a run of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct TextDecorationLines {
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub overline: bool,
    #[serde(default)]
    pub line_through: bool,
}

/// Stroke pattern used for text decorations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TextDecorationStyle {
    #[default]
    Solid,
    Double,
    Dotted,
    Dashed,
    Wavy,
}

/// Decoration styling shared by underline, overline, and line-through.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TextDecoration {
    #[serde(default)]
    pub lines: TextDecorationLines,
    #[serde(default)]
    pub color: Option<Color>,
    #[serde(default)]
    pub style: TextDecorationStyle,
    #[serde(default)]
    pub thickness: Option<LayoutUnit>,
}

impl Hash for TextDecoration {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.lines.hash(state);
        self.color.hash(state);
        self.style.hash(state);
        self.thickness.map(f32::to_bits).hash(state);
    }
}

/// A shadow painted from the shaped glyph outline.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TextShadow {
    pub color: Color,
    pub offset: (LayoutUnit, LayoutUnit),
    pub blur_radius: LayoutUnit,
}

impl Hash for TextShadow {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.color.hash(state);
        self.offset.0.to_bits().hash(state);
        self.offset.1.to_bits().hash(state);
        self.blur_radius.to_bits().hash(state);
    }
}

/// An OpenType feature setting, such as `liga=0` or `ss01=1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FontFeature {
    pub tag: String,
    pub value: u32,
}

/// A variable-font axis setting, such as `wght=525`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontVariation {
    pub axis: String,
    pub value: f32,
}

impl Hash for FontVariation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.axis.hash(state);
        self.value.to_bits().hash(state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TextBaseline {
    #[default]
    Alphabetic,
    Ideographic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TextLeadingDistribution {
    #[default]
    Proportional,
    Even,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TextLineBreakPolicy {
    #[default]
    Auto,
    Normal,
    Strict,
    Loose,
    Anywhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TextHyphenation {
    #[default]
    None,
    Auto,
}

/// Typography that is not part of Fission's original compact text style.
///
/// Keeping these properties together lets the IR grow without introducing an
/// "advanced text" render path. All text widgets and renderers continue to use
/// the same [`TextStyle`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TextTypography {
    #[serde(default)]
    pub font_fallback: Vec<String>,
    #[serde(default)]
    pub word_spacing: LayoutUnit,
    #[serde(default)]
    pub decoration: TextDecoration,
    #[serde(default)]
    pub shadows: Vec<TextShadow>,
    #[serde(default)]
    pub foreground: Option<Fill>,
    #[serde(default)]
    pub background: Option<Fill>,
    #[serde(default)]
    pub font_features: Vec<FontFeature>,
    #[serde(default)]
    pub font_variations: Vec<FontVariation>,
    #[serde(default)]
    pub baseline: TextBaseline,
    #[serde(default)]
    pub leading_distribution: TextLeadingDistribution,
    #[serde(default)]
    pub line_break: TextLineBreakPolicy,
    #[serde(default)]
    pub hyphenation: TextHyphenation,
}

impl Hash for TextTypography {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.font_fallback.hash(state);
        self.word_spacing.to_bits().hash(state);
        self.decoration.hash(state);
        self.shadows.hash(state);
        self.foreground.hash(state);
        self.background.hash(state);
        self.font_features.hash(state);
        self.font_variations.hash(state);
        self.baseline.hash(state);
        self.leading_distribution.hash(state);
        self.line_break.hash(state);
        self.hyphenation.hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextStyle {
    pub font_size: LayoutUnit,
    pub color: Color,
    /// Compatibility alias for `typography.decoration.lines.underline`.
    pub underline: bool,
    #[serde(default)]
    pub font_family: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default = "text_weight_default")]
    pub font_weight: u16,
    #[serde(default)]
    pub font_style: FontStyle,
    #[serde(default)]
    pub line_height: Option<LayoutUnit>,
    #[serde(default)]
    pub letter_spacing: LayoutUnit,
    /// Optional legacy flat background highlight color for this run.
    #[serde(default)]
    pub background_color: Option<Color>,
    #[serde(default)]
    pub typography: TextTypography,
}

impl Hash for TextStyle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.font_size.to_bits().hash(state);
        self.color.hash(state);
        self.underline.hash(state);
        self.font_family.hash(state);
        self.locale.hash(state);
        self.font_weight.hash(state);
        self.font_style.hash(state);
        self.line_height.map(f32::to_bits).hash(state);
        self.letter_spacing.to_bits().hash(state);
        self.background_color.hash(state);
        self.typography.hash(state);
    }
}

const fn text_weight_default() -> u16 {
    400
}
