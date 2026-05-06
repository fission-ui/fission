use fission_core::Runtime;
use fission_core::TextMeasurer;
use fission_render_vello::parley::FontContext;
use fission_render_vello::VelloTextMeasurer;
use std::sync::{Arc, Mutex};

#[test]
#[ignore]
fn test_text_input_hit_test_with_vello() {
    let font_cx = Arc::new(Mutex::new(FontContext::default()));
    let measurer = Arc::new(VelloTextMeasurer::new(font_cx.clone()));
    let runtime = Runtime::default().with_measurer(measurer.clone());

    let text = "Hello World";
    let font_size = 16.0;

    let (width_he, _) = measurer.measure("He", font_size, None);
    let (width_hel, _) = measurer.measure("Hel", font_size, None);

    let mid = (width_he + width_hel) / 2.0;

    let idx_left =
        runtime.caret_from_point_in_text(text, font_size, 0.0, 1000.0, 1000.0, 0.0, mid - 0.1);
    assert_eq!(idx_left, 2, "Click left of 'l' center should yield index 2");

    let idx_right =
        runtime.caret_from_point_in_text(text, font_size, 0.0, 1000.0, 1000.0, 0.0, mid + 0.1);
    assert_eq!(
        idx_right, 3,
        "Click right of 'l' center should yield index 3"
    );

    let text_var = "iiiMMM";
    let (width_i, _) = measurer.measure("i", font_size, None);
    let (width_m, _) = measurer.measure("M", font_size, None);

    assert!(
        width_m > width_i * 1.5,
        "M should be significantly wider than i in variable width font"
    );

    let (width_3i, _) = measurer.measure("iii", font_size, None);
    let (width_3i_m, _) = measurer.measure("iiiM", font_size, None);

    let mid_m = (width_3i + width_3i_m) / 2.0;

    let idx_m = runtime.caret_from_point_in_text(
        text_var,
        font_size,
        0.0,
        1000.0,
        1000.0,
        0.0,
        mid_m + 0.1,
    );
    assert_eq!(idx_m, 4, "Click right of center of M should yield index 4");
}
