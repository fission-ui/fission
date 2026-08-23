use std::collections::HashSet;

use fission_core::ui::{Container, Scroll};
use fission_core::{Runtime, ScrollStateMap, WidgetIdExt};
use fission_ir::{CoreIR, FlexDirection, LayoutOp, Op, WidgetId};

#[test]
fn scroll_state_discards_unmounted_scroll_nodes() {
    let active = WidgetId::explicit("active-scroll");
    let inactive = WidgetId::explicit("inactive-scroll");
    let mut scroll = ScrollStateMap::default();
    scroll.set_offset(active, 40.0);
    scroll.set_offset(inactive, 120.0);

    scroll.retain_active(&HashSet::from([active]));

    assert_eq!(scroll.get_offset(active), 40.0);
    assert_eq!(scroll.get_offset(inactive), 0.0);
}

#[test]
fn runtime_reconciles_replacement_scroll_state_before_layout() {
    let review = WidgetId::explicit("flow.review.scroll");
    let installing = WidgetId::explicit("flow.installing.scroll");
    let mut runtime = Runtime::default();

    runtime.runtime_state.scroll.set_offset(review, 640.0);
    runtime.reconcile_ir(&scroll_ir(review));
    assert_eq!(runtime.runtime_state.scroll.get_offset(review), 640.0);

    runtime.reconcile_ir(&scroll_ir(installing));

    assert_eq!(runtime.runtime_state.scroll.get_offset(review), 0.0);
    assert_eq!(runtime.runtime_state.scroll.get_offset(installing), 0.0);
}

#[test]
fn runtime_preserves_scroll_state_for_the_same_mounted_identity() {
    let scroll = WidgetId::explicit("flow.review.scroll");
    let mut runtime = Runtime::default();
    runtime.runtime_state.scroll.set_offset(scroll, 320.0);

    runtime.reconcile_ir(&scroll_ir(scroll));
    runtime.reconcile_ir(&scroll_ir(scroll));

    assert_eq!(runtime.runtime_state.scroll.get_offset(scroll), 320.0);
}

#[test]
fn explicit_screen_scope_changes_implicit_descendant_scroll_identity() {
    let review = scoped_scroll_id("flow.review");
    let installing = scoped_scroll_id("flow.installing");

    assert_ne!(review, installing);
}

fn scoped_scroll_id(scope: &str) -> WidgetId {
    let widget = Container::new(Scroll::default()).id(WidgetId::explicit(scope));
    let ir = fission_core::internal::lower_widget_to_ir(&widget);
    ir.nodes
        .iter()
        .find_map(|(id, node)| {
            matches!(node.op, Op::Layout(LayoutOp::Scroll { .. })).then_some(*id)
        })
        .expect("scoped screen should contain a scroll node")
}

fn scroll_ir(scroll: WidgetId) -> CoreIR {
    let mut ir = CoreIR::new();
    ir.add_node(
        scroll,
        Op::Layout(LayoutOp::Scroll {
            direction: FlexDirection::Column,
            show_scrollbar: false,
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 1.0,
            flex_shrink: 0.0,
        }),
        Vec::new(),
    );
    ir.set_root(scroll);
    ir
}
