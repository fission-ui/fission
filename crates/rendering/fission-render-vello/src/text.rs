use fission_diagnostics::prelude as diag;
use fission_ir::op::{
    decode_inline_widget_marker, FontStyle as IrFontStyle, RichTextAnnotation, TextParagraphStyle,
    TextRun,
};
use fission_ir::ActionEntry;
use fission_layout::{LineMetric, RichTextInlineBox, RichTextLayoutInfo, TextMeasurer};
use fission_render::TextStyle as RenderTextStyle;
use parley::layout::{Layout, PositionedLayoutItem};
use parley::style::{
    FontStack, FontStyle as ParleyFontStyle, FontWeight, LineHeight, StyleProperty,
};
use parley::InlineBox;
use parley::{FontContext, LayoutContext};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub struct ParleyBrush(pub [u8; 4]);

impl Default for ParleyBrush {
    fn default() -> Self {
        Self([0, 0, 0, 255])
    }
}

const SIMPLE_CACHE_CAP: usize = 4096;
const RICH_CACHE_CAP: usize = 2048;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RichCacheKey {
    text: String,
    base_size_bits: u32,
    base_color_rgba: [u8; 4],
    width_bits: Option<u32>,
    styles: Vec<RichStyleKey>,
    inline_boxes: Vec<InlineBoxKey>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RichStyleKey {
    range: std::ops::Range<usize>,
    font_size_bits: u32,
    color_rgba: [u8; 4],
    underline: bool,
    font_family: Option<String>,
    locale: Option<String>,
    font_weight: u16,
    font_style: IrFontStyle,
    line_height_bits: Option<u32>,
    letter_spacing_bits: u32,
    background_color: Option<[u8; 4]>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct InlineBoxKey {
    id: u64,
    index: usize,
    width_bits: u32,
    height_bits: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct RichInlineBox {
    pub(crate) id: u64,
    pub(crate) index: usize,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct RichLayoutInput {
    pub(crate) text: String,
    pub(crate) base_size: f32,
    pub(crate) base_color: fission_render::Color,
    pub(crate) styles: Vec<(std::ops::Range<usize>, RenderTextStyle)>,
    pub(crate) inline_boxes: Vec<RichInlineBox>,
}

pub(crate) fn text_style_requires_rich_layout(style: &RenderTextStyle) -> bool {
    style.font_family.is_some()
        || style.font_weight != 400
        || style.font_style != IrFontStyle::Normal
        || style.line_height.is_some()
        || style.letter_spacing != 0.0
        || style.background_color.is_some()
}

fn parley_font_style(style: IrFontStyle) -> ParleyFontStyle {
    match style {
        IrFontStyle::Normal => ParleyFontStyle::Normal,
        IrFontStyle::Italic => ParleyFontStyle::Italic,
    }
}

pub struct VelloTextMeasurer {
    font_cx: Arc<Mutex<FontContext>>,
    layout_cx: Mutex<LayoutContext<ParleyBrush>>,
    // Simple cache for single-style text grouped by metrics key.
    simple_cache: Mutex<HashMap<(u32, Option<u32>), HashMap<String, Arc<Layout<ParleyBrush>>>>>,
    // Cache for rich text layouts
    rich_cache: Mutex<HashMap<RichCacheKey, Arc<Layout<ParleyBrush>>>>,
    default_family: String,
}

impl VelloTextMeasurer {
    pub fn new(font_cx: Arc<Mutex<FontContext>>) -> Self {
        Self {
            font_cx,
            layout_cx: Mutex::new(LayoutContext::new()),
            simple_cache: Mutex::new(HashMap::new()),
            rich_cache: Mutex::new(HashMap::new()),
            default_family: "system-ui".to_string(),
        }
    }

    pub fn new_with_default_family(
        font_cx: Arc<Mutex<FontContext>>,
        family: impl Into<String>,
    ) -> Self {
        Self {
            font_cx,
            layout_cx: Mutex::new(LayoutContext::new()),
            simple_cache: Mutex::new(HashMap::new()),
            rich_cache: Mutex::new(HashMap::new()),
            default_family: family.into(),
        }
    }

    pub fn font_cx(&self) -> Arc<Mutex<FontContext>> {
        self.font_cx.clone()
    }

    fn width_bits(width: Option<f32>) -> Option<u32> {
        width.map(|w| ((w * 4.0).round() / 4.0).to_bits())
    }

    fn inline_box_key(inline_box: &RichInlineBox) -> InlineBoxKey {
        InlineBoxKey {
            id: inline_box.id,
            index: inline_box.index,
            width_bits: inline_box.width.to_bits(),
            height_bits: inline_box.height.to_bits(),
        }
    }

    fn layout_measured_size(layout: &Layout<ParleyBrush>) -> (f32, f32) {
        let height = layout.lines().fold(0.0_f32, |height, line| {
            let metrics = line.metrics();
            let line_height = metrics
                .line_height
                .max(metrics.ascent + metrics.descent)
                .max(1.0);
            height.max(metrics.baseline - metrics.ascent + line_height)
        });
        (layout.width(), height)
    }

    pub(crate) fn rich_layout_input_from_render_runs(
        runs: &[fission_render::TextRun],
    ) -> RichLayoutInput {
        let mut text = String::new();
        let mut styles = Vec::new();
        let mut inline_boxes = Vec::new();
        let mut start = 0usize;

        for run in runs {
            if run.text.is_empty() {
                if let Some(marker) = decode_inline_widget_marker(run.style.font_family.as_deref())
                {
                    inline_boxes.push(RichInlineBox {
                        id: marker.id,
                        index: start,
                        width: marker.width,
                        height: marker.height,
                    });
                    continue;
                }
            }

            text.push_str(&run.text);
            let end = start + run.text.len();
            styles.push((start..end, run.style.clone()));
            start = end;
        }

        let (base_size, base_color) =
            if let Some(first) = runs.iter().find(|run| !run.text.is_empty()) {
                (first.style.font_size, first.style.color)
            } else {
                (
                    14.0,
                    fission_render::Color {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 255,
                    },
                )
            };

        RichLayoutInput {
            text,
            base_size,
            base_color,
            styles,
            inline_boxes,
        }
    }

    fn rich_layout_input_from_ir_runs(runs: &[TextRun]) -> RichLayoutInput {
        let render_runs = runs
            .iter()
            .map(|run| fission_render::TextRun {
                text: run.text.clone(),
                style: RenderTextStyle {
                    font_size: run.style.font_size,
                    color: fission_render::Color {
                        r: run.style.color.r,
                        g: run.style.color.g,
                        b: run.style.color.b,
                        a: run.style.color.a,
                    },
                    underline: run.style.underline,
                    font_family: run.style.font_family.clone(),
                    locale: run.style.locale.clone(),
                    font_weight: run.style.font_weight,
                    font_style: run.style.font_style,
                    line_height: run.style.line_height,
                    letter_spacing: run.style.letter_spacing,
                    background_color: run.style.background_color.map(|c| fission_render::Color {
                        r: c.r,
                        g: c.g,
                        b: c.b,
                        a: c.a,
                    }),
                },
            })
            .collect::<Vec<_>>();
        Self::rich_layout_input_from_render_runs(&render_runs)
    }

    pub fn get_layout(
        &self,
        text: &str,
        font_size: f32,
        width: Option<f32>,
    ) -> Arc<Layout<ParleyBrush>> {
        let start = Instant::now();
        let cache_key = (font_size.to_bits(), Self::width_bits(width));

        {
            let cache = self.simple_cache.lock().unwrap();
            if let Some(bucket) = cache.get(&cache_key) {
                if let Some(layout) = bucket.get(text) {
                    return layout.clone();
                }
            }
        }

        let mut font_cx = self.font_cx.lock().unwrap();
        let mut layout_cx = self.layout_cx.lock().unwrap();

        let mut builder = layout_cx.ranged_builder(&mut font_cx, text, 1.0, false);

        builder.push_default(StyleProperty::FontSize(font_size));
        builder.push_default(StyleProperty::FontStack(FontStack::Source(Cow::Owned(
            self.default_family.clone(),
        ))));

        let mut layout = builder.build(text);
        layout.break_all_lines(width);

        let layout_arc = Arc::new(layout);

        let mut cache = self.simple_cache.lock().unwrap();
        let total_entries: usize = cache.values().map(|bucket| bucket.len()).sum();
        if total_entries >= SIMPLE_CACHE_CAP {
            if let Some((_k, bucket)) = cache.iter_mut().find(|(_, bucket)| !bucket.is_empty()) {
                if let Some(first_key) = bucket.keys().next().cloned() {
                    bucket.remove(&first_key);
                }
            }
            cache.retain(|_, bucket| !bucket.is_empty());
        }
        cache
            .entry(cache_key)
            .or_default()
            .insert(text.to_string(), layout_arc.clone());

        let duration = start.elapsed().as_nanos() as u64;
        diag::emit(
            diag::DiagCategory::Paint,
            diag::DiagLevel::Debug,
            diag::DiagEventKind::TextLayoutPerformance {
                text_len: text.len() as u32,
                is_rich: false,
                duration_ns: duration,
            },
        );

        layout_arc
    }

    pub(crate) fn hit_test_layout_impl(
        text: &str,
        layout: &Layout<ParleyBrush>,
        x: f32,
        y: f32,
    ) -> usize {
        let mut target_line: Option<(usize, usize)> = None;
        let mut best_distance = f32::INFINITY;

        for line in layout.lines() {
            let range = line.text_range();
            let metrics = line.metrics();
            let top = metrics.baseline - metrics.ascent;
            let bottom = metrics.baseline + metrics.descent;

            if y >= top && y <= bottom {
                target_line = Some((range.start, range.end));
                break;
            }

            let distance = if y < top { top - y } else { y - bottom };
            if distance < best_distance {
                best_distance = distance;
                target_line = Some((range.start, range.end));
            }
        }

        let Some((line_start, line_end)) = target_line else {
            return text.len();
        };

        if x <= 0.0 {
            return line_start;
        }

        let mut fallback_idx = line_start;

        for line in layout.lines() {
            let line_range = line.text_range();
            if line_range.start != line_start || line_range.end != line_end {
                continue;
            }

            for item in line.items() {
                if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                    let style_range = glyph_run.run().text_range();
                    let run_start = style_range.start.max(line_range.start);
                    let run_end = style_range.end.min(line_range.end);
                    if run_end <= run_start {
                        continue;
                    }

                    let mut cursor_x = glyph_run.offset();
                    if x <= cursor_x {
                        return run_start;
                    }

                    let mut idx = run_start;
                    for glyph in glyph_run.glyphs() {
                        if idx >= run_end {
                            break;
                        }
                        let mid = cursor_x + (glyph.advance * 0.5);
                        if x < mid {
                            return idx;
                        }
                        cursor_x += glyph.advance;
                        idx = Self::next_char_boundary(text, idx).min(run_end);
                    }

                    if x <= cursor_x {
                        return idx.min(run_end);
                    }

                    fallback_idx = run_end;
                }
            }
            break;
        }

        fallback_idx.min(text.len())
    }

    #[allow(dead_code)]
    pub(crate) fn hit_test_layout_index_at_point(
        text: &str,
        layout: &Layout<ParleyBrush>,
        x: f32,
        y: f32,
    ) -> Option<usize> {
        for line in layout.lines() {
            let metrics = line.metrics();
            let top = metrics.baseline - metrics.ascent;
            let bottom = metrics.baseline + metrics.descent;
            if y < top || y > bottom {
                continue;
            }

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

            if !left.is_finite() || x < left || x > right {
                return None;
            }

            return Some(Self::hit_test_layout_impl(text, layout, x, y));
        }

        None
    }

    fn next_char_boundary(text: &str, idx: usize) -> usize {
        if idx >= text.len() {
            return text.len();
        }
        let mut iter = text[idx..].char_indices();
        let _ = iter.next();
        if let Some((next_off, _)) = iter.next() {
            idx + next_off
        } else {
            text.len()
        }
    }

    fn prev_char_boundary(text: &str, idx: usize) -> usize {
        if idx == 0 || idx > text.len() {
            return 0;
        }
        let mut prev = 0usize;
        for (offset, _) in text[..idx].char_indices() {
            prev = offset;
        }
        prev
    }

    pub(crate) fn layout_rich(
        &self,
        text: &str,
        base_size: f32,
        base_color: fission_render::Color,
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
        inline_boxes: &[RichInlineBox],
        width: Option<f32>,
    ) -> Arc<Layout<ParleyBrush>> {
        let start = Instant::now();
        let style_keys: Vec<RichStyleKey> = styles
            .iter()
            .map(|(r, s)| RichStyleKey {
                range: r.clone(),
                font_size_bits: s.font_size.to_bits(),
                color_rgba: [s.color.r, s.color.g, s.color.b, s.color.a],
                underline: s.underline,
                font_family: s.font_family.clone(),
                locale: s.locale.clone(),
                font_weight: s.font_weight,
                font_style: s.font_style,
                line_height_bits: s.line_height.map(f32::to_bits),
                letter_spacing_bits: s.letter_spacing.to_bits(),
                background_color: s.background_color.map(|c| [c.r, c.g, c.b, c.a]),
            })
            .collect();
        let inline_box_keys = inline_boxes
            .iter()
            .map(Self::inline_box_key)
            .collect::<Vec<_>>();

        let cache_key = RichCacheKey {
            text: text.to_string(),
            base_size_bits: base_size.to_bits(),
            base_color_rgba: [base_color.r, base_color.g, base_color.b, base_color.a],
            width_bits: Self::width_bits(width),
            styles: style_keys,
            inline_boxes: inline_box_keys,
        };

        {
            let cache = self.rich_cache.lock().unwrap();
            if let Some(layout) = cache.get(&cache_key) {
                let duration = start.elapsed().as_nanos() as u64;
                diag::emit(
                    diag::DiagCategory::Paint,
                    diag::DiagLevel::Debug,
                    diag::DiagEventKind::TextLayoutPerformance {
                        text_len: text.len() as u32,
                        is_rich: true,
                        duration_ns: duration,
                    },
                );
                return layout.clone();
            }
        }

        let mut font_cx = self.font_cx.lock().unwrap();
        let mut layout_cx = self.layout_cx.lock().unwrap();

        let mut builder = layout_cx.ranged_builder(&mut font_cx, text, 1.0, false);
        builder.push_default(StyleProperty::FontSize(base_size));
        builder.push_default(StyleProperty::FontStack(FontStack::Source(Cow::Owned(
            self.default_family.clone(),
        ))));
        let brush = ParleyBrush([base_color.r, base_color.g, base_color.b, base_color.a]);
        builder.push_default(StyleProperty::Brush(brush));
        for inline_box in inline_boxes {
            builder.push_inline_box(InlineBox {
                id: inline_box.id,
                index: inline_box.index,
                width: inline_box.width,
                height: inline_box.height,
            });
        }

        for (range, style) in styles {
            let brush = ParleyBrush([style.color.r, style.color.g, style.color.b, style.color.a]);
            builder.push(StyleProperty::Brush(brush), range.clone());
            builder.push(StyleProperty::FontSize(style.font_size), range.clone());
            if let Some(font_family) = &style.font_family {
                builder.push(
                    StyleProperty::FontStack(FontStack::Source(Cow::Owned(font_family.clone()))),
                    range.clone(),
                );
            }
            if let Some(locale) = &style.locale {
                builder.push(StyleProperty::Locale(Some(locale.as_str())), range.clone());
            }
            builder.push(
                StyleProperty::FontWeight(FontWeight::new(style.font_weight as f32)),
                range.clone(),
            );
            builder.push(
                StyleProperty::FontStyle(parley_font_style(style.font_style)),
                range.clone(),
            );
            if let Some(line_height) = style.line_height {
                builder.push(
                    StyleProperty::LineHeight(LineHeight::Absolute(line_height)),
                    range.clone(),
                );
            }
            if style.letter_spacing != 0.0 {
                builder.push(
                    StyleProperty::LetterSpacing(style.letter_spacing),
                    range.clone(),
                );
            }
            if style.underline {
                builder.push(StyleProperty::Underline(true), range.clone());
            }
        }

        let mut layout = builder.build(text);
        layout.break_all_lines(width);
        let layout_arc = Arc::new(layout);

        {
            let mut cache = self.rich_cache.lock().unwrap();
            if cache.len() >= RICH_CACHE_CAP {
                if let Some(first_key) = cache.keys().next().cloned() {
                    cache.remove(&first_key);
                }
            }
            cache.insert(cache_key, layout_arc.clone());
        }

        let duration = start.elapsed().as_nanos() as u64;
        diag::emit(
            diag::DiagCategory::Paint,
            diag::DiagLevel::Debug,
            diag::DiagEventKind::TextLayoutPerformance {
                text_len: text.len() as u32,
                is_rich: true,
                duration_ns: duration,
            },
        );

        layout_arc
    }
}

fn upsert_resolved_action(actions: &mut Vec<ActionEntry>, action: ActionEntry) {
    actions.retain(|entry| entry.trigger != action.trigger);
    actions.push(action);
}

fn annotation_contains_index(text: &str, annotation: &RichTextAnnotation, idx: usize) -> bool {
    if annotation.range.start >= annotation.range.end {
        return false;
    }
    if annotation.range.contains(&idx) {
        return true;
    }
    if idx == annotation.range.end {
        let prev = VelloTextMeasurer::prev_char_boundary(text, idx);
        return prev >= annotation.range.start && prev < annotation.range.end;
    }
    false
}

pub(crate) fn resolve_rich_text_annotation_at_index(
    text: &str,
    annotations: &[RichTextAnnotation],
    idx: usize,
) -> Option<RichTextAnnotation> {
    let mut matches = annotations
        .iter()
        .filter(|annotation| annotation_contains_index(text, annotation, idx))
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return None;
    }

    matches.sort_by(|left, right| {
        let left_len = left.range.end.saturating_sub(left.range.start);
        let right_len = right.range.end.saturating_sub(right.range.start);
        right_len
            .cmp(&left_len)
            .then_with(|| left.range.start.cmp(&right.range.start))
    });

    let mut resolved = RichTextAnnotation {
        range: matches.last().expect("matched annotation").range.clone(),
        semantics_label: None,
        semantics_identifier: None,
        spell_out: None,
        mouse_cursor: None,
        actions: Vec::new(),
    };

    for annotation in matches {
        if annotation.semantics_label.is_some() {
            resolved.semantics_label = annotation.semantics_label.clone();
        }
        if annotation.semantics_identifier.is_some() {
            resolved.semantics_identifier = annotation.semantics_identifier.clone();
        }
        if annotation.spell_out.is_some() {
            resolved.spell_out = annotation.spell_out;
        }
        if annotation.mouse_cursor.is_some() {
            resolved.mouse_cursor = annotation.mouse_cursor;
        }
        for action in &annotation.actions {
            upsert_resolved_action(&mut resolved.actions, action.clone());
        }
    }

    Some(resolved)
}

impl TextMeasurer for VelloTextMeasurer {
    fn measure(&self, text: &str, font_size: f32, available_width: Option<f32>) -> (f32, f32) {
        let layout = self.get_layout(text, font_size, available_width);
        Self::layout_measured_size(&layout)
    }

    fn measure_rich_text(&self, runs: &[TextRun], available_width: Option<f32>) -> (f32, f32) {
        if runs.is_empty() {
            return (0.0, 0.0);
        }

        let start = Instant::now();
        let rich = Self::rich_layout_input_from_ir_runs(runs);
        let layout = self.layout_rich(
            &rich.text,
            rich.base_size,
            rich.base_color,
            &rich.styles,
            &rich.inline_boxes,
            available_width,
        );

        let duration = start.elapsed().as_nanos() as u64;
        diag::emit(
            diag::DiagCategory::Paint,
            diag::DiagLevel::Debug,
            diag::DiagEventKind::TextLayoutPerformance {
                text_len: rich.text.len() as u32,
                is_rich: true,
                duration_ns: duration,
            },
        );

        Self::layout_measured_size(&layout)
    }

    fn layout_rich_text(
        &self,
        runs: &[TextRun],
        available_width: Option<f32>,
    ) -> RichTextLayoutInfo {
        if runs.is_empty() {
            return RichTextLayoutInfo {
                width: 0.0,
                height: 0.0,
                inline_boxes: Vec::new(),
            };
        }

        let rich = Self::rich_layout_input_from_ir_runs(runs);
        let layout = self.layout_rich(
            &rich.text,
            rich.base_size,
            rich.base_color,
            &rich.styles,
            &rich.inline_boxes,
            available_width,
        );
        let mut inline_boxes = Vec::new();
        for line in layout.lines() {
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

        let (width, height) = Self::layout_measured_size(&layout);
        RichTextLayoutInfo {
            width,
            height,
            inline_boxes,
        }
    }

    fn hit_test_rich(
        &self,
        runs: &[TextRun],
        available_width: Option<f32>,
        x: f32,
        y: f32,
    ) -> usize {
        if runs.is_empty() {
            return 0;
        }

        // When all runs share the same style, the renderer takes a fast-path
        // through `get_layout()` (the simple/plain cache).  We must do the same
        // here so we look up the SAME cached Parley Layout the renderer painted
        // with, rather than creating a separate entry in the rich cache.
        if let Some(first) = runs.first() {
            let has_inline_boxes = runs.iter().any(|run| {
                run.text.is_empty()
                    && decode_inline_widget_marker(run.style.font_family.as_deref()).is_some()
            });
            if runs.iter().all(|r| r.style == first.style)
                && !has_inline_boxes
                && !text_style_requires_rich_layout(&RenderTextStyle {
                    font_size: first.style.font_size,
                    color: fission_render::Color {
                        r: first.style.color.r,
                        g: first.style.color.g,
                        b: first.style.color.b,
                        a: first.style.color.a,
                    },
                    underline: first.style.underline,
                    font_family: first.style.font_family.clone(),
                    locale: first.style.locale.clone(),
                    font_weight: first.style.font_weight,
                    font_style: first.style.font_style,
                    line_height: first.style.line_height,
                    letter_spacing: first.style.letter_spacing,
                    background_color: first.style.background_color.map(|c| fission_render::Color {
                        r: c.r,
                        g: c.g,
                        b: c.b,
                        a: c.a,
                    }),
                })
            {
                let mut full_text = String::new();
                for run in runs {
                    full_text.push_str(&run.text);
                }
                let layout = self.get_layout(&full_text, first.style.font_size, available_width);
                return Self::hit_test_layout_impl(&full_text, &layout, x, y);
            }
        }

        // Multi-style path: build the same rich layout the renderer uses.
        let rich = Self::rich_layout_input_from_ir_runs(runs);
        let layout = self.layout_rich(
            &rich.text,
            rich.base_size,
            rich.base_color,
            &rich.styles,
            &rich.inline_boxes,
            available_width,
        );

        // Reuse the same hit-test logic as plain text but on the rich layout
        Self::hit_test_layout_impl(&rich.text, &layout, x, y)
    }

    fn resolve_rich_text_annotation_at_point(
        &self,
        runs: &[TextRun],
        available_width: Option<f32>,
        x: f32,
        y: f32,
        _paragraph_style: TextParagraphStyle,
        annotations: &[RichTextAnnotation],
    ) -> Option<RichTextAnnotation> {
        if annotations.is_empty() || runs.is_empty() {
            return None;
        }

        let rich = Self::rich_layout_input_from_ir_runs(runs);
        let layout = self.layout_rich(
            &rich.text,
            rich.base_size,
            rich.base_color,
            &rich.styles,
            &rich.inline_boxes,
            available_width,
        );
        let idx = Self::hit_test_layout_impl(&rich.text, &layout, x, y);
        resolve_rich_text_annotation_at_index(&rich.text, annotations, idx)
    }

    fn hit_test(
        &self,
        text: &str,
        font_size: f32,
        available_width: Option<f32>,
        x: f32,
        y: f32,
    ) -> usize {
        if text.is_empty() {
            return 0;
        }
        let layout = self.get_layout(text, font_size, available_width);
        Self::hit_test_layout_impl(text, &layout, x, y)
    }

    /// Shared hit-test logic over any parley Layout (plain or rich).
    // hit_test_layout_impl is in impl VelloTextMeasurer (inherent block)

    fn get_line_metrics(
        &self,
        text: &str,
        font_size: f32,
        available_width: Option<f32>,
    ) -> Vec<LineMetric> {
        let layout = self.get_layout(text, font_size, available_width);
        layout
            .lines()
            .map(|line| {
                let metrics = line.metrics();
                LineMetric {
                    start_index: line.text_range().start,
                    end_index: line.text_range().end,
                    baseline: metrics.baseline,
                    height: metrics.line_height,
                    width: metrics.advance,
                }
            })
            .collect()
    }

    fn get_caret_position(
        &self,
        text: &str,
        font_size: f32,
        available_width: Option<f32>,
        caret_index: usize,
    ) -> (f32, f32) {
        if text.is_empty() {
            return (0.0, 0.0);
        }

        let layout = self.get_layout(text, font_size, available_width);
        let idx = caret_index.min(text.len());
        let line_count = layout.lines().count();

        for (line_idx, line) in layout.lines().enumerate() {
            let line_range = line.text_range();
            let is_last_line = line_idx + 1 == line_count;
            if !((idx >= line_range.start && idx < line_range.end)
                || (is_last_line && idx == line_range.end))
            {
                continue;
            }

            let mut x_pos = 0.0;
            for item in line.items() {
                if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                    let style_range = glyph_run.run().text_range();
                    let run_start = style_range.start.max(line_range.start);
                    let run_end = style_range.end.min(line_range.end);
                    if run_end <= run_start {
                        continue;
                    }

                    if idx < run_start {
                        break;
                    }

                    if idx >= run_start && idx <= run_end {
                        let mut local_x = glyph_run.offset();
                        let mut current = run_start;
                        for glyph in glyph_run.glyphs() {
                            if current >= idx || current >= run_end {
                                break;
                            }
                            local_x += glyph.advance;
                            current = Self::next_char_boundary(text, current).min(run_end);
                        }
                        x_pos = local_x;
                        break;
                    }

                    x_pos = glyph_run.offset() + glyph_run.advance();
                }
            }

            return (x_pos, line.metrics().baseline);
        }

        (0.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_ir::op::{Color, FontStyle, TextStyle};
    use fission_layout::TextMeasurer;

    fn measurer() -> VelloTextMeasurer {
        VelloTextMeasurer::new(Arc::new(Mutex::new(FontContext::new())))
    }

    fn text_run(text: &str, font_size: f32, line_height: f32) -> TextRun {
        TextRun {
            text: text.to_string(),
            style: TextStyle {
                font_size,
                color: Color::BLACK,
                underline: false,
                font_family: None,
                locale: None,
                font_weight: 800,
                font_style: FontStyle::Normal,
                line_height: Some(line_height),
                letter_spacing: 0.0,
                background_color: None,
            },
        }
    }

    #[test]
    fn rich_text_measurement_height_tracks_wrapped_lines() {
        let measurer = measurer();
        let runs = vec![text_run("Capability-driven field service", 22.0, 28.0)];

        let (_, one_line_height) = measurer.measure_rich_text(&runs, None);
        let (wrapped_width, wrapped_height) = measurer.measure_rich_text(&runs, Some(170.0));

        assert!(wrapped_width <= 170.5);
        assert!(
            wrapped_height > one_line_height * 1.5,
            "wrapped text should report multi-line height; one_line={one_line_height}, wrapped={wrapped_height}"
        );
    }
}
