use std::collections::HashSet;

use fission_ir::op::{TextDirection, TextWidthBasis};
use fission_layout::{
    LayoutRect, LayoutSize, ParagraphAffinity, ParagraphCaret, ParagraphCluster,
    ParagraphDescription, ParagraphDirection, ParagraphGeometry, ParagraphHitRegion,
    ParagraphInlineBox, ParagraphLine, Utf8Index, Utf8Range,
};
use parley::editing::Cursor;
use parley::layout::{Affinity, BreakReason, Layout, PositionedLayoutItem};
use unicode_segmentation::UnicodeSegmentation;

use crate::paragraph::{
    paragraph_line_trim, paragraph_line_visual_bounds, paragraph_y_offset,
    ParagraphLineVisualBounds,
};
use crate::text::ParleyBrush;

use super::source_map::ParagraphSourceMap;

pub(super) fn build_geometry(
    description: &ParagraphDescription,
    layout: &Layout<ParleyBrush>,
    source_map: &ParagraphSourceMap,
    text_byte_offset: usize,
) -> ParagraphGeometry {
    let total_lines = layout.len();
    let visible_line_count = description
        .paragraph_style
        .max_lines
        .map(|limit| limit.min(total_lines))
        .unwrap_or(total_lines);
    let first_line = layout.get(0);
    let y_offset = paragraph_y_offset(
        first_line.as_ref(),
        description.paragraph_style.text_height_behavior,
        visible_line_count == 1,
    );

    let (lines, inline_boxes) = visible_lines_and_inline_boxes(
        description,
        layout,
        source_map,
        text_byte_offset,
        visible_line_count,
        y_offset,
    );
    let grapheme_boundaries = grapheme_boundaries(&description.text);
    let (clusters, mut hit_regions) = clusters_and_hit_regions(
        layout,
        source_map,
        text_byte_offset,
        &lines,
        &grapheme_boundaries,
    );
    add_inline_hit_regions(&inline_boxes, &lines, &mut hit_regions);

    let carets = caret_geometry(
        description,
        layout,
        source_map,
        text_byte_offset,
        y_offset,
        &lines,
        &inline_boxes,
        &grapheme_boundaries,
    );
    ensure_line_hit_regions(&lines, &carets, &mut hit_regions);
    let measured_width = lines
        .iter()
        .map(|line| line.rect.right().max(0.0))
        .fold(0.0_f32, f32::max);
    let width = match (
        description.paragraph_style.text_width_basis,
        description.width_constraint,
    ) {
        (TextWidthBasis::Parent, Some(width)) => width,
        _ => measured_width,
    };
    let height = lines
        .iter()
        .map(|line| line.rect.bottom().max(0.0))
        .fold(0.0_f32, f32::max);
    let content_widths = layout.calculate_content_widths();
    let min_intrinsic_width = content_widths.min.max(0.0);
    let max_intrinsic_width = content_widths.max.max(min_intrinsic_width);
    let first_baseline = lines.first().map(|line| line.baseline);
    let last_baseline = lines.last().map(|line| line.baseline);

    ParagraphGeometry::new(LayoutSize::new(width, height))
        .with_intrinsic_widths(min_intrinsic_width, max_intrinsic_width)
        .with_baselines(first_baseline, last_baseline)
        .with_lines(lines)
        .with_clusters(clusters)
        .with_carets(carets)
        .with_hit_regions(hit_regions)
        .with_inline_boxes(
            inline_boxes
                .into_iter()
                .map(|item| item.output)
                .collect::<Vec<_>>(),
        )
}

#[derive(Debug, Clone, Copy)]
struct PositionedInline {
    output: ParagraphInlineBox,
    line_index: usize,
    direction: ParagraphDirection,
}

