use std::collections::HashSet;
use std::sync::Arc;

use fission_core::authoring::LoweringContext;
use fission_core::env::{Env, RuntimeState};
use fission_core::internal::build_layout_tree;
use fission_core::ui::{Container, Scroll, TextInput, Widget};
use fission_core::{Runtime, ScrollStateMap, WidgetIdExt};
use fission_ir::{CoreIR, FlexDirection, LayoutOp, Op, WidgetId};
use fission_layout::{LayoutEngine, LayoutSize, LineMetric, TextMeasurer};

struct FixedTextMeasurer;

impl TextMeasurer for FixedTextMeasurer {
    fn measure(&self, text: &str, _font_size: f32, _available_width: Option<f32>) -> (f32, f32) {
        (text.chars().count() as f32 * 10.0, 20.0)
    }

    fn get_line_metrics(
        &self,
        text: &str,
        _font_size: f32,
        _available_width: Option<f32>,
    ) -> Vec<LineMetric> {
        vec![LineMetric {
            start_index: 0,
            end_index: text.len(),
            baseline: 16.0,
            height: 20.0,
            width: text.chars().count() as f32 * 10.0,
        }]
    }

    fn get_caret_position(
        &self,
        text: &str,
        _font_size: f32,
        _available_width: Option<f32>,
        caret_index: usize,
    ) -> (f32, f32) {
        (
            text[..caret_index.min(text.len())].chars().count() as f32 * 10.0,
            16.0,
        )
    }
}

fn focused_text_input_frame(
    state: &RuntimeState,
    engine: &mut LayoutEngine,
    input_id: WidgetId,
    value: &str,
    width: f32,
) -> (CoreIR, fission_layout::LayoutSnapshot, WidgetId) {
    let env = Env {
        viewport_size: LayoutSize::new(width, 80.0),
        ..Env::default()
    };
    let widget: Widget = TextInput {
        id: Some(input_id),
        semantics_identifier: Some("responsive-title-input".into()),
        value: value.to_string(),
        width: Some(width),
        height: Some(48.0),
        font_size: Some(16.0),
        ..Default::default()
    }
    .into();
    let mut lowering = LoweringContext::new(&env, state, None, None);
    let root = fission_core::internal::lower_widget(&widget, &mut lowering);
    lowering.set_root(root);
    let ir = lowering.into_ir();
    let scroll_id = ir
        .nodes
        .iter()
        .find_map(|(id, node)| {
            matches!(node.op, Op::Layout(LayoutOp::Scroll { .. })).then_some(*id)
        })
        .expect("text input scroll node");
    let input = build_layout_tree(&ir, &env);
    let layout = engine
        .compute_layout(&input, root, LayoutSize::new(width, 80.0), &|_| 0.0)
        .expect("text input layout");
    (ir, layout, scroll_id)
}

#[test]
fn focused_text_input_keeps_caret_visible_when_viewport_narrows() {
    let input_id = WidgetId::explicit("responsive-title-input");
    let value = "A long document title replaced through LiveTest";
    let measurer = Arc::new(FixedTextMeasurer);
    let mut engine = LayoutEngine::new().with_measurer(measurer.clone());
    let mut runtime = Runtime::default().with_measurer(measurer);
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(input_id));
    runtime
        .runtime_state
        .text_edit
        .sync_from_runtime(input_id, value, None, None, false);
    runtime
        .runtime_state
        .text_edit
        .set_caret(input_id, value.len(), None);

    let (desktop_ir, desktop_layout, desktop_scroll_id) =
        focused_text_input_frame(&runtime.runtime_state, &mut engine, input_id, value, 640.0);
    runtime.reconcile_ir(&desktop_ir);
    runtime.post_layout_hook(&desktop_ir, &desktop_layout);
    assert_eq!(
        runtime.runtime_state.scroll.get_offset(desktop_scroll_id),
        0.0
    );

    let (phone_ir, phone_layout, phone_scroll_id) =
        focused_text_input_frame(&runtime.runtime_state, &mut engine, input_id, value, 180.0);
    runtime.reconcile_ir(&phone_ir);
    runtime.post_layout_hook(&phone_ir, &phone_layout);

    let phone_offset = runtime.runtime_state.scroll.get_offset(phone_scroll_id);
    assert!(
        phone_offset > 0.0,
        "shrinking the viewport must reveal the retained caret at the end of the value"
    );
    let phone_viewport_width = phone_layout
        .get_node_geometry(phone_scroll_id)
        .expect("phone text input scroll geometry")
        .rect
        .width();
    let caret_right = value.chars().count() as f32 * 10.0 + 2.0;
    assert!(
        caret_right - phone_offset <= phone_viewport_width - 3.0 + 0.001,
        "the retained caret should remain inside the phone viewport's trailing scroll margin"
    );
}

#[test]
fn responsive_text_input_scroll_uses_semantic_caret_without_retained_session() {
    let desktop_id = WidgetId::explicit("responsive-title-input.desktop");
    let phone_id = WidgetId::explicit("responsive-title-input.phone");
    let value = "A long document title replaced through LiveTest";
    let measurer = Arc::new(FixedTextMeasurer);
    let mut engine = LayoutEngine::new().with_measurer(measurer.clone());
    let mut runtime = Runtime::default().with_measurer(measurer);

    let (desktop_ir, desktop_layout, desktop_scroll_id) = focused_text_input_frame(
        &runtime.runtime_state,
        &mut engine,
        desktop_id,
        value,
        640.0,
    );
    runtime
        .set_focused_widget(
            &desktop_ir,
            Some(desktop_id),
            fission_core::TextEditSource::Programmatic,
        )
        .expect("focus the desktop input");
    runtime
        .reconcile_focus_with_layout(&desktop_ir, &desktop_layout)
        .unwrap();
    runtime.post_layout_hook(&desktop_ir, &desktop_layout);
    assert_eq!(
        runtime.runtime_state.scroll.get_offset(desktop_scroll_id),
        0.0
    );

    let (phone_ir, phone_layout, phone_scroll_id) =
        focused_text_input_frame(&runtime.runtime_state, &mut engine, phone_id, value, 180.0);
    runtime
        .reconcile_focus_with_layout(&phone_ir, &phone_layout)
        .unwrap();
    assert_eq!(runtime.runtime_state.interaction.focused, Some(phone_id));
    assert!(runtime.runtime_state.text_edit.get(phone_id).is_none());
    runtime.post_layout_hook(&phone_ir, &phone_layout);

    assert_eq!(
        runtime.runtime_state.text_edit.get(phone_id).unwrap().caret,
        value.len(),
        "responsive focus replacement should inherit the lowered end selection"
    );
    let phone_offset = runtime.runtime_state.scroll.get_offset(phone_scroll_id);
    let phone_viewport_width = phone_layout
        .get_node_geometry(phone_scroll_id)
        .expect("phone text input scroll geometry")
        .rect
        .width();
    let caret_right = value.chars().count() as f32 * 10.0 + 2.0;
    assert!(
        phone_offset > 0.0,
        "responsive focus replacement without an edit session must scroll to the semantic caret"
    );
    assert!(
        caret_right - phone_offset <= phone_viewport_width - 3.0 + 0.001,
        "the inherited caret should remain inside the phone viewport's trailing scroll margin"
    );
}

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
