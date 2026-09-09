use anyhow::Result;
use fission_core::event::{ExternalDragEvent, InputEvent, PointerButton, PointerEvent};
use fission_core::{
    Action, ActionId, ActionInput, ActionRegistry, GlobalState, ReducerContext, Runtime, WidgetId,
};
use fission_ir::op::LayoutOp;
use fission_ir::semantics::{ActionTrigger, Role};
use fission_ir::{ActionEntry, ActionSet, CoreIR, Op, Semantics, StructuralOp};
use fission_layout::{LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default)]
struct DragState {
    events: Vec<String>,
}

impl GlobalState for DragState {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RecordDrag {
    event: String,
}

impl Action for RecordDrag {
    fn static_id() -> ActionId {
        ActionId::from_name("tests::drag_drop::RecordDrag")
    }
}

fn record_drag(state: &mut DragState, action: RecordDrag, ctx: &mut ReducerContext<DragState>) {
    let detail = match ctx.input.unscoped() {
        ActionInput::InternalDrop { payload, .. } => {
            format!("internal:{}", String::from_utf8_lossy(payload))
        }
        ActionInput::Drop { paths, .. } => format!("files:{}", paths.join(",")),
        ActionInput::Pointer { .. } => "pointer".into(),
        other => format!("{other:?}"),
    };
    state.events.push(format!("{}:{detail}", action.event));
}

fn action(event: &str) -> ActionEntry {
    let action = RecordDrag {
        event: event.into(),
    };
    ActionEntry {
        trigger: match event {
            "drop" => ActionTrigger::Drop,
            "enter" => ActionTrigger::DragEnter,
            "leave" => ActionTrigger::DragLeave,
            "start" => ActionTrigger::DragStart,
            "update" => ActionTrigger::DragUpdate,
            "end" => ActionTrigger::DragEnd,
            _ => ActionTrigger::Default,
        },
        action_id: RecordDrag::static_id().as_u128(),
        payload_data: Some(action.encode()),
    }
}

fn semantics(role: Role, entries: Vec<ActionEntry>) -> Op {
    Op::Semantics(Semantics {
        role,
        actions: ActionSet { entries },
        ..Default::default()
    })
}

fn drag_tree() -> (CoreIR, LayoutSnapshot, WidgetId, WidgetId, WidgetId) {
    let root = WidgetId::explicit("drag.root");
    let source = WidgetId::explicit("drag.source");
    let target = WidgetId::explicit("drag.target");

    let mut ir = CoreIR::default();
    let mut source_sem = Semantics {
        role: Role::Button,
        identifier: Some("demo.drag.source".into()),
        drag_payload: Some(b"card-1".to_vec()),
        actions: ActionSet {
            entries: vec![action("start"), action("end")],
        },
        ..Default::default()
    };
    source_sem.draggable = true;

    let target_sem = match semantics(
        Role::Generic,
        vec![action("enter"), action("leave"), action("drop")],
    ) {
        Op::Semantics(mut sem) => {
            sem.identifier = Some("demo.drop.target".into());
            Op::Semantics(sem)
        }
        op => op,
    };

    ir.add_node(source, Op::Semantics(source_sem), Vec::new());
    ir.add_node(target, target_sem, Vec::new());
    ir.add_node(root, Op::Layout(LayoutOp::ZStack), vec![source, target]);
    ir.set_root(root);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(260.0, 120.0));
    layout.nodes.insert(
        root,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 260.0, 120.0),
            content_size: LayoutSize::new(260.0, 120.0),
        },
    );
    layout.nodes.insert(
        source,
        LayoutNodeGeometry {
            rect: LayoutRect::new(10.0, 10.0, 70.0, 50.0),
            content_size: LayoutSize::new(70.0, 50.0),
        },
    );
    layout.nodes.insert(
        target,
        LayoutNodeGeometry {
            rect: LayoutRect::new(130.0, 10.0, 90.0, 70.0),
            content_size: LayoutSize::new(90.0, 70.0),
        },
    );

    (ir, layout, root, source, target)
}

fn runtime() -> Result<Runtime> {
    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(DragState::default()))?;
    let mut registry = ActionRegistry::<DragState>::new();
    registry
        .register(record_drag as fn(&mut DragState, RecordDrag, &mut ReducerContext<DragState>));
    runtime.absorb_registry(registry);
    Ok(runtime)
}

fn state(runtime: &Runtime) -> &DragState {
    runtime.get_app_state::<DragState>().expect("drag state")
}

