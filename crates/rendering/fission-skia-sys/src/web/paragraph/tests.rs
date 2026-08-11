use crate::ffi;
use crate::{
    ParagraphCapabilities, ParagraphCaret, ParagraphCluster, ParagraphColor, ParagraphFontFeature,
    ParagraphFontVariation, ParagraphHitRegion, ParagraphInlineBox, ParagraphInlineObject,
    ParagraphLine, ParagraphOutput, ParagraphPreedit, ParagraphRange, ParagraphRect,
    ParagraphRequest, ParagraphSize, ParagraphTextStyleRun, UnresolvedGlyph,
};

use super::*;
use crate::web::ResourceHandle;

fn request() -> WebParagraphRequest {
    let text = "Fission \u{fffc} مرحبا";
    let mut style = ParagraphTextStyleRun::new(
        ParagraphRange::new(0, text.len() as u64),
        18.0,
        ParagraphColor::rgba(10, 20, 30, 255),
    );
    style.font_family = Some("Inter".into());
    style.locale = Some("en-GB".into());
    style.underline = true;
    style.font_weight = 600;
    style.line_height = Some(1.25);
    style.letter_spacing = 0.5;
    style.background_color = Some(ParagraphColor::rgba(1, 2, 3, 4));
    style.font_width = 0.9;
    style.word_spacing = 1.5;
    style.variations = vec![ParagraphFontVariation {
        tag: u32::from_be_bytes(*b"wght"),
        value: 600.0,
    }];
    style.features = vec![ParagraphFontFeature {
        tag: u32::from_be_bytes(*b"liga"),
        value: 1,
    }];
    let mut paragraph = ParagraphRequest::new(text, vec![style]);
    paragraph.width_constraint = Some(320.0);
    paragraph.locale = Some("en-GB".into());
    paragraph.selection = Some(ParagraphRange::new(0, 7));
    paragraph.preedit = Some(ParagraphPreedit {
        range: ParagraphRange::new(12, 14),
        selection: ParagraphRange::new(12, 14),
    });
    paragraph.inline_objects = vec![ParagraphInlineObject {
        id: 9,
        range: ParagraphRange::new(8, 11),
        width: 12.0,
        height: 10.0,
        baseline: 8.0,
    }];
    paragraph.font_catalog_generation = 4;
    paragraph.fallback_families = vec!["Noto Sans".into()];
    WebParagraphRequest {
        paragraph,
        fonts: vec![WebParagraphFont {
            handle: ResourceHandle {
                slot: 3,
                generation: 2,
            },
            family: "Inter".into(),
        }],
    }
}

fn response() -> WebParagraphResponse {
    let rect = ParagraphRect {
        x: 0.0,
        y: 0.0,
        width: 120.0,
        height: 24.0,
    };
    WebParagraphResponse {
        handle: ResourceHandle {
            slot: 0x8000_0001,
            generation: 1,
        },
        approximate_bytes: 4096,
        output: ParagraphOutput {
            index_encoding: ffi::INDEX_UTF16,
            capabilities: ParagraphCapabilities::from_known_bits(ffi::PARAGRAPH_ALL_CAPABILITIES)
                .unwrap(),
            size: ParagraphSize {
                width: 120.0,
                height: 24.0,
            },
            min_intrinsic_width: 60.0,
            max_intrinsic_width: 180.0,
            first_baseline: Some(18.0),
            last_baseline: Some(18.0),
            lines: vec![ParagraphLine {
                range: ffi::TextRange { start: 0, end: 15 },
                rect,
                baseline: 18.0,
                ascent: 16.0,
                descent: 4.0,
                leading: 4.0,
                hard_break: 0,
                direction: ffi::DIRECTION_LTR,
            }]
            .into_boxed_slice(),
            clusters: vec![ParagraphCluster {
                range: ffi::TextRange { start: 0, end: 7 },
                rect: ParagraphRect {
                    width: 50.0,
                    ..rect
                },
                line_index: 0,
                direction: ffi::DIRECTION_LTR,
                starts_grapheme: 1,
                starts_word: 1,
                reserved: 0,
            }]
            .into_boxed_slice(),
            carets: vec![ParagraphCaret {
                index: 0,
                affinity: ffi::AFFINITY_DOWNSTREAM,
                reserved: 0,
                rect: ParagraphRect { width: 1.0, ..rect },
                line_index: 0,
            }]
            .into_boxed_slice(),
            hit_regions: vec![ParagraphHitRegion {
                rect,
                index: 0,
                affinity: ffi::AFFINITY_DOWNSTREAM,
                reserved: 0,
                line_index: 0,
            }]
            .into_boxed_slice(),
            inline_boxes: vec![ParagraphInlineBox {
                id: 9,
                range: ffi::TextRange { start: 8, end: 9 },
                rect: ParagraphRect {
                    x: 50.0,
                    width: 12.0,
                    height: 10.0,
                    ..rect
                },
                baseline: 8.0,
                reserved: 0,
            }]
            .into_boxed_slice(),
            unresolved_glyphs: vec![UnresolvedGlyph {
                range: ffi::TextRange { start: 10, end: 15 },
                codepoint_start: 0,
                codepoint_count: 1,
            }]
            .into_boxed_slice(),
            unresolved_codepoints: vec![0x0645].into_boxed_slice(),
        },
    }
}

