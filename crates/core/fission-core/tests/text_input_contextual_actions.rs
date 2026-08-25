use std::collections::BTreeMap;

use fission_core::env::{
    ContextMenuState, InteractionStateMap, ScrollStateMap, SelectableTextStateMap, TextEditStateMap,
};
use fission_core::event::{ImeEvent, InputEvent, KeyCode, KeyEvent};
use fission_core::input::text::TextInputController;
use fission_core::input::{
    prepare_scoped_text_input_change, prepare_text_input_change, ControllerContext,
    InputController, TextEditingConvention,
};
use fission_core::internal::{self, BuildCtx};
use fission_core::ui::{Column, TextInput, Widget};
use fission_core::{
    build, Action, ActionEnvelope, ActionId, ActionInput, Env, GlobalState, ReducerContext,
    Runtime, StateField, UpdateTextInput, View, WidgetId, WidgetIdExt,
};
use fission_ir::semantics::{ActionEntry, ActionSet, ActionTrigger, Role};
use fission_ir::{CoreIR, Op, Semantics};
use fission_layout::{LayoutSize, LayoutSnapshot};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct EditText;

impl Action for EditText {
    fn static_id() -> ActionId {
        ActionId::from_name("text_input_actions::EditText")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct UpdateField(String);

impl Action for UpdateField {
    fn static_id() -> ActionId {
        ActionId::from_name("text_input_actions::UpdateField")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Draft {
    values: BTreeMap<String, String>,
    last_node: Option<WidgetId>,
    last_selection: Option<(usize, usize)>,
    invocations: usize,
    lifecycle: Vec<(
        String,
        fission_core::TextEditPhase,
        fission_core::TextEditSource,
    )>,
    validation: Option<(
        fission_ir::semantics::TextFieldValidationState,
        Option<String>,
    )>,
}

impl GlobalState for Draft {}

fn update_field(draft: &mut Draft, action: UpdateField, context: &mut ReducerContext<Draft>) {
    let Some(change) = context.input.text_change() else {
        return;
    };
    draft
        .values
        .insert(action.0.clone(), change.new_text.clone());
    draft.last_node = Some(change.node_id);
    draft.last_selection = Some((change.new_caret, change.new_anchor));
    draft.invocations += 1;
    draft
        .lifecycle
        .push((action.0, change.phase, change.source));
    if let Some(state) = change.validation_state {
        draft.validation = Some((state, change.validation_message.clone()));
    }
}

fn envelope<A: Action>(action: A) -> ActionEnvelope {
    ActionEnvelope {
        id: A::static_id(),
        payload: action.encode(),
    }
}

fn bound_field_action(field: &str) -> ActionEnvelope {
    envelope(UpdateField(field.to_string()))
}

fn text_semantics<'a>(ir: &'a CoreIR, identifier: &str) -> (WidgetId, &'a Semantics) {
    ir.nodes
        .iter()
        .find_map(|(id, node)| match &node.op {
            Op::Semantics(semantics)
                if semantics.role == Role::TextInput
                    && semantics.identifier.as_deref() == Some(identifier) =>
            {
                Some((*id, semantics))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("text semantics `{identifier}`"))
}

fn one_action_semantics(action: &ActionEnvelope, value: &str) -> Semantics {
    Semantics {
        role: Role::TextInput,
        value: Some(value.to_string()),
        actions: ActionSet {
            entries: vec![ActionEntry {
                trigger: ActionTrigger::TextChanged,
                action_id: action.id.as_u128(),
                payload_data: Some(action.payload.clone()),
            }],
        },
        focusable: true,
        ..Semantics::default()
    }
}

#[test]
fn lowering_preserves_unit_and_field_action_payloads() {
    let actions = [envelope(EditText), bound_field_action("smtp_host")];

    for (index, action) in actions.into_iter().enumerate() {
        let identifier = format!("field.{index}");
        let ir = internal::lower_widget_to_ir(&Widget::from(TextInput {
            semantics_identifier: Some(identifier.clone()),
            on_input: Some(action.clone()),
            ..Default::default()
        }));
        let (_, semantics) = text_semantics(&ir, &identifier);
        let entry = semantics.actions.entries.first().expect("input action");
        assert_eq!(entry.trigger, ActionTrigger::TextChanged);
        assert_eq!(entry.action_id, action.id.as_u128());
        assert_eq!(
            entry.payload_data.as_deref(),
            Some(action.payload.as_slice())
        );
    }
}

#[test]
fn complete_controlled_value_lowers_text_selection_and_composition_atomically() {
    let text = "a😀bc";
    let editing_value = fission_core::TextEditingValue::new(
        text,
        fission_core::TextSelection::new(
            text,
            "a😀".len(),
            1,
            fission_core::TextAffinity::Upstream,
        )
        .unwrap(),
        Some(fission_core::TextRange::new(text, 1, "a😀".len()).unwrap()),
    )
    .unwrap();
    let ir = internal::lower_widget_to_ir(&Widget::from(
        TextInput::default()
            .semantics_identifier("controlled")
            .editing_value(editing_value),
    ));
    let (_, semantics) = text_semantics(&ir, "controlled");
    assert_eq!(semantics.value.as_deref(), Some(text));
    assert_eq!(semantics.text_selection, Some(("a😀".len(), 1)));
    assert_eq!(semantics.ime_preedit_range, Some((1, "a😀".len())));
}

#[test]
fn field_contract_lowers_validation_and_autofill_group() {
    let ir = internal::lower_widget_to_ir(&Widget::from(
        TextInput::default()
            .semantics_identifier("account-email")
            .name("email")
            .autofill_group("account")
            .required(true)
            .min_length(3)
            .validation_pattern(".+@.+")
            .validation_state(fission_ir::semantics::TextFieldValidationState::Invalid)
            .validation_message("Enter a valid email address"),
    ));
    let (_, semantics) = text_semantics(&ir, "account-email");
    assert_eq!(semantics.text_field_name.as_deref(), Some("email"));
    assert_eq!(semantics.autofill_group.as_deref(), Some("account"));
    assert!(semantics.required);
    assert_eq!(semantics.min_length, Some(3));
    assert_eq!(semantics.validation_pattern.as_deref(), Some(".+@.+"));
    assert_eq!(
        semantics.validation_state,
        fission_ir::semantics::TextFieldValidationState::Invalid
    );
    assert_eq!(
        semantics.validation_message.as_deref(),
        Some("Enter a valid email address")
    );
}

#[test]
fn validation_pattern_uses_the_same_field_contract_on_canvas_targets() {
    let invalid = internal::lower_widget_to_ir(&Widget::from(
        TextInput::default()
            .value("not-an-email")
            .semantics_identifier("pattern.invalid")
            .validation_pattern(".+@.+"),
    ));
    assert_eq!(
        text_semantics(&invalid, "pattern.invalid")
            .1
            .validation_state,
        fission_ir::semantics::TextFieldValidationState::Invalid
    );

    let valid = internal::lower_widget_to_ir(&Widget::from(
        TextInput::default()
            .value("person@example.com")
            .semantics_identifier("pattern.valid")
            .validation_pattern(".+@.+"),
    ));
    assert_ne!(
        text_semantics(&valid, "pattern.valid").1.validation_state,
        fission_ir::semantics::TextFieldValidationState::Invalid
    );
}

struct ValidatedForm;

impl From<ValidatedForm> for Widget {
    fn from(_: ValidatedForm) -> Widget {
        let (context, _) = build::current::<Draft>();
        TextInput {
            id: Some(WidgetId::explicit("account.email")),
            semantics_identifier: Some("account.email".into()),
            name: Some("email".into()),
            form_id: Some("account".into()),
            required: true,
            validation_message: Some("Email is required".into()),
            on_validation: Some(context.bind(
                UpdateField("email".into()),
                fission_core::reduce!(update_field),
            )),
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn form_validation_dispatches_typed_results_with_static_context() {
    let mut runtime = Runtime::default().with_global_state(Draft::default());
    let env = Env::default();
    let (widget, context) = {
        let state = runtime.get_global_state::<Draft>().unwrap();
        let view = View::new(state, &runtime.runtime_state, &env, None);
        let mut context = BuildCtx::<Draft>::new();
        let widget = build::enter(&mut context, &view, || ValidatedForm.into());
        (widget, context)
    };
    let ir = internal::lower_widget_to_ir(&widget);
    runtime.absorb_registry(context.registry);
    assert!(
        runtime.queue_runtime_effect(fission_core::RuntimeEffect::TextFormValidation {
            form_id: "account".into(),
        })
    );
    assert!(runtime.post_layout_hook(&ir, &LayoutSnapshot::default()));

    let state = runtime.get_global_state::<Draft>().unwrap();
    assert_eq!(
        state.validation,
        Some((
            fission_ir::semantics::TextFieldValidationState::Invalid,
            Some("Email is required".into()),
        ))
    );
    assert_eq!(
        state.lifecycle.last(),
        Some(&(
            "email".into(),
            fission_core::TextEditPhase::Validated,
            fission_core::TextEditSource::Programmatic,
        ))
    );
}

#[test]
fn preparation_separates_static_context_from_complete_edit_details() {
    let action = bound_field_action("smtp_host");
    let node_id = WidgetId::explicit("smtp-host");
    let semantics = one_action_semantics(&action, "");

    let (prepared, input) =
        prepare_text_input_change(&semantics, node_id, "greenmail".into(), 7, 2).unwrap();

    assert_eq!(prepared, action);
    let change = input.text_change().expect("text change");
    assert_eq!(change.node_id, node_id);
    assert_eq!(change.new_text, "greenmail");
    assert_eq!(change.new_caret, 7);
    assert_eq!(change.new_anchor, 2);
}

#[test]
fn scoped_preparation_resolves_the_nearest_action_scope() {
    let action = bound_field_action("smtp_host");
    let node_id = WidgetId::explicit("scoped-input");
    let mut semantics = one_action_semantics(&action, "");
    semantics.action_scope_id = Some(73);
    let mut ir = CoreIR::default();
    ir.nodes.insert(
        node_id,
        fission_ir::CoreNode {
            id: node_id,
            parent: None,
            children: Vec::new(),
            op: Op::Semantics(semantics),
            composite: fission_ir::CompositeStyle::default(),
            hash: 0,
        },
    );
    let semantics = match &ir.nodes[&node_id].op {
        Op::Semantics(semantics) => semantics,
        _ => unreachable!(),
    };

    let (prepared, input) =
        prepare_scoped_text_input_change(&ir, semantics, node_id, "greenmail".into(), 9, 4)
            .unwrap();

    assert_eq!(prepared, action);
    assert_eq!(input.action_scope_id(), Some(73));
    assert_eq!(input.scoped_target(), Some(node_id));
    assert_eq!(input.text_change().unwrap().new_text, "greenmail");
}

fn dispatch_native_edit(
    event: InputEvent,
    initial_text: &str,
    caret: usize,
) -> (WidgetId, ActionEnvelope, ActionInput) {
    let action = bound_field_action("smtp_host");
    let node_id = WidgetId::explicit("native-input");
    let mut semantics = one_action_semantics(&action, initial_text);
    semantics.action_scope_id = Some(91);
    let mut ir = CoreIR::default();
    ir.nodes.insert(
        node_id,
        fission_ir::CoreNode {
            id: node_id,
            parent: None,
            children: Vec::new(),
            op: Op::Semantics(semantics),
            composite: fission_ir::CompositeStyle::default(),
            hash: 0,
        },
    );

    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 40.0));
    let mut text_edit = TextEditStateMap::default();
    text_edit.sync_from_runtime(node_id, initial_text, None, None, false);
    text_edit.set_caret(node_id, caret, Some(caret));
    let mut selectable_text = SelectableTextStateMap::default();
    let mut context_menu = ContextMenuState::default();
    let mut interaction = InteractionStateMap::default();
    interaction.set_focused(Some(node_id));
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let viewport = fission_core::ViewportStateMap::default();
    let mut context = ControllerContext {
        ir: &ir,
        layout: &layout,
        text_edit: &mut text_edit,
        selectable_text: &mut selectable_text,
        context_menu: &mut context_menu,
        interaction: &mut interaction,
        scroll: &mut scroll,
        viewport: &viewport,
        gesture: &mut gesture,
        editing_convention: TextEditingConvention::Standard,
        current_time: 0,
        clipboard: None,
        measurer: None,
        dispatched_actions: Vec::new(),
    };

    assert!(TextInputController.handle_event(&mut context, &event));
    assert_eq!(context.dispatched_actions.len(), 1);
    context.dispatched_actions.pop().unwrap()
}

#[test]
fn keyboard_edit_preserves_action_and_reports_post_edit_selection() {
    let (target, action, input) = dispatch_native_edit(
        InputEvent::Keyboard(KeyEvent::Down {
            key_code: KeyCode::Char('Z'),
            modifiers: 0,
        }),
        "ab",
        1,
    );

    assert_eq!(target, WidgetId::explicit("native-input"));
    assert_eq!(action, bound_field_action("smtp_host"));
    assert_eq!(input.action_scope_id(), Some(91));
    let change = input.text_change().unwrap();
    assert_eq!(change.new_text, "aZb");
    assert_eq!((change.new_caret, change.new_anchor), (2, 2));
}

#[test]
fn ime_commit_uses_the_same_text_input_contract() {
    let (_, action, input) = dispatch_native_edit(
        InputEvent::Ime(ImeEvent::Commit { text: "界".into() }),
        "ab",
        1,
    );

    assert_eq!(action, bound_field_action("smtp_host"));
    let change = input.text_change().unwrap();
    assert_eq!(change.new_text, "a界b");
    assert_eq!((change.new_caret, change.new_anchor), (4, 4));
}

#[test]
fn complete_value_reconciliation_uses_the_same_payload_preserving_contract() {
    let text = "corrected";
    let value = fission_core::TextEditingValue::new(
        text,
        fission_core::TextSelection::collapsed(fission_core::TextPosition::at_end(text)),
        None,
    )
    .unwrap();
    let (_, action, input) = dispatch_native_edit(
        InputEvent::TextEdit(fission_core::TextEditCommand::SetValue {
            value,
            source: fission_core::TextEditSource::Autocorrect,
            phase: fission_core::TextValuePhase::Committed,
        }),
        "corected",
        "corected".len(),
    );
    assert_eq!(action, bound_field_action("smtp_host"));
    let change = input.text_change().unwrap();
    assert_eq!(change.old_value.text, "corected");
    assert_eq!(change.new_value.text, "corrected");
    assert_eq!(change.source, fission_core::TextEditSource::Autocorrect);
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SearchState {
    value: String,
    invocations: usize,
    node: Option<WidgetId>,
    selection: Option<(usize, usize)>,
}

impl GlobalState for SearchState {}

fn edit_search(state: &mut SearchState, _: EditText, context: &mut ReducerContext<SearchState>) {
    let Some(change) = context.input.text_change() else {
        return;
    };
    state.value = change.new_text.clone();
    state.invocations += 1;
    state.node = Some(change.node_id);
    state.selection = Some((change.new_caret, change.new_anchor));
}

struct GlobalEditor;

impl From<GlobalEditor> for Widget {
    fn from(_: GlobalEditor) -> Widget {
        let (context, _) = build::current::<SearchState>();
        TextInput {
            semantics_identifier: Some("global.search".into()),
            on_input: Some(context.bind(EditText, fission_core::reduce!(edit_search))),
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn simple_static_action_reads_the_edit_from_reducer_input() {
    let mut state = SearchState::default();
    let runtime_state = fission_core::RuntimeState::default();
    let env = Env::default();
    let view = View::new(&state, &runtime_state, &env, None);
    let mut context = BuildCtx::<SearchState>::new();
    let widget = build::enter(&mut context, &view, || GlobalEditor.into());
    let ir = internal::lower_widget_to_ir(&widget);
    let (node_id, semantics) = text_semantics(&ir, "global.search");
    let (action, input) =
        prepare_text_input_change(semantics, node_id, "query".into(), 5, 1).unwrap();

    context
        .registry
        .dispatch_with_input(&mut state, &action, node_id, &input)
        .unwrap();

    assert_eq!(state.value, "query");
    assert_eq!(state.invocations, 1);
    assert_eq!(state.node, Some(node_id));
    assert_eq!(state.selection, Some((5, 1)));
}

struct GlobalFields;

impl From<GlobalFields> for Widget {
    fn from(_: GlobalFields) -> Widget {
        let (context, view) = build::current::<Draft>();
        Column {
            children: ["first.host", "first.port", "second.host", "second.port"]
                .into_iter()
                .map(|field| {
                    TextInput {
                        id: Some(WidgetId::explicit(field)),
                        semantics_identifier: Some(field.into()),
                        value: view.state().values.get(field).cloned().unwrap_or_default(),
                        on_input: Some(context.bind(
                            UpdateField(field.into()),
                            fission_core::reduce!(update_field),
                        )),
                        ..Default::default()
                    }
                    .into()
                })
                .collect(),
            ..Default::default()
        }
        .into()
    }
}

fn rebuild_global_fields(runtime: &mut Runtime, env: &Env) -> CoreIR {
    let (widget, context) = {
        let state = runtime.get_global_state::<Draft>().unwrap();
        let view = View::new(state, &runtime.runtime_state, env, None);
        let mut context = BuildCtx::<Draft>::new();
        let widget = build::enter(&mut context, &view, || GlobalFields.into());
        (widget, context)
    };
    let ir = internal::lower_widget_to_ir(&widget);
    runtime.clear_reducers();
    runtime.absorb_registry(context.registry);
    ir
}

#[test]
fn repeated_global_bindings_invoke_once_per_field_edit() {
    let mut runtime = Runtime::default().with_global_state(Draft::default());
    let env = Env::default();
    let edits = [
        ("first.host", "greenmail"),
        ("first.port", "3025"),
        ("second.host", "mailpit"),
        ("second.port", "1025"),
    ];

    for (index, (field, value)) in edits.into_iter().enumerate() {
        let ir = rebuild_global_fields(&mut runtime, &env);
        dispatch_named_edit(&mut runtime, &ir, field, value);
        let state = runtime.get_global_state::<Draft>().unwrap();
        assert_eq!(state.invocations, index + 1);
        assert_eq!(state.values.get(field).unwrap(), value);
    }
}

struct FocusFields;

impl From<FocusFields> for Widget {
    fn from(_: FocusFields) -> Widget {
        let (context, _) = build::current::<Draft>();
        Column {
            children: ["focus.first", "focus.second"]
                .into_iter()
                .map(|field| {
                    let action = || {
                        context.bind(
                            UpdateField(field.into()),
                            fission_core::reduce!(update_field),
                        )
                    };
                    TextInput {
                        id: Some(WidgetId::explicit(field)),
                        semantics_identifier: Some(field.into()),
                        value: field.into(),
                        select_all_on_focus: field == "focus.first",
                        on_focus: Some(action()),
                        on_blur: Some(action()),
                        on_tap_outside: Some(action()),
                        ..Default::default()
                    }
                    .into()
                })
                .collect(),
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn focus_transitions_preserve_context_and_report_typed_session_input() {
    let mut runtime = Runtime::default().with_global_state(Draft::default());
    let env = Env::default();
    let (widget, context) = {
        let state = runtime.get_global_state::<Draft>().unwrap();
        let view = View::new(state, &runtime.runtime_state, &env, None);
        let mut context = BuildCtx::<Draft>::new();
        let widget = build::enter(&mut context, &view, || FocusFields.into());
        (widget, context)
    };
    let ir = internal::lower_widget_to_ir(&widget);
    runtime.absorb_registry(context.registry);
    let first = text_semantics(&ir, "focus.first").0;
    let second = text_semantics(&ir, "focus.second").0;

    assert!(runtime
        .set_focused_widget(&ir, Some(first), fission_core::TextEditSource::Programmatic)
        .unwrap());
    assert!(runtime.post_layout_hook(&ir, &LayoutSnapshot::default()));
    let selected = runtime.runtime_state.text_edit.get(first).unwrap();
    assert_eq!((selected.anchor, selected.caret), (0, "focus.first".len()));

    assert!(runtime
        .set_focused_widget(&ir, Some(second), fission_core::TextEditSource::Keyboard)
        .unwrap());
    let state = runtime.get_global_state::<Draft>().unwrap();
    assert_eq!(
        state.lifecycle,
        vec![
            (
                "focus.first".into(),
                fission_core::TextEditPhase::Focused,
                fission_core::TextEditSource::Programmatic,
            ),
            (
                "focus.first".into(),
                fission_core::TextEditPhase::Blurred,
                fission_core::TextEditSource::Keyboard,
            ),
            (
                "focus.second".into(),
                fission_core::TextEditPhase::Focused,
                fission_core::TextEditSource::Keyboard,
            ),
        ]
    );
}

#[test]
fn pointer_focus_transition_dispatches_tap_outside_blur_and_focus_once() {
    let mut runtime = Runtime::default().with_global_state(Draft::default());
    let env = Env::default();
    let (widget, context) = {
        let state = runtime.get_global_state::<Draft>().unwrap();
        let view = View::new(state, &runtime.runtime_state, &env, None);
        let mut context = BuildCtx::<Draft>::new();
        let widget = build::enter(&mut context, &view, || FocusFields.into());
        (widget, context)
    };
    let ir = internal::lower_widget_to_ir(&widget);
    runtime.absorb_registry(context.registry);
    let first = text_semantics(&ir, "focus.first").0;
    let second = text_semantics(&ir, "focus.second").0;

    runtime
        .set_focused_widget(&ir, Some(first), fission_core::TextEditSource::Programmatic)
        .unwrap();
    runtime
        .get_global_state_mut::<Draft>()
        .unwrap()
        .lifecycle
        .clear();
    runtime
        .set_focused_widget(&ir, Some(second), fission_core::TextEditSource::Pointer)
        .unwrap();

    assert_eq!(
        runtime.get_global_state::<Draft>().unwrap().lifecycle,
        vec![
            (
                "focus.first".into(),
                fission_core::TextEditPhase::TapOutside,
                fission_core::TextEditSource::Pointer,
            ),
            (
                "focus.first".into(),
                fission_core::TextEditPhase::Blurred,
                fission_core::TextEditSource::Pointer,
            ),
            (
                "focus.second".into(),
                fission_core::TextEditPhase::Focused,
                fission_core::TextEditSource::Pointer,
            ),
        ]
    );
}

struct LocalEditor {
    scope: &'static str,
}

impl From<LocalEditor> for Widget {
    fn from(editor: LocalEditor) -> Widget {
        let (context, _) = build::current::<()>();
        let draft = StateField::new("LocalEditor", "draft", Draft::default());
        let value = draft.get();
        let mut children: Vec<Widget> = ["host", "port"]
            .into_iter()
            .map(|field| {
                TextInput {
                    id: Some(WidgetId::explicit(&format!("{}.{}", editor.scope, field))),
                    semantics_identifier: Some(format!("{}.{}", editor.scope, field)),
                    value: value.values.get(field).cloned().unwrap_or_default(),
                    on_input: Some(context.bind_local(
                        UpdateField(field.into()),
                        draft.clone(),
                        fission_core::reduce!(update_field),
                    )),
                    ..Default::default()
                }
                .into()
            })
            .collect();
        children.push(
            TextInput {
                semantics_identifier: Some(format!("{}.invocations", editor.scope)),
                value: value.invocations.to_string(),
                enabled: false,
                read_only: true,
                ..Default::default()
            }
            .into(),
        );

        Column {
            children,
            ..Default::default()
        }
        .into()
    }
}

struct LocalEditors;

impl From<LocalEditors> for Widget {
    fn from(_: LocalEditors) -> Widget {
        Column {
            children: vec![
                LocalEditor { scope: "first" }.id(WidgetId::explicit("editor.first")),
                LocalEditor { scope: "second" }.id(WidgetId::explicit("editor.second")),
            ],
            ..Default::default()
        }
        .into()
    }
}

fn rebuild_local_editors(runtime: &mut Runtime, env: &Env) -> CoreIR {
    let (widget, context) = {
        let view = View::new(&(), &runtime.runtime_state, env, None);
        let mut context = BuildCtx::<()>::new();
        let widget = build::enter(&mut context, &view, || LocalEditors.into());
        (widget, context)
    };
    let ir = internal::lower_widget_to_ir(&widget);
    runtime.clear_reducers();
    runtime.absorb_registry(context.registry);
    ir
}

fn dispatch_named_edit(runtime: &mut Runtime, ir: &CoreIR, identifier: &str, text: &str) {
    let (node_id, semantics) = text_semantics(ir, identifier);
    let (action, input) =
        prepare_text_input_change(semantics, node_id, text.into(), text.len(), text.len()).unwrap();
    runtime
        .dispatch_with_input(action, node_id, &input)
        .unwrap();
}

fn rendered_value<'a>(ir: &'a CoreIR, identifier: &str) -> &'a str {
    text_semantics(ir, identifier).1.value.as_deref().unwrap()
}

#[test]
fn repeated_local_fields_rebuild_once_per_edit_without_duplicate_reducers() {
    let mut runtime = Runtime::default();
    let env = Env::default();
    let mut ir = rebuild_local_editors(&mut runtime, &env);
    let edits = [
        ("first.host", "greenmail", "first.invocations", "1"),
        ("first.port", "3025", "first.invocations", "2"),
        ("second.host", "mailpit", "second.invocations", "1"),
        ("second.port", "1025", "second.invocations", "2"),
    ];

    for (field, value, counter, expected_count) in edits {
        dispatch_named_edit(&mut runtime, &ir, field, value);
        ir = rebuild_local_editors(&mut runtime, &env);
        assert_eq!(rendered_value(&ir, field), value);
        assert_eq!(rendered_value(&ir, counter), expected_count);
    }

    assert_eq!(rendered_value(&ir, "first.host"), "greenmail");
    assert_eq!(rendered_value(&ir, "first.port"), "3025");
    assert_eq!(rendered_value(&ir, "second.host"), "mailpit");
    assert_eq!(rendered_value(&ir, "second.port"), "1025");
}

#[test]
fn local_action_deserialization_failure_is_sanitized_and_recoverable() {
    let mut runtime = Runtime::default();
    let env = Env::default();
    let ir = rebuild_local_editors(&mut runtime, &env);
    let (node_id, semantics) = text_semantics(&ir, "first.host");
    let (valid, input) =
        prepare_text_input_change(semantics, node_id, "greenmail".into(), 9, 9).unwrap();
    let malformed = ActionEnvelope {
        id: valid.id,
        payload: br#"{"credential":"credential-value"}"#.to_vec(),
    };

    let error = runtime
        .dispatch_with_input(malformed, node_id, &input)
        .expect_err("malformed local action payload must fail");
    assert_eq!(error.to_string(), "failed to deserialize action payload");
    assert!(!format!("{error:#?}").contains("credential-value"));

    runtime
        .dispatch_with_input(valid, node_id, &input)
        .expect("failed deserialization must not remove a local reducer");
    let rebuilt = rebuild_local_editors(&mut runtime, &env);
    assert_eq!(rendered_value(&rebuilt, "first.host"), "greenmail");
    assert_eq!(rendered_value(&rebuilt, "first.invocations"), "1");
}

#[derive(Debug, Clone, Default)]
struct MulticastState {
    first: usize,
    second: usize,
}

impl GlobalState for MulticastState {}

fn first_handler(state: &mut MulticastState, _: EditText) {
    state.first += 1;
}

fn second_handler(state: &mut MulticastState, _: EditText) {
    state.second += 1;
}

#[test]
fn typed_action_registry_handlers_remain_multicast() {
    let mut registry = fission_core::ActionRegistry::<MulticastState>::new();
    registry.register(first_handler as fn(&mut MulticastState, EditText));
    registry.register(second_handler as fn(&mut MulticastState, EditText));
    let mut state = MulticastState::default();

    registry
        .dispatch(
            &mut state,
            &envelope(EditText),
            WidgetId::explicit("target"),
        )
        .unwrap();

    assert_eq!(state.first, 1);
    assert_eq!(state.second, 1);
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum SensitiveAction {
    Allowed,
}

impl Action for SensitiveAction {
    fn static_id() -> ActionId {
        ActionId::from_name("text_input_actions::SensitiveAction")
    }
}

#[derive(Debug, Clone, Default)]
struct SensitiveState {
    invocations: usize,
}

impl GlobalState for SensitiveState {}

fn handle_sensitive(state: &mut SensitiveState, _: SensitiveAction) {
    state.invocations += 1;
}

#[test]
fn deserialization_failure_is_sanitized_and_does_not_remove_the_reducer() {
    let mut runtime = Runtime::default().with_global_state(SensitiveState::default());
    let mut registry = fission_core::ActionRegistry::<SensitiveState>::new();
    registry.register(handle_sensitive as fn(&mut SensitiveState, SensitiveAction));
    runtime.absorb_registry(registry);
    let node_id = WidgetId::explicit("malformed-action");
    let malformed = ActionEnvelope {
        id: SensitiveAction::static_id(),
        payload: br#""credential-value""#.to_vec(),
    };
    let input = ActionInput::TextChanged(UpdateTextInput {
        node_id,
        new_text: "new value".into(),
        new_caret: 9,
        new_anchor: 9,
        ..Default::default()
    });

    let error = runtime
        .dispatch_with_input(malformed, node_id, &input)
        .expect_err("malformed action payload must fail");
    assert_eq!(error.to_string(), "failed to deserialize action payload");
    let rendered = format!("{error:#?}");
    assert!(!rendered.contains("credential-value"));

    runtime
        .dispatch_with_input(envelope(SensitiveAction::Allowed), node_id, &input)
        .expect("a failed deserialization must not remove the reducer");
    assert_eq!(
        runtime
            .get_app_state::<SensitiveState>()
            .unwrap()
            .invocations,
        1
    );
}
