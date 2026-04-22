use fission_layout::{TextMeasurer, LineMetric};
use fission_ir::op::{TextRun, LayoutUnit};
use parley::layout::{Layout, PositionedLayoutItem};
use parley::style::{FontStack, StyleProperty};
use parley::{FontContext, LayoutContext};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use fission_render::TextStyle as RenderTextStyle;
use fission_diagnostics::prelude as diag;

#[derive(Clone, Debug, PartialEq)]
pub struct ParleyBrush(pub [u8; 4]);

impl Default for ParleyBrush {
    fn default() -> Self { Self([0, 0, 0, 255]) }
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
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RichStyleKey {
    range: std::ops::Range<usize>,
    font_size_bits: u32,
    color_rgba: [u8; 4],
    underline: bool,
    background_color: Option<[u8; 4]>,
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

    pub fn new_with_default_family(font_cx: Arc<Mutex<FontContext>>, family: impl Into<String>) -> Self {
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

    pub fn get_layout(&self, text: &str, font_size: f32, width: Option<f32>) -> Arc<Layout<ParleyBrush>> {
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
        builder.push_default(StyleProperty::FontStack(FontStack::Source(Cow::Owned(self.default_family.clone()))));
        
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

    pub fn layout_rich(&self, text: &str, base_size: f32, base_color: fission_render::Color, styles: &[(std::ops::Range<usize>, RenderTextStyle)], width: Option<f32>) -> Arc<Layout<ParleyBrush>> {
        let start = Instant::now();
        let style_keys: Vec<RichStyleKey> = styles.iter().map(|(r, s)| RichStyleKey {
            range: r.clone(),
            font_size_bits: s.font_size.to_bits(),
            color_rgba: [s.color.r, s.color.g, s.color.b, s.color.a],
            underline: s.underline,
            background_color: s.background_color.map(|c| [c.r, c.g, c.b, c.a]),
        }).collect();

        let cache_key = RichCacheKey {
            text: text.to_string(),
            base_size_bits: base_size.to_bits(),
            base_color_rgba: [base_color.r, base_color.g, base_color.b, base_color.a],
            width_bits: Self::width_bits(width),
            styles: style_keys,
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
        builder.push_default(StyleProperty::FontStack(FontStack::Source(Cow::Owned(self.default_family.clone()))));
        let brush = ParleyBrush([base_color.r, base_color.g, base_color.b, base_color.a]);
        builder.push_default(StyleProperty::Brush(brush));
        
        for (range, style) in styles {
            let brush = ParleyBrush([style.color.r, style.color.g, style.color.b, style.color.a]);
            builder.push(StyleProperty::Brush(brush), range.clone());
            builder.push(StyleProperty::FontSize(style.font_size), range.clone());
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

impl TextMeasurer for VelloTextMeasurer {
    fn measure(&self, text: &str, font_size: f32, available_width: Option<f32>) -> (f32, f32) {
        let layout = self.get_layout(text, font_size, available_width);
        (layout.width(), layout.height())
    }

    fn measure_rich_text(&self, runs: &[TextRun], available_width: Option<f32>) -> (f32, f32) {
        if runs.is_empty() {
            return (0.0, 0.0);
        }
        
        let start = Instant::now();
        let mut full_text = String::new();
        let mut styles = Vec::new();
        let mut offset = 0;
        for run in runs {
            let len = run.text.len();
            full_text.push_str(&run.text);
            styles.push((
                offset..(offset + len),
                RenderTextStyle {
                    font_size: run.style.font_size,
                    color: fission_render::Color {
                        r: run.style.color.r,
                        g: run.style.color.g,
                        b: run.style.color.b,
                        a: run.style.color.a,
                    },
                    underline: run.style.underline,
                    background_color: run.style.background_color.map(|c| fission_render::Color {
                        r: c.r, g: c.g, b: c.b, a: c.a,
                    }),
                },
            ));
            offset += len;
        }

        let (base_size, base_color) = if let Some(first) = runs.first() {
            (first.style.font_size, fission_render::Color {
                r: first.style.color.r,
                g: first.style.color.g,
                b: first.style.color.b,
                a: first.style.color.a,
            })
        } else {
            (16.0, fission_render::Color { r: 0, g: 0, b: 0, a: 255 })
        };

        let layout = self.layout_rich(&full_text, base_size, base_color, &styles, available_width);
        
        let duration = start.elapsed().as_nanos() as u64;
        diag::emit(
            diag::DiagCategory::Paint,
            diag::DiagLevel::Debug,
            diag::DiagEventKind::TextLayoutPerformance {
                text_len: full_text.len() as u32,
                is_rich: true,
                duration_ns: duration,
            },
        );

        (layout.width(), layout.height())
    }

    fn hit_test(&self, text: &str, font_size: f32, available_width: Option<f32>, x: f32, y: f32) -> usize {
        if text.is_empty() {
            return 0;
        }

        let layout = self.get_layout(text, font_size, available_width);
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

    fn get_line_metrics(&self, text: &str, font_size: f32, available_width: Option<f32>) -> Vec<LineMetric> {
        let layout = self.get_layout(text, font_size, available_width);
        layout.lines().map(|line| {
            let metrics = line.metrics();
            LineMetric {
                start_index: line.text_range().start,
                end_index: line.text_range().end,
                baseline: metrics.baseline,
                height: metrics.line_height, 
                width: metrics.advance, 
            }
        }).collect()
    }

    fn get_caret_position(&self, text: &str, font_size: f32, available_width: Option<f32>, caret_index: usize) -> (f32, f32) {
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
