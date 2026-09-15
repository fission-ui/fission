//! Layout regression: the filter chips must never run under the sort select in
//! narrow windows.

use fission::layout::LayoutSize;
use fission::render::LayoutRect;
use fission_ir::Op;
use fission_test::TestHarness;
use inbox::{create_env, InboxApp, InboxState};

fn pump_at(width: f32, height: f32) -> TestHarness<InboxState> {
    let mut h = TestHarness::new(InboxState::default()).with_root_widget(InboxApp);
    h.env = create_env();
    h.env.viewport_size = LayoutSize::new(width, height);
    h.pump().expect("inbox should build and lay out");
    h
}

fn intersects(a: &LayoutRect, b: &LayoutRect) -> bool {
    a.origin.x < b.origin.x + b.size.width
        && b.origin.x < a.origin.x + a.size.width
        && a.origin.y < b.origin.y + b.size.height
        && b.origin.y < a.origin.y + a.size.height
}

fn text_rects(h: &TestHarness<InboxState>, needle: &str) -> Vec<LayoutRect> {
    let ir = h.last_ir.as_ref().expect("pump should produce IR");
    let snapshot = h.last_snapshot.as_ref().expect("pump should lay out");
    ir.nodes
        .iter()
        .filter(|(_, node)| node.op.text().as_deref() == Some(needle))
        .filter_map(|(id, _)| snapshot.get_node_rect(*id))
        .filter(|rect| rect.size.width > 0.0 && rect.size.height > 0.0)
        .collect()
}

fn sort_select_rect(h: &TestHarness<InboxState>) -> LayoutRect {
    let ir = h.last_ir.as_ref().expect("pump should produce IR");
    let snapshot = h.last_snapshot.as_ref().expect("pump should lay out");
    ir.nodes
        .iter()
        .find_map(|(id, node)| match &node.op {
            Op::Semantics(s) if s.identifier.as_deref() == Some("inbox.sort") => {
                snapshot.get_node_rect(*id)
            }
            _ => None,
        })
        .expect("sort select trigger should be laid out")
}

fn assert_chips_clear_of_sort(width: f32) {
    let h = pump_at(width, 800.0);
    let select = sort_select_rect(&h);
    for label in ["All", "Unread", "Starred"] {
        let rects = text_rects(&h, label);
        assert!(!rects.is_empty(), "{label} chip should render at {width}px");
        for rect in rects {
            assert!(
                !intersects(&rect, &select),
                "{label} chip {rect:?} overlaps the sort select {select:?} at {width}px"
            );
        }
    }
}

#[test]
fn filter_chips_clear_sort_select_at_390() {
    assert_chips_clear_of_sort(390.0);
}

#[test]
fn filter_chips_clear_sort_select_at_600() {
    assert_chips_clear_of_sort(600.0);
}

#[test]
fn filter_chips_clear_sort_select_at_1200() {
    assert_chips_clear_of_sort(1200.0);
}
