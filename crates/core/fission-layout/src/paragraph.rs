use crate::{LayoutPoint, LayoutRect, LayoutSize, LayoutUnit};
use fission_ir::op::{
    TextAlign, TextDirection, TextDisplayBreak, TextParagraphLayoutStyle, TextParagraphStyleSpan,
};
use serde::{Deserialize, Serialize};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

/// A contiguous logical-text segment after applying visual-only display breaks.
#[derive(Debug, Clone, PartialEq)]
pub struct TextDisplaySegment {
    /// Grapheme range in the unchanged logical text.
    pub grapheme_range: Range<usize>,
    /// UTF-8 byte range assigned to this segment in the unchanged logical text.
    /// It includes a trailing line separator when the break follows one.
    pub byte_range: Range<usize>,
    /// Range shaped and painted for this segment. A line separator immediately
    /// before a display break remains in `byte_range` but is omitted here so
    /// it cannot create a trailing blank line before the gap.
    pub shape_byte_range: Range<usize>,
    /// Break after this segment, if it is not the final segment.
    pub break_after: Option<TextDisplayBreak>,
    /// Style active for this segment, inherited from the latest span.
    pub paragraph_style: Option<TextParagraphLayoutStyle>,
    /// Whether a layout boundary follows this segment, including a zero-gap
    /// paragraph-style boundary.
    pub boundary_after: bool,
}

/// Split text at valid UTF-8 byte offsets that are also grapheme boundaries.
///
/// Breaks at the beginning or end of the value, negative/non-finite gaps, and
/// offsets inside a grapheme or outside the value are ignored. Duplicate offsets
/// are coalesced to their largest requested gap.
pub fn split_text_display_segments(
    text: &str,
    breaks: &[TextDisplayBreak],
) -> Vec<TextDisplaySegment> {
    split_text_layout_segments(text, breaks, &[])
}