#[test]
fn internal_drag_dispatches_drop_payload_and_drag_boundaries() -> Result<()> {
    let (ir, layout, _, _, _) = drag_tree();
    let mut runtime = runtime()?;
    let source_point = LayoutPoint::new(20.0, 20.0);
    let target_point = LayoutPoint::new(150.0, 30.0);

    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: source_point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Move {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: target_point,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: target_point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;

    assert_eq!(
        state(&runtime).events,
        vec![
            "enter:pointer",
            "start:pointer",
            "drop:internal:card-1",
            "end:pointer",
            "leave:pointer",
        ]
    );
    assert!(runtime.runtime_state.gesture.drag_session.is_none());
    Ok(())
}

#[test]
fn drag_leave_fires_when_internal_drag_moves_off_target() -> Result<()> {
    let (ir, layout, _, _, _) = drag_tree();
    let mut runtime = runtime()?;

    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: LayoutPoint::new(20.0, 20.0),
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Move {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: LayoutPoint::new(150.0, 30.0),
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Move {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: LayoutPoint::new(245.0, 100.0),
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;

    assert!(state(&runtime).events.contains(&"enter:pointer".into()));
    assert!(state(&runtime).events.contains(&"leave:pointer".into()));
    Ok(())
}

#[test]
fn active_drag_is_cancelled_when_its_retained_subtree_becomes_interaction_inert() -> Result<()> {
    let (mut ir, layout, root, source, _) = drag_tree();
    let Op::Semantics(source_semantics) = &mut ir.nodes.get_mut(&source).unwrap().op else {
        panic!("drag source must be semantic");
    };
    source_semantics.actions.entries.push(action("update"));

    let mut runtime = runtime()?;
    let source_point = LayoutPoint::new(20.0, 20.0);
    let drag_point = LayoutPoint::new(150.0, 30.0);

    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: source_point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Move {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: drag_point,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;

    assert_eq!(
        state(&runtime).events,
        vec!["enter:pointer", "start:pointer", "update:pointer"]
    );
    assert!(runtime.runtime_state.gesture.is_panning);
    assert!(runtime
        .runtime_state
        .gesture
        .drag_session
        .as_ref()
        .is_some_and(|session| session.target_node.is_some()));

    ir.nodes.get_mut(&root).unwrap().op =
        Op::Structural(StructuralOp::InteractionInert { stable_hash: 1 });
    runtime.reconcile_ir(&ir);

    assert!(!runtime.runtime_state.gesture.is_panning);
    assert!(runtime.runtime_state.gesture.start_point.is_none());
    assert!(runtime.runtime_state.gesture.last_point.is_none());
    assert!(runtime.runtime_state.gesture.target_node.is_none());
    assert!(runtime.runtime_state.gesture.dragging_payload.is_none());
    assert!(runtime.runtime_state.gesture.pressed_button.is_none());
    assert!(runtime.runtime_state.gesture.drag_session.is_none());
    assert!(runtime.runtime_state.interaction.pressed.is_empty());
    assert!(runtime.runtime_state.interaction.last_down_point.is_none());

    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Move {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: LayoutPoint::new(45.0, 30.0),
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: LayoutPoint::new(45.0, 30.0),
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;

    assert_eq!(
        state(&runtime).events,
        vec!["enter:pointer", "start:pointer", "update:pointer"],
        "the inert exit frame must receive no update, end, leave, or drop action"
    );
    Ok(())
}

#[test]
fn inert_drop_target_is_cleared_without_cancelling_its_active_drag_source() -> Result<()> {
    let (mut ir, mut layout, root, source, target) = drag_tree();
    let Op::Semantics(source_semantics) = &mut ir.nodes.get_mut(&source).unwrap().op else {
        panic!("drag source must be semantic");
    };
    source_semantics.actions.entries.push(action("update"));

    let mut runtime = runtime()?;
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: LayoutPoint::new(20.0, 20.0),
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Move {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: LayoutPoint::new(150.0, 30.0),
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;

    assert_eq!(
        state(&runtime).events,
        vec!["enter:pointer", "start:pointer", "update:pointer"]
    );

    let inert_target = WidgetId::explicit("drag.target.exit");
    ir.add_node(
        inert_target,
        Op::Structural(StructuralOp::InteractionInert { stable_hash: 2 }),
        vec![target],
    );
    ir.nodes.get_mut(&inert_target).unwrap().parent = Some(root);
    ir.nodes.get_mut(&root).unwrap().children = vec![source, inert_target];
    layout.nodes.insert(
        inert_target,
        LayoutNodeGeometry {
            rect: LayoutRect::new(130.0, 10.0, 90.0, 70.0),
            content_size: LayoutSize::new(90.0, 70.0),
        },
    );

    runtime.reconcile_ir(&ir);

    let session = runtime
        .runtime_state
        .gesture
        .drag_session
        .as_ref()
        .expect("the active source keeps its drag session");
    assert_eq!(session.source_node, Some(source));
    assert!(session.target_node.is_none());
    assert!(session.target_identifier.is_none());
    assert!(runtime.runtime_state.gesture.is_panning);

    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Move {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: LayoutPoint::new(160.0, 35.0),
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: LayoutPoint::new(160.0, 35.0),
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;

    assert_eq!(
        state(&runtime).events,
        vec![
            "enter:pointer",
            "start:pointer",
            "update:pointer",
            "update:pointer",
            "end:pointer",
        ],
        "the inert target must receive neither leave nor drop while the active source continues"
    );
    assert!(runtime.runtime_state.gesture.drag_session.is_none());
    Ok(())
}

#[test]
fn external_file_drop_dispatches_paths_to_hovered_drop_target() -> Result<()> {
    let (ir, layout, _, _, _) = drag_tree();
    let mut runtime = runtime()?;
    let point = LayoutPoint::new(150.0, 30.0);

    runtime.handle_input(
        InputEvent::ExternalDrag(ExternalDragEvent::Hover {
            point,
            paths: vec!["/tmp/report.pdf".into()],
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;
    runtime.handle_input(
        InputEvent::ExternalDrag(ExternalDragEvent::Drop {
            point,
            paths: vec!["/tmp/report.pdf".into(), "/tmp/photo.png".into()],
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;

    assert_eq!(
        state(&runtime).events,
        vec![
            "enter:pointer",
            "drop:files:/tmp/report.pdf,/tmp/photo.png",
            "leave:pointer",
        ]
    );
    assert!(runtime.runtime_state.gesture.drag_session.is_none());
    Ok(())
}
