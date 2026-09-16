//! Layout regressions for the gallery's navigation and headings at the width
//! the example showcase gives its preview.

use chart_gallery::{GalleryApp, GalleryState};
use fission::layout::LayoutRect;
use fission_test::{op_text, TestHarness};

const WIDTH: f32 = 880.0;
const HEIGHT: f32 = 660.0;

fn pump(state: GalleryState) -> TestHarness<GalleryState> {
    let mut harness = TestHarness::new(state).with_root_widget(GalleryApp);
    harness.env.viewport_size = fission::layout::LayoutSize::new(WIDTH, HEIGHT);
    harness.pump().expect("pump chart gallery");
    harness
}

fn text_rects(harness: &TestHarness<GalleryState>, needle: &str) -> Vec<LayoutRect> {
    let ir = harness.last_ir.as_ref().expect("ir");
    let snapshot = harness.last_snapshot.as_ref().expect("snapshot");
    ir.nodes
        .iter()
        .filter(|(_, node)| op_text(&node.op).is_some_and(|text| text == needle))
        .filter_map(|(id, _)| snapshot.get_node_rect(*id))
        .filter(|rect| rect.size.width > 0.0 && rect.size.height > 0.0)
        .collect()
}

/// The narrow chart picker used to be one horizontal scroll row that cut off
/// its last tab. Every chart in the chosen category must now be fully visible.
#[test]
fn compact_chart_picker_shows_its_last_chart_within_bounds() {
    // Coordinates is the last built-in category, so its charts sat far past the
    // right edge of the old single scrolling row.
    let harness = pump(GalleryState {
        selected_category: 7,
        selected_chart: 0,
        ..GalleryState::default()
    });

    for label in ["Polar Bar", "Single Axis"] {
        let rects = text_rects(&harness, label);
        let rect = rects
            .first()
            .unwrap_or_else(|| panic!("{label:?} is not laid out"));
        assert!(
            rect.x() >= 0.0 && rect.right() <= WIDTH + 0.5 && rect.bottom() <= HEIGHT + 0.5,
            "{label:?} ({rect:?}) is outside the {WIDTH}x{HEIGHT} viewport"
        );
    }
}

/// Standalone, the gallery names itself once and titles the overview page
/// after the tab that opens it, not with a second app name.
#[test]
fn standalone_gallery_has_a_single_app_title() {
    let harness = pump(GalleryState::default());

    assert_eq!(text_rects(&harness, "Chart Gallery").len(), 1);
    assert!(text_rects(&harness, "Chart Showcase").is_empty());
}

/// Embedded in the showcase, whose header already names the example, the
/// gallery drops its own app title in both layouts.
#[test]
fn embedded_gallery_drops_its_app_title() {
    for width in [WIDTH, 1280.0] {
        let mut harness =
            TestHarness::new(chart_gallery::embedded_state()).with_root_widget(GalleryApp);
        harness.env.viewport_size = fission::layout::LayoutSize::new(width, HEIGHT);
        harness.pump().expect("pump chart gallery");
        assert!(
            text_rects(&harness, "Chart Gallery").is_empty(),
            "app title shown at width {width}"
        );
    }
}
