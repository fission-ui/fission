use fission_core::ui::{Scroll, Text, TextContent};
use fission_core::{Clock, InputEvent, LayoutPoint, PointerEvent};
use fission_render::DisplayOp;
use fission_test::TestHarness;

#[test]
fn test_scroll_input_updates_display_list() {
    let mut harness = TestHarness::new_with_mock_measurer(Clock::default());

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

    let debug = std::env::var("FISSION_TEST_DEBUG").is_ok();

    // Verify hit test finds something at (5,5)
    if debug {
        if let (Some(ir), Some(snap)) = (&harness.last_ir, &harness.last_snapshot) {
            let pre_hit = fission_core::hit_test::hit_test_with_scroll(
                ir,
                snap,
                &harness.runtime.runtime_state.scroll,
                LayoutPoint::new(5.0, 5.0),
            );
            eprintln!("Debug: pre_hit={:?}", pre_hit);

            // List scroll nodes present
            for (id, node) in &ir.nodes {
                if let fission_core::Op::Layout(fission_core::LayoutOp::Scroll { .. }) = node.op {
                    eprintln!("Debug: scroll_node id={:?}", id);
                }
            }
        }
    }

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

    // Inspect updated offset before pumping
    // Dump any non-zero scroll offsets for nodes in the current IR
    if debug {
        if let Some(ir) = &harness.last_ir {
            for (id, _node) in &ir.nodes {
                let off = harness.runtime.runtime_state.scroll.get_offset(*id);
                if off != 0.0 {
                    eprintln!("Debug: node {:?} offset {}", id, off);
                }
            }
        }
    }

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

    assert!(
        found_translate,
        "Did not find expected translation of -10.0 in display list"
    );
}
