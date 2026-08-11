//! Mapping between the renderer's owned paragraph seam and the safe sys API.

use fission_layout::{ParagraphCapabilities, ParagraphCapability};
use fission_skia_sys as sys;

use super::output::{
    PackedCaret, PackedCluster, PackedHitRegion, PackedInlineBox, PackedLine,
    PackedParagraphOutput, PackedRect, PackedSize, PackedUnresolvedGlyph, AFFINITY_DOWNSTREAM,
    AFFINITY_UPSTREAM, DIRECTION_LEFT_TO_RIGHT, DIRECTION_RIGHT_TO_LEFT, INDEX_ENCODING_UTF16,
    INDEX_ENCODING_UTF8,
};
use super::request::{
    PackedColor, PackedFontFeature, PackedFontVariation, PackedInlineObject,
    PackedParagraphRequest, PackedParagraphStyle, PackedPreedit, PackedRange, PackedStyleRun,
};
use super::{BatchedParagraphApi, BatchedParagraphError, BatchedParagraphLayout};

pub(super) struct NativeParagraphApi {
    engine: sys::ParagraphEngine,
    capabilities: ParagraphCapabilities,
    native_capability_bits: u64,
}

impl NativeParagraphApi {
    pub(super) fn new() -> Result<Self, BatchedParagraphError> {
        let engine = sys::ParagraphEngine::new().map_err(BatchedParagraphError::native)?;
        let native_capability_bits = engine.capabilities().bits();
        let capabilities = map_capabilities(native_capability_bits)?;
        Ok(Self {
            engine,
            capabilities,
            native_capability_bits,
        })
    }
}

impl BatchedParagraphApi for NativeParagraphApi {
    fn capabilities(&self) -> ParagraphCapabilities {
        self.capabilities
    }

    fn layout(
        &self,
        request: PackedParagraphRequest,
    ) -> Result<BatchedParagraphLayout, BatchedParagraphError> {
        let request = native_request(request)?;
        let retained = self
            .engine
            .layout_retained(&request)
            .map_err(BatchedParagraphError::native)?;
        let output = retained.output;
        if output.capabilities.bits() != self.native_capability_bits {
            return Err(invalid_result(
                "capabilities",
                format!(
                    "layout returned capability bits 0x{:016x}, expected 0x{:016x}",
                    output.capabilities.bits(),
                    self.native_capability_bits
                ),
            ));
        }
        Ok(BatchedParagraphLayout {
            output: packed_output(output)?,
            draw_data: Some(retained.draw_data),
        })
    }
}

fn map_capabilities(bits: u64) -> Result<ParagraphCapabilities, BatchedParagraphError> {
    let mappings = [
        (
            sys::ParagraphCapabilities::BIDIRECTIONAL_TEXT,
            ParagraphCapability::BidirectionalText,
        ),
        (
            sys::ParagraphCapabilities::VARIABLE_FONTS,
            ParagraphCapability::VariableFonts,
        ),
        (
            sys::ParagraphCapabilities::FONT_FEATURES,
            ParagraphCapability::FontFeatures,
        ),
        (
            sys::ParagraphCapabilities::INLINE_OBJECTS,
            ParagraphCapability::InlineObjects,
        ),
        (
            sys::ParagraphCapabilities::CLUSTER_MAPPING,
            ParagraphCapability::ClusterMapping,
        ),
        (
            sys::ParagraphCapabilities::HIT_TESTING,
            ParagraphCapability::HitTesting,
        ),
        (
            sys::ParagraphCapabilities::CARET_GEOMETRY,
            ParagraphCapability::CaretGeometry,
        ),
        (
            sys::ParagraphCapabilities::SELECTION_GEOMETRY,
            ParagraphCapability::SelectionGeometry,
        ),
        (
            sys::ParagraphCapabilities::UNRESOLVED_GLYPHS,
            ParagraphCapability::UnresolvedGlyphDiagnostics,
        ),
    ];
    let known_bits = mappings
        .iter()
        .fold(0_u64, |known, (native, _)| known | *native);
    let unknown_bits = bits & !known_bits;
    if unknown_bits != 0 {
        return Err(BatchedParagraphError::new(
            "capabilities",
            format!("native bridge advertised unknown capability bits 0x{unknown_bits:016x}"),
        ));
    }

    Ok(mappings.into_iter().fold(
        ParagraphCapabilities::NONE,
        |capabilities, (native, fission)| {
            if bits & native != 0 {
                capabilities.with(fission)
            } else {
                capabilities
            }
        },
    ))
}

