#![cfg(feature = "interactive-canvas")]

use fission_core::event::{InputEvent, PointerButton, PointerEvent, PointerId, PointerKind};
use fission_core::{
    Action, ActionId, ActionRegistry, CanvasInteraction, CanvasInteractionKind,
    CanvasInteractionPhase, GlobalState, ReducerContext, Runtime,
};
use fission_ir::semantics::{ActionEntry, ActionSet, ActionTrigger, Role};
use fission_ir::{
    CanvasTarget, CanvasTargetKind, CoreIR, LayoutOp, Op, Semantics, ViewportBoundary,
    ViewportClip, ViewportPanAxis, ViewportTransform, ViewportZoomPolicy, WidgetId,
};
use fission_layout::{LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct UpdateCanvas(String);

impl Action for UpdateCanvas {
    fn static_id() -> ActionId {
        ActionId::from_name("canvas_input::UpdateCanvas")
    }
}

#[derive(Debug, Default)]
struct CanvasState {
    payloads: Vec<String>,
    interactions: Vec<CanvasInteraction>,
}

impl GlobalState for CanvasState {}

fn update(
    state: &mut CanvasState,
    action: UpdateCanvas,
    context: &mut ReducerContext<CanvasState>,
) {
    let Some(interaction) = context.input.canvas_interaction() else {
        return;
    };
    state.payloads.push(action.0);
    state.interactions.push(interaction.clone());
}

fn canvas_tree() -> (CoreIR, LayoutSnapshot, WidgetId) {
    let viewer = WidgetId::explicit("canvas");
    let node = WidgetId::explicit("canvas.node");
    let action = UpdateCanvas("document-alpha".into());
    let entries = [
        ActionTrigger::DragStart,
        ActionTrigger::DragUpdate,
        ActionTrigger::DragEnd,
    ]
    .into_iter()
    .map(|trigger| ActionEntry {
        trigger,
        action_id: UpdateCanvas::static_id().as_u128(),
        payload_data: Some(action.encode()),
    })
    .collect();

    let mut ir = CoreIR::default();
    ir.add_node(
        node,
        Op::Semantics(Semantics {
            role: Role::Generic,
            actions: ActionSet { entries },
            canvas_target: Some(CanvasTarget {
                canvas_id: viewer.as_u128(),
                kind: CanvasTargetKind::Node {
                    node_id: 91,
                    bounds: [-20.0, 10.0, 40.0, 30.0],
                },
                selection_policy: fission_ir::CanvasSelectionPolicy::Single,
                snap_spacing: Some(10.0),
                snap_threshold: 3.0,
            }),
            draggable: true,
            ..Default::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        viewer,
        Op::Layout(LayoutOp::InteractiveViewport {
            initial_transform: ViewportTransform::new(50.0, 20.0, 2.0),
            controlled_transform: None,
            pan_axis: ViewportPanAxis::Both,
            boundary: ViewportBoundary::Unbounded,
            clip: ViewportClip::HardEdge,
            zoom_policy: ViewportZoomPolicy::WheelWithModifier,
            min_scale: 0.25,
            max_scale: 4.0,
            friction: 0.0,
            on_interaction_start: None,
            on_interaction_update: None,
            on_interaction_end: None,
        }),
        vec![node],
    );
    ir.set_root(viewer);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(300.0, 200.0));
    layout.nodes.insert(
        viewer,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 300.0, 200.0),
            content_size: LayoutSize::new(300.0, 200.0),
        },
    );
    layout.nodes.insert(
        node,
        LayoutNodeGeometry {
            rect: LayoutRect::new(-20.0, 10.0, 40.0, 30.0),
            content_size: LayoutSize::new(40.0, 30.0),
        },
    );
    (ir, layout, viewer)
}

fn pointer(point: LayoutPoint, phase: u8) -> InputEvent {
    let common = (PointerId::MOUSE, PointerKind::Mouse, point);
    InputEvent::Pointer(match phase {
        0 => PointerEvent::Down {
            pointer_id: common.0,
            kind: common.1,
            point: common.2,
            button: PointerButton::Primary,
            modifiers: 1,
        },
        1 => PointerEvent::Move {
            pointer_id: common.0,
            kind: common.1,
            point: common.2,
            modifiers: 1,
        },
        _ => PointerEvent::Up {
            pointer_id: common.0,
            kind: common.1,
            point: common.2,
            button: PointerButton::Primary,
            modifiers: 1,
        },
    })
}

#[test]
fn node_drag_preserves_context_and_delivers_world_geometry() {
    let (ir, layout, viewer) = canvas_tree();
    let mut runtime = Runtime::default();
    runtime
        .add_app_state(Box::new(CanvasState::default()))
        .unwrap();
    let mut registry = ActionRegistry::<CanvasState>::new();
    registry.register(update as fn(&mut CanvasState, UpdateCanvas, &mut ReducerContext<_>));
    runtime.absorb_registry(registry);
    runtime.post_layout_hook(&ir, &layout);

    // World (-10, 20) maps to screen (30, 60). A 20px drag is 10 world units.
    for event in [
        pointer(LayoutPoint::new(30.0, 60.0), 0),
        pointer(LayoutPoint::new(50.0, 60.0), 1),
        pointer(LayoutPoint::new(50.0, 60.0), 2),
    ] {
        runtime.handle_input(event, &ir, &layout).unwrap();
    }

    let state = runtime.get_app_state::<CanvasState>().unwrap();
    assert_eq!(state.payloads, ["document-alpha"; 3]);
    assert_eq!(
        state
            .interactions
            .iter()
            .map(|interaction| interaction.phase)
            .collect::<Vec<_>>(),
        [
            CanvasInteractionPhase::Start,
            CanvasInteractionPhase::Update,
            CanvasInteractionPhase::End,
        ]
    );
    let update = &state.interactions[1];
    assert_eq!(update.canvas_id, viewer);
    assert_eq!(update.kind, CanvasInteractionKind::MoveNode { node_id: 91 });
    assert_eq!(
        update.selection_policy,
        fission_ir::CanvasSelectionPolicy::Single
    );
    assert_eq!(update.world_delta, LayoutPoint::new(10.0, 0.0));
    assert_eq!(
        update.bounds_after,
        Some(LayoutRect::new(-10.0, 10.0, 40.0, 30.0))
    );
    assert_eq!(update.modifiers, 1);
    assert_eq!(update.input_kind, PointerKind::Mouse);
    assert_eq!(
        runtime.runtime_state.viewport.transform(viewer),
        Some(ViewportTransform::new(50.0, 20.0, 2.0))
    );
}

#[test]
fn cancelling_node_drag_dispatches_cancel_and_clears_the_sequence() {
    let (ir, layout, _) = canvas_tree();
    let mut runtime = Runtime::default();
    runtime
        .add_app_state(Box::new(CanvasState::default()))
        .unwrap();
    let mut registry = ActionRegistry::<CanvasState>::new();
    registry.register(update as fn(&mut CanvasState, UpdateCanvas, &mut ReducerContext<_>));
    runtime.absorb_registry(registry);
    runtime.post_layout_hook(&ir, &layout);

    runtime
        .handle_input(pointer(LayoutPoint::new(30.0, 60.0), 0), &ir, &layout)
        .unwrap();
    runtime
        .handle_input(pointer(LayoutPoint::new(50.0, 60.0), 1), &ir, &layout)
        .unwrap();
    runtime
        .handle_input(
            InputEvent::Pointer(PointerEvent::Cancel {
                pointer_id: PointerId::MOUSE,
                kind: PointerKind::Mouse,
                point: LayoutPoint::new(50.0, 60.0),
                modifiers: 1,
            }),
            &ir,
            &layout,
        )
        .unwrap();

    let state = runtime.get_app_state::<CanvasState>().unwrap();
    assert_eq!(
        state
            .interactions
            .last()
            .map(|interaction| interaction.phase),
        Some(CanvasInteractionPhase::Cancel)
    );
    assert!(runtime.runtime_state.gesture.pressed_button.is_none());
    assert!(runtime.runtime_state.gesture.target_node.is_none());
}

#[test]
fn second_contact_cancels_an_active_node_drag_before_viewport_capture() {
    let (ir, layout, _) = canvas_tree();
    let mut runtime = Runtime::default();
    runtime
        .add_app_state(Box::new(CanvasState::default()))
        .unwrap();
    let mut registry = ActionRegistry::<CanvasState>::new();
    registry.register(update as fn(&mut CanvasState, UpdateCanvas, &mut ReducerContext<_>));
    runtime.absorb_registry(registry);
    runtime.post_layout_hook(&ir, &layout);

    let touch = |pointer_id, point, down| {
        InputEvent::Pointer(if down {
            PointerEvent::Down {
                pointer_id: PointerId(pointer_id),
                kind: PointerKind::Touch,
                point,
                button: PointerButton::Primary,
                modifiers: 0,
            }
        } else {
            PointerEvent::Move {
                pointer_id: PointerId(pointer_id),
                kind: PointerKind::Touch,
                point,
                modifiers: 0,
            }
        })
    };
    for event in [
        touch(1, LayoutPoint::new(30.0, 60.0), true),
        touch(1, LayoutPoint::new(50.0, 60.0), false),
        touch(2, LayoutPoint::new(90.0, 60.0), true),
    ] {
        runtime.handle_input(event, &ir, &layout).unwrap();
    }

    let state = runtime.get_app_state::<CanvasState>().unwrap();
    assert_eq!(
        state
            .interactions
            .last()
            .map(|interaction| interaction.phase),
        Some(CanvasInteractionPhase::Cancel)
    );
}
