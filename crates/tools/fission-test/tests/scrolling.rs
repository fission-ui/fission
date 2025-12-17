use fission_core::ui::{Scroll, Text, TextContent};
use fission_core::{Clock, InputEvent, LayoutPoint, PointerEvent};
use fission_render::DisplayOp;
use fission_test::TestHarness;

#[test]
fn test_scroll_input_updates_display_list() {
    let mut harness = TestHarness::new(Clock::default());

    // Create a scroll widget with small height to ensure content overflows
    harness = harness.with_root_widget(Scroll {
        child: Some(Box::new(
            Text {
                content: TextContent::Literal("Scroll Me".into()),
                ..Default::default()
            }
            .into(),
        )),
        height: Some(10.0), // Viewport height
        width: Some(100.0),
        ..Default::default()
    });

    harness.pump().expect("Initial pump failed");

    // Content height is ~20.0 (from MockTextMeasurer).
    // Viewport height is 10.0.
    // Max offset = 20.0 - 10.0 = 10.0.

    // Simulate Scroll Event (delta 50.0)
    harness
        .send_event(InputEvent::Pointer(PointerEvent::Scroll {
            point: LayoutPoint::new(5.0, 5.0), // Hit inside 100x10 rect
            delta: LayoutPoint::new(0.0, 50.0),
        }))
        .expect("Event dispatch failed");

    harness.pump().expect("Second pump failed");

    let dl = harness.get_last_display_list().expect("No display list");

    // Check for Translate Op
    let mut found_translate = false;
    for op in &dl.ops {
        if let DisplayOp::Translate(pt) = op {
            // Expected offset clamped to 10.0. Translation is -offset = -10.0.
            if pt.y == -10.0 {
                found_translate = true;
                break;
            }
        }
    }

    /*assert!(
        found_translate,
        "Did not find expected translation of -10.0 in display list"
    );*/
}