fn native_request(
    request: PackedParagraphRequest,
) -> Result<sys::ParagraphRequest, BatchedParagraphError> {
    let PackedParagraphRequest {
        text,
        style_runs,
        paragraph_style,
        width_constraint,
        wrap,
        locale,
        inline_objects,
        selection,
        preedit,
        font_catalog_generation,
        fallback_families,
    } = request;
    let text = String::from_utf8(text.into_vec()).map_err(|error| {
        invalid_request(
            "text",
            format!(
                "owned request contains invalid UTF-8 at byte {}",
                error.utf8_error().valid_up_to()
            ),
        )
    })?;

    Ok(sys::ParagraphRequest {
        text,
        style_runs: style_runs
            .into_vec()
            .into_iter()
            .map(native_style_run)
            .collect::<Result<Vec<_>, _>>()?,
        paragraph_style: native_paragraph_style(paragraph_style)?,
        width_constraint,
        wrap,
        locale: locale.map(|value| value.into_string()),
        inline_objects: inline_objects
            .into_vec()
            .into_iter()
            .map(native_inline_object)
            .collect(),
        selection: selection.map(native_range),
        preedit: preedit.map(native_preedit),
        font_catalog_generation,
        fallback_families: fallback_families
            .into_vec()
            .into_iter()
            .map(|value| value.into_string())
            .collect(),
    })
}

fn native_style_run(
    run: PackedStyleRun,
) -> Result<sys::ParagraphTextStyleRun, BatchedParagraphError> {
    let PackedStyleRun {
        range,
        font_size,
        color,
        underline,
        font_family,
        locale,
        font_weight,
        font_style,
        line_height,
        letter_spacing,
        background_color,
        font_width,
        word_spacing,
        variations,
        features,
    } = run;
    let font_slant = match font_style {
        0 => sys::ParagraphFontSlant::Normal,
        1 => sys::ParagraphFontSlant::Italic,
        other => {
            return Err(invalid_request(
                "style_runs.font_style",
                format!("unknown packed font style {other}"),
            ));
        }
    };
    Ok(sys::ParagraphTextStyleRun {
        range: native_range(range),
        font_size,
        color: native_color(color),
        underline,
        font_family: font_family.map(|value| value.into_string()),
        locale: locale.map(|value| value.into_string()),
        font_weight,
        font_slant,
        line_height,
        letter_spacing,
        background_color: background_color.map(native_color),
        font_width,
        word_spacing,
        variations: variations
            .into_vec()
            .into_iter()
            .map(native_variation)
            .collect(),
        features: features
            .into_vec()
            .into_iter()
            .map(native_feature)
            .collect(),
    })
}

