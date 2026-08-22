#![cfg(feature = "interactive-canvas")]

use fission_core::event::{InputEvent, PointerButton, PointerEvent, PointerId, PointerKind};
use fission_core::{
    Action, ActionId, ActionRegistry, GlobalState, ReducerContext, Runtime,
    ViewportInteractionPhase,
};
use fission_ir::semantics::{ActionEntry, ActionSet, ActionTrigger, Role};
use fission_ir::{
    CoreIR, LayoutOp, Op, Semantics, ViewportBoundary, ViewportClip, ViewportPanAxis,
    ViewportTransform, ViewportZoomPolicy, WidgetId,
};
use fission_layout::{LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RecordViewport(String);

impl Action for RecordViewport {
    fn static_id() -> ActionId {
        ActionId::from_name("viewport_input::RecordViewport")
    }
}

#[derive(Debug, Default)]
struct ViewportState {
    payloads: Vec<String>,
    phases: Vec<ViewportInteractionPhase>,
    transforms: Vec<ViewportTransform>,
}

impl GlobalState for ViewportState {}

fn record(
    state: &mut ViewportState,
    action: RecordViewport,
    context: &mut ReducerContext<ViewportState>,
) {
    let Some(interaction) = context.input.viewport_interaction() else {
        return;
    };
    state.payloads.push(action.0);
    state.phases.push(interaction.phase);
    state.transforms.push(interaction.transform);
}

fn action(trigger: ActionTrigger, label: &'static str) -> ActionEntry {
    let action = RecordViewport(label.into());
    ActionEntry {
        trigger,
        action_id: RecordViewport::static_id().as_u128(),
        payload_data: Some(action.encode()),
    }
}

fn viewport_tree(draggable_child: bool) -> (CoreIR, LayoutSnapshot, WidgetId) {
    let viewer = WidgetId::explicit("viewport");
    let child = WidgetId::explicit("viewport.child");
    let mut ir = CoreIR::default();
    ir.add_node(
        child,
        Op::Semantics(Semantics {
            role: Role::Generic,
            draggable: draggable_child,
            actions: ActionSet {
                entries: if draggable_child {
                    vec![action(ActionTrigger::DragUpdate, "child")]
                } else {
                    vec![action(ActionTrigger::Default, "child")]
                },
            },
            ..Default::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        viewer,
        Op::Layout(LayoutOp::InteractiveViewport {
            initial_transform: ViewportTransform::IDENTITY,
            controlled_transform: None,
            pan_axis: ViewportPanAxis::Both,
            boundary: ViewportBoundary::Unbounded,
            clip: ViewportClip::HardEdge,
            zoom_policy: ViewportZoomPolicy::WheelWithModifier,
            min_scale: 0.25,
            max_scale: 4.0,
            friction: 0.0,
            on_interaction_start: Some(action(ActionTrigger::ViewportInteractionStart, "start")),
            on_interaction_update: Some(action(ActionTrigger::ViewportInteractionUpdate, "update")),
            on_interaction_end: Some(action(ActionTrigger::ViewportInteractionEnd, "end")),
        }),
        vec![child],
    );
    ir.set_root(viewer);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(300.0, 200.0));
    for id in [viewer, child] {
        layout.nodes.insert(
            id,
            LayoutNodeGeometry {
                rect: LayoutRect::new(0.0, 0.0, 300.0, 200.0),
                content_size: LayoutSize::new(300.0, 200.0),
            },
        );
    }
    (ir, layout, viewer)
}

fn runtime(ir: &CoreIR, layout: &LayoutSnapshot) -> Runtime {
    let mut runtime = Runtime::default();
    runtime
        .add_app_state(Box::new(ViewportState::default()))
        .unwrap();
    let mut registry = ActionRegistry::<ViewportState>::new();
    registry.register(record as fn(&mut ViewportState, RecordViewport, &mut ReducerContext<_>));
    runtime.absorb_registry(registry);
    runtime.post_layout_hook(ir, layout);
    runtime
}

fn pointer(
    pointer_id: u128,
    kind: PointerKind,
    point: (f32, f32),
    phase: fn(PointerId, PointerKind, LayoutPoint) -> PointerEvent,
) -> InputEvent {
    InputEvent::Pointer(phase(
        PointerId(pointer_id),
        kind,
        LayoutPoint::new(point.0, point.1),
    ))
}

fn down(id: PointerId, kind: PointerKind, point: LayoutPoint) -> PointerEvent {
    PointerEvent::Down {
        pointer_id: id,
        kind,
        point,
        button: PointerButton::Primary,
        modifiers: 0,
    }
}

fn moved(id: PointerId, kind: PointerKind, point: LayoutPoint) -> PointerEvent {
    PointerEvent::Move {
        pointer_id: id,
        kind,
        point,
        modifiers: 0,
    }
}

fn up(id: PointerId, kind: PointerKind, point: LayoutPoint) -> PointerEvent {
    PointerEvent::Up {
        pointer_id: id,
        kind,
        point,
        button: PointerButton::Primary,
        modifiers: 0,
    }
}

#[test]
fn mouse_pan_preserves_payload_and_delivers_typed_camera_changes() {
    let (ir, layout, viewer) = viewport_tree(false);
    let mut runtime = runtime(&ir, &layout);
    runtime
        .handle_input(
            pointer(0, PointerKind::Mouse, (10.0, 10.0), down),
            &ir,
            &layout,
        )
        .unwrap();
    runtime
        .handle_input(
            pointer(0, PointerKind::Mouse, (30.0, 25.0), moved),
            &ir,
            &layout,
        )
        .unwrap();
    runtime
        .handle_input(
            pointer(0, PointerKind::Mouse, (30.0, 25.0), up),
            &ir,
            &layout,
        )
        .unwrap();

    let state = runtime.get_app_state::<ViewportState>().unwrap();
    assert_eq!(state.payloads, ["start", "update", "end"]);
    assert_eq!(
        state.phases,
        [
            ViewportInteractionPhase::Start,
            ViewportInteractionPhase::Update,
            ViewportInteractionPhase::End,
        ]
    );
    assert_eq!(
        runtime.runtime_state.viewport.transform(viewer),
        Some(ViewportTransform::new(20.0, 15.0, 1.0))
    );
}

#[test]
fn two_touch_pinch_is_order_independent_and_two_to_one_does_not_jump() {
    let (ir, layout, viewer) = viewport_tree(false);
    let mut runtime = runtime(&ir, &layout);
    for event in [
        pointer(9, PointerKind::Touch, (50.0, 50.0), down),
        pointer(3, PointerKind::Touch, (150.0, 50.0), down),
        pointer(3, PointerKind::Touch, (200.0, 50.0), moved),
        pointer(3, PointerKind::Touch, (200.0, 50.0), up),
    ] {
        runtime.handle_input(event, &ir, &layout).unwrap();
    }
    let before = runtime.runtime_state.viewport.transform(viewer).unwrap();
    runtime
        .handle_input(
            pointer(9, PointerKind::Touch, (55.0, 50.0), moved),
            &ir,
            &layout,
        )
        .unwrap();
    let after = runtime.runtime_state.viewport.transform(viewer).unwrap();
    assert_eq!(after.translation[0] - before.translation[0], 5.0);
    assert_eq!(after.scale, before.scale);
}

#[test]
fn draggable_child_wins_single_pointer_arbitration() {
    let (ir, layout, viewer) = viewport_tree(true);
    let mut runtime = runtime(&ir, &layout);
    runtime
        .handle_input(
            pointer(0, PointerKind::Mouse, (10.0, 10.0), down),
            &ir,
            &layout,
        )
        .unwrap();
    runtime
        .handle_input(
            pointer(0, PointerKind::Mouse, (40.0, 30.0), moved),
            &ir,
            &layout,
        )
        .unwrap();
    assert_eq!(
        runtime.runtime_state.viewport.transform(viewer),
        Some(ViewportTransform::IDENTITY)
    );
}

#[test]
fn controlled_transform_remains_the_authority_on_every_rebuild() {
    let (mut ir, layout, viewer) = viewport_tree(false);
    let declared = ViewportTransform::new(12.0, 18.0, 2.0);
    let Op::Layout(LayoutOp::InteractiveViewport {
        controlled_transform,
        ..
    }) = &mut ir.nodes.get_mut(&viewer).expect("viewer node").op
    else {
        panic!("expected interactive viewport");
    };
    *controlled_transform = Some(declared);

    let mut runtime = runtime(&ir, &layout);
    runtime
        .runtime_state
        .viewport
        .set_transform(viewer, ViewportTransform::new(90.0, 70.0, 3.0));
    runtime.post_layout_hook(&ir, &layout);

    assert_eq!(
        runtime.runtime_state.viewport.transform(viewer),
        Some(declared)
    );
}

#[test]
fn inertia_uses_the_runtime_clock() {
    let (mut ir, layout, viewer) = viewport_tree(false);
    let Op::Layout(LayoutOp::InteractiveViewport { friction, .. }) =
        &mut ir.nodes.get_mut(&viewer).expect("viewer node").op
    else {
        panic!("expected interactive viewport");
    };
    *friction = 0.0000135;

    let mut runtime = runtime(&ir, &layout);
    runtime
        .handle_input(
            pointer(0, PointerKind::Mouse, (10.0, 10.0), down),
            &ir,
            &layout,
        )
        .unwrap();
    runtime.tick(16).unwrap();
    runtime
        .handle_input(
            pointer(0, PointerKind::Mouse, (30.0, 10.0), moved),
            &ir,
            &layout,
        )
        .unwrap();
    runtime
        .handle_input(
            pointer(0, PointerKind::Mouse, (30.0, 10.0), up),
            &ir,
            &layout,
        )
        .unwrap();
    let before = runtime.runtime_state.viewport.transform(viewer).unwrap();

    runtime.tick(16).unwrap();
    assert!(runtime.post_layout_hook(&ir, &layout));
    let after = runtime.runtime_state.viewport.transform(viewer).unwrap();

    assert!(after.translation[0] > before.translation[0]);
    assert_eq!(after.translation[1], before.translation[1]);
}
