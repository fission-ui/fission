use fission_ir::op::{
    TextAlign, TextDirection, TextHeightBehavior, TextOverflow, TextParagraphStyle, TextWidthBasis,
};
use fission_render::TextStyle as RenderTextStyle;
use parley::layout::{Alignment as ParleyAlignment, AlignmentOptions, PositionedLayoutItem};

use crate::text::ParleyBrush;

pub(crate) const PARAGRAPH_FADE_SLICE_COUNT: usize = 8;
const PARAGRAPH_FADE_MIN_SPAN: f32 = 8.0;
const PARAGRAPH_FADE_RIGHT_MULTIPLIER: f32 = 1.5;
const PARAGRAPH_FADE_BOTTOM_FRACTION: f32 = 0.5;
pub(crate) const LTR_DIRECTION_MARK: &str = "\u{200E}";
pub(crate) const RTL_DIRECTION_MARK: &str = "\u{200F}";

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ParagraphLineVisualBounds {
    pub(crate) left: f32,
    pub(crate) right: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ParagraphFade {
    Right { start: f32, end: f32 },
    Bottom { start: f32, end: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TextClip {
    pub(crate) left: f32,
    pub(crate) right: f32,
    pub(crate) top: f32,
    pub(crate) bottom: f32,
}

impl TextClip {
    pub(crate) fn intersects_y(self, top: f32, bottom: f32) -> bool {
        self.bottom >= self.top && bottom >= self.top && top <= self.bottom
    }

    pub(crate) fn intersects_x(self, left: f32, right: f32) -> bool {
        self.right >= self.left && right >= self.left && left <= self.right
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TextBackgroundSegment {
    pub(crate) left: f32,
    pub(crate) right: f32,
}

pub(crate) fn text_background_segments_for_cluster_ranges(
    clusters: impl IntoIterator<Item = (std::ops::Range<usize>, f32, f32)>,
    style_range: &std::ops::Range<usize>,
    clip: Option<TextClip>,
) -> Vec<TextBackgroundSegment> {
    let mut segments = Vec::new();
    let mut current: Option<TextBackgroundSegment> = None;

    for (cluster_range, cluster_left, cluster_right) in clusters {
        let overlaps =
            style_range.start < cluster_range.end && style_range.end > cluster_range.start;
        if !overlaps {
            if let Some(segment) = current.take() {
                segments.push(segment);
            }
            continue;
        }

        let mut left = cluster_left.min(cluster_right);
        let mut right = cluster_left.max(cluster_right);
        if let Some(clip) = clip {
            left = left.max(clip.left);
            right = right.min(clip.right);
        }
        if right <= left {
            if let Some(segment) = current.take() {
                segments.push(segment);
            }
            continue;
        }

        match &mut current {
            Some(segment) if left <= segment.right + 0.5 => {
                segment.right = segment.right.max(right);
            }
            Some(_) => {
                segments.push(current.take().expect("segment checked above"));
                current = Some(TextBackgroundSegment { left, right });
            }
            None => {
                current = Some(TextBackgroundSegment { left, right });
            }
        }
    }

    if let Some(segment) = current {
        segments.push(segment);
    }

    segments
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedParagraphLayout {
    pub(crate) text: String,
    pub(crate) base_style: RenderTextStyle,
    pub(crate) styles: Vec<(std::ops::Range<usize>, RenderTextStyle)>,
    pub(crate) inline_boxes: Vec<crate::text::RichInlineBox>,
    pub(crate) caret_index: Option<usize>,
    #[allow(dead_code)]
    pub(crate) text_byte_offset: usize,
}

fn paragraph_style_with_strut(
    style: &RenderTextStyle,
    paragraph: TextParagraphStyle,
) -> RenderTextStyle {
    let mut style = style.clone();
    if let Some(strut_line_height) = paragraph.strut_line_height {
        style.line_height = Some(
            style
                .line_height
                .map_or(strut_line_height, |height| height.max(strut_line_height)),
        );
    }
    style
}

pub(crate) fn prepare_paragraph_layout(
    text: &str,
    base_style: &RenderTextStyle,
    paragraph: TextParagraphStyle,
    inline_boxes: &[crate::text::RichInlineBox],
    styles: &[(std::ops::Range<usize>, RenderTextStyle)],
    caret_index: Option<usize>,
) -> PreparedParagraphLayout {
    let base_style = paragraph_style_with_strut(base_style, paragraph);
    let mut styles = if styles.is_empty() && !text.is_empty() {
        vec![(0..text.len(), base_style.clone())]
    } else {
        styles
            .iter()
            .map(|(range, style)| (range.clone(), paragraph_style_with_strut(style, paragraph)))
            .collect()
    };
    let mut inline_boxes = inline_boxes.to_vec();
    let mut text = text.to_string();
    let mut caret_index = caret_index;
    let mut text_byte_offset = 0usize;

    let direction_mark = match paragraph.text_direction {
        TextDirection::Auto => None,
        TextDirection::Ltr => Some(LTR_DIRECTION_MARK),
        TextDirection::Rtl => Some(RTL_DIRECTION_MARK),
    };

    if let Some(direction_mark) =
        direction_mark.filter(|_| !text.is_empty() || !inline_boxes.is_empty())
    {
        let prefix_len = direction_mark.len();
        text_byte_offset = prefix_len;
        text.insert_str(0, direction_mark);
        for (range, _) in &mut styles {
            range.start += prefix_len;
            range.end += prefix_len;
        }
        styles.insert(0, (0..prefix_len, base_style.clone()));
        for inline_box in &mut inline_boxes {
            inline_box.index += prefix_len;
        }
        caret_index = caret_index.map(|index| index + prefix_len);
    }

    PreparedParagraphLayout {
        text,
        base_style,
        styles,
        inline_boxes,
        caret_index,
        text_byte_offset,
    }
}

pub(crate) fn paragraph_line_trim(
    line: &parley::layout::Line<'_, ParleyBrush>,
    behavior: TextHeightBehavior,
    is_first_visible_line: bool,
    is_last_visible_line: bool,
) -> (f32, f32) {
    let metrics = line.metrics();
    let top_trim = if is_first_visible_line && !behavior.apply_height_to_first_ascent {
        (metrics.baseline - metrics.ascent).max(0.0)
    } else {
        0.0
    };
    let bottom_trim = if is_last_visible_line && !behavior.apply_height_to_last_descent {
        (metrics.line_height - (metrics.baseline + metrics.descent)).max(0.0)
    } else {
        0.0
    };
    (top_trim, bottom_trim)
}

pub(crate) fn paragraph_y_offset(
    line: Option<&parley::layout::Line<'_, ParleyBrush>>,
    behavior: TextHeightBehavior,
    is_last_visible_line: bool,
) -> f32 {
    line.map_or(0.0, |line| {
        let (top_trim, _) = paragraph_line_trim(line, behavior, true, is_last_visible_line);
        -top_trim
    })
}

pub(crate) fn paragraph_alignment(text_align: TextAlign) -> ParleyAlignment {
    match text_align {
        TextAlign::Start => ParleyAlignment::Start,
        TextAlign::Left => ParleyAlignment::Left,
        TextAlign::Center => ParleyAlignment::Center,
        TextAlign::Right => ParleyAlignment::Right,
        TextAlign::End => ParleyAlignment::End,
        TextAlign::Justify => ParleyAlignment::Justify,
    }
}

pub(crate) fn paragraph_alignment_options(text_align: TextAlign) -> AlignmentOptions {
    AlignmentOptions {
        align_when_overflowing: !matches!(text_align, TextAlign::Justify),
    }
}

pub(crate) fn paragraph_alignment_width(
    layout: &parley::layout::Layout<ParleyBrush>,
    bounds: fission_render::LayoutRect,
    paragraph: TextParagraphStyle,
) -> Option<f32> {
    let width = match paragraph.text_width_basis {
        TextWidthBasis::Parent => bounds.width(),
        TextWidthBasis::LongestLine => layout.width(),
    };

    (width.is_finite() && width > 0.0).then_some(width)
}

pub(crate) fn paragraph_line_visual_bounds(
    line: &parley::layout::Line<'_, ParleyBrush>,
) -> Option<ParagraphLineVisualBounds> {
    let mut left = f32::INFINITY;
    let mut right = f32::NEG_INFINITY;

    for item in line.items() {
        match item {
            PositionedLayoutItem::GlyphRun(glyph_run) => {
                left = left.min(glyph_run.offset());
                right = right.max(glyph_run.offset() + glyph_run.advance());
            }
            PositionedLayoutItem::InlineBox(inline_box) => {
                left = left.min(inline_box.x);
                right = right.max(inline_box.x + inline_box.width);
            }
        }
    }

    if left.is_finite() && right.is_finite() {
        Some(ParagraphLineVisualBounds { left, right })
    } else {
        None
    }
}

pub(crate) fn paragraph_fade(
    paragraph: TextParagraphStyle,
    bounds: fission_render::LayoutRect,
    line_height: f32,
    line_width: f32,
    is_last_visible_line: bool,
    has_more_lines: bool,
    overflows_horizontally: bool,
) -> Option<ParagraphFade> {
    if !matches!(paragraph.overflow, TextOverflow::Fade) || !is_last_visible_line {
        return None;
    }

    if has_more_lines {
        let fade_height = (line_height * PARAGRAPH_FADE_BOTTOM_FRACTION)
            .max(1.0)
            .min(bounds.height().max(1.0));
        return Some(ParagraphFade::Bottom {
            start: (line_height - fade_height).max(0.0),
            end: line_height,
        });
    }

    if !overflows_horizontally || bounds.width() <= 0.0 {
        return None;
    }

    let fade_width = line_width
        .min(bounds.width())
        .min((line_height * PARAGRAPH_FADE_RIGHT_MULTIPLIER).max(PARAGRAPH_FADE_MIN_SPAN));
    if fade_width <= 0.0 {
        return None;
    }

    Some(ParagraphFade::Right {
        start: (bounds.width() - fade_width).max(0.0),
        end: bounds.width(),
    })
}
