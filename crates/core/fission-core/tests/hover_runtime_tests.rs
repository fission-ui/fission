use anyhow::Result;
use fission_core::action::AppState;
use fission_core::event::{InputEvent, PointerEvent};
use fission_core::{ActionId, NodeId, Runtime};
use fission_ir::semantics::{ActionTrigger, MouseCursor, Role};
use fission_ir::{ActionEntry, ActionSet, Op, Semantics};
use fission_layout::{LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot};

#[derive(Debug, Default)]
struct HoverState {
    outer_enter: usize,
    outer_exit: usize,
    inner_enter: usize,
    inner_exit: usize,
}

impl AppState for HoverState {}

fn inc_outer_enter(
    state: &mut HoverState,
    _action: &fission_core::ActionEnvelope,
    _target: NodeId,
) -> Result<()> {
    state.outer_enter += 1;
    Ok(())
}

fn inc_outer_exit(
    state: &mut HoverState,
    _action: &fission_core::ActionEnvelope,
    _target: NodeId,
) -> Result<()> {
    state.outer_exit += 1;
    Ok(())
}

fn inc_inner_enter(
    state: &mut HoverState,
    _action: &fission_core::ActionEnvelope,
    _target: NodeId,
) -> Result<()> {
    state.inner_enter += 1;
    Ok(())
}

fn inc_inner_exit(
    state: &mut HoverState,
    _action: &fission_core::ActionEnvelope,
    _target: NodeId,
) -> Result<()> {
    state.inner_exit += 1;
    Ok(())
}

fn hover_action(trigger: ActionTrigger, action_id: ActionId) -> ActionEntry {
    ActionEntry {
        trigger,
        action_id: action_id.as_u128(),
        payload_data: Some(Vec::new()),
    }
}

fn build_hover_ir() -> (fission_ir::CoreIR, LayoutSnapshot) {
    let outer_id = NodeId::explicit("outer");
    let inner_id = NodeId::explicit("inner");

    let outer_enter = ActionId::from_name("tests::hover_outer_enter");
    let outer_exit = ActionId::from_name("tests::hover_outer_exit");
    let inner_enter = ActionId::from_name("tests::hover_inner_enter");
    let inner_exit = ActionId::from_name("tests::hover_inner_exit");

    let inner_semantics = Semantics {
        role: Role::Generic,
        actions: ActionSet {
            entries: vec![
                hover_action(ActionTrigger::HoverEnter, inner_enter),
                hover_action(ActionTrigger::HoverExit, inner_exit),
                ActionEntry::hover_cursor(MouseCursor::Text),
            ],
        },
        ..Semantics::default()
    };
    let outer_semantics = Semantics {
        role: Role::Generic,
        actions: ActionSet {
            entries: vec![
                hover_action(ActionTrigger::HoverEnter, outer_enter),
                hover_action(ActionTrigger::HoverExit, outer_exit),
                ActionEntry::hover_cursor(MouseCursor::Pointer),
            ],
        },
        ..Semantics::default()
    };

    let mut ir = fission_ir::CoreIR::default();
    ir.add_node(inner_id, Op::Semantics(inner_semantics), Vec::new());
    ir.add_node(outer_id, Op::Semantics(outer_semantics), vec![inner_id]);
    ir.set_root(outer_id);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(200.0, 200.0));
    layout.nodes.insert(
        outer_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
            content_size: LayoutSize::new(100.0, 100.0),
        },
    );
    layout.nodes.insert(
        inner_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(10.0, 10.0, 30.0, 30.0),
            content_size: LayoutSize::new(30.0, 30.0),
        },
    );

    (ir, layout)
}

fn register_hover_reducers(runtime: &mut Runtime) -> Result<()> {
    runtime.add_app_state(Box::new(HoverState::default()))?;
    runtime.register_reducer::<HoverState>(
        ActionId::from_name("tests::hover_outer_enter"),
        inc_outer_enter,
    )?;
    runtime.register_reducer::<HoverState>(
        ActionId::from_name("tests::hover_outer_exit"),
        inc_outer_exit,
    )?;
    runtime.register_reducer::<HoverState>(
        ActionId::from_name("tests::hover_inner_enter"),
        inc_inner_enter,
    )?;
    runtime.register_reducer::<HoverState>(
        ActionId::from_name("tests::hover_inner_exit"),
        inc_inner_exit,
    )?;
    Ok(())
}

