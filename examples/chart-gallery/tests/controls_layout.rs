use chart_gallery::{GalleryApp, GalleryState};
use fission::prelude::WidgetId;
use fission_test::{op_text, TestHarness};

fn text_rect(harness: &TestHarness<GalleryState>, needle: &str) -> fission::layout::LayoutRect {
    let ir = harness.last_ir.as_ref().expect("ir");
    let snapshot = harness.last_snapshot.as_ref().expect("snapshot");
    ir.nodes
        .iter()
        .filter(|(_, node)| op_text(&node.op).is_some_and(|text| text.starts_with(needle)))
        .find_map(|(id, _)| snapshot.get_node_rect(*id))
        .unwrap_or_else(|| panic!("no laid-out text starting with {needle:?}"))
}

/// The controls must keep their natural height instead of being squashed so
/// the slider row paints over the helper text, and stay on screen.
#[test]
fn chart_controls_keep_their_height_below_the_showcase() {
    let mut harness = TestHarness::new(GalleryState::default()).with_root_widget(GalleryApp);
    harness.env.viewport_size = fission::layout::LayoutSize::new(1024.0, 800.0);
    harness.pump().expect("pump chart gallery");

    let helper = text_rect(&harness, "Interact with the chart");
    let scale_label = text_rect(&harness, "Data scale");
    let snapshot = harness.last_snapshot.as_ref().expect("snapshot");
    let slider = snapshot
        .get_node_rect(WidgetId::explicit("chart-gallery.control.expanded.scale"))
        .expect("slider rect");

    for (name, rect) in [("slider", slider), ("Data scale label", scale_label)] {
        assert!(
            rect.bottom() <= helper.y() + 0.5,
            "{name} ({rect:?}) overlaps the helper text ({helper:?})"
        );
    }
    assert!(
        helper.bottom() <= 800.0 + 0.5,
        "helper text ({helper:?}) is pushed off screen"
    );
}
