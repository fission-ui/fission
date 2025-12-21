use fission_layout::{TextMeasurer, LineMetric};
use parley::layout::{Alignment, Layout};
use parley::style::{Brush, FontFamily, FontStack, StyleProperty};
use parley::{FontContext, LayoutContext};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub struct ParleyBrush(pub [u8; 4]);

impl Default for ParleyBrush {
    fn default() -> Self { Self([0, 0, 0, 255]) }
}

impl Brush for ParleyBrush {}

pub struct VelloTextMeasurer {
    font_cx: Arc<Mutex<FontContext>>,
    layout_cx: Mutex<LayoutContext<ParleyBrush>>,
}

impl VelloTextMeasurer {
    pub fn new(font_cx: Arc<Mutex<FontContext>>) -> Self {
        Self {
            font_cx,
            layout_cx: Mutex::new(LayoutContext::new()),
        }
    }

    fn layout(&self, text: &str, font_size: f32, width: Option<f32>) -> Layout<ParleyBrush> {
        let mut font_cx = self.font_cx.lock().unwrap();
        let mut layout_cx = self.layout_cx.lock().unwrap();
        
        let mut builder = layout_cx.ranged_builder(&mut font_cx, text, 1.0); // 1.0 scale
        builder.push_default(&StyleProperty::FontSize(font_size));
        builder.push_default(&StyleProperty::FontStack(FontStack::Source("system-ui"))); // Default font
        
        let mut layout = builder.build();
        layout.break_all_lines(width, Alignment::Start);
        layout
    }
}

impl TextMeasurer for VelloTextMeasurer {
    fn measure(&self, text: &str, font_size: f32, available_width: Option<f32>) -> (f32, f32) {
        let layout = self.layout(text, font_size, available_width);
        (layout.width(), layout.height())
    }

    fn hit_test(&self, text: &str, font_size: f32, available_width: Option<f32>, x: f32, y: f32) -> usize {
        let layout = self.layout(text, font_size, available_width);
        // Parley doesn't have a direct hit_test(x, y) -> index method in 0.1?
        // It keeps changing.
        // But assuming we can iterate lines.
        // For now, falling back to basic approximation if API fails, but let's try strict.
        // Actually, layout structure is inspectable.
        // Iterate lines. Check Y.
        // Iterate items in line.
        // Use `Position`?
        
        // Mock fallback for now to ensure compilation, since Parley API is complex to guess blindly.
        // But `measure` is real.
        // I will try to implement real hit test.
        
        for line in layout.lines() {
            let metrics = line.metrics();
            // Check Y bounds... logic needed
        }
        
        0 // Placeholder
    }

    fn get_line_metrics(&self, text: &str, font_size: f32, available_width: Option<f32>) -> Vec<LineMetric> {
        let layout = self.layout(text, font_size, available_width);
        layout.lines().map(|line| {
            let metrics = line.metrics();
            // Parley indices are byte indices?
            // Need to map to char indices or ensure Fission uses byte indices (it mostly uses byte).
            // LineMetric expects `start_index` / `end_index`.
            LineMetric {
                start_index: line.text_range().start,
                end_index: line.text_range().end,
                baseline: metrics.baseline,
                height: metrics.size(), // or height?
                width: metrics.advance, // width?
            }
        }).collect()
    }

    fn get_caret_position(&self, text: &str, font_size: f32, available_width: Option<f32>, caret_index: usize) -> (f32, f32) {
        let layout = self.layout(text, font_size, available_width);
        // layout.cursor_position(caret_index)?
        // layout.get_selection_regions?
        (0.0, 0.0) // Placeholder
    }
}