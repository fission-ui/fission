use fission_test::TestHarness;
use fission_core::{InputEvent, PointerEvent, LayoutPoint, AppState};
use fission_core::ui::{Scroll, Text, TextContent};
use fission_render::DisplayOp;
use serde::{Serialize, Deserialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct DummyState;
impl AppState for DummyState {}

#[test]
fn test_scroll_input_updates_display_list() {
    let mut harness = TestHarness::new(DummyState); 
    
    harness = harness.with_root_widget(Scroll {
        child: Some(Box::new(Text { content: TextContent::Literal("Scroll Me".into()), ..Default::default() }.into())),
        ..Default::default()
    });
    
    harness.pump().expect("Initial pump failed");
    
    // Initial Check: Should not have significant translation
    
    // Simulate Scroll Event
    harness.send_event(InputEvent::Pointer(PointerEvent::Scroll {
        point: LayoutPoint::new(10.0, 10.0),
        delta: LayoutPoint::new(0.0, 50.0), // Scroll "Down"
    })).expect("Event dispatch failed");
    
    harness.pump().expect("Second pump failed");
    
    let dl = harness.get_last_display_list().expect("No display list");
    
    // Check for Translate Op
    let mut found_translate = false;
    for op in dl.ops {
        if let DisplayOp::Translate(pt) = op {
            if pt.y == -50.0 {
                found_translate = true;
                break;
            }
        }
    }
    
    assert!(found_translate, "Did not find expected translation of -50.0 in display list");
}