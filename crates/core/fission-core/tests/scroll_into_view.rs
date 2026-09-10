use fission_core::{
    Effect, EffectEnvelope, Effects, GlobalState, Runtime, RuntimeEffect, ScrollAlignment,
    ScrollAxis, ScrollBehavior, ScrollIntoViewRequest,
};
use fission_ir::{CoreIR, FlexDirection, LayoutOp, Op, WidgetId};
use fission_layout::{LayoutNodeGeometry, LayoutRect, LayoutSize, LayoutSnapshot};

fn box_op() -> Op {
    Op::Layout(LayoutOp::Box {
        width: None,
        height: None,
        min_width: None,
        max_width: None,
        min_height: None,
        max_height: None,
        padding: [0.0; 4],
        flex_grow: 0.0,
        flex_shrink: 0.0,
        aspect_ratio: None,
    })
}

fn scroll_op(direction: FlexDirection) -> Op {
    Op::Layout(LayoutOp::Scroll {
        direction,
        show_scrollbar: true,
        width: Some(100.0),
        height: Some(100.0),
        min_width: None,
        max_width: None,
        min_height: None,
        max_height: None,
        padding: [0.0; 4],
        flex_grow: 0.0,
        flex_shrink: 0.0,
    })
}

fn vertical_scroll_tree() -> (CoreIR, LayoutSnapshot, WidgetId, WidgetId) {
    let root = WidgetId::from_u128(1);
    let scroll = WidgetId::from_u128(2);
    let target = WidgetId::from_u128(3);

    let mut ir = CoreIR::new();
    ir.add_node(target, box_op(), vec![]);
    ir.add_node(scroll, scroll_op(FlexDirection::Column), vec![target]);
    ir.add_node(root, box_op(), vec![scroll]);
    ir.set_root(root);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    layout.nodes.insert(
        root,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
            content_size: LayoutSize::new(100.0, 100.0),
        },
    );
    layout.nodes.insert(
        scroll,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
            content_size: LayoutSize::new(100.0, 500.0),
        },
    );
    layout.nodes.insert(
        target,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 260.0, 100.0, 40.0),
            content_size: LayoutSize::new(100.0, 40.0),
        },
    );

    (ir, layout, scroll, target)
}

fn vertical_scroll_tree_with_active_descendant(
) -> (CoreIR, LayoutSnapshot, WidgetId, WidgetId, WidgetId) {
    let (mut ir, layout, scroll, target) = vertical_scroll_tree();
    let owner = WidgetId::from_u128(4);
    ir.add_node(
        owner,
        Op::Semantics(fission_ir::Semantics {
            role: fission_ir::Role::ComboBox,
            active_descendant: Some(target),
            ..Default::default()
        }),
        vec![],
    );
    let root = ir.root.expect("test root");
    ir.nodes
        .get_mut(&root)
        .expect("test root node")
        .children
        .push(owner);
    ir.nodes.get_mut(&owner).expect("owner node").parent = Some(root);

    (ir, layout, scroll, target, owner)
}

fn request(container: Option<WidgetId>, target: WidgetId) -> ScrollIntoViewRequest {
    ScrollIntoViewRequest {
        container,
        target,
        axis: ScrollAxis::Vertical,
        alignment: ScrollAlignment::Start,
        padding: [0.0, 0.0, 24.0, 12.0],
        behavior: ScrollBehavior::Instant,
        if_needed: false,
    }
}

#[derive(Debug, Default)]
struct TestState;

impl GlobalState for TestState {}

#[test]
fn effects_builder_emits_scroll_into_view_runtime_effect() {
    let target = WidgetId::from_u128(99);
    let mut effects = Effects::<TestState>::new_headless(10);

    let req_id = effects.scroll_into_view(ScrollIntoViewRequest {
        container: None,
        target,
        axis: ScrollAxis::Vertical,
        alignment: ScrollAlignment::Start,
        padding: [0.0; 4],
        behavior: ScrollBehavior::Instant,
        if_needed: true,
    });

    assert_eq!(req_id, 10);
    assert!(matches!(
        effects.out.first().map(|env| &env.effect),
        Some(Effect::Runtime(RuntimeEffect::ScrollIntoView(request))) if request.target == target
    ));
}

#[test]
fn explicit_vertical_scroll_into_view_aligns_target_to_start_padding() {
    let (ir, layout, scroll, target) = vertical_scroll_tree();
    let mut runtime = Runtime::default();

    runtime.queue_scroll_into_view(request(Some(scroll), target));

    assert!(runtime.post_layout_hook(&ir, &layout));
    assert_eq!(runtime.runtime_state.scroll.get_offset(scroll), 236.0);
}

#[test]
fn runtime_effect_queue_is_drained_after_layout() {
    let (ir, layout, scroll, target) = vertical_scroll_tree();
    let mut runtime = Runtime::default();
    runtime.pending_effects.push(EffectEnvelope {
        req_id: 7,
        effect: Effect::Runtime(RuntimeEffect::ScrollIntoView(request(Some(scroll), target))),
        on_ok: None,
        on_err: None,
        service_bindings: None,
        resource: None,
    });

    assert!(runtime.post_layout_hook(&ir, &layout));
    assert!(runtime.pending_effects.is_empty());
    assert_eq!(runtime.runtime_state.scroll.get_offset(scroll), 236.0);
}

