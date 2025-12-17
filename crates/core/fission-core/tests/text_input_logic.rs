use fission_core::Runtime;
use fission_layout::TextMeasurer;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

struct MockMeasurer;

impl TextMeasurer for MockMeasurer {
    fn measure(&self, text: &str, font_size: f32, _available_width: Option<f32>) -> (f32, f32) {
        // Simple mock: 10px per char
        let width = text.chars().count() as f32 * 10.0;
        (width, font_size)
    }
    fn hit_test(&self, text: &str, _font_size: f32, _available_width: Option<f32>, x: f32, _y: f32) -> usize {
        let char_width = 10.0;
        if x <= 0.0 { return 0; }
        let char_idx = (x / char_width).round() as usize;
        let mut byte_offset = 0;
        for (idx, g) in text.grapheme_indices(true).take(char_idx) {
            byte_offset = idx + g.len();
        }
        byte_offset
    }
    fn get_line_metrics(&self, text: &str, font_size: f32, available_width: Option<f32>) -> Vec<fission_layout::LineMetric> {
        vec![fission_layout::LineMetric {
            start_index: 0,
            end_index: text.len(),
            baseline: font_size,
            height: font_size,
            width: self.measure(text, font_size, available_width).0,
        }]
    }
    fn get_caret_position(&self, text: &str, font_size: f32, _available_width: Option<f32>, caret_index: usize) -> (f32, f32) {
        let char_width = 10.0;
        let x = text.graphemes(true).take(caret_index).map(|g| g.len()).sum::<usize>() as f32 * char_width;
        let y = font_size; // Baseline
        (x, y)
    }
}

#[test]
fn test_caret_hit_test_precise() {
    let measurer = Arc::new(MockMeasurer);
    let runtime = Runtime::default().with_measurer(measurer);

    let text = "Hello";
    let font_size = 16.0;
    let viewport_x = 0.0;
    let viewport_w = 100.0;
    let content_w = 50.0;
    let scroll_offset = 0.0;

    // "H"  0-10
    // "e" 10-20
    // "l" 20-30
    // "l" 30-40
    // "o" 40-50

    // Click at 4.0 (Left of center of 'H') -> 0
    assert_eq!(runtime.caret_from_point_in_text(text, font_size, viewport_x, viewport_w, content_w, scroll_offset, 4.0), 0);
    
    // Click at 6.0 (Right of center of 'H') -> 1
    assert_eq!(runtime.caret_from_point_in_text(text, font_size, viewport_x, viewport_w, content_w, scroll_offset, 6.0), 1);
    
    // Click at 14.0 (Left of center of 'e') -> 1
    assert_eq!(runtime.caret_from_point_in_text(text, font_size, viewport_x, viewport_w, content_w, scroll_offset, 14.0), 1);
    
    // Click at 16.0 (Right of center of 'e') -> 2
    assert_eq!(runtime.caret_from_point_in_text(text, font_size, viewport_x, viewport_w, content_w, scroll_offset, 16.0), 2);

    // Click at 55.0 (Past end)
    assert_eq!(runtime.caret_from_point_in_text(text, font_size, viewport_x, viewport_w, content_w, scroll_offset, 55.0), 5);
}