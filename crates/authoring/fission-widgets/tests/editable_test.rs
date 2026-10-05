use fission_core::authoring::BuildCtx;
use fission_core::{build, ActionEnvelope, ActionId, GlobalState, View, WidgetId};
use fission_ir::ActionTrigger;
use fission_widgets::Editable;

#[derive(Clone, Debug, Default)]
struct State;

impl GlobalState for State {}

fn action(name: &str) -> ActionEnvelope {
    ActionEnvelope {
        id: ActionId::from_name(name),
        payload: Vec::new(),
    }
}

#[test]
fn editing_wires_submit_and_cancel_to_enter_and_escape_contracts() {
    let state = State;
    let runtime = fission_core::RuntimeState::default();
    let env = fission_core::Env::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<State>::new();
    let submit = action("editable.submit");
    let cancel = action("editable.cancel");
    let widget = build::enter(&mut ctx, &view, || {
        Editable {
            id: Some(WidgetId::explicit("project-name")),
            value: "Fission".into(),
            placeholder: "Project name".into(),
            is_editing: true,
            on_input: None,
            on_submit: Some(submit.clone()),
            on_edit: None,
            on_cancel: Some(cancel.clone()),
        }
        .into()
    });
    let ir = fission_core::internal::lower_widget_to_ir(&widget);

    assert!(ir.nodes.values().any(|node| matches!(
        &node.op,
        fission_ir::Op::Semantics(semantics) if semantics.actions.entries.iter().any(|entry|
            entry.trigger == ActionTrigger::Submit
                && entry.action_id == submit.id.as_u128())
    )));
    assert!(ir.nodes.values().any(|node| matches!(
        &node.op,
        fission_ir::Op::Semantics(semantics) if semantics.actions.entries.iter().any(|entry|
            entry.trigger == ActionTrigger::Dismiss
                && entry.action_id == cancel.id.as_u128())
    )));
}
