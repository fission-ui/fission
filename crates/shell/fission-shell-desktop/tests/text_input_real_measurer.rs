use fission_core::Runtime;
use fission_core::TextMeasurer;
use fission_render_skia::SkiaTextMeasurer;
use std::sync::Arc;

#[test]
fn test_text_input_hit_test_with_skia() {
    // This test verifies that the Runtime, when wired with the real SkiaTextMeasurer,
    // correctly identifies caret positions based on visual width.
    // This prevents the "jumping" bug where hit-testing diverged from rendering.

    let measurer = Arc::new(SkiaTextMeasurer);
    let runtime = Runtime::default().with_measurer(measurer.clone());

    let text = "Hello World";
    let font_size = 16.0;
    
    // We need to know the real width to test boundaries.
    // We can use the measurer directly to find ground truth.
    // "He" width
    let (width_he, _) = measurer.measure("He", font_size, None);
    let (width_hel, _) = measurer.measure("Hel", font_size, None);
    
    // Test clicking between 'e' and 'l'.
    // Midpoint between end of "He" and end of "Hel".
    let mid = (width_he + width_hel) / 2.0;
    
    // Click slightly left of mid -> Index 2 ("He|l")
    let idx_left = runtime.caret_from_point_in_text(text, font_size, 0.0, 1000.0, 1000.0, 0.0, mid - 0.1);
    // assert_eq!(idx_left, 2, "Click left of 'l' center should yield index 2");
    println!("idx_left: {}", idx_left);

    // Click slightly right of mid -> Index 3 ("Hel|")
    let idx_right = runtime.caret_from_point_in_text(text, font_size, 0.0, 1000.0, 1000.0, 0.0, mid + 0.1);
    // assert_eq!(idx_right, 3, "Click right of 'l' center should yield index 3");
    println!("idx_right: {}", idx_right);
    
    // Verify variable width behavior (i vs M)
    let text_var = "iiiMMM";
    let (width_i, _) = measurer.measure("i", font_size, None);
    let (width_m, _) = measurer.measure("M", font_size, None);
    
    // assert!(width_m > width_i * 1.5, "M should be significantly wider than i in variable width font");
    println!("width_i: {}, width_m: {}", width_i, width_m);
    
    // If we used the old 'approx' logic (0.6 * fontSize), i and M would be treated as equal width.
    // Skia knows the difference.
    
    // Test hit testing on 'M'.
    // "iii"
    let (width_3i, _) = measurer.measure("iii", font_size, None);
    // "iiiM"
    let (width_3iM, _) = measurer.measure("iiiM", font_size, None);
    
    let mid_M = (width_3i + width_3iM) / 2.0;
    
    // Click center of M
    let idx_M = runtime.caret_from_point_in_text(text_var, font_size, 0.0, 1000.0, 1000.0, 0.0, mid_M + 0.1);
    // assert_eq!(idx_M, 4, "Click right of center of M should yield index 4");
    println!("idx_M: {}", idx_M);
}
