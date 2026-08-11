use std::sync::{Arc, Mutex};

use fission_ir::op::{Color, TextParagraphStyle, TextStyle};
use fission_layout::{
    LayoutPoint, LayoutSize, ParagraphAffinity, ParagraphDescription, ParagraphEngine,
    ParagraphError, ParagraphInlineObject, ParagraphStyleRun, Utf8Index, Utf8Range,
};

use super::output::{
    PackedCaret, PackedCluster, PackedHitRegion, PackedInlineBox, PackedLine,
    PackedParagraphOutput, PackedRect, PackedSize, PackedUnresolvedGlyph, AFFINITY_DOWNSTREAM,
    DIRECTION_LEFT_TO_RIGHT, INDEX_ENCODING_UTF16, INDEX_ENCODING_UTF8,
};
use super::request::{PackedParagraphRequest, PackedRange};
use super::{
    BatchedParagraphApi, BatchedParagraphError, SkiaParagraphEngine,
    COMPLETE_PARAGRAPH_CAPABILITIES,
};

struct MockApi {
    output: PackedParagraphOutput,
    requests: Arc<Mutex<Vec<PackedParagraphRequest>>>,
}

impl BatchedParagraphApi for MockApi {
    fn capabilities(&self) -> fission_layout::ParagraphCapabilities {
        COMPLETE_PARAGRAPH_CAPABILITIES
    }

    fn layout(
        &self,
        request: PackedParagraphRequest,
    ) -> Result<PackedParagraphOutput, BatchedParagraphError> {
        self.requests.lock().unwrap().push(request);
        Ok(self.output.clone())
    }
}

fn engine(
    output: PackedParagraphOutput,
) -> (SkiaParagraphEngine, Arc<Mutex<Vec<PackedParagraphRequest>>>) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    (
        SkiaParagraphEngine::with_api(MockApi {
            output,
            requests: Arc::clone(&requests),
        }),
        requests,
    )
}

fn range(start: usize, end: usize) -> Utf8Range {
    Utf8Range::from_byte_offsets(start, end).unwrap()
}

fn packed_range(start: u64, end: u64) -> PackedRange {
    PackedRange { start, end }
}

fn description(text: &str) -> ParagraphDescription {
    ParagraphDescription::new(
        text,
        vec![ParagraphStyleRun::new(
            range(0, text.len()),
            TextStyle {
                font_size: 16.0,
                color: Color::BLACK,
                underline: false,
                font_family: Some("Inter".into()),
                locale: Some("en-GB".into()),
                font_weight: 400,
                font_style: Default::default(),
                line_height: None,
                letter_spacing: 0.0,
                background_color: None,
            },
        )],
        TextParagraphStyle::default(),
        Some(300.0),
    )
}

fn rect(x: f32, width: f32) -> PackedRect {
    PackedRect {
        x,
        y: 0.0,
        width,
        height: 20.0,
    }
}

fn base_output(index_encoding: u8, text_end: u64) -> PackedParagraphOutput {
    PackedParagraphOutput {
        index_encoding,
        size: PackedSize {
            width: 30.0,
            height: 20.0,
        },
        min_intrinsic_width: 10.0,
        max_intrinsic_width: 30.0,
        first_baseline: Some(15.0),
        last_baseline: Some(15.0),
        lines: vec![PackedLine {
            range: packed_range(0, text_end),
            rect: rect(0.0, 30.0),
            baseline: 15.0,
            ascent: 12.0,
            descent: 4.0,
            leading: 4.0,
            hard_break: false,
            direction: DIRECTION_LEFT_TO_RIGHT,
        }]
        .into(),
        clusters: Box::new([]),
        carets: vec![PackedCaret {
            index: 0,
            affinity: AFFINITY_DOWNSTREAM,
            rect: PackedRect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 20.0,
            },
            line_index: 0,
        }]
        .into(),
        hit_regions: vec![PackedHitRegion {
            rect: rect(0.0, 30.0),
            index: 0,
            affinity: AFFINITY_DOWNSTREAM,
            line_index: 0,
        }]
        .into(),
        inline_boxes: Box::new([]),
        unresolved_glyphs: Box::new([]),
        unresolved_codepoints: Box::new([]),
    }
}

