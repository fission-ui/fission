use fission_core::internal::BuildCtx;
use fission_core::ui::{Button, Text, Widget};
use fission_core::{build, ActionEnvelope, ActionId, Env, RuntimeState, View, WidgetId};
use fission_ir::{ActionTrigger, Op, Role};
use fission_widgets::Popover;

#[test]
fn supplied_trigger_remains_the_single_action_owner() {
    let action = ActionEnvelope {
        id: ActionId::from_name("popover_trigger_test::Toggle"),
        payload: vec![1, 2, 3],
    };
    let env = Env::default();
    let runtime = RuntimeState::default();
    let view = View::new(&(), &runtime, &env, None);
    let mut context = BuildCtx::<()>::new();

    let widget: Widget = build::enter(&mut context, &view, || {
        Popover {
            id: WidgetId::explicit("details-popover"),
            is_open: false,
            on_close: None,
            trigger: Button {
                child: Some(Text::new("Details").into()),
                on_press: Some(action.clone()),
                ..Default::default()
            }
            .into(),
            content: Text::new("Popover content").into(),
            motion: None,
        }
        .into()
    });

    let ir = fission_core::internal::lower_widget_to_ir(&widget);
    let buttons: Vec<_> = ir
        .nodes
        .values()
        .filter_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.role == Role::Button => Some(semantics),
            _ => None,
        })
        .collect();

    assert_eq!(buttons.len(), 1, "Popover must not add a second control");
    let actions: Vec<_> = buttons[0]
        .actions
        .entries
        .iter()
        .filter(|entry| entry.trigger == ActionTrigger::Default)
        .collect();
    assert_eq!(actions.len(), 1, "activation must dispatch exactly once");
    assert_eq!(actions[0].action_id, action.id.as_u128());
    assert_eq!(
        actions[0].payload_data.as_deref(),
        Some([1, 2, 3].as_slice())
    );
}