#[test]
fn paragraph_request_round_trips_canonically() {
    let value = request();
    let encoded = encode_paragraph_request(&value).unwrap();
    let decoded = decode_paragraph_request(&encoded).unwrap();
    assert_eq!(decoded, value);
    assert_eq!(encode_paragraph_request(&decoded).unwrap(), encoded);
}

#[test]
fn paragraph_response_round_trips_canonically() {
    let value = response();
    let encoded = encode_paragraph_response(&value).unwrap();
    let decoded = decode_paragraph_response(&encoded).unwrap();
    assert_eq!(decoded, value);
    assert_eq!(encode_paragraph_response(&decoded).unwrap(), encoded);
}

#[test]
fn malformed_headers_and_noncanonical_options_are_rejected() {
    let mut reserved = encode_paragraph_request(&request()).unwrap();
    reserved[6] = 1;
    assert_eq!(
        decode_paragraph_request(&reserved),
        Err(ParagraphWireError::NonZeroReserved)
    );

    let mut absent_width_with_payload = encode_paragraph_request(&request()).unwrap();
    let flags = u32::from_le_bytes(absent_width_with_payload[12..16].try_into().unwrap());
    absent_width_with_payload[12..16].copy_from_slice(&(flags & !(1 << 1)).to_le_bytes());
    assert_eq!(
        decode_paragraph_request(&absent_width_with_payload),
        Err(ParagraphWireError::InvalidValue(
            "paragraph width constraint"
        ))
    );

    let mut wrong_length = encode_paragraph_response(&response()).unwrap();
    wrong_length[8..12].copy_from_slice(&128_u32.to_le_bytes());
    assert_eq!(
        decode_paragraph_response(&wrong_length),
        Err(ParagraphWireError::LengthMismatch)
    );
}

#[test]
fn invalid_ranges_handles_and_codepoint_spans_fail_before_host_use() {
    let mut bad_request = request();
    bad_request.paragraph.style_runs[0].range = ParagraphRange::new(13, 14);
    assert_eq!(
        encode_paragraph_request(&bad_request),
        Err(ParagraphWireError::InvalidValue("style range"))
    );

    let mut bad_handle = request();
    bad_handle.fonts[0].handle.slot = 0;
    assert_eq!(
        encode_paragraph_request(&bad_handle),
        Err(ParagraphWireError::InvalidValue("paragraph font handle"))
    );

    let mut bad_response = response();
    bad_response.output.unresolved_glyphs[0].codepoint_count = 2;
    assert_eq!(
        encode_paragraph_response(&bad_response),
        Err(ParagraphWireError::InvalidValue(
            "paragraph unresolved glyph codepoint span"
        ))
    );
}

#[test]
fn normalized_paragraph_contract_is_enforced_on_the_wire() {
    let mut style_gap = request();
    style_gap.paragraph.style_runs[0].range.start = 1;
    assert_eq!(
        encode_paragraph_request(&style_gap),
        Err(ParagraphWireError::InvalidValue("style range coverage"))
    );

    let mut invalid_preedit = request();
    invalid_preedit
        .paragraph
        .preedit
        .as_mut()
        .unwrap()
        .selection = ParagraphRange::new(0, 7);
    assert_eq!(
        encode_paragraph_request(&invalid_preedit),
        Err(ParagraphWireError::InvalidValue(
            "paragraph preedit selection"
        ))
    );

    let mut invalid_inline = request();
    invalid_inline.paragraph.inline_objects[0].range = ParagraphRange::new(6, 7);
    assert_eq!(
        encode_paragraph_request(&invalid_inline),
        Err(ParagraphWireError::InvalidValue("inline object range"))
    );
}