#[test]
fn omitted_container_uses_nearest_matching_scroll_ancestor() {
    let (ir, mut layout, scroll, target) = vertical_scroll_tree();
    layout.nodes.get_mut(&target).unwrap().rect = LayoutRect::new(0.0, 160.0, 100.0, 30.0);
    let mut runtime = Runtime::default();

    runtime.queue_scroll_into_view(ScrollIntoViewRequest {
        container: None,
        target,
        axis: ScrollAxis::Vertical,
        alignment: ScrollAlignment::Nearest,
        padding: [0.0; 4],
        behavior: ScrollBehavior::Instant,
        if_needed: false,
    });

    assert!(runtime.post_layout_hook(&ir, &layout));
    assert_eq!(runtime.runtime_state.scroll.get_offset(scroll), 90.0);
}

#[test]
fn newly_mounted_active_descendant_is_centred_once_in_its_scroll_ancestor() {
    let (ir, layout, scroll, _target, _owner) = vertical_scroll_tree_with_active_descendant();
    let mut runtime = Runtime::default();

    assert!(runtime.post_layout_hook(&ir, &layout));
    assert_eq!(runtime.runtime_state.scroll.get_offset(scroll), 230.0);

    runtime.runtime_state.scroll.set_offset(scroll, 0.0);
    assert!(!runtime.post_layout_hook(&ir, &layout));
    assert_eq!(runtime.runtime_state.scroll.get_offset(scroll), 0.0);
}

#[test]
fn removing_and_restoring_active_descendant_reveals_it_again() {
    let (mut ir, layout, scroll, _target, owner) = vertical_scroll_tree_with_active_descendant();
    let mut runtime = Runtime::default();

    assert!(runtime.post_layout_hook(&ir, &layout));
    runtime.runtime_state.scroll.set_offset(scroll, 0.0);

    let active = match &mut ir.nodes.get_mut(&owner).expect("owner node").op {
        Op::Semantics(semantics) => &mut semantics.active_descendant,
        _ => panic!("owner must remain semantic"),
    };
    let target = active.take().expect("active descendant");
    assert!(!runtime.post_layout_hook(&ir, &layout));

    let active = match &mut ir.nodes.get_mut(&owner).expect("owner node").op {
        Op::Semantics(semantics) => &mut semantics.active_descendant,
        _ => panic!("owner must remain semantic"),
    };
    *active = Some(target);
    assert!(runtime.post_layout_hook(&ir, &layout));
    assert_eq!(runtime.runtime_state.scroll.get_offset(scroll), 230.0);
}

#[test]
fn missing_active_descendant_does_not_request_frames_forever() {
    let (mut ir, layout, _scroll, target, owner) = vertical_scroll_tree_with_active_descendant();
    ir.nodes.remove(&target);

    let mut runtime = Runtime::default();
    assert!(
        !runtime.post_layout_hook(&ir, &layout),
        "a stale semantic relationship cannot be repaired by another identical frame"
    );
    assert!(!runtime.post_layout_hook(&ir, &layout));

    let Op::Semantics(semantics) = &mut ir.nodes.get_mut(&owner).unwrap().op else {
        unreachable!();
    };
    semantics.active_descendant = None;
    assert!(!runtime.post_layout_hook(&ir, &layout));
}

#[test]
fn nearest_alignment_keeps_already_visible_target_stationary() {
    let (ir, mut layout, scroll, target) = vertical_scroll_tree();
    layout.nodes.get_mut(&target).unwrap().rect = LayoutRect::new(0.0, 220.0, 100.0, 30.0);
    let mut runtime = Runtime::default();
    runtime.runtime_state.scroll.set_offset(scroll, 200.0);

    runtime.queue_scroll_into_view(ScrollIntoViewRequest {
        container: Some(scroll),
        target,
        axis: ScrollAxis::Vertical,
        alignment: ScrollAlignment::Nearest,
        padding: [0.0; 4],
        behavior: ScrollBehavior::Instant,
        if_needed: false,
    });

    assert!(!runtime.post_layout_hook(&ir, &layout));
    assert_eq!(runtime.runtime_state.scroll.get_offset(scroll), 200.0);
}

#[test]
fn nearest_alignment_uses_leading_edge_for_target_larger_than_viewport() {
    let (ir, mut layout, scroll, target) = vertical_scroll_tree();
    layout.nodes.get_mut(&target).unwrap().rect = LayoutRect::new(0.0, 40.0, 100.0, 180.0);
    let mut runtime = Runtime::default();

    runtime.queue_scroll_into_view(ScrollIntoViewRequest {
        container: Some(scroll),
        target,
        axis: ScrollAxis::Vertical,
        alignment: ScrollAlignment::Nearest,
        padding: [0.0; 4],
        behavior: ScrollBehavior::Instant,
        if_needed: true,
    });

    assert!(runtime.post_layout_hook(&ir, &layout));
    assert_eq!(runtime.runtime_state.scroll.get_offset(scroll), 40.0);
}

#[test]
fn missing_target_is_retried_once_after_next_layout() {
    let (mut ir, mut layout, scroll, target) = vertical_scroll_tree();
    ir.nodes.remove(&target);
    layout.nodes.remove(&target);
    let mut runtime = Runtime::default();

    runtime.queue_scroll_into_view(request(Some(scroll), target));

    assert!(runtime.post_layout_hook(&ir, &layout));
    assert_eq!(runtime.runtime_state.scroll.get_offset(scroll), 0.0);

    let (complete_ir, complete_layout, _, _) = vertical_scroll_tree();
    ir = complete_ir;
    layout = complete_layout;

    assert!(runtime.post_layout_hook(&ir, &layout));
    assert_eq!(runtime.runtime_state.scroll.get_offset(scroll), 236.0);
}
