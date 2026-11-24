use super::*;

/// Resolve the same natively aligned Parley geometry that paragraph painting
/// consumes. Cached layouts remain unaligned and shared; align a private clone.
pub(super) fn resolve_styled_paragraph(
    measurer: &VelloTextMeasurer,
    runs: &[TextRun],
    available_width: Option<f32>,
    style: fission_ir::op::TextParagraphLayoutStyle,
) -> ResolvedParagraphLayout {
    if runs.is_empty() {
        return ResolvedParagraphLayout::empty(available_width);
    }
    let rich = VelloTextMeasurer::rich_layout_input_from_ir_runs(runs);
    let base_style =
        crate::VelloRenderer::paragraph_base_style(rich.base_size, rich.base_color, false);
    let paragraph = TextParagraphStyle {
        text_align: style.text_align,
        text_direction: style.text_direction,
        ..TextParagraphStyle::default()
    };
    let prepared = crate::prepare_paragraph_layout(
        &rich.text,
        &base_style,
        paragraph,
        &rich.inline_boxes,
        &rich.styles,
        None,
    );
    let cached = measurer.layout_rich(
        &prepared.text,
        prepared.base_style.font_size,
        prepared.base_style.color,
        &prepared.styles,
        &prepared.inline_boxes,
        available_width,
    );
    let mut layout = (*cached).clone();
    let alignment_width = available_width.unwrap_or_else(|| layout.width());
    if alignment_width.is_finite() && alignment_width > 0.0 {
        if layout.width() <= alignment_width + crate::TEXT_ALIGNMENT_BREAK_EPSILON {
            layout.break_all_lines(Some(alignment_width + crate::TEXT_ALIGNMENT_BREAK_EPSILON));
        }
        layout.align(
            crate::paragraph_alignment(style.text_align),
            crate::paragraph_alignment_options(style.text_align),
        );
    }
    let prefix_len = prepared.text_byte_offset;
    let prepared_rich = RichLayoutInput {
        text: prepared.text,
        base_size: prepared.base_style.font_size,
        base_color: prepared.base_style.color,
        styles: prepared.styles,
        inline_boxes: prepared.inline_boxes,
    };
    let mut resolved = resolve_layout(&prepared_rich, &layout, available_width);
    if prefix_len > 0 {
        remove_direction_prefix(&mut resolved, prefix_len);
    }
    resolved
}

/// The painter adds an invisible direction mark to force an authored base
/// direction. Keep its geometry decisions but expose only original text indices.
fn remove_direction_prefix(resolved: &mut ResolvedParagraphLayout, prefix_len: usize) {
    for line in &mut resolved.lines {
        line.start_index = line.start_index.saturating_sub(prefix_len);
        line.end_index = line.end_index.saturating_sub(prefix_len);
    }
    let mut remap = vec![None; resolved.clusters.len()];
    let mut old_index = 0;
    let mut next_index = 0;
    resolved.clusters.retain_mut(|cluster| {
        let keep = cluster.end_index > prefix_len;
        if keep {
            remap[old_index] = Some(next_index);
            next_index += 1;
            cluster.start_index = cluster.start_index.saturating_sub(prefix_len);
            cluster.end_index -= prefix_len;
        }
        old_index += 1;
        keep
    });
    resolved.glyphs.retain_mut(|glyph| {
        let Some(index) = remap.get(glyph.cluster_index).copied().flatten() else {
            return false;
        };
        glyph.cluster_index = index;
        true
    });
    resolved.selection_boxes.retain_mut(|selection| {
        if selection.end_index <= prefix_len {
            return false;
        }
        selection.start_index = selection.start_index.saturating_sub(prefix_len);
        selection.end_index -= prefix_len;
        true
    });
    resolved.caret_stops.retain_mut(|caret| {
        if caret.index < prefix_len {
            return false;
        }
        caret.index -= prefix_len;
        true
    });
}