#[test]
fn one_owned_result_drives_geometry_queries_and_diagnostics() {
    let text = "a\u{fffc}b";
    let mut description = description(text);
    description.selection = Some(range(0, text.len()));
    description.inline_objects.push(ParagraphInlineObject {
        id: 42,
        range: range(1, 4),
        size: LayoutSize::new(10.0, 20.0),
        baseline: 15.0,
    });

    let mut output = base_output(INDEX_ENCODING_UTF8, text.len() as u64);
    output.clusters = vec![
        PackedCluster {
            range: packed_range(0, 1),
            rect: rect(0.0, 10.0),
            line_index: 0,
            direction: DIRECTION_LEFT_TO_RIGHT,
            starts_grapheme: true,
            starts_word: true,
        },
        PackedCluster {
            range: packed_range(4, 5),
            rect: rect(20.0, 10.0),
            line_index: 0,
            direction: DIRECTION_LEFT_TO_RIGHT,
            starts_grapheme: true,
            starts_word: true,
        },
    ]
    .into();
    output.carets = [0, 1, 4, 5]
        .into_iter()
        .map(|index| PackedCaret {
            index,
            affinity: AFFINITY_DOWNSTREAM,
            rect: PackedRect {
                x: index as f32 * 5.0,
                y: 0.0,
                width: 1.0,
                height: 20.0,
            },
            line_index: 0,
        })
        .collect();
    output.inline_boxes = vec![PackedInlineBox {
        id: 42,
        range: packed_range(1, 4),
        rect: rect(10.0, 10.0),
        baseline: 15.0,
    }]
    .into();
    output.unresolved_codepoints = vec!['b' as u32].into();
    output.unresolved_glyphs = vec![PackedUnresolvedGlyph {
        range: packed_range(4, 5),
        codepoint_start: 0,
        codepoint_count: 1,
    }]
    .into();

    let (engine, requests) = engine(output);
    let result = engine.layout(&description).unwrap();

    assert_eq!(result.geometry().lines().len(), 1);
    assert_eq!(result.geometry().clusters().len(), 2);
    assert_eq!(result.geometry().carets().len(), 4);
    assert_eq!(result.geometry().hit_regions().len(), 1);
    assert_eq!(result.geometry().inline_boxes().len(), 1);
    assert_eq!(
        result.selection_boxes(range(0, text.len())).unwrap().len(),
        3
    );
    assert_eq!(
        result
            .caret(Utf8Index::new(4), ParagraphAffinity::Downstream)
            .unwrap()
            .unwrap()
            .index
            .byte_offset(),
        4
    );
    assert_eq!(
        result.hit_test(LayoutPoint::new(2.0, 2.0)).unwrap().index,
        Utf8Index::new(0)
    );
    assert_eq!(result.unresolved_glyphs()[0].codepoints, vec!['b' as u32]);
    assert_eq!(requests.lock().unwrap()[0].text.as_ref(), text.as_bytes());
    assert!(result.draw_data().is_none());
}

