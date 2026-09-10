use fission_core::internal::BuildCtx;
use fission_core::ui::{Column, TextInput};
use fission_core::{build, ActionEnvelope, ActionId, Env, GlobalState, View, WidgetId};
use fission_ir::{
    ActionTrigger, CoreIR, LayoutOp, Op, PaintOp, Role, Semantics, TextFieldValidationState,
};
use fission_widgets::{Combobox, FormControl, Select};

const LABEL_ID_PATH: &[u32] = &[0x4c41_424c];
const MESSAGE_ID_PATH: &[u32] = &[0x4445_5343];

#[derive(Clone, Debug, Default)]
struct State;

impl GlobalState for State {}

fn action(name: &str) -> ActionEnvelope {
    ActionEnvelope {
        id: ActionId::from_name(name),
        payload: b"null".to_vec(),
    }
}

fn lower(build_control: impl FnOnce() -> FormControl, env: &Env) -> CoreIR {
    let state = State;
    let runtime = fission_core::RuntimeState::default();
    let view = View::new(&state, &runtime, env, None);
    let mut ctx = BuildCtx::<State>::new();
    let widget = build::enter(&mut ctx, &view, || build_control().into());
    fission_core::internal::lower_widget_to_ir(&widget)
}

fn semantics_at(ir: &CoreIR, id: WidgetId) -> &Semantics {
    match &ir.nodes[&id].op {
        Op::Semantics(semantics) => semantics,
        op => panic!("expected semantics at {id:?}, got {op:?}"),
    }
}

fn rich_text_style(
    ir: &CoreIR,
    expected_text: &str,
) -> (f32, u16, Option<f32>, fission_ir::op::Color) {
    ir.nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Paint(PaintOp::DrawRichText { runs, .. }) => runs
                .iter()
                .find(|run| run.text == expected_text)
                .map(|run| {
                    (
                        run.style.font_size,
                        run.style.font_weight,
                        run.style.line_height,
                        run.style.color,
                    )
                }),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing rich text {expected_text:?}"))
}

fn subtree_has_stroke_color(ir: &CoreIR, root: WidgetId, expected: fission_ir::op::Color) -> bool {
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        let Some(node) = ir.nodes.get(&id) else {
            continue;
        };
        if matches!(
            &node.op,
            Op::Paint(PaintOp::DrawRect {
                stroke: Some(stroke),
                ..
            }) if stroke.fill == fission_ir::op::Fill::Solid(expected)
        ) {
            return true;
        }
        pending.extend(node.children.iter().copied());
    }
    false
}

#[test]
fn form_control_uses_compact_theme_anatomy_and_stable_relations() {
    let mut env = Env::default();
    env.theme.tokens.spacing.s = 9.0;
    env.theme.components.text_input.label_style.font_size = Some(17.0);
    env.theme.components.text_input.label_style.font_weight = Some(600);
    env.theme.components.text_input.label_style.line_height = Some(17.0);
    env.theme.components.text_input.helper_style.font_size = Some(13.0);
    env.theme.components.text_input.helper_style.font_weight = Some(400);
    env.theme.components.text_input.helper_style.line_height = Some(19.0);

    let control_id = WidgetId::explicit("recovery-email.field");
    let input_id = WidgetId::explicit("recovery-email.input");
    let label_id = WidgetId::derived(control_id.as_u128(), LABEL_ID_PATH);
    let message_id = WidgetId::derived(control_id.as_u128(), MESSAGE_ID_PATH);
    let ir = lower(
        || FormControl {
            id: Some(control_id),
            label: Some("Recovery email".into()),
            child: TextInput {
                id: Some(input_id),
                value: "avery@".into(),
                on_input: Some(action("recovery-email.changed")),
                ..Default::default()
            }
            .into(),
            error: Some("Enter a complete email address.".into()),
            helper: Some("Used to restore access.".into()),
            required: true,
        },
        &env,
    );

    let group = semantics_at(&ir, ir.root.expect("form-control semantics root"));
    assert_eq!(group.labelled_by, vec![label_id]);
    assert_eq!(group.described_by, vec![message_id]);

    let label = semantics_at(&ir, label_id);
    assert_eq!(label.role, Role::Text);
    assert_eq!(label.label.as_deref(), Some("Recovery email"));
    let message = semantics_at(&ir, message_id);
    assert_eq!(message.role, Role::Text);
    assert_eq!(
        message.label.as_deref(),
        Some("Enter a complete email address.")
    );

    let input = semantics_at(&ir, input_id);
    assert_eq!(input.labelled_by, vec![label_id]);
    assert_eq!(input.described_by, vec![message_id]);
    assert!(input.required);
    assert_eq!(input.validation_state, TextFieldValidationState::Invalid);
    assert_eq!(
        input.validation_message.as_deref(),
        Some("Enter a complete email address.")
    );
    assert_eq!(input.value.as_deref(), Some("avery@"));
    assert!(input.text_editable);
    assert!(input
        .actions
        .entries
        .iter()
        .any(|entry| entry.trigger == ActionTrigger::TextChanged));
    assert!(
        subtree_has_stroke_color(&ir, input_id, env.theme.tokens.colors.error),
        "FormControl.error must select the input error recipe during lowering"
    );

    match &ir.nodes[&control_id].op {
        Op::Layout(LayoutOp::Flex { gap, .. }) => assert_eq!(*gap, Some(9.0)),
        op => panic!("expected form-control column at {control_id:?}, got {op:?}"),
    }

    let label_style = rich_text_style(&ir, "Recovery email *");
    assert_eq!(label_style.0, 17.0);
    assert_eq!(label_style.1, 600);
    assert_eq!(label_style.2, Some(17.0));
    assert_eq!(label_style.3, env.theme.tokens.colors.error);

    let error_style = rich_text_style(&ir, "Enter a complete email address.");
    assert_eq!(error_style.0, 13.0);
    assert_eq!(error_style.1, 400);
    assert_eq!(error_style.2, Some(19.0));
    assert_eq!(error_style.3, env.theme.tokens.colors.error);
    assert!(ir.nodes.values().all(|node| match &node.op {
        Op::Paint(PaintOp::DrawText { text, .. }) => text != "Used to restore access.",
        Op::Paint(PaintOp::DrawRichText { runs, .. }) => {
            runs.iter().all(|run| run.text != "Used to restore access.")
        }
        _ => true,
    }));
}

