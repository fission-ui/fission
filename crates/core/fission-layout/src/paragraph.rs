use crate::{LayoutPoint, LayoutRect, LayoutSize, LayoutUnit};
use serde::{Deserialize, Serialize};

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