#[test]
fn utf16_native_indices_are_normalized_to_source_utf8_boundaries() {
    let text = "A😀e\u{301}Z";
    let mut output = base_output(INDEX_ENCODING_UTF16, 6);
    output.clusters = [(0, 1), (1, 3), (3, 5), (5, 6)]
        .into_iter()
        .enumerate()
        .map(|(position, (start, end))| PackedCluster {
            range: packed_range(start, end),
            rect: rect(position as f32 * 7.0, 7.0),
            line_index: 0,
            direction: DIRECTION_LEFT_TO_RIGHT,
            starts_grapheme: true,
            starts_word: position == 0,
        })
        .collect();
    output.carets = [0, 1, 3, 5, 6]
        .into_iter()
        .map(|index| PackedCaret {
            index,
            affinity: AFFINITY_DOWNSTREAM,
            rect: PackedRect {
                x: index as f32,
                y: 0.0,
                width: 1.0,
                height: 20.0,
            },
            line_index: 0,
        })
        .collect();
    output.unresolved_codepoints = vec!['😀' as u32].into();
    output.unresolved_glyphs = vec![PackedUnresolvedGlyph {
        range: packed_range(1, 3),
        codepoint_start: 0,
        codepoint_count: 1,
    }]
    .into();

    let (engine, _) = engine(output);
    let result = engine.layout(&description(text)).unwrap();
    let ranges = result
        .geometry()
        .clusters()
        .iter()
        .map(|cluster| {
            (
                cluster.range.start().byte_offset(),
                cluster.range.end().byte_offset(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(ranges, vec![(0, 1), (1, 5), (5, 8), (8, 9)]);
    assert_eq!(result.unresolved_glyphs()[0].range, range(1, 5));
    assert!(result
        .caret(Utf8Index::new(8), ParagraphAffinity::Downstream)
        .unwrap()
        .is_some());
}

#[test]
fn native_offset_inside_utf8_scalar_is_rejected() {
    let text = "😀";
    let mut output = base_output(INDEX_ENCODING_UTF8, text.len() as u64);
    output.clusters = vec![PackedCluster {
        range: packed_range(0, 2),
        rect: rect(0.0, 10.0),
        line_index: 0,
        direction: DIRECTION_LEFT_TO_RIGHT,
        starts_grapheme: true,
        starts_word: true,
    }]
    .into();

    let (engine, _) = engine(output);
    assert!(matches!(
        engine.layout(&description(text)),
        Err(ParagraphError::InvalidResult {
            field: "clusters.range",
            ..
        })
    ));
}

#[test]
fn native_offset_inside_utf16_surrogate_pair_is_rejected() {
    let text = "😀";
    let mut output = base_output(INDEX_ENCODING_UTF16, 2);
    output.clusters = vec![PackedCluster {
        range: packed_range(0, 2),
        rect: rect(0.0, 10.0),
        line_index: 0,
        direction: DIRECTION_LEFT_TO_RIGHT,
        starts_grapheme: true,
        starts_word: true,
    }]
    .into();
    output.carets[0].index = 1;

    let (engine, _) = engine(output);
    assert!(matches!(
        engine.layout(&description(text)),
        Err(ParagraphError::InvalidResult {
            field: "carets.index",
            ..
        })
    ));
}

#[test]
fn malformed_native_enums_and_geometry_are_rejected() {
    let mut bad_direction = base_output(INDEX_ENCODING_UTF8, 1);
    bad_direction.lines[0].direction = 99;
    let (bad_direction_engine, _) = engine(bad_direction);
    assert!(matches!(
        bad_direction_engine.layout(&description("a")),
        Err(ParagraphError::InvalidResult {
            field: "lines.direction",
            ..
        })
    ));

    let mut bad_line_index = base_output(INDEX_ENCODING_UTF8, 1);
    bad_line_index.clusters = vec![PackedCluster {
        range: packed_range(0, 1),
        rect: rect(0.0, 10.0),
        line_index: 4,
        direction: DIRECTION_LEFT_TO_RIGHT,
        starts_grapheme: true,
        starts_word: true,
    }]
    .into();
    let (engine, _) = engine(bad_line_index);
    assert!(matches!(
        engine.layout(&description("a")),
        Err(ParagraphError::InvalidResult {
            field: "clusters.line_index",
            ..
        })
    ));
}

#[test]
fn malformed_unresolved_codepoint_span_is_rejected() {
    let mut output = base_output(INDEX_ENCODING_UTF8, 1);
    output.clusters = vec![PackedCluster {
        range: packed_range(0, 1),
        rect: rect(0.0, 10.0),
        line_index: 0,
        direction: DIRECTION_LEFT_TO_RIGHT,
        starts_grapheme: true,
        starts_word: true,
    }]
    .into();
    output.unresolved_codepoints = vec!['a' as u32].into();
    output.unresolved_glyphs = vec![PackedUnresolvedGlyph {
        range: packed_range(0, 1),
        codepoint_start: 1,
        codepoint_count: 1,
    }]
    .into();

    let (engine, _) = engine(output);
    assert!(matches!(
        engine.layout(&description("a")),
        Err(ParagraphError::InvalidResult {
            field: "unresolved_glyphs.codepoints",
            ..
        })
    ));
}

#[test]
fn cache_key_tracks_font_catalog_generation() {
    let output = {
        let mut output = base_output(INDEX_ENCODING_UTF8, 1);
        output.clusters = vec![PackedCluster {
            range: packed_range(0, 1),
            rect: rect(0.0, 10.0),
            line_index: 0,
            direction: DIRECTION_LEFT_TO_RIGHT,
            starts_grapheme: true,
            starts_word: true,
        }]
        .into();
        output
    };
    let (engine, _) = engine(output);
    let first = description("a");
    let mut second = first.clone();
    second.font_catalog_generation = 1;

    assert_ne!(
        engine.layout(&first).unwrap().cache_key(),
        engine.layout(&second).unwrap().cache_key()
    );
}

#[test]
fn unsupported_native_stub_fails_explicitly() {
    let mut description = description("plain");
    description.paragraph_style.text_direction = fission_ir::op::TextDirection::Ltr;
    let error = SkiaParagraphEngine::default()
        .layout(&description)
        .unwrap_err();

    assert!(matches!(
        error,
        ParagraphError::Backend {
            ref backend,
            ref details,
        } if backend == "skia-skparagraph" && details.contains("has not been installed")
    ));
}
