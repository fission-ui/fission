use crate::authoring::Lower;
use crate::env::{Env, RuntimeState};
use crate::lowering::{build_layout_tree, LoweringContext};
use crate::ui::widgets::container::Container;
use crate::ui::widgets::responsive::Responsive;
use crate::ui::widgets::text_input::TextInput;
use crate::Runtime;
use fission_ir::WidgetId;
use fission_layout::{LayoutEngine, LayoutSize};

#[test]
fn responsive_focus_anchor_survives_branch_without_matching_input() {
    let input_id = WidgetId::explicit("responsive-wide-text-input");
    let responsive_id = WidgetId::explicit("responsive-editor");
    let widget = Responsive::new(Container::default().width(320.0).height(48.0))
        .id(responsive_id)
        .case_min_width_with(900.0, || TextInput {
            id: Some(input_id),
            semantics_identifier: Some("document.body".into()),
            value: "document body".into(),
            ..Default::default()
        });

    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let mut cx = LoweringContext::new(&env, &runtime_state, None, None);
    let root_id = widget.lower(&mut cx);
    cx.ir.root = Some(root_id);

    let layout_nodes = build_layout_tree(&cx.ir, &env);
    let mut engine = LayoutEngine::new();
    engine.rebuild(&layout_nodes).unwrap();
    let wide_layout = engine
        .compute_layout(
            &layout_nodes,
            root_id,
            LayoutSize::new(1_100.0, 700.0),
            &|_| 0.0,
        )
        .unwrap();
    let compact_layout = engine
        .compute_layout(
            &layout_nodes,
            root_id,
            LayoutSize::new(600.0, 700.0),
            &|_| 0.0,
        )
        .unwrap();
    let restored_wide_layout = engine
        .compute_layout(
            &layout_nodes,
            root_id,
            LayoutSize::new(1_100.0, 700.0),
            &|_| 0.0,
        )
        .unwrap();

    assert!(wide_layout.get_node_geometry(input_id).is_some());
    assert!(compact_layout.get_node_geometry(input_id).is_none());
    assert!(restored_wide_layout.get_node_geometry(input_id).is_some());

    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(input_id));
    runtime
        .reconcile_focus_with_layout(&cx.ir, &wide_layout)
        .unwrap();
    assert_eq!(runtime.runtime_state.interaction.focused, Some(input_id));

    runtime
        .reconcile_focus_with_layout(&cx.ir, &compact_layout)
        .unwrap();
    assert_eq!(runtime.runtime_state.interaction.focused, None);

    runtime
        .reconcile_focus_with_layout(&cx.ir, &restored_wide_layout)
        .unwrap();
    assert_eq!(runtime.runtime_state.interaction.focused, Some(input_id));
}