fn visible_lines_and_inline_boxes(
    description: &ParagraphDescription,
    layout: &Layout<ParleyBrush>,
    source_map: &ParagraphSourceMap,
    text_byte_offset: usize,
    visible_line_count: usize,
    y_offset: f32,
) -> (Vec<ParagraphLine>, Vec<PositionedInline>) {
    let mut lines = Vec::with_capacity(visible_line_count);
    let mut inline_boxes = Vec::new();

    for (line_index, line) in layout.lines().take(visible_line_count).enumerate() {
        let metrics = *line.metrics();
        let is_last_visible = line_index + 1 == visible_line_count;
        let (top_trim, bottom_trim) = paragraph_line_trim(
            &line,
            description.paragraph_style.text_height_behavior,
            line_index == 0,
            is_last_visible,
        );
        let line_height = metrics
            .line_height
            .max(metrics.ascent + metrics.descent)
            .max(1.0);
        let top = metrics.baseline - metrics.ascent + y_offset;
        let visual_height = (line_height - top_trim - bottom_trim).max(0.0);
        let visual_bounds =
            paragraph_line_visual_bounds(&line).unwrap_or(ParagraphLineVisualBounds {
                left: metrics.offset,
                right: metrics.offset + metrics.advance,
            });
        let mut source_range =
            prepared_range_to_source(line.text_range(), source_map, text_byte_offset);
        let mut has_source_coverage = !source_range.is_empty();
        let direction = line_direction(description, &line);

        for item in line.items() {
            let PositionedLayoutItem::InlineBox(positioned) = item else {
                continue;
            };
            let Some(input) = description
                .inline_objects
                .iter()
                .find(|inline| inline.id == positioned.id)
            else {
                continue;
            };
            source_range = if has_source_coverage {
                union_ranges(source_range, input.range)
            } else {
                has_source_coverage = true;
                input.range
            };
            inline_boxes.push(PositionedInline {
                output: ParagraphInlineBox {
                    id: input.id,
                    range: input.range,
                    rect: LayoutRect::new(
                        positioned.x,
                        positioned.y + y_offset,
                        positioned.width.max(0.0),
                        positioned.height.max(0.0),
                    ),
                    baseline: positioned.y + positioned.height + y_offset,
                },
                line_index,
                direction,
            });
        }

        lines.push(ParagraphLine {
            range: source_range,
            rect: LayoutRect::new(
                visual_bounds.left,
                top,
                (visual_bounds.right - visual_bounds.left).max(0.0),
                visual_height,
            ),
            baseline: metrics.baseline + y_offset,
            ascent: metrics.ascent,
            descent: metrics.descent,
            leading: metrics.leading,
            hard_break: line.break_reason() == BreakReason::Explicit,
            direction,
        });
    }

    (lines, inline_boxes)
}

fn line_direction(
    description: &ParagraphDescription,
    line: &parley::layout::Line<'_, ParleyBrush>,
) -> ParagraphDirection {
    match description.paragraph_style.text_direction {
        TextDirection::Ltr => ParagraphDirection::LeftToRight,
        TextDirection::Rtl => ParagraphDirection::RightToLeft,
        TextDirection::Auto => line
            .runs()
            .next()
            .map(|run| direction(run.is_rtl()))
            .unwrap_or(ParagraphDirection::LeftToRight),
    }
}

fn prepared_range_to_source(
    range: std::ops::Range<usize>,
    source_map: &ParagraphSourceMap,
    text_byte_offset: usize,
) -> Utf8Range {
    let start = range.start.saturating_sub(text_byte_offset);
    let end = range.end.saturating_sub(text_byte_offset);
    source_map.shaped_range_to_source(start..end)
}

fn clusters_and_hit_regions(
    layout: &Layout<ParleyBrush>,
    source_map: &ParagraphSourceMap,
    text_byte_offset: usize,
    lines: &[ParagraphLine],
    grapheme_boundaries: &HashSet<usize>,
) -> (Vec<ParagraphCluster>, Vec<ParagraphHitRegion>) {
    let mut clusters = Vec::new();
    let mut hit_regions = Vec::new();

    for (line_index, line) in layout.lines().take(lines.len()).enumerate() {
        for run in line.runs() {
            for cluster in run.clusters() {
                let prepared_range = cluster.text_range();
                if prepared_range.end <= text_byte_offset {
                    continue;
                }
                let source_range =
                    prepared_range_to_source(prepared_range, source_map, text_byte_offset);
                if source_range.is_empty() {
                    continue;
                }
                let Some(x) = cluster.visual_offset() else {
                    continue;
                };
                let cluster_direction = direction(cluster.is_rtl());
                let rect = LayoutRect::new(
                    x,
                    lines[line_index].rect.y(),
                    cluster.advance().max(0.0),
                    lines[line_index].rect.height(),
                );
                clusters.push(ParagraphCluster {
                    range: source_range,
                    rect,
                    line_index,
                    direction: cluster_direction,
                    starts_grapheme: grapheme_boundaries
                        .contains(&source_range.start().byte_offset()),
                    starts_word: cluster.is_word_boundary(),
                });
                add_cluster_hit_regions(
                    rect,
                    source_range,
                    line_index,
                    cluster_direction,
                    &mut hit_regions,
                );
            }
        }
    }

    (clusters, hit_regions)
}