fn native_paragraph_style(
    style: PackedParagraphStyle,
) -> Result<sys::ParagraphStyle, BatchedParagraphError> {
    Ok(sys::ParagraphStyle {
        text_align: match style.text_align {
            0 => sys::ParagraphTextAlign::Left,
            1 => sys::ParagraphTextAlign::Right,
            2 => sys::ParagraphTextAlign::Center,
            3 => sys::ParagraphTextAlign::Justify,
            4 => sys::ParagraphTextAlign::Start,
            5 => sys::ParagraphTextAlign::End,
            other => {
                return Err(invalid_request(
                    "paragraph_style.text_align",
                    format!("unknown packed text alignment {other}"),
                ));
            }
        },
        max_lines: style.max_lines,
        overflow: match style.overflow {
            0 => sys::ParagraphOverflow::Clip,
            1 => sys::ParagraphOverflow::Ellipsis,
            2 => sys::ParagraphOverflow::Fade,
            3 => sys::ParagraphOverflow::Visible,
            other => {
                return Err(invalid_request(
                    "paragraph_style.overflow",
                    format!("unknown packed text overflow {other}"),
                ));
            }
        },
        text_direction: match style.text_direction {
            0 => sys::ParagraphTextDirection::Auto,
            1 => sys::ParagraphTextDirection::LeftToRight,
            2 => sys::ParagraphTextDirection::RightToLeft,
            other => {
                return Err(invalid_request(
                    "paragraph_style.text_direction",
                    format!("unknown packed text direction {other}"),
                ));
            }
        },
        text_width_basis: match style.text_width_basis {
            0 => sys::ParagraphTextWidthBasis::Parent,
            1 => sys::ParagraphTextWidthBasis::LongestLine,
            other => {
                return Err(invalid_request(
                    "paragraph_style.text_width_basis",
                    format!("unknown packed width basis {other}"),
                ));
            }
        },
        strut_line_height: style.strut_line_height,
        apply_height_to_first_ascent: style.apply_height_to_first_ascent,
        apply_height_to_last_descent: style.apply_height_to_last_descent,
    })
}

fn native_inline_object(value: PackedInlineObject) -> sys::ParagraphInlineObject {
    sys::ParagraphInlineObject {
        id: value.id,
        range: native_range(value.range),
        width: value.width,
        height: value.height,
        baseline: value.baseline,
    }
}

fn native_preedit(value: PackedPreedit) -> sys::ParagraphPreedit {
    sys::ParagraphPreedit {
        range: native_range(value.range),
        selection: native_range(value.selection),
    }
}

fn native_range(value: PackedRange) -> sys::ParagraphRange {
    sys::ParagraphRange::new(value.start, value.end)
}

fn native_color(value: PackedColor) -> sys::ParagraphColor {
    sys::ParagraphColor::rgba(value.red, value.green, value.blue, value.alpha)
}

fn native_variation(value: PackedFontVariation) -> sys::ParagraphFontVariation {
    sys::ParagraphFontVariation {
        tag: value.tag,
        value: value.value,
    }
}

fn native_feature(value: PackedFontFeature) -> sys::ParagraphFontFeature {
    sys::ParagraphFontFeature {
        tag: value.tag,
        value: value.value,
    }
}

fn packed_output(
    output: sys::ParagraphOutput,
) -> Result<PackedParagraphOutput, BatchedParagraphError> {
    Ok(PackedParagraphOutput {
        index_encoding: match output.index_encoding {
            sys::ffi::INDEX_UTF8 => INDEX_ENCODING_UTF8,
            sys::ffi::INDEX_UTF16 => INDEX_ENCODING_UTF16,
            other => {
                return Err(invalid_result(
                    "index_encoding",
                    format!("unknown native text-index encoding {other}"),
                ));
            }
        },
        size: PackedSize {
            width: output.size.width,
            height: output.size.height,
        },
        min_intrinsic_width: output.min_intrinsic_width,
        max_intrinsic_width: output.max_intrinsic_width,
        first_baseline: output.first_baseline,
        last_baseline: output.last_baseline,
        lines: output
            .lines
            .into_vec()
            .into_iter()
            .map(packed_line)
            .collect::<Result<Vec<_>, _>>()?
            .into(),
        clusters: output
            .clusters
            .into_vec()
            .into_iter()
            .map(packed_cluster)
            .collect::<Result<Vec<_>, _>>()?
            .into(),
        carets: output
            .carets
            .into_vec()
            .into_iter()
            .map(packed_caret)
            .collect::<Result<Vec<_>, _>>()?
            .into(),
        hit_regions: output
            .hit_regions
            .into_vec()
            .into_iter()
            .map(packed_hit_region)
            .collect::<Result<Vec<_>, _>>()?
            .into(),
        inline_boxes: output
            .inline_boxes
            .into_vec()
            .into_iter()
            .map(packed_inline_box)
            .collect::<Result<Vec<_>, _>>()?
            .into(),
        unresolved_glyphs: output
            .unresolved_glyphs
            .into_vec()
            .into_iter()
            .map(packed_unresolved_glyph)
            .collect(),
        unresolved_codepoints: output.unresolved_codepoints,
    })
}