#[test]
fn explicit_form_control_anatomy_does_not_collide_with_nested_composite_nodes() {
    let env = Env::default();
    let field_id = WidgetId::explicit("collision-regression.field");
    let select_id = WidgetId::explicit("collision-regression.select");
    let label_id = WidgetId::derived(field_id.as_u128(), LABEL_ID_PATH);

    // Lowering allocates ordinary internal nodes beneath `field_id`. The
    // retained label must therefore use a disjoint anatomy path rather than a
    // short positional path that the lowering sequence can also allocate.
    let ir = lower(
        || FormControl {
            id: Some(field_id),
            label: Some("Role".into()),
            child: Select {
                id: select_id,
                selected_label: Some("Editor".into()),
                items: Vec::new(),
                is_open: false,
                on_toggle: Some(action("collision-regression.toggle")),
                trigger_semantics_identifier: Some("collision-regression.trigger".into()),
                placeholder: "Choose a role".into(),
                width: Some(240.0),
            }
            .into(),
            error: None,
            helper: None,
            required: false,
        },
        &env,
    );

    assert_eq!(semantics_at(&ir, label_id).label.as_deref(), Some("Role"));
    assert_eq!(
        semantics_at(&ir, WidgetId::derived(select_id.as_u128(), &[0, 1])).labelled_by,
        vec![label_id]
    );
}

#[test]
fn form_control_relates_helper_text_when_there_is_no_error() {
    let env = Env::default();
    let control_id = WidgetId::explicit("display-name.field");
    let message_id = WidgetId::derived(control_id.as_u128(), MESSAGE_ID_PATH);
    let ir = lower(
        || FormControl {
            id: Some(control_id),
            label: Some("Display name".into()),
            child: TextInput::default().into(),
            error: None,
            helper: Some("Visible to your team.".into()),
            required: false,
        },
        &env,
    );

    assert_eq!(
        semantics_at(&ir, ir.root.expect("form-control semantics root")).described_by,
        vec![message_id]
    );
    assert_eq!(
        semantics_at(&ir, message_id).label.as_deref(),
        Some("Visible to your team.")
    );
    let helper_style = rich_text_style(&ir, "Visible to your team.");
    assert_eq!(helper_style.0, env.theme.tokens.typography.font_size_base);
    assert_eq!(
        helper_style.3,
        env.theme
            .components
            .text_input
            .helper_style
            .text_color
            .expect("default helper text colour")
    );
}

