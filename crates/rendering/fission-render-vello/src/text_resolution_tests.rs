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

fn close(actual: f32, expected: f32, context: &str) {
    assert!(
        (actual - expected).abs() < 0.01,
        "{context}: actual={actual}, expected={expected}"
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
        close(
            actual.rect.x(),
            x,
            &format!("cluster byte range {range:?}, line {line_index}, x"),
        );
    }
    for caret in &resolved.caret_stops {
        let affinity = if caret.upstream && caret.index != 0 {
            Affinity::Upstream
        } else {
            Affinity::Downstream
        };
        let native_index = caret.index + prepared.text_byte_offset;
        let expected =
            Cursor::from_byte_index(&native, native_index, affinity).geometry(&native, 0.0);
        // Fission's authored baseline/leading policy shifts glyphs, clusters,
        // and caret tops together. Parley's cursor returns the raw block top;
        // its x and height stay authoritative, while y includes that policy.
        let baseline_shift = prepared
            .styles
            .iter()
            .find(|(range, _)| {
                if affinity == Affinity::Upstream {
                    range.start < native_index && native_index <= range.end
                } else {
                    range.start <= native_index && native_index < range.end
                }
            })
            .or_else(|| {
                if native_index == 0 {
                    prepared.styles.first()
                } else if native_index == prepared.text.len() {
                    prepared.styles.last()
                } else {
                    None
                }
            })
            .and_then(|(_, style)| {
                native
                    .lines()
                    .find(|line| {
                        let metrics = line.metrics();
                        metrics.block_min_coord == expected.y0 as f32
                            && metrics.block_max_coord == expected.y1 as f32
                    })
                    .map(|line| {
                        let metrics = line.metrics();
                        typography_baseline_shift(
                            &style.typography,
                            metrics.ascent,
                            metrics.descent,
                            metrics.line_height,
                        )
                    })
            })
            .unwrap_or(0.0);
        let context = format!(
            "caret byte {}, {affinity:?}, align {:?}, direction {:?}, native index {native_index}, baseline shift {baseline_shift}",
            caret.index, style.text_align, style.text_direction,
        );
        close(
            caret.position.x,
            expected.x0 as f32,
            &format!("{context}, x"),
        );
        close(
            caret.position.y,
            expected.y0 as f32 + baseline_shift,
            &format!("{context}, top"),
        );
        close(
            caret.height,
            (expected.y1 - expected.y0) as f32,
            &format!("{context}, height"),
        );
        assert!(rich.text.is_char_boundary(caret.index));
    }
    assert_eq!(resolved.inline_boxes.len(), expected_boxes.len());
    for (actual, expected) in resolved.inline_boxes.iter().zip(expected_boxes) {
        assert_eq!(actual.id, expected.id);
        close(
            actual.x,
            expected.x,
            &format!("inline box {}, x", actual.id),
        );
        close(
            actual.y,
            expected.y,
            &format!("inline box {}, y", actual.id),
        );
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

#[test]
fn alignment_preserves_fissions_existing_baseline_and_leading_policy() {
    let measurer = VelloTextMeasurer::new(Arc::new(Mutex::new(FontContext::new())));
    let mut run =
        text_run("A paragraph with authored baseline positioning wraps onto another line");
    run.style.typography.baseline = fission_ir::op::TextBaseline::Ideographic;
    let runs = [run];
    let plain = measurer.resolve_rich_text(&runs, Some(150.0));
    for text_direction in [TextDirection::Auto, TextDirection::Ltr] {
        let aligned = measurer.resolve_rich_text_with_paragraph_style(
            &runs,
            Some(150.0),
            TextParagraphLayoutStyle {
                indent: 0.0,
                text_align: TextAlign::Center,
                text_direction,
            },
        );
        assert_eq!(plain.lines, aligned.lines);
        assert!(!plain.caret_stops.is_empty());
        assert_eq!(plain.caret_stops.len(), aligned.caret_stops.len());
        assert!(plain
            .caret_stops
            .iter()
            .any(|caret| caret.position.y != 0.0));
        for (before, after) in plain.caret_stops.iter().zip(&aligned.caret_stops) {
            assert_eq!(
                (before.index, before.upstream),
                (after.index, after.upstream)
            );
            close(
                after.position.y,
                before.position.y,
                &format!("caret byte {}, top", before.index),
            );
            close(
                after.height,
                before.height,
                &format!("caret byte {}, height", before.index),
            );
        }
        for (before, after) in plain.clusters.iter().zip(&aligned.clusters) {
            close(
                after.rect.y(),
                before.rect.y(),
                &format!("cluster byte {}, top", before.start_index),
            );
        }
        for (before, after) in plain.glyphs.iter().zip(&aligned.glyphs) {
            close(after.position.y, before.position.y, "glyph baseline");
        }
    }
}

#[test]
fn mixed_height_wrap_affinities_use_their_actual_native_line_metrics() {
    let measurer = VelloTextMeasurer::new(Arc::new(Mutex::new(FontContext::new())));
    let mut small =
        text_run("Small text needs several short wrapped lines before the large heading starts. ");
    small.style.font_size = 12.0;
    small.style.line_height = Some(16.0);
    let mut large = text_run("large words wrap here");
    large.style.font_size = 30.0;
    large.style.line_height = Some(44.0);
    // An explicit inline badge guarantees one taller line independently of
    // platform font/run metric aggregation. Both sides still soft-wrap.
    let mut badge = text_run("");
    badge.style.font_family = Some(encode_inline_widget_marker(9, 18.0, 80.0));
    let runs = [small, badge, large];
    for text_direction in [TextDirection::Auto, TextDirection::Ltr] {
        let resolved = assert_native_geometry(
            &measurer,
            &runs,
            90.0,
            TextParagraphLayoutStyle {
                indent: 0.0,
                text_align: TextAlign::Center,
                text_direction,
            },
        );
        assert!(resolved.lines.len() > 1);
        let boundary = resolved
            .lines
            .windows(2)
            .find(|lines| lines[0].height != lines[1].height)
            .unwrap_or_else(|| {
                panic!(
                    "fixture must cross into a taller inline-badge line: {:?}",
                    resolved.lines
                )
            })[1]
            .start_index;
        assert_ne!(
            resolved.caret(boundary, true).unwrap().position.y,
            resolved.caret(boundary, false).unwrap().position.y,
            "upstream/downstream stops at byte {boundary} must retain distinct wrapped lines",
        );
    }
}