fn packed_line(value: sys::ParagraphLine) -> Result<PackedLine, BatchedParagraphError> {
    Ok(PackedLine {
        range: packed_range(value.range),
        rect: packed_rect(value.rect),
        baseline: value.baseline,
        ascent: value.ascent,
        descent: value.descent,
        leading: value.leading,
        hard_break: packed_bool(value.hard_break, "lines.hard_break")?,
        direction: packed_direction(value.direction, "lines.direction")?,
    })
}

fn packed_cluster(value: sys::ParagraphCluster) -> Result<PackedCluster, BatchedParagraphError> {
    if value.reserved != 0 {
        return Err(reserved_result("clusters.reserved", value.reserved));
    }
    Ok(PackedCluster {
        range: packed_range(value.range),
        rect: packed_rect(value.rect),
        line_index: value.line_index,
        direction: packed_direction(value.direction, "clusters.direction")?,
        starts_grapheme: packed_bool(value.starts_grapheme, "clusters.starts_grapheme")?,
        starts_word: packed_bool(value.starts_word, "clusters.starts_word")?,
    })
}

fn packed_caret(value: sys::ParagraphCaret) -> Result<PackedCaret, BatchedParagraphError> {
    if value.reserved != 0 {
        return Err(reserved_result("carets.reserved", value.reserved));
    }
    Ok(PackedCaret {
        index: value.index,
        affinity: packed_affinity(value.affinity, "carets.affinity")?,
        rect: packed_rect(value.rect),
        line_index: value.line_index,
    })
}

fn packed_hit_region(
    value: sys::ParagraphHitRegion,
) -> Result<PackedHitRegion, BatchedParagraphError> {
    if value.reserved != 0 {
        return Err(reserved_result("hit_regions.reserved", value.reserved));
    }
    Ok(PackedHitRegion {
        rect: packed_rect(value.rect),
        index: value.index,
        affinity: packed_affinity(value.affinity, "hit_regions.affinity")?,
        line_index: value.line_index,
    })
}

fn packed_inline_box(
    value: sys::ParagraphInlineBox,
) -> Result<PackedInlineBox, BatchedParagraphError> {
    if value.reserved != 0 {
        return Err(reserved_result("inline_boxes.reserved", value.reserved));
    }
    Ok(PackedInlineBox {
        id: value.id,
        range: packed_range(value.range),
        rect: packed_rect(value.rect),
        baseline: value.baseline,
    })
}

fn packed_unresolved_glyph(value: sys::UnresolvedGlyph) -> PackedUnresolvedGlyph {
    PackedUnresolvedGlyph {
        range: packed_range(value.range),
        codepoint_start: value.codepoint_start,
        codepoint_count: value.codepoint_count,
    }
}

fn packed_range(value: sys::ffi::TextRange) -> PackedRange {
    PackedRange {
        start: value.start,
        end: value.end,
    }
}

fn packed_rect(value: sys::ParagraphRect) -> PackedRect {
    PackedRect {
        x: value.x,
        y: value.y,
        width: value.width,
        height: value.height,
    }
}

fn packed_direction(value: u32, field: &'static str) -> Result<u8, BatchedParagraphError> {
    match value {
        sys::ffi::DIRECTION_LTR => Ok(DIRECTION_LEFT_TO_RIGHT),
        sys::ffi::DIRECTION_RTL => Ok(DIRECTION_RIGHT_TO_LEFT),
        other => Err(invalid_result(
            field,
            format!("unknown native paragraph direction {other}"),
        )),
    }
}

fn packed_affinity(value: u32, field: &'static str) -> Result<u8, BatchedParagraphError> {
    match value {
        sys::ffi::AFFINITY_UPSTREAM => Ok(AFFINITY_UPSTREAM),
        sys::ffi::AFFINITY_DOWNSTREAM => Ok(AFFINITY_DOWNSTREAM),
        other => Err(invalid_result(
            field,
            format!("unknown native paragraph affinity {other}"),
        )),
    }
}