#[test]
fn form_control_relates_the_actual_read_only_select_trigger() {
    let env = Env::default();
    let field_id = WidgetId::explicit("country.field");
    let select_id = WidgetId::explicit("country.select");
    let trigger_id = WidgetId::derived(select_id.as_u128(), &[0, 1]);
    let label_id = WidgetId::derived(field_id.as_u128(), LABEL_ID_PATH);
    let message_id = WidgetId::derived(field_id.as_u128(), MESSAGE_ID_PATH);
    let toggle = action("country.toggle");
    let ir = lower(
        || FormControl {
            id: Some(field_id),
            label: Some("Country".into()),
            child: Select {
                id: select_id,
                selected_label: Some("Canada".into()),
                items: Vec::new(),
                is_open: false,
                on_toggle: Some(toggle.clone()),
                trigger_semantics_identifier: Some("country.trigger".into()),
                placeholder: "Choose a country".into(),
                width: Some(240.0),
            }
            .into(),
            error: Some("Choose a supported country.".into()),
            helper: None,
            required: true,
        },
        &env,
    );

    let trigger = semantics_at(&ir, trigger_id);
    assert_eq!(trigger.role, Role::ComboBox);
    assert_eq!(trigger.labelled_by, vec![label_id]);
    assert_eq!(trigger.described_by, vec![message_id]);
    assert!(trigger.required);
    assert_eq!(trigger.validation_state, TextFieldValidationState::Invalid);
    assert!(
        subtree_has_stroke_color(&ir, trigger_id, env.theme.tokens.colors.error),
        "FormControl.error must select the Select trigger error recipe"
    );
    assert_eq!(trigger.value.as_deref(), Some("Canada"));
    assert!(!trigger.text_editable);
    assert!(trigger.actions.entries.iter().any(|entry| {
        entry.trigger == ActionTrigger::Default && entry.action_id == toggle.id.as_u128()
    }));
}

#[test]
fn form_control_relates_the_actual_editable_combobox_input() {
    let env = Env::default();
    let field_id = WidgetId::explicit("assignee.field");
    let combobox_id = WidgetId::explicit("assignee.combobox");
    let input_id = WidgetId::derived(combobox_id.as_u128(), &[0, 1]);
    let label_id = WidgetId::derived(field_id.as_u128(), LABEL_ID_PATH);
    let message_id = WidgetId::derived(field_id.as_u128(), MESSAGE_ID_PATH);
    let changed = action("assignee.changed");
    let ir = lower(
        || FormControl {
            id: Some(field_id),
            label: Some("Assignee".into()),
            child: Combobox {
                id: combobox_id,
                value: "Av".into(),
                items: vec!["Avery".into()],
                is_open: false,
                width: Some(240.0),
                max_popup_height: None,
                on_input: Some(changed.clone()),
                on_select: None,
                on_toggle: Some(action("assignee.toggle")),
            }
            .into(),
            error: Some("Choose an available person.".into()),
            helper: None,
            required: true,
        },
        &env,
    );

    let input = semantics_at(&ir, input_id);
    assert_eq!(input.role, Role::ComboBox);
    assert!(input.text_editable);
    assert_eq!(input.labelled_by, vec![label_id]);
    assert_eq!(input.described_by, vec![message_id]);
    assert!(input.required);
    assert_eq!(input.validation_state, TextFieldValidationState::Invalid);
    assert_eq!(
        input.validation_message.as_deref(),
        Some("Choose an available person.")
    );
    assert!(
        subtree_has_stroke_color(&ir, input_id, env.theme.tokens.colors.error),
        "FormControl.error must select the Combobox input error recipe"
    );
    assert!(input.actions.entries.iter().any(|entry| {
        entry.trigger == ActionTrigger::TextChanged && entry.action_id == changed.id.as_u128()
    }));
}

#[test]
fn form_control_does_not_guess_between_multiple_controls() {
    let env = Env::default();
    let field_id = WidgetId::explicit("ambiguous.field");
    let first_id = WidgetId::explicit("ambiguous.first");
    let second_id = WidgetId::explicit("ambiguous.second");
    let label_id = WidgetId::derived(field_id.as_u128(), LABEL_ID_PATH);
    let message_id = WidgetId::derived(field_id.as_u128(), MESSAGE_ID_PATH);
    let ir = lower(
        || FormControl {
            id: Some(field_id),
            label: Some("Ambiguous pair".into()),
            child: Column {
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
            .into(),
            error: Some("This wrapper cannot choose a field.".into()),
            helper: None,
            required: true,
        },
        &env,
    );

    let group = semantics_at(&ir, ir.root.expect("form-control semantics root"));
    assert_eq!(group.labelled_by, vec![label_id]);
    assert_eq!(group.described_by, vec![message_id]);
    for input_id in [first_id, second_id] {
        let input = semantics_at(&ir, input_id);
        assert!(input.labelled_by.is_empty());
        assert!(input.described_by.is_empty());
        assert!(!input.required);
        assert_eq!(
            input.validation_state,
            TextFieldValidationState::Unvalidated
        );
        assert_eq!(input.validation_message, None);
    }
}
