//! Visual-hierarchy regression: the counter offers one primary action.
//!
//! Two filled buttons side by side give a view no main action, so decrementing
//! is drawn as an outline button while incrementing keeps the filled primary
//! treatment.

use fission::op::{Fill, Op, PaintOp};
use fission_test::{op_text, TestHarness};

const WIDTH: f32 = 600.0;
const HEIGHT: f32 = 400.0;

fn pump() -> TestHarness<()> {
    let mut harness = TestHarness::new(()).with_root_widget(counter::CounterApp {});
    harness.env.viewport_size = fission::layout::LayoutSize::new(WIDTH, HEIGHT);
    harness.pump().expect("pump counter");
    harness
}

/// The painted background behind a labelled control.
///
/// A button paints its surface as a rectangle above its label, so the search
/// walks up from the text to the nearest ancestor that paints one.
fn button_background(harness: &TestHarness<()>, label: &str) -> Option<Fill> {
    let ir = harness.last_ir.as_ref().expect("ir");
    let text = ir
        .nodes
        .values()
        .find(|node| op_text(&node.op).as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no text node reads {label:?}"));

    let mut current = text.parent;
    while let Some(id) = current {
        let node = &ir.nodes[&id];
        let painted = node.children.iter().find_map(|child| match &ir.nodes[child].op {
            Op::Paint(PaintOp::DrawRect { fill, .. }) => fill.clone(),
            _ => None,
        });
        if painted.is_some() {
            return painted;
        }
        current = node.parent;
    }
    None
}

#[test]
fn decrement_does_not_share_the_increment_button_fill() {
    let harness = pump();
    let decrement = button_background(&harness, "Decrement");
    let increment = button_background(&harness, "Increment");

    assert!(
        increment.is_some(),
        "the primary action should paint a filled surface"
    );
    assert_ne!(
        decrement, increment,
        "Decrement and Increment must not paint the same fill; one primary action per view"
    );
}

#[test]
fn both_counter_buttons_are_laid_out() {
    let harness = pump();
    let ir = harness.last_ir.as_ref().expect("ir");
    let snapshot = harness.last_snapshot.as_ref().expect("snapshot");

    for label in ["Decrement", "Increment"] {
        let rect = ir
            .nodes
            .iter()
            .filter(|(_, node)| op_text(&node.op).as_deref() == Some(label))
            .find_map(|(id, _)| snapshot.get_node_rect(*id))
            .unwrap_or_else(|| panic!("{label:?} is not laid out"));
        assert!(
            rect.width() > 0.0 && rect.height() > 0.0,
            "{label:?} should occupy space, got {rect:?}"
        );
    }
}