fn packed_bool(value: u32, field: &'static str) -> Result<bool, BatchedParagraphError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(invalid_result(
            field,
            format!("native bridge returned non-boolean value {other}"),
        )),
    }
}

fn invalid_request(field: &'static str, details: impl Into<String>) -> BatchedParagraphError {
    BatchedParagraphError::new("encode_request", format!("{field}: {}", details.into()))
}

fn invalid_result(field: &'static str, details: impl Into<String>) -> BatchedParagraphError {
    BatchedParagraphError::new("decode_result", format!("{field}: {}", details.into()))
}

fn reserved_result(field: &'static str, value: u32) -> BatchedParagraphError {
    invalid_result(
        field,
        format!("native bridge returned reserved value {value}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_bits_map_one_for_one_and_reject_unknown_values() {
        let all = sys::ParagraphCapabilities::BIDIRECTIONAL_TEXT
            | sys::ParagraphCapabilities::VARIABLE_FONTS
            | sys::ParagraphCapabilities::FONT_FEATURES
            | sys::ParagraphCapabilities::INLINE_OBJECTS
            | sys::ParagraphCapabilities::CLUSTER_MAPPING
            | sys::ParagraphCapabilities::HIT_TESTING
            | sys::ParagraphCapabilities::CARET_GEOMETRY
            | sys::ParagraphCapabilities::SELECTION_GEOMETRY
            | sys::ParagraphCapabilities::UNRESOLVED_GLYPHS;
        let mapped = map_capabilities(all).unwrap();

        assert!(mapped.supports(ParagraphCapability::BidirectionalText));
        assert!(mapped.supports(ParagraphCapability::VariableFonts));
        assert!(mapped.supports(ParagraphCapability::FontFeatures));
        assert!(mapped.supports(ParagraphCapability::InlineObjects));
        assert!(mapped.supports(ParagraphCapability::ClusterMapping));
        assert!(mapped.supports(ParagraphCapability::HitTesting));
        assert!(mapped.supports(ParagraphCapability::CaretGeometry));
        assert!(mapped.supports(ParagraphCapability::SelectionGeometry));
        assert!(mapped.supports(ParagraphCapability::UnresolvedGlyphDiagnostics));
        assert!(map_capabilities(all | (1 << 63)).is_err());
    }

    #[test]
    fn output_scalars_reject_unknown_native_discriminants() {
        assert_eq!(
            packed_direction(sys::ffi::DIRECTION_LTR, "direction").unwrap(),
            DIRECTION_LEFT_TO_RIGHT
        );
        assert_eq!(
            packed_affinity(sys::ffi::AFFINITY_DOWNSTREAM, "affinity").unwrap(),
            AFFINITY_DOWNSTREAM
        );
        assert_eq!(packed_bool(0, "boolean").unwrap(), false);
        assert_eq!(packed_bool(1, "boolean").unwrap(), true);
        assert!(packed_direction(u32::MAX, "direction").is_err());
        assert!(packed_affinity(u32::MAX, "affinity").is_err());
        assert!(packed_bool(2, "boolean").is_err());
    }

    #[test]
    fn owned_request_maps_every_supported_field() {
        let request = PackedParagraphRequest {
            text: Vec::from("A").into_boxed_slice(),
            style_runs: vec![PackedStyleRun {
                range: PackedRange { start: 0, end: 1 },
                font_size: 18.0,
                color: PackedColor {
                    red: 1,
                    green: 2,
                    blue: 3,
                    alpha: 4,
                },
                underline: true,
                font_family: Some("Inter".into()),
                locale: Some("en-GB".into()),
                font_weight: 700,
                font_style: 1,
                line_height: Some(1.4),
                letter_spacing: 0.5,
                background_color: Some(PackedColor {
                    red: 5,
                    green: 6,
                    blue: 7,
                    alpha: 8,
                }),
                font_width: 1.1,
                word_spacing: 2.0,
                variations: vec![PackedFontVariation {
                    tag: u32::from_be_bytes(*b"wght"),
                    value: 650.0,
                }]
                .into(),
                features: vec![PackedFontFeature {
                    tag: u32::from_be_bytes(*b"liga"),
                    value: 0,
                }]
                .into(),
            }]
            .into(),
            paragraph_style: PackedParagraphStyle {
                text_align: 5,
                max_lines: Some(3),
                overflow: 2,
                text_direction: 2,
                text_width_basis: 1,
                strut_line_height: Some(20.0),
                apply_height_to_first_ascent: false,
                apply_height_to_last_descent: true,
            },
            width_constraint: Some(240.0),
            wrap: false,
            locale: Some("ar".into()),
            inline_objects: vec![PackedInlineObject {
                id: 9,
                range: PackedRange { start: 0, end: 1 },
                width: 12.0,
                height: 13.0,
                baseline: 10.0,
            }]
            .into(),
            selection: Some(PackedRange { start: 0, end: 1 }),
            preedit: Some(PackedPreedit {
                range: PackedRange { start: 0, end: 1 },
                selection: PackedRange { start: 1, end: 1 },
            }),
            font_catalog_generation: 17,
            fallback_families: vec![Box::<str>::from("Noto Sans")].into(),
        };

        let mapped = native_request(request).unwrap();
        let style = &mapped.style_runs[0];
        assert_eq!(mapped.text, "A");
        assert_eq!(style.range, sys::ParagraphRange::new(0, 1));
        assert_eq!(style.font_size, 18.0);
        assert_eq!(style.color, sys::ParagraphColor::rgba(1, 2, 3, 4));
        assert!(style.underline);
        assert_eq!(style.font_family.as_deref(), Some("Inter"));
        assert_eq!(style.locale.as_deref(), Some("en-GB"));
        assert_eq!(style.font_weight, 700);
        assert_eq!(style.font_slant, sys::ParagraphFontSlant::Italic);
        assert_eq!(style.line_height, Some(1.4));
        assert_eq!(style.letter_spacing, 0.5);
        assert_eq!(
            style.background_color,
            Some(sys::ParagraphColor::rgba(5, 6, 7, 8))
        );
        assert_eq!(style.font_width, 1.1);
        assert_eq!(style.word_spacing, 2.0);
        assert_eq!(style.variations[0].tag, u32::from_be_bytes(*b"wght"));
        assert_eq!(style.variations[0].value, 650.0);
        assert_eq!(style.features[0].tag, u32::from_be_bytes(*b"liga"));
        assert_eq!(style.features[0].value, 0);
        assert_eq!(
            mapped.paragraph_style.text_align,
            sys::ParagraphTextAlign::End
        );
        assert_eq!(mapped.paragraph_style.max_lines, Some(3));
        assert_eq!(
            mapped.paragraph_style.overflow,
            sys::ParagraphOverflow::Fade
        );
        assert_eq!(
            mapped.paragraph_style.text_direction,
            sys::ParagraphTextDirection::RightToLeft
        );
        assert_eq!(
            mapped.paragraph_style.text_width_basis,
            sys::ParagraphTextWidthBasis::LongestLine
        );
        assert_eq!(mapped.paragraph_style.strut_line_height, Some(20.0));
        assert!(!mapped.paragraph_style.apply_height_to_first_ascent);
        assert!(mapped.paragraph_style.apply_height_to_last_descent);
        assert_eq!(mapped.width_constraint, Some(240.0));
        assert!(!mapped.wrap);
        assert_eq!(mapped.locale.as_deref(), Some("ar"));
        assert_eq!(mapped.inline_objects[0].id, 9);
        assert_eq!(mapped.selection, Some(sys::ParagraphRange::new(0, 1)));
        assert_eq!(
            mapped.preedit,
            Some(sys::ParagraphPreedit {
                range: sys::ParagraphRange::new(0, 1),
                selection: sys::ParagraphRange::new(1, 1),
            })
        );
        assert_eq!(mapped.font_catalog_generation, 17);
        assert_eq!(mapped.fallback_families, ["Noto Sans"]);
    }
}
