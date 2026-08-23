use anyhow::Result;
use fission_core::event::{InputEvent, PointerButton, PointerEvent};
use fission_core::{ActionEnvelope, ActionId, GlobalState, Runtime, WidgetId};
use fission_ir::op::LayoutOp;
use fission_ir::semantics::{ActionTrigger, FocusPolicy, Role};
use fission_ir::{ActionEntry, ActionSet, CoreIR, Op, Semantics};
use fission_layout::{LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot};

#[derive(Debug, Default)]
struct CommandState {
    count: usize,
    last_target: Option<WidgetId>,
}

impl GlobalState for CommandState {}

fn record_command(
    state: &mut CommandState,
    _action: &ActionEnvelope,
    target: WidgetId,
) -> Result<()> {
    state.count += 1;
    state.last_target = Some(target);
    Ok(())
}

fn command_action_id() -> ActionId {
    ActionId::from_name("tests::focus_policy_command")
}

fn build_ir(button_policy: FocusPolicy) -> (CoreIR, LayoutSnapshot, WidgetId, WidgetId) {
    let root_id = WidgetId::explicit("root");
    let input_id = WidgetId::explicit("editor");
    let button_id = WidgetId::explicit("ribbon_bold");

    let mut ir = CoreIR::default();
    ir.add_node(
        input_id,
        Op::Semantics(Semantics {
            role: Role::TextInput,
            focusable: true,
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        button_id,
        Op::Semantics(Semantics {
            role: Role::Button,
            focusable: true,
            focus_policy: button_policy,
            actions: ActionSet {
                entries: vec![ActionEntry {
                    trigger: ActionTrigger::Default,
                    action_id: command_action_id().as_u128(),
                    payload_data: Some(Vec::new()),
                }],
            },
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        root_id,
        Op::Layout(LayoutOp::ZStack),
        vec![input_id, button_id],
    );
    ir.set_root(root_id);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(300.0, 140.0));
    layout.nodes.insert(
        root_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 300.0, 140.0),
            content_size: LayoutSize::new(300.0, 140.0),
        },
    );
    layout.nodes.insert(
        input_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(12.0, 60.0, 240.0, 60.0),
            content_size: LayoutSize::new(240.0, 60.0),
        },
    );
    layout.nodes.insert(
        button_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(12.0, 12.0, 80.0, 32.0),
            content_size: LayoutSize::new(80.0, 32.0),
        },
    );

    (ir, layout, input_id, button_id)
}

fn click_ribbon_button(runtime: &mut Runtime, ir: &CoreIR, layout: &LayoutSnapshot) -> Result<()> {
    let point = LayoutPoint::new(24.0, 24.0);
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
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
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        ir,
        layout,
    )
}

#[test]
fn preserve_current_on_pointer_button_keeps_editor_focus_and_dispatches() -> Result<()> {
    let (ir, layout, input_id, button_id) = build_ir(FocusPolicy::PreserveCurrentOnPointer);
    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(CommandState::default()))?;
    runtime.register_reducer::<CommandState>(command_action_id(), record_command)?;
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(input_id));

    click_ribbon_button(&mut runtime, &ir, &layout)?;

    assert_eq!(runtime.runtime_state.interaction.focused, Some(input_id));
    let state = runtime
        .get_app_state::<CommandState>()
        .expect("command state");
    assert_eq!(state.count, 1);
    assert_eq!(state.last_target, Some(button_id));
    Ok(())
}

#[test]
fn default_pointer_focus_button_still_takes_focus() -> Result<()> {
    let (ir, layout, input_id, button_id) = build_ir(FocusPolicy::FocusOnPointer);
    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(input_id));

    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: LayoutPoint::new(24.0, 24.0),
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;

    assert_eq!(runtime.runtime_state.interaction.focused, Some(button_id));
    Ok(())
}
