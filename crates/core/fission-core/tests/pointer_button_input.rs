use fission_core::event::{InputEvent, PointerButton, PointerEvent};
use fission_core::{ActionEnvelope, ActionId, GlobalState, Runtime};
use fission_ir::semantics::ActionTrigger;
use fission_ir::{
    ActionEntry, ActionSet, CompositeStyle, CoreIR, CoreNode, Op, Role, Semantics, WidgetId,
};
use fission_layout::{LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot};

#[derive(Debug, Default)]
struct ClickState {
    primary: usize,
    secondary: usize,
}

impl GlobalState for ClickState {}

#[test]
fn primary_click_dispatches_default_action_only() -> anyhow::Result<()> {
    let (mut runtime, ir, layout, _) = click_runtime(true)?;

    click(
        &mut runtime,
        &ir,
        &layout,
        PointerButton::Primary,
        PointerButton::Primary,
    )?;

    let state = runtime.get_app_state::<ClickState>().expect("click state");
    assert_eq!(state.primary, 1);
    assert_eq!(state.secondary, 0);
    Ok(())
}

#[test]
fn secondary_click_never_falls_through_to_default_action() -> anyhow::Result<()> {
    let (mut runtime, ir, layout, node_id) = click_runtime(true)?;

    click(
        &mut runtime,
        &ir,
        &layout,
        PointerButton::Secondary,
        PointerButton::Secondary,
    )?;

    let state = runtime.get_app_state::<ClickState>().expect("click state");
    assert_eq!(state.primary, 0);
    assert_eq!(state.secondary, 1);
    assert_eq!(runtime.runtime_state.interaction.focused, None);
    assert!(!runtime.runtime_state.interaction.is_pressed(node_id));
    Ok(())
}

#[test]
fn secondary_click_without_secondary_action_does_not_activate_default() -> anyhow::Result<()> {
    let (mut runtime, ir, layout, _) = click_runtime(false)?;

    click(
        &mut runtime,
        &ir,
        &layout,
        PointerButton::Secondary,
        PointerButton::Secondary,
    )?;

    let state = runtime.get_app_state::<ClickState>().expect("click state");
    assert_eq!(state.primary, 0);
    assert_eq!(state.secondary, 0);
    Ok(())
}

#[test]
fn mismatched_pointer_buttons_cancel_activation() -> anyhow::Result<()> {
    let (mut runtime, ir, layout, _) = click_runtime(true)?;

    click(
        &mut runtime,
        &ir,
        &layout,
        PointerButton::Secondary,
        PointerButton::Primary,
    )?;

    let state = runtime.get_app_state::<ClickState>().expect("click state");
    assert_eq!(state.primary, 0);
    assert_eq!(state.secondary, 0);
    Ok(())
}

#[test]
fn primary_release_without_primary_press_does_not_activate_default() -> anyhow::Result<()> {
    let (mut runtime, ir, layout, _) = click_runtime(true)?;
    let point = LayoutPoint::new(20.0, 20.0);

    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Up {
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;

    let state = runtime.get_app_state::<ClickState>().expect("click state");
    assert_eq!(state.primary, 0);
    assert_eq!(state.secondary, 0);
    Ok(())
}

fn click_runtime(
    with_secondary_action: bool,
) -> anyhow::Result<(Runtime, CoreIR, LayoutSnapshot, WidgetId)> {
    let node_id = WidgetId::explicit("button");
    let primary_id = ActionId::from_name("pointer_button_test::Primary");
    let secondary_id = ActionId::from_name("pointer_button_test::Secondary");
    let mut entries = vec![action_entry(ActionTrigger::Default, primary_id)];
    if with_secondary_action {
        entries.push(action_entry(ActionTrigger::SecondaryClick, secondary_id));
    }

    let mut ir = CoreIR::default();
    ir.root = Some(node_id);
    ir.nodes.insert(
        node_id,
        CoreNode {
            id: node_id,
            op: Op::Semantics(Semantics {
                role: Role::Button,
                focusable: true,
                actions: ActionSet { entries },
                ..Default::default()
            }),
            composite: CompositeStyle::default(),
            children: Vec::new(),
            parent: None,
            hash: 0,
        },
    );

    let mut layout = LayoutSnapshot::new(LayoutSize::new(200.0, 100.0));
    layout.nodes.insert(
        node_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(10.0, 10.0, 100.0, 40.0),
            content_size: LayoutSize::new(100.0, 40.0),
        },
    );

    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(ClickState::default()))?;
    runtime.register_reducer::<ClickState>(primary_id, record_primary)?;
    runtime.register_reducer::<ClickState>(secondary_id, record_secondary)?;
    Ok((runtime, ir, layout, node_id))
}

fn action_entry(trigger: ActionTrigger, action_id: ActionId) -> ActionEntry {
    ActionEntry {
        trigger,
        action_id: action_id.as_u128(),
        payload_data: Some(Vec::new()),
    }
}

fn click(
    runtime: &mut Runtime,
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    down_button: PointerButton,
    up_button: PointerButton,
) -> anyhow::Result<()> {
    let point = LayoutPoint::new(20.0, 20.0);
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            point,
            button: down_button,
            modifiers: 0,
        }),
        ir,
        layout,
    )?;
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Up {
            point,
            button: up_button,
            modifiers: 0,
        }),
        ir,
        layout,
    )
}

fn record_primary(
    state: &mut ClickState,
    _action: &ActionEnvelope,
    _target: WidgetId,
) -> anyhow::Result<()> {
    state.primary += 1;
    Ok(())
}

fn record_secondary(
    state: &mut ClickState,
    _action: &ActionEnvelope,
    _target: WidgetId,
) -> anyhow::Result<()> {
    state.secondary += 1;
    Ok(())
}