fn add_cluster_hit_regions(
    rect: LayoutRect,
    range: Utf8Range,
    line_index: usize,
    direction: ParagraphDirection,
    output: &mut Vec<ParagraphHitRegion>,
) {
    let width = rect.width().max(1.0);
    let half = width * 0.5;
    let (left_index, left_affinity, right_index, right_affinity) = match direction {
        ParagraphDirection::LeftToRight => (
            range.start(),
            ParagraphAffinity::Downstream,
            range.end(),
            ParagraphAffinity::Upstream,
        ),
        ParagraphDirection::RightToLeft => (
            range.end(),
            ParagraphAffinity::Upstream,
            range.start(),
            ParagraphAffinity::Downstream,
        ),
    };
    output.push(ParagraphHitRegion {
        rect: LayoutRect::new(rect.x(), rect.y(), half, rect.height()),
        index: left_index,
        affinity: left_affinity,
        line_index,
    });
    output.push(ParagraphHitRegion {
        rect: LayoutRect::new(rect.x() + half, rect.y(), width - half, rect.height()),
        index: right_index,
        affinity: right_affinity,
        line_index,
    });
}

fn add_inline_hit_regions(
    inline_boxes: &[PositionedInline],
    lines: &[ParagraphLine],
    output: &mut Vec<ParagraphHitRegion>,
) {
    for inline in inline_boxes {
        let mut rect = inline.output.rect;
        rect.origin.y = lines[inline.line_index].rect.y();
        rect.size.height = lines[inline.line_index].rect.height();
        add_cluster_hit_regions(
            rect,
            inline.output.range,
            inline.line_index,
            inline.direction,
            output,
        );
    }
}

fn caret_geometry(
    description: &ParagraphDescription,
    layout: &Layout<ParleyBrush>,
    source_map: &ParagraphSourceMap,
    text_byte_offset: usize,
    y_offset: f32,
    lines: &[ParagraphLine],
    inline_boxes: &[PositionedInline],
    grapheme_boundaries: &HashSet<usize>,
) -> Vec<ParagraphCaret> {
    let mut carets = Vec::new();

    for inline in inline_boxes {
        let (start_x, end_x) = match inline.direction {
            ParagraphDirection::LeftToRight => (inline.output.rect.x(), inline.output.rect.right()),
            ParagraphDirection::RightToLeft => (inline.output.rect.right(), inline.output.rect.x()),
        };
        push_caret_unique(
            &mut carets,
            ParagraphCaret {
                index: inline.output.range.start(),
                affinity: ParagraphAffinity::Downstream,
                rect: LayoutRect::new(
                    start_x,
                    lines[inline.line_index].rect.y(),
                    1.0,
                    lines[inline.line_index].rect.height(),
                ),
                line_index: inline.line_index,
            },
        );
        push_caret_unique(
            &mut carets,
            ParagraphCaret {
                index: inline.output.range.end(),
                affinity: ParagraphAffinity::Upstream,
                rect: LayoutRect::new(
                    end_x,
                    lines[inline.line_index].rect.y(),
                    1.0,
                    lines[inline.line_index].rect.height(),
                ),
                line_index: inline.line_index,
            },
        );
    }

    let mut boundaries = grapheme_boundaries.iter().copied().collect::<Vec<_>>();
    boundaries.sort_unstable();
    for source_index in boundaries {
        let prepared_index = source_map.source_to_shaped(source_index) + text_byte_offset;
        for affinity in [Affinity::Downstream, Affinity::Upstream] {
            let cursor = Cursor::from_byte_index(layout, prepared_index, affinity);
            let Some(line_index) = cursor_line_index(&cursor, layout, lines.len()) else {
                continue;
            };
            let bounds = cursor.geometry(layout, 1.0);
            let rect = clamp_vertical(
                bounding_box_to_rect(bounds, y_offset),
                lines[line_index].rect,
            );
            push_caret_unique(
                &mut carets,
                ParagraphCaret {
                    index: Utf8Index::new(source_index),
                    affinity: paragraph_affinity(cursor.affinity()),
                    rect,
                    line_index,
                },
            );
        }
    }

    // Parley always creates metrics for an empty paragraph. Preserve a legal
    // caret even if the Unicode boundary iterator and visual-cluster lookup
    // yielded no record on a platform with no resolved default font.
    if carets.is_empty() && description.text.is_empty() && !lines.is_empty() {
        carets.push(ParagraphCaret {
            index: Utf8Index::new(0),
            affinity: ParagraphAffinity::Downstream,
            rect: LayoutRect::new(0.0, lines[0].rect.y(), 1.0, lines[0].rect.height()),
            line_index: 0,
        });
    }

    ensure_line_carets(lines, &mut carets);

    carets.sort_by_key(|caret| {
        (
            caret.index.byte_offset(),
            match caret.affinity {
                ParagraphAffinity::Upstream => 0,
                ParagraphAffinity::Downstream => 1,
            },
        )
    });
    carets
}