/// Split a logical value at visual breaks and paragraph-style boundaries.
///
/// Style spans may start at byte offset zero or at the end of the value. When
/// any spans are supplied, explicit LF/CRLF separators are also boundaries so
/// each paragraph can wrap and align independently while retaining its
/// original byte range.
pub fn split_text_layout_segments(
    text: &str,
    breaks: &[TextDisplayBreak],
    paragraph_styles: &[TextParagraphStyleSpan],
) -> Vec<TextDisplaySegment> {
    let mut grapheme_bytes = text
        .grapheme_indices(true)
        .map(|(byte_offset, _)| byte_offset)
        .collect::<Vec<_>>();
    grapheme_bytes.push(text.len());
    let grapheme_count = grapheme_bytes.len().saturating_sub(1);

    #[derive(Clone, Copy)]
    struct Boundary {
        grapheme_index: usize,
        display_break: Option<TextDisplayBreak>,
        paragraph_style: Option<TextParagraphLayoutStyle>,
    }

    let mut boundaries = breaks
        .iter()
        .copied()
        .filter_map(|display_break| {
            let grapheme_index = grapheme_bytes
                .binary_search(&display_break.text_offset)
                .ok()?;
            let valid_gap = display_break.gap_after.is_finite() && display_break.gap_after >= 0.0;
            (valid_gap && grapheme_index > 0 && grapheme_index < grapheme_count).then_some(
                Boundary {
                    grapheme_index,
                    display_break: Some(display_break),
                    paragraph_style: None,
                },
            )
        })
        .collect::<Vec<_>>();

    for span in paragraph_styles {
        let Ok(grapheme_index) = grapheme_bytes.binary_search(&span.text_offset) else {
            continue;
        };
        let mut style = span.style;
        style.indent = if style.indent.is_finite() {
            style.indent.max(0.0)
        } else {
            0.0
        };
        boundaries.push(Boundary {
            grapheme_index,
            display_break: None,
            paragraph_style: Some(style),
        });
    }

    if !paragraph_styles.is_empty() {
        for (grapheme_index, grapheme) in text.grapheme_indices(true) {
            if grapheme == "\n" || grapheme == "\r\n" {
                let offset = grapheme_index + grapheme.len();
                if let Ok(boundary_index) = grapheme_bytes.binary_search(&offset) {
                    boundaries.push(Boundary {
                        grapheme_index: boundary_index,
                        display_break: None,
                        paragraph_style: None,
                    });
                }
            }
        }
    }

    boundaries.sort_by_key(|boundary| boundary.grapheme_index);
    let mut unique_boundaries: Vec<Boundary> = Vec::with_capacity(boundaries.len());
    for boundary in boundaries {
        if let Some(previous) = unique_boundaries.last_mut() {
            if previous.grapheme_index == boundary.grapheme_index {
                if let Some(display_break) = boundary.display_break {
                    if let Some(previous_break) = &mut previous.display_break {
                        previous_break.gap_after =
                            previous_break.gap_after.max(display_break.gap_after);
                    } else {
                        previous.display_break = Some(display_break);
                    }
                }
                if boundary.paragraph_style.is_some() {
                    previous.paragraph_style = boundary.paragraph_style;
                }
                continue;
            }
        }
        unique_boundaries.push(boundary);
    }

    let mut style = unique_boundaries
        .iter()
        .take_while(|boundary| boundary.grapheme_index == 0)
        .filter_map(|boundary| boundary.paragraph_style)
        .last();
    let mut segments = Vec::with_capacity(unique_boundaries.len() + 1);
    let mut start = 0usize;
    let mut terminal_style_boundary = false;
    for boundary in unique_boundaries {
        if boundary.grapheme_index == 0 {
            continue;
        }
        let end = boundary.grapheme_index;
        let byte_start = grapheme_bytes[start];
        let byte_end = grapheme_bytes[end];
        let shape_end = if byte_start < byte_end && text[..byte_end].ends_with("\r\n") {
            byte_end - 2
        } else if byte_start < byte_end && text[..byte_end].ends_with('\n') {
            byte_end - 1
        } else {
            byte_end
        };
        segments.push(TextDisplaySegment {
            grapheme_range: start..end,
            byte_range: byte_start..byte_end,
            shape_byte_range: byte_start..shape_end,
            break_after: boundary.display_break,
            paragraph_style: style,
            boundary_after: true,
        });
        start = end;
        if let Some(next_style) = boundary.paragraph_style {
            style = Some(next_style);
        }
        terminal_style_boundary |= end == grapheme_count && boundary.paragraph_style.is_some();
    }

    let trailing_empty_paragraph =
        !paragraph_styles.is_empty() && (text.ends_with('\n') || text.ends_with("\r\n"));
    if start < grapheme_count
        || segments.is_empty()
        || terminal_style_boundary
        || trailing_empty_paragraph
    {
        segments.push(TextDisplaySegment {
            grapheme_range: start..grapheme_count,
            byte_range: grapheme_bytes[start]..grapheme_bytes[grapheme_count],
            shape_byte_range: grapheme_bytes[start]..grapheme_bytes[grapheme_count],
            break_after: None,
            paragraph_style: style,
            boundary_after: false,
        });
    }

    segments
}

/// Per-line metrics from the same shaped paragraph used for paint and hit testing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineMetric {
    /// Byte index where this line starts in the source string.
    pub start_index: usize,
    /// Byte index where this line ends in the source string (exclusive).
    pub end_index: usize,
    /// Distance from the top of the paragraph to the alphabetic baseline.
    pub baseline: LayoutUnit,
    /// Total line height, including leading.
    pub height: LayoutUnit,
    /// Shaped width of the line.
    pub width: LayoutUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RichTextInlineBox {
    pub id: u64,
    pub x: LayoutUnit,
    pub y: LayoutUnit,
    pub width: LayoutUnit,
    pub height: LayoutUnit,
}

/// One visually positioned shaping cluster. Byte ranges always fall on UTF-8
/// boundaries and may contain more than one scalar value (for example a
/// ligature, combining sequence, or emoji ZWJ sequence).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParagraphCluster {
    pub start_index: usize,
    pub end_index: usize,
    pub line_index: usize,
    pub rect: LayoutRect,
    pub is_rtl: bool,
}

/// One shaped glyph and its association with a logical cluster.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParagraphGlyph {
    pub id: u32,
    pub style_index: usize,
    pub cluster_index: usize,
    pub position: LayoutPoint,
    pub advance: LayoutUnit,
}

/// An atomic selectable visual box associated with a logical source range.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParagraphSelectionBox {
    pub start_index: usize,
    pub end_index: usize,
    pub line_index: usize,
    pub rect: LayoutRect,
}