pub(super) fn resolve_layout(
    rich: &RichLayoutInput,
    layout: &Layout<ParleyBrush>,
    available_width: Option<f32>,
) -> ResolvedParagraphLayout {
    let mut inline_boxes = Vec::new();
    let mut lines = Vec::new();
    let mut clusters = Vec::new();
    let mut glyphs = Vec::new();
    let mut selection_boxes = Vec::new();
    let mut clusters_by_start = HashMap::new();
    let mut clusters_by_end = HashMap::new();
    let mut caret_indices = vec![0, rich.text.len()];
    for (line_index, line) in layout.lines().enumerate() {
        let metrics = line.metrics();
        let range = line.text_range();
        lines.push(LineMetric {
            start_index: range.start,
            end_index: range.end,
            baseline: metrics.baseline,
            height: metrics
                .line_height
                .max(metrics.ascent + metrics.descent)
                .max(1.0),
            width: (metrics.advance - metrics.trailing_whitespace).max(0.0),
        });
        let top = metrics.baseline - metrics.ascent;
        let cluster_height = (metrics.ascent + metrics.descent).max(1.0);
        for run in line.runs() {
            let run_range = run.text_range();
            let typography = rich
                .styles
                .iter()
                .find(|(range, _)| range.start < run_range.end && range.end > run_range.start)
                .map(|(_, style)| style.typography.clone())
                .unwrap_or_default();
            let baseline_shift = typography_baseline_shift(
                &typography,
                metrics.ascent,
                metrics.descent,
                metrics.line_height,
            );
            for cluster in run.visual_clusters() {
                let range = cluster.text_range();
                let caret_cluster = cache_caret_cluster(&cluster);
                clusters_by_start.insert(range.start, caret_cluster);
                clusters_by_end.insert(range.end, caret_cluster);
                let cluster_index = clusters.len();
                let cluster_x = cluster.visual_offset().unwrap_or(metrics.offset);
                let cluster_rect = LayoutRect::new(
                    cluster_x,
                    top + baseline_shift,
                    cluster.advance().max(0.0),
                    cluster_height,
                );
                caret_indices.push(range.start);
                caret_indices.push(range.end);
                clusters.push(ParagraphCluster {
                    start_index: range.start,
                    end_index: range.end,
                    line_index,
                    rect: cluster_rect,
                    is_rtl: cluster.is_rtl(),
                });
                selection_boxes.push(ParagraphSelectionBox {
                    start_index: range.start,
                    end_index: range.end,
                    line_index,
                    rect: cluster_rect,
                });
                let mut glyph_x = cluster_x;
                for glyph in cluster.glyphs() {
                    glyphs.push(ParagraphGlyph {
                        id: glyph.id as u32,
                        style_index: glyph.style_index(),
                        cluster_index,
                        position: LayoutPoint::new(
                            glyph_x + glyph.x,
                            metrics.baseline + baseline_shift - glyph.y,
                        ),
                        advance: glyph.advance,
                    });
                    glyph_x += glyph.advance;
                }
            }
        }
        for item in line.items() {
            if let PositionedLayoutItem::InlineBox(inline_box) = item {
                inline_boxes.push(RichTextInlineBox {
                    id: inline_box.id,
                    x: inline_box.x,
                    y: inline_box.y,
                    width: inline_box.width,
                    height: inline_box.height,
                });
            }
        }
    }
    caret_indices.sort_unstable();
    caret_indices.dedup();
    let mut caret_stops = Vec::with_capacity(caret_indices.len() * 2);
    for index in caret_indices {
        for (upstream, affinity) in [(false, Affinity::Downstream), (true, Affinity::Upstream)] {
            let (caret_x, caret_y, caret_bottom) = cursor_rect_from_clusters(
                &layout,
                index,
                affinity,
                &clusters_by_start,
                &clusters_by_end,
            );
            let baseline_shift = rich
                .styles
                .iter()
                .find(|(range, _)| range.start <= index && index <= range.end)
                .and_then(|(_, style)| {
                    layout
                        .lines()
                        .find(|line| {
                            let range = line.text_range();
                            range.start <= index && index <= range.end
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
            caret_stops.push(ParagraphCaretStop {
                index,
                upstream,
                position: LayoutPoint::new(caret_x, caret_y + baseline_shift),
                height: caret_bottom - caret_y,
            });
        }
    }
    let (width, height) = VelloTextMeasurer::layout_measured_size(&layout);
    ResolvedParagraphLayout {
        constraint_width: available_width,
        size: fission_layout::LayoutSize::new(width, height),
        lines,
        inline_boxes,
        clusters,
        glyphs,
        caret_stops,
        selection_boxes,
        display_breaks: Vec::new(),
    }
}