fn cursor_line_index(
    cursor: &Cursor,
    layout: &Layout<ParleyBrush>,
    visible_line_count: usize,
) -> Option<usize> {
    let y = cursor.geometry(layout, 0.0).y0 as f32;
    let line_count = layout.len();
    let line_index = layout.lines().enumerate().find_map(|(line_index, line)| {
        let metrics = line.metrics();
        let is_last = line_index + 1 == line_count;
        let contains = if y < metrics.min_coord {
            line_index == 0
        } else if is_last {
            y <= metrics.max_coord
        } else {
            y < metrics.max_coord
        };
        contains.then_some(line_index)
    })?;
    (line_index < visible_line_count).then_some(line_index)
}

fn grapheme_boundaries(text: &str) -> HashSet<usize> {
    let mut boundaries = text
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .collect::<HashSet<_>>();
    boundaries.insert(0);
    boundaries.insert(text.len());
    boundaries
}

fn bounding_box_to_rect(bounds: parley::BoundingBox, y_offset: f32) -> LayoutRect {
    LayoutRect::new(
        bounds.x0 as f32,
        bounds.y0 as f32 + y_offset,
        (bounds.x1 - bounds.x0).max(0.0) as f32,
        (bounds.y1 - bounds.y0).max(0.0) as f32,
    )
}

fn clamp_vertical(mut rect: LayoutRect, line_rect: LayoutRect) -> LayoutRect {
    let top = rect.y().max(line_rect.y());
    let bottom = rect.bottom().min(line_rect.bottom()).max(top);
    rect.origin.y = top;
    rect.size.height = bottom - top;
    rect
}

fn paragraph_affinity(affinity: Affinity) -> ParagraphAffinity {
    match affinity {
        Affinity::Upstream => ParagraphAffinity::Upstream,
        Affinity::Downstream => ParagraphAffinity::Downstream,
    }
}

fn direction(is_rtl: bool) -> ParagraphDirection {
    if is_rtl {
        ParagraphDirection::RightToLeft
    } else {
        ParagraphDirection::LeftToRight
    }
}

fn union_ranges(left: Utf8Range, right: Utf8Range) -> Utf8Range {
    Utf8Range::from_byte_offsets(
        left.start().byte_offset().min(right.start().byte_offset()),
        left.end().byte_offset().max(right.end().byte_offset()),
    )
    .expect("union is ordered")
}

fn push_caret_unique(output: &mut Vec<ParagraphCaret>, caret: ParagraphCaret) {
    if output.iter().any(|existing| {
        existing.index == caret.index
            && existing.affinity == caret.affinity
            && existing.line_index == caret.line_index
    }) {
        return;
    }
    output.push(caret);
}

fn ensure_line_carets(lines: &[ParagraphLine], output: &mut Vec<ParagraphCaret>) {
    for (line_index, line) in lines.iter().enumerate() {
        if output.iter().any(|caret| caret.line_index == line_index) {
            continue;
        }
        let x = match line.direction {
            ParagraphDirection::LeftToRight => line.rect.x(),
            ParagraphDirection::RightToLeft => line.rect.right(),
        };
        output.push(ParagraphCaret {
            index: line.range.start(),
            affinity: ParagraphAffinity::Downstream,
            rect: LayoutRect::new(x, line.rect.y(), 1.0, line.rect.height().max(1.0)),
            line_index,
        });
    }
}

fn ensure_line_hit_regions(
    lines: &[ParagraphLine],
    carets: &[ParagraphCaret],
    output: &mut Vec<ParagraphHitRegion>,
) {
    for (line_index, line) in lines.iter().enumerate() {
        if output.iter().any(|region| region.line_index == line_index) {
            continue;
        }
        let Some(caret) = carets.iter().find(|caret| caret.line_index == line_index) else {
            continue;
        };
        output.push(ParagraphHitRegion {
            rect: LayoutRect::new(
                line.rect.x(),
                line.rect.y(),
                line.rect.width().max(1.0),
                line.rect.height().max(1.0),
            ),
            index: caret.index,
            affinity: caret.affinity,
            line_index,
        });
    }
}