/// A legal caret stop resolved by the shaping backend.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParagraphCaretStop {
    pub index: usize,
    pub upstream: bool,
    pub position: LayoutPoint,
    pub height: LayoutUnit,
}

/// Resolved geometry for a display-only break, in paragraph-local coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ResolvedParagraphDisplayBreak {
    /// UTF-8 byte offset in the unchanged logical text.
    pub text_offset: usize,
    /// Paragraph-local rectangle reserved for the visual gap.
    pub gap_rect: LayoutRect,
}

/// Immutable, backend-neutral summary of one resolved paragraph.
///
/// The width is the exact wrapping constraint. Layout snapshots retain this
/// result so paint, hit testing, caret/selection geometry, accessibility, and
/// IME positioning can consume the same paragraph decision rather than derive
/// another width from an ancestor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedParagraphLayout {
    pub constraint_width: Option<LayoutUnit>,
    pub size: LayoutSize,
    pub lines: Vec<LineMetric>,
    pub inline_boxes: Vec<RichTextInlineBox>,
    /// Visual-order glyph/cluster mapping used by hit testing and selection.
    pub clusters: Vec<ParagraphCluster>,
    /// Shaped glyph positions mapped back to entries in `clusters`.
    pub glyphs: Vec<ParagraphGlyph>,
    /// Legal caret positions, including bidi-affinity alternatives.
    pub caret_stops: Vec<ParagraphCaretStop>,
    /// Atomic boxes from which arbitrary selection geometry is formed.
    pub selection_boxes: Vec<ParagraphSelectionBox>,
    /// Layout-only page/line gaps exposed for aligned page backgrounds.
    #[serde(default)]
    pub display_breaks: Vec<ResolvedParagraphDisplayBreak>,
}

impl ResolvedParagraphLayout {
    pub fn empty(constraint_width: Option<LayoutUnit>) -> Self {
        Self {
            constraint_width,
            size: LayoutSize::ZERO,
            lines: Vec::new(),
            inline_boxes: Vec::new(),
            clusters: Vec::new(),
            glyphs: Vec::new(),
            caret_stops: Vec::new(),
            selection_boxes: Vec::new(),
            display_breaks: Vec::new(),
        }
    }

    /// Resolve a point to the nearest shaped cluster boundary.
    pub fn hit_test(&self, point: LayoutPoint) -> usize {
        let Some(cluster) = self.clusters.iter().min_by(|left, right| {
            distance_to_rect(point, left.rect).total_cmp(&distance_to_rect(point, right.rect))
        }) else {
            return 0;
        };
        let after_midpoint = point.x >= cluster.rect.x() + cluster.rect.width() * 0.5;
        match (cluster.is_rtl, after_midpoint) {
            (false, false) | (true, true) => cluster.start_index,
            (false, true) | (true, false) => cluster.end_index,
        }
    }

    /// Return the resolved caret geometry for a byte index and affinity.
    pub fn caret(&self, index: usize, upstream: bool) -> Option<ParagraphCaretStop> {
        self.caret_stops
            .iter()
            .find(|stop| stop.index == index && stop.upstream == upstream)
            .copied()
            .or_else(|| {
                self.caret_stops
                    .iter()
                    .filter(|stop| stop.upstream == upstream)
                    .min_by_key(|stop| stop.index.abs_diff(index))
                    .copied()
            })
    }

    /// Return visual rectangles for a logical byte range. Disjoint rectangles
    /// are preserved for bidi text; adjacent clusters on one line are merged.
    pub fn selection_rects(&self, start: usize, end: usize) -> Vec<LayoutRect> {
        let range = start.min(end)..start.max(end);
        let mut selected = self
            .selection_boxes
            .iter()
            .filter(|selection| {
                selection.start_index < range.end && selection.end_index > range.start
            })
            .copied()
            .collect::<Vec<_>>();
        selected.sort_by(|left, right| {
            left.line_index
                .cmp(&right.line_index)
                .then_with(|| left.rect.x().total_cmp(&right.rect.x()))
        });
        let mut rects: Vec<LayoutRect> = Vec::new();
        for selection in selected {
            if let Some(last) = rects.last_mut() {
                let same_line = (last.y() - selection.rect.y()).abs() < 0.5
                    && (last.height() - selection.rect.height()).abs() < 0.5;
                if same_line && selection.rect.x() <= last.right() + 0.5 {
                    let right = last.right().max(selection.rect.right());
                    last.size.width = right - last.x();
                    continue;
                }
            }
            rects.push(selection.rect);
        }
        rects
    }
}

