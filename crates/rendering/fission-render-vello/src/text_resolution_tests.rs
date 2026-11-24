use super::*;
use fission_ir::op::{
    encode_inline_widget_marker, Color, FontStyle, TextAlign, TextDirection,
    TextParagraphLayoutStyle, TextStyle,
};
use parley::layout::Cursor;

fn text_run(text: &str) -> TextRun {
    TextRun {
        text: text.into(),
        style: TextStyle {
            font_size: 18.0,
            color: Color::BLACK,
            underline: false,
            font_family: None,
            locale: None,
            font_weight: 400,
            font_style: FontStyle::Normal,
            line_height: Some(24.0),
            letter_spacing: 0.0,
            background_color: None,
            typography: Default::default(),
        },
    }
}

fn close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.01,
        "actual={actual}, expected={expected}"
    );
}

fn assert_native_geometry(
    measurer: &VelloTextMeasurer,
    runs: &[TextRun],
    width: f32,
    style: TextParagraphLayoutStyle,
) -> ResolvedParagraphLayout {
    let rich = VelloTextMeasurer::rich_layout_input_from_ir_runs(runs);
    let base_style =
        crate::VelloRenderer::paragraph_base_style(rich.base_size, rich.base_color, false);
    let prepared = crate::prepare_paragraph_layout(
        &rich.text,
        &base_style,
        TextParagraphStyle {
            text_align: style.text_align,
            text_direction: style.text_direction,
            ..TextParagraphStyle::default()
        },
        &rich.inline_boxes,
        &rich.styles,
        None,
    );
    let mut native = (*measurer.layout_rich(
        &prepared.text,
        prepared.base_style.font_size,
        prepared.base_style.color,
        &prepared.styles,
        &prepared.inline_boxes,
        Some(width),
    ))
    .clone();
    if native.width() <= width + crate::TEXT_ALIGNMENT_BREAK_EPSILON {
        native.break_all_lines(Some(width + crate::TEXT_ALIGNMENT_BREAK_EPSILON));
    }
    native.align(
        crate::paragraph_alignment(style.text_align),
        crate::paragraph_alignment_options(style.text_align),
    );

    let resolved = measurer.resolve_rich_text_with_paragraph_style(runs, Some(width), style);
    let mut expected_clusters = Vec::new();
    let mut expected_boxes = Vec::new();
    for (line_index, line) in native.lines().enumerate() {
        for run in line.runs() {
            for cluster in run.visual_clusters() {
                let range = cluster.text_range();
                if range.end <= prepared.text_byte_offset {
                    continue;
                }
                expected_clusters.push((
                    range.start.saturating_sub(prepared.text_byte_offset)
                        ..range.end - prepared.text_byte_offset,
                    line_index,
                    cluster.visual_offset().unwrap_or(line.metrics().offset),
                ));
            }
        }
        for item in line.items() {
            if let PositionedLayoutItem::InlineBox(inline_box) = item {
                expected_boxes.push(inline_box);
            }
        }
    }
    assert_eq!(resolved.clusters.len(), expected_clusters.len());
    for (actual, (range, line_index, x)) in resolved.clusters.iter().zip(expected_clusters) {
        assert_eq!(actual.start_index..actual.end_index, range);
        assert_eq!(actual.line_index, line_index);
        close(actual.rect.x(), x);
    }
    for caret in &resolved.caret_stops {
        let affinity = if caret.upstream {
            Affinity::Upstream
        } else {
            Affinity::Downstream
        };
        let expected =
            Cursor::from_byte_index(&native, caret.index + prepared.text_byte_offset, affinity)
                .geometry(&native, 0.0);
        close(caret.position.x, expected.x0 as f32);
        close(caret.position.y, expected.y0 as f32);
        close(caret.height, (expected.y1 - expected.y0) as f32);
        assert!(rich.text.is_char_boundary(caret.index));
    }
    assert_eq!(resolved.inline_boxes.len(), expected_boxes.len());
    for (actual, expected) in resolved.inline_boxes.iter().zip(expected_boxes) {
        assert_eq!(actual.id, expected.id);
        close(actual.x, expected.x);
        close(actual.y, expected.y);
    }
    resolved
}

#[test]
fn aligned_wrapped_carets_match_native_parley_without_mutating_cached_layouts() {
    let measurer = VelloTextMeasurer::new(Arc::new(Mutex::new(FontContext::new())));
    let runs = [text_run(
        "A paragraph wraps here with cafe\u{301} and a short end",
    )];
    let unaligned = measurer.resolve_rich_text(&runs, Some(110.0));
    for text_align in [TextAlign::Center, TextAlign::End] {
        for text_direction in [TextDirection::Auto, TextDirection::Ltr] {
            let resolved = assert_native_geometry(
                &measurer,
                &runs,
                110.0,
                TextParagraphLayoutStyle {
                    indent: 0.0,
                    text_align,
                    text_direction,
                },
            );
            assert!(resolved.lines.len() > 1);
        }
    }
    assert_eq!(measurer.resolve_rich_text(&runs, Some(110.0)), unaligned);
}

#[test]
fn rtl_justification_and_trailing_whitespace_keep_native_caret_and_cluster_geometry() {
    let measurer = VelloTextMeasurer::new(Arc::new(Mutex::new(FontContext::new())));
    let resolved = assert_native_geometry(
        &measurer,
        &[text_run("مرحبا بكم في واجهة مستخدم حديثة وبعض الكلمات  ")],
        140.0,
        TextParagraphLayoutStyle {
            indent: 0.0,
            text_align: TextAlign::Justify,
            text_direction: TextDirection::Rtl,
        },
    );
    assert!(resolved.lines.len() > 1);
    assert!(resolved.clusters.iter().any(|cluster| cluster.is_rtl));
}

#[test]
fn inline_boxes_follow_native_alignment_and_justification() {
    let measurer = VelloTextMeasurer::new(Arc::new(Mutex::new(FontContext::new())));
    let mut marker = text_run("");
    marker.style.font_family = Some(encode_inline_widget_marker(42, 36.0, 18.0));
    let runs = [
        text_run("before "),
        marker,
        text_run(" after and several more words"),
    ];
    for text_align in [TextAlign::Center, TextAlign::Justify] {
        let resolved = assert_native_geometry(
            &measurer,
            &runs,
            160.0,
            TextParagraphLayoutStyle {
                indent: 0.0,
                text_align,
                text_direction: TextDirection::Ltr,
            },
        );
        assert_eq!(resolved.inline_boxes.len(), 1);
        assert_eq!(resolved.inline_boxes[0].id, 42);
    }
}

#[test]
fn empty_intrinsic_styled_paragraph_has_finite_geometry() {
    let measurer = VelloTextMeasurer::new(Arc::new(Mutex::new(FontContext::new())));
    for text_align in [TextAlign::Center, TextAlign::End] {
        let resolved = measurer.resolve_rich_text_with_paragraph_style(
            &[text_run("")],
            None,
            TextParagraphLayoutStyle {
                indent: 0.0,
                text_align,
                text_direction: TextDirection::Ltr,
            },
        );
        assert!(resolved.size.width.is_finite() && resolved.size.height.is_finite());
        assert!(resolved.caret_stops.iter().all(|caret| {
            caret.index == 0 && caret.position.x.is_finite() && caret.position.y.is_finite()
        }));
    }
}
