use fission_layout::{TextMeasurer, LineMetric};
use fission_ir::op::TextRun;
use parley::layout::Layout;
use parley::style::{FontStack, StyleProperty};
use parley::{FontContext, LayoutContext};
use std::sync::{Arc, Mutex};
use std::borrow::Cow;
use std::collections::HashMap;
use fission_render::TextStyle as RenderTextStyle;

#[derive(Clone, Debug, PartialEq)]
pub struct ParleyBrush(pub [u8; 4]);

impl Default for ParleyBrush {
    fn default() -> Self { Self([0, 0, 0, 255]) }
}

#[derive(Hash, PartialEq, Eq)]
struct SimpleLayoutKey {
    text: String,
    font_size_bits: u32,
    width_bits: Option<u32>,
}

pub struct VelloTextMeasurer {
    font_cx: Arc<Mutex<FontContext>>,
    layout_cx: Mutex<LayoutContext<ParleyBrush>>,
    // Simple cache for single-style text
    simple_cache: Mutex<HashMap<SimpleLayoutKey, Arc<Layout<ParleyBrush>>>>,
    default_family: String,
}

impl VelloTextMeasurer {
    pub fn new(font_cx: Arc<Mutex<FontContext>>) -> Self {
        Self {
            font_cx,
            layout_cx: Mutex::new(LayoutContext::new()),
            simple_cache: Mutex::new(HashMap::new()),
            default_family: "system-ui".to_string(),
        }
    }

    pub fn new_with_default_family(font_cx: Arc<Mutex<FontContext>>, family: impl Into<String>) -> Self {
        Self {
            font_cx,
            layout_cx: Mutex::new(LayoutContext::new()),
            simple_cache: Mutex::new(HashMap::new()),
            default_family: family.into(),
        }
    }

    pub fn font_cx(&self) -> Arc<Mutex<FontContext>> {
        self.font_cx.clone()
    }

    pub fn get_layout(&self, text: &str, font_size: f32, width: Option<f32>) -> Arc<Layout<ParleyBrush>> {
        let key = SimpleLayoutKey {
            text: text.to_string(),
            font_size_bits: font_size.to_bits(),
            width_bits: width.map(|w| w.to_bits()),
        };

        let mut cache = self.simple_cache.lock().unwrap();
        if let Some(layout) = cache.get(&key) {
            return layout.clone();
        }

        let mut font_cx = self.font_cx.lock().unwrap();
        let mut layout_cx = self.layout_cx.lock().unwrap();
        
        let mut builder = layout_cx.ranged_builder(&mut font_cx, text, 1.0, false);
        
        builder.push_default(StyleProperty::FontSize(font_size));
        builder.push_default(StyleProperty::FontStack(FontStack::Source(Cow::Owned(self.default_family.clone()))));
        
        let mut layout = builder.build(text);
        layout.break_all_lines(width);
        
        let layout_arc = Arc::new(layout);
        
        // Simple eviction: if too big, clear.
        if cache.len() > 500 {
            cache.clear();
        }
        cache.insert(key, layout_arc.clone());
        
        layout_arc
    }

    pub fn layout_rich(&self, text: &str, base_size: f32, base_color: fission_render::Color, styles: &[(std::ops::Range<usize>, RenderTextStyle)], width: Option<f32>) -> Layout<ParleyBrush> {
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
        layout
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
        // TODO: Cache rich text layouts too. For now, only simple text is cached.
        
        let mut font_cx = self.font_cx.lock().unwrap();
        let mut layout_cx = self.layout_cx.lock().unwrap();
        
        let mut full_text = String::new();
        for run in runs {
            full_text.push_str(&run.text);
        }
        
        let mut builder = layout_cx.ranged_builder(&mut font_cx, &full_text, 1.0, false);
        builder.push_default(StyleProperty::FontStack(FontStack::Source(Cow::Owned(self.default_family.clone()))));
        builder.push_default(StyleProperty::FontSize(16.0));
        
        let mut offset = 0;
        for run in runs {
            let len = run.text.len();
            if len > 0 {
                let range = offset..(offset + len);
                builder.push(StyleProperty::FontSize(run.style.font_size), range.clone());
                let color = run.style.color;
                let brush = ParleyBrush([color.r, color.g, color.b, color.a]);
                builder.push(StyleProperty::Brush(brush), range.clone());
                
                offset += len;
            }
        }
        
        let mut layout = builder.build(&full_text);
        layout.break_all_lines(available_width);
        (layout.width(), layout.height())
    }

    fn hit_test(&self, text: &str, font_size: f32, available_width: Option<f32>, _x: f32, _y: f32) -> usize {
        let _layout = self.get_layout(text, font_size, available_width);
        // Simplified hit test: find line, then glyph. 
        // For now, return 0 or implement properly if needed for selection.
        0
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

    fn get_caret_position(&self, text: &str, font_size: f32, available_width: Option<f32>, _caret_index: usize) -> (f32, f32) {
        let _layout = self.get_layout(text, font_size, available_width);
        (0.0, 0.0)
    }
}