/// Applies paragraph alignment to geometry produced by a measurer that only
/// resolves left-aligned runs. Backends with native paragraph alignment can
/// override `TextMeasurer::resolve_rich_text_with_paragraph_style` instead.
pub(super) fn align_resolved_paragraph(
    paragraph: &mut ResolvedParagraphLayout,
    text: &str,
    width: Option<LayoutUnit>,
    alignment: TextAlign,
    direction: TextDirection,
) {
    if paragraph.lines.is_empty()
        || alignment == TextAlign::Start && direction == TextDirection::Ltr
    {
        return;
    }

    let available = width.unwrap_or(paragraph.size.width).max(0.0);
    let mut line_offsets = Vec::with_capacity(paragraph.lines.len());

    for (line_index, line) in paragraph.lines.iter().enumerate() {
        let rtl = match direction {
            TextDirection::Ltr => false,
            TextDirection::Rtl => true,
            TextDirection::Auto => paragraph
                .clusters
                .iter()
                .find(|cluster| cluster.line_index == line_index)
                .is_some_and(|cluster| cluster.is_rtl),
        };
        let slack = (available - line.width).max(0.0);
        let justify = alignment == TextAlign::Justify && line_index + 1 < paragraph.lines.len();
        let mut spaces = paragraph
            .clusters
            .iter()
            .enumerate()
            .filter(|(_, cluster)| cluster.line_index == line_index)
            .filter_map(|(index, cluster)| {
                let content = text.get(cluster.start_index..cluster.end_index)?;
                (!content.is_empty() && content.chars().all(char::is_whitespace))
                    .then_some((index, cluster.rect))
            })
            .collect::<Vec<_>>();
        spaces.sort_by(|left, right| left.1.x().total_cmp(&right.1.x()));
        let extra_per_space = if justify && !spaces.is_empty() {
            slack / spaces.len() as f32
        } else {
            0.0
        };

        let alignment_offset = match alignment {
            TextAlign::Left => 0.0,
            TextAlign::Right => slack,
            TextAlign::Center => slack * 0.5,
            TextAlign::Start => {
                if rtl {
                    slack
                } else {
                    0.0
                }
            }
            TextAlign::End => {
                if rtl {
                    0.0
                } else {
                    slack
                }
            }
            TextAlign::Justify => 0.0,
        };
        line_offsets.push((alignment_offset, extra_per_space, spaces.clone()));

        let mut expanded_before = 0.0;
        let mut clusters = paragraph
            .clusters
            .iter()
            .enumerate()
            .filter(|(_, cluster)| cluster.line_index == line_index)
            .map(|(index, cluster)| (index, cluster.rect.x()))
            .collect::<Vec<_>>();
        clusters.sort_by(|left, right| left.1.total_cmp(&right.1));
        let mut space_indices = spaces.iter().map(|(index, _)| *index).peekable();
        for (cluster_index, _) in clusters {
            let is_space = space_indices.peek().copied() == Some(cluster_index);
            paragraph.clusters[cluster_index].rect.origin.x += alignment_offset + expanded_before;
            if is_space {
                paragraph.clusters[cluster_index].rect.size.width += extra_per_space;
                expanded_before += extra_per_space;
                space_indices.next();
            }
        }
    }

    for selection in &mut paragraph.selection_boxes {
        let Some((offset, extra, spaces)) = line_offsets.get(selection.line_index) else {
            continue;
        };
        let mut expanded_before = 0.0;
        for (cluster_index, rect) in spaces {
            if rect.x() + rect.width() <= selection.rect.x() {
                expanded_before += *extra;
            } else if selection.start_index == paragraph.clusters[*cluster_index].start_index {
                selection.rect.size.width += *extra;
            }
        }
        selection.rect.origin.x += *offset + expanded_before;
    }

    for glyph in &mut paragraph.glyphs {
        let Some(cluster) = paragraph.clusters.get(glyph.cluster_index) else {
            continue;
        };
        let original_x = glyph.position.x;
        let Some((offset, extra, spaces)) = line_offsets.get(cluster.line_index) else {
            continue;
        };
        let mut expanded_before = 0.0;
        for (space_index, rect) in spaces {
            if rect.x() + rect.width() <= original_x {
                expanded_before += *extra;
            } else if *space_index == glyph.cluster_index {
                expanded_before += *extra;
            }
        }
        glyph.position.x += *offset + expanded_before;
    }

    for caret in &mut paragraph.caret_stops {
        let Some((line_index, _)) =
            paragraph
                .lines
                .iter()
                .enumerate()
                .min_by(|(_, left), (_, right)| {
                    (left.baseline - caret.position.y)
                        .abs()
                        .total_cmp(&(right.baseline - caret.position.y).abs())
                })
        else {
            continue;
        };
        let Some((offset, extra, spaces)) = line_offsets.get(line_index) else {
            continue;
        };
        let mut expanded_before = 0.0;
        for (_, rect) in spaces {
            if rect.x() + rect.width() <= caret.position.x {
                expanded_before += *extra;
            } else if caret.position.x > rect.x() {
                let progress =
                    ((caret.position.x - rect.x()) / rect.width().max(1.0)).clamp(0.0, 1.0);
                expanded_before += *extra * progress;
            }
        }
        caret.position.x += *offset + expanded_before;
    }

    for inline_box in &mut paragraph.inline_boxes {
        let Some((line_index, _)) =
            paragraph
                .lines
                .iter()
                .enumerate()
                .min_by(|(_, left), (_, right)| {
                    (left.baseline - inline_box.y)
                        .abs()
                        .total_cmp(&(right.baseline - inline_box.y).abs())
                })
        else {
            continue;
        };
        if let Some((offset, _, _)) = line_offsets.get(line_index) {
            inline_box.x += *offset;
        }
    }
}