fn move_pointer(
    runtime: &mut Runtime,
    ir: &fission_ir::CoreIR,
    layout: &LayoutSnapshot,
    x: f32,
    y: f32,
) {
    runtime
        .handle_input(
            InputEvent::Pointer(PointerEvent::Move {
                point: LayoutPoint::new(x, y),
                modifiers: 0,
            }),
            ir,
            layout,
        )
        .expect("pointer move should succeed");
}

#[test]
fn hover_dispatches_once_per_transition_and_tracks_cursor_target() -> Result<()> {
    let mut runtime = Runtime::default();
    register_hover_reducers(&mut runtime)?;
    let (ir, layout) = build_hover_ir();

    move_pointer(&mut runtime, &ir, &layout, 20.0, 20.0);
    {
        let hover = runtime.get_app_state::<HoverState>().expect("hover state");
        assert_eq!(hover.outer_enter, 1);
        assert_eq!(hover.outer_exit, 0);
        assert_eq!(hover.inner_enter, 1);
        assert_eq!(hover.inner_exit, 0);
    }
    assert_eq!(
        runtime.runtime_state.interaction.cursor(),
        MouseCursor::Text
    );

    move_pointer(&mut runtime, &ir, &layout, 25.0, 25.0);
    {
        let hover = runtime.get_app_state::<HoverState>().expect("hover state");
        assert_eq!(hover.outer_enter, 1);
        assert_eq!(hover.outer_exit, 0);
        assert_eq!(hover.inner_enter, 1);
        assert_eq!(hover.inner_exit, 0);
    }
    assert_eq!(
        runtime.runtime_state.interaction.cursor(),
        MouseCursor::Text
    );

    move_pointer(&mut runtime, &ir, &layout, 80.0, 80.0);
    {
        let hover = runtime.get_app_state::<HoverState>().expect("hover state");
        assert_eq!(hover.outer_enter, 1);
        assert_eq!(hover.outer_exit, 0);
        assert_eq!(hover.inner_enter, 1);
        assert_eq!(hover.inner_exit, 1);
    }
    assert_eq!(
        runtime.runtime_state.interaction.cursor(),
        MouseCursor::Pointer
    );

    move_pointer(&mut runtime, &ir, &layout, 150.0, 150.0);
    {
        let hover = runtime.get_app_state::<HoverState>().expect("hover state");
        assert_eq!(hover.outer_enter, 1);
        assert_eq!(hover.outer_exit, 1);
        assert_eq!(hover.inner_enter, 1);
        assert_eq!(hover.inner_exit, 1);
    }
    assert_eq!(
        runtime.runtime_state.interaction.cursor(),
        MouseCursor::Default
    );

    Ok(())
}

#[test]
fn clear_hover_state_dispatches_exit_once() -> Result<()> {
    let mut runtime = Runtime::default();
    register_hover_reducers(&mut runtime)?;
    let (ir, layout) = build_hover_ir();

    move_pointer(&mut runtime, &ir, &layout, 20.0, 20.0);
    assert!(runtime.clear_hover_state(&ir, Some(LayoutPoint::new(20.0, 20.0)))?);
    assert!(!runtime.clear_hover_state(&ir, Some(LayoutPoint::new(20.0, 20.0)))?);

    let hover = runtime.get_app_state::<HoverState>().expect("hover state");
    assert_eq!(hover.outer_enter, 1);
    assert_eq!(hover.outer_exit, 1);
    assert_eq!(hover.inner_enter, 1);
    assert_eq!(hover.inner_exit, 1);
    assert_eq!(
        runtime.runtime_state.interaction.cursor(),
        MouseCursor::Default
    );

    Ok(())
}
