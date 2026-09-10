use fission_core::ui::{Checkbox, Column, Text, TextInput, Widget};
use fission_core::{ActionEnvelope, ActionId, WidgetId};
use fission_ir::{ActionTrigger, Op, Semantics, TextFieldValidationState};

fn semantics_at(ir: &fission_ir::CoreIR, id: WidgetId) -> &Semantics {
    match &ir.nodes[&id].op {
        Op::Semantics(semantics) => semantics,
        op => panic!("expected semantics at {id:?}, got {op:?}"),
    }
}

#[test]
fn form_field_relationships_augment_one_direct_control_without_replacing_its_state() {
    let control_id = WidgetId::explicit("terms");
    let label_id = WidgetId::explicit("terms.label");
    let message_id = WidgetId::explicit("terms.message");
    let toggle = ActionEnvelope {
        id: ActionId::from_name("terms.toggle"),
        payload: b"null".to_vec(),
    };
    let widget: Widget = Checkbox {
        id: Some(control_id),
        checked: true,
        on_toggle: Some(toggle.clone()),
        ..Default::default()
    }
    .into();
    let widget = widget.with_form_field_relationships(
        vec![label_id],
        vec![message_id],
        true,
        Some("Acceptance is required.".into()),
    );

    let ir = fission_core::internal::lower_widget_to_ir(&widget);
    let control = semantics_at(&ir, control_id);
    assert_eq!(control.labelled_by, vec![label_id]);
    assert_eq!(control.described_by, vec![message_id]);
    assert!(control.required);
    assert_eq!(control.validation_state, TextFieldValidationState::Invalid);
    assert_eq!(
        control.validation_message.as_deref(),
        Some("Acceptance is required.")
    );
    assert_eq!(control.checked, Some(true));
    assert!(control.actions.entries.iter().any(|entry| {
        entry.trigger == ActionTrigger::Default && entry.action_id == toggle.id.as_u128()
    }));
}

#[test]
fn form_field_relationships_leave_an_ambiguous_subtree_unchanged() {
    let first_id = WidgetId::explicit("first");
    let second_id = WidgetId::explicit("second");
    let widget: Widget = Column {
        children: vec![
            TextInput {
                id: Some(first_id),
                ..Default::default()
            }
            .into(),
            TextInput {
                id: Some(second_id),
                ..Default::default()
            }
            .into(),
        ],
        ..Default::default()
    }
    .into();
    let widget = widget.with_form_field_relationships(
        vec![WidgetId::explicit("pair.label")],
        vec![WidgetId::explicit("pair.message")],
        true,
        Some("No unique control.".into()),
    );

    let ir = fission_core::internal::lower_widget_to_ir(&widget);
    for control_id in [first_id, second_id] {
        let control = semantics_at(&ir, control_id);
        assert!(control.labelled_by.is_empty());
        assert!(control.described_by.is_empty());
        assert!(!control.required);
        assert_eq!(
            control.validation_state,
            TextFieldValidationState::Unvalidated
        );
        assert_eq!(control.validation_message, None);
    }
}

#[test]
fn form_field_relationships_leave_a_subtree_without_a_control_unchanged() {
    let widget: Widget = Text::new("Read-only detail").into();
    let widget = widget.with_form_field_relationships(
        vec![WidgetId::explicit("detail.label")],
        vec![WidgetId::explicit("detail.message")],
        true,
        Some("Not applicable.".into()),
    );

    let ir = fission_core::internal::lower_widget_to_ir(&widget);
    assert!(ir.nodes.values().all(|node| match &node.op {
        Op::Semantics(semantics) => {
            semantics.labelled_by.is_empty()
                && semantics.described_by.is_empty()
                && !semantics.required
        }
        _ => true,
    }));
}