pub(super) fn inset_resolved_paragraph(
    paragraph: &mut ResolvedParagraphLayout,
    indent: LayoutUnit,
    direction: TextDirection,
) {
    let indent = if indent.is_finite() {
        indent.max(0.0)
    } else {
        0.0
    };
    if indent == 0.0 {
        return;
    }
    let offset = match direction {
        TextDirection::Rtl => 0.0,
        TextDirection::Ltr => indent,
        TextDirection::Auto => {
            let first_line_is_rtl = paragraph
                .clusters
                .iter()
                .find(|cluster| cluster.line_index == 0)
                .is_some_and(|cluster| cluster.is_rtl);
            if first_line_is_rtl {
                0.0
            } else {
                indent
            }
        }
    };
    for cluster in &mut paragraph.clusters {
        cluster.rect.origin.x += offset;
    }
    for selection in &mut paragraph.selection_boxes {
        selection.rect.origin.x += offset;
    }
    for glyph in &mut paragraph.glyphs {
        glyph.position.x += offset;
    }
    for caret in &mut paragraph.caret_stops {
        caret.position.x += offset;
    }
    for inline_box in &mut paragraph.inline_boxes {
        inline_box.x += offset;
    }
    paragraph.size.width += indent;
}

fn distance_to_rect(point: LayoutPoint, rect: LayoutRect) -> LayoutUnit {
    let dx = if point.x < rect.x() {
        rect.x() - point.x
    } else if point.x > rect.right() {
        point.x - rect.right()
    } else {
        0.0
    };
    let dy = if point.y < rect.y() {
        rect.y() - point.y
    } else if point.y > rect.bottom() {
        point.y - rect.bottom()
    } else {
        0.0
    };
    dx * dx + dy * dy
}

/// Compatibility view for callers interested only in size and inline boxes.
#[derive(Debug, Clone, PartialEq)]
pub struct RichTextLayoutInfo {
    pub width: LayoutUnit,
    pub height: LayoutUnit,
    pub inline_boxes: Vec<RichTextInlineBox>,
}

impl From<ResolvedParagraphLayout> for RichTextLayoutInfo {
    fn from(layout: ResolvedParagraphLayout) -> Self {
        Self {
            width: layout.size.width,
            height: layout.size.height,
            inline_boxes: layout.inline_boxes,
        }
    }
}
