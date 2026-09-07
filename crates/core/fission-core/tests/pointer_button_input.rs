use fission_core::event::{
    GestureEvent, InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent,
};
use fission_core::{
    Action, ActionEnvelope, ActionId, ActionRegistry, GlobalState, ReducerContext, Runtime,
};
use fission_ir::semantics::ActionTrigger;
use fission_ir::{
    ActionEntry, ActionSet, CompositeStyle, CoreIR, CoreNode, Op, Role, Semantics, WidgetId,
};
use fission_layout::{LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default)]
struct ClickState {
    primary: usize,
    secondary: usize,
    long_press: usize,
    long_press_was_pointer: bool,
}

impl GlobalState for ClickState {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RecordLongPress;

impl Action for RecordLongPress {
    fn static_id() -> ActionId {
        ActionId::from_name("pointer_button_test::LongPress")
    }
}

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
    assert_eq!(state.long_press, 0);
    Ok(())
}

#[test]
fn held_primary_with_long_press_dispatches_long_press_instead_of_default() -> anyhow::Result<()> {
    let (mut runtime, ir, layout, _) = click_runtime_with_long_press()?;
    let point = LayoutPoint::new(20.0, 20.0);

    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;
    runtime.tick(500)?;
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
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
    assert_eq!(state.long_press, 1);
    assert!(state.long_press_was_pointer);
    assert!(runtime.runtime_state.gesture.pointer_down_at.is_none());
    assert!(!runtime.runtime_state.gesture.long_press_dispatched);
    Ok(())
}

#[test]
fn platform_long_press_then_release_does_not_dispatch_twice() -> anyhow::Result<()> {
    let (mut runtime, ir, layout, _) = click_runtime_with_long_press()?;
    let point = LayoutPoint::new(20.0, 20.0);

    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;
    runtime.tick(500)?;
    runtime.handle_input(
        InputEvent::Gesture(GestureEvent::LongPress { point }),
        &ir,
        &layout,
    )?;
    runtime.handle_input(
        InputEvent::Gesture(GestureEvent::LongPress { point }),
        &ir,
        &layout,
    )?;
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
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
    assert_eq!(state.long_press, 1);
    assert!(state.long_press_was_pointer);
    assert!(runtime.runtime_state.gesture.pointer_down_at.is_none());
    assert!(!runtime.runtime_state.gesture.long_press_dispatched);
    Ok(())
}

#[test]
fn held_primary_without_long_press_action_still_dispatches_default() -> anyhow::Result<()> {
    let (mut runtime, ir, layout, _) = click_runtime(false)?;
    let point = LayoutPoint::new(20.0, 20.0);

    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;
    runtime.tick(500)?;
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;

    let state = runtime.get_app_state::<ClickState>().expect("click state");
    assert_eq!(state.primary, 1);
    assert_eq!(state.secondary, 0);
    assert_eq!(state.long_press, 0);
    Ok(())
}

#[test]
fn produced_text_activation_keys_dispatch_default_action() -> anyhow::Result<()> {
    for (key_code, text) in [(KeyCode::Enter, "\r"), (KeyCode::Space, " ")] {
        let (mut runtime, ir, layout, node_id) = click_runtime(false)?;
        runtime.runtime_state.interaction.set_focused(Some(node_id));

        runtime.handle_input(
            InputEvent::Keyboard(KeyEvent::DownWithText {
                key_code,
                modifiers: 0,
                text: text.into(),
            }),
            &ir,
            &layout,
        )?;

        let state = runtime.get_app_state::<ClickState>().expect("click state");
        assert_eq!(state.primary, 1);
        assert_eq!(state.secondary, 0);
    }
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
            pointer_id: Default::default(),
            kind: Default::default(),
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

#[test]
fn semantic_context_menu_request_dispatches_secondary_only() -> anyhow::Result<()> {
    let (mut runtime, ir, layout, _) = click_runtime(true)?;

    runtime.handle_input(
        InputEvent::ContextMenuRequested {
            point: LayoutPoint::new(20.0, 20.0),
            modifiers: 0,
        },
        &ir,
        &layout,
    )?;

    let state = runtime.get_app_state::<ClickState>().expect("click state");
    assert_eq!(state.primary, 0);
    assert_eq!(state.secondary, 1);
    Ok(())
}

fn click_runtime(
    with_secondary_action: bool,
) -> anyhow::Result<(Runtime, CoreIR, LayoutSnapshot, WidgetId)> {
    click_runtime_with_actions(with_secondary_action, false)
}

fn click_runtime_with_long_press() -> anyhow::Result<(Runtime, CoreIR, LayoutSnapshot, WidgetId)> {
    click_runtime_with_actions(false, true)
}

fn click_runtime_with_actions(
    with_secondary_action: bool,
    with_long_press_action: bool,
) -> anyhow::Result<(Runtime, CoreIR, LayoutSnapshot, WidgetId)> {
    let node_id = WidgetId::explicit("button");
    let primary_id = ActionId::from_name("pointer_button_test::Primary");
    let secondary_id = ActionId::from_name("pointer_button_test::Secondary");
    let long_press_id = RecordLongPress::static_id();
    let mut entries = vec![action_entry(ActionTrigger::Default, primary_id)];
    if with_secondary_action {
        entries.push(action_entry(ActionTrigger::SecondaryClick, secondary_id));
    }
    if with_long_press_action {
        entries.push(ActionEntry {
            trigger: ActionTrigger::LongPress,
            action_id: long_press_id.as_u128(),
            payload_data: Some(RecordLongPress.encode()),
        });
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
    let mut registry = ActionRegistry::<ClickState>::new();
    registry.register(
        record_long_press as fn(&mut ClickState, RecordLongPress, &mut ReducerContext<ClickState>),
    );
    runtime.absorb_registry(registry);
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
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: down_button,
            modifiers: 0,
        }),
        ir,
        layout,
    )?;
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
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

fn record_long_press(
    state: &mut ClickState,
    _action: RecordLongPress,
    context: &mut ReducerContext<ClickState>,
) {
    state.long_press += 1;
    state.long_press_was_pointer = context.input.as_pointer().is_some();
}
