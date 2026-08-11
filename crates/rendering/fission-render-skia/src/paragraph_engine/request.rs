use fission_ir::op::{FontStyle, TextAlign, TextDirection, TextOverflow, TextWidthBasis};
use fission_layout::{ParagraphDescription, ParagraphError, Utf8Range};

/// The owned, backend-facing representation of one normalized paragraph.
///
/// This deliberately contains no borrowed Rust data and no native handles. A
/// future `fission-skia-sys` implementation can flatten it into one batched C
/// request without extending any lifetimes across the ABI boundary.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PackedParagraphRequest {
    pub(super) text: Box<[u8]>,
    pub(super) style_runs: Box<[PackedStyleRun]>,
    pub(super) paragraph_style: PackedParagraphStyle,
    pub(super) width_constraint: Option<f32>,
    pub(super) wrap: bool,
    pub(super) locale: Option<Box<str>>,
    pub(super) inline_objects: Box<[PackedInlineObject]>,
    pub(super) selection: Option<PackedRange>,
    pub(super) preedit: Option<PackedPreedit>,
    pub(super) font_catalog_generation: u64,
    pub(super) fallback_families: Box<[Box<str>]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PackedRange {
    pub(super) start: u64,
    pub(super) end: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PackedColor {
    pub(super) red: u8,
    pub(super) green: u8,
    pub(super) blue: u8,
    pub(super) alpha: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PackedStyleRun {
    pub(super) range: PackedRange,
    pub(super) font_size: f32,
    pub(super) color: PackedColor,
    pub(super) underline: bool,
    pub(super) font_family: Option<Box<str>>,
    pub(super) locale: Option<Box<str>>,
    pub(super) font_weight: u16,
    pub(super) font_style: u8,
    pub(super) line_height: Option<f32>,
    pub(super) letter_spacing: f32,
    pub(super) background_color: Option<PackedColor>,
    pub(super) font_width: f32,
    pub(super) word_spacing: f32,
    pub(super) variations: Box<[PackedFontVariation]>,
    pub(super) features: Box<[PackedFontFeature]>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PackedFontVariation {
    pub(super) tag: u32,
    pub(super) value: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PackedFontFeature {
    pub(super) tag: u32,
    pub(super) value: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PackedParagraphStyle {
    pub(super) text_align: u8,
    pub(super) max_lines: Option<u64>,
    pub(super) overflow: u8,
    pub(super) text_direction: u8,
    pub(super) text_width_basis: u8,
    pub(super) strut_line_height: Option<f32>,
    pub(super) apply_height_to_first_ascent: bool,
    pub(super) apply_height_to_last_descent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PackedInlineObject {
    pub(super) id: u64,
    pub(super) range: PackedRange,
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) baseline: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PackedPreedit {
    pub(super) range: PackedRange,
    pub(super) selection: PackedRange,
}

impl PackedParagraphRequest {
    pub(super) fn from_description(
        description: &ParagraphDescription,
    ) -> Result<Self, ParagraphError> {
        description.validate()?;

        let style_runs = description
            .style_runs
            .iter()
            .map(|run| {
                Ok(PackedStyleRun {
                    range: packed_range(run.range)?,
                    font_size: run.style.font_size,
                    color: PackedColor {
                        red: run.style.color.r,
                        green: run.style.color.g,
                        blue: run.style.color.b,
                        alpha: run.style.color.a,
                    },
                    underline: run.style.underline,
                    font_family: run.style.font_family.clone().map(String::into_boxed_str),
                    locale: run.style.locale.clone().map(String::into_boxed_str),
                    font_weight: run.style.font_weight,
                    font_style: match run.style.font_style {
                        FontStyle::Normal => 0,
                        FontStyle::Italic => 1,
                    },
                    line_height: run.style.line_height,
                    letter_spacing: run.style.letter_spacing,
                    background_color: run.style.background_color.map(|color| PackedColor {
                        red: color.r,
                        green: color.g,
                        blue: color.b,
                        alpha: color.a,
                    }),
                    font_width: run.font_width,
                    word_spacing: run.word_spacing,
                    variations: run
                        .variations
                        .iter()
                        .map(|variation| PackedFontVariation {
                            tag: variation.tag,
                            value: variation.value,
                        })
                        .collect(),
                    features: run
                        .features
                        .iter()
                        .map(|feature| PackedFontFeature {
                            tag: feature.tag,
                            value: feature.value,
                        })
                        .collect(),
                })
            })
            .collect::<Result<Vec<_>, ParagraphError>>()?;

        let paragraph = description.paragraph_style;
        let paragraph_style = PackedParagraphStyle {
            text_align: match paragraph.text_align {
                TextAlign::Left => 0,
                TextAlign::Right => 1,
                TextAlign::Center => 2,
                TextAlign::Justify => 3,
                TextAlign::Start => 4,
                TextAlign::End => 5,
            },
            max_lines: paragraph
                .max_lines
                .map(|value| checked_u64(value, "paragraph_style.max_lines"))
                .transpose()?,
            overflow: match paragraph.overflow {
                TextOverflow::Clip => 0,
                TextOverflow::Ellipsis => 1,
                TextOverflow::Fade => 2,
                TextOverflow::Visible => 3,
            },
            text_direction: match paragraph.text_direction {
                TextDirection::Auto => 0,
                TextDirection::Ltr => 1,
                TextDirection::Rtl => 2,
            },
            text_width_basis: match paragraph.text_width_basis {
                TextWidthBasis::Parent => 0,
                TextWidthBasis::LongestLine => 1,
            },
            strut_line_height: paragraph.strut_line_height,
            apply_height_to_first_ascent: paragraph
                .text_height_behavior
                .apply_height_to_first_ascent,
            apply_height_to_last_descent: paragraph
                .text_height_behavior
                .apply_height_to_last_descent,
        };

        let inline_objects = description
            .inline_objects
            .iter()
            .map(|inline| {
                Ok(PackedInlineObject {
                    id: inline.id,
                    range: packed_range(inline.range)?,
                    width: inline.size.width,
                    height: inline.size.height,
                    baseline: inline.baseline,
                })
            })
            .collect::<Result<Vec<_>, ParagraphError>>()?;

        Ok(Self {
            text: description.text.as_bytes().into(),
            style_runs: style_runs.into(),
            paragraph_style,
            width_constraint: description.width_constraint,
            wrap: description.wrap,
            locale: description.locale.clone().map(String::into_boxed_str),
            inline_objects: inline_objects.into(),
            selection: description.selection.map(packed_range).transpose()?,
            preedit: description
                .preedit
                .map(|preedit| -> Result<PackedPreedit, ParagraphError> {
                    Ok(PackedPreedit {
                        range: packed_range(preedit.range)?,
                        selection: packed_range(preedit.selection)?,
                    })
                })
                .transpose()?,
            font_catalog_generation: description.font_catalog_generation,
            fallback_families: description
                .fallback_families
                .iter()
                .cloned()
                .map(String::into_boxed_str)
                .collect(),
        })
    }
}

fn packed_range(range: Utf8Range) -> Result<PackedRange, ParagraphError> {
    Ok(PackedRange {
        start: checked_u64(range.start().byte_offset(), "range.start")?,
        end: checked_u64(range.end().byte_offset(), "range.end")?,
    })
}

fn checked_u64(value: usize, field: &'static str) -> Result<u64, ParagraphError> {
    u64::try_from(value).map_err(|_| {
        ParagraphError::invalid_description(field, "value does not fit the native u64 contract")
    })
}
