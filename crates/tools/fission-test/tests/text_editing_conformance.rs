use std::sync::{Arc, Mutex};

use anyhow::Result;
use fission_core::env::Clipboard;
use fission_core::event::{EditingCommand, InputEvent};
use fission_core::ui::{Column, TextInput, Widget};
use fission_core::{
    GlobalState, ReducerContext, TextAffinity, TextEditBoundary, TextEditCommand,
    TextEditDirection, TextEditPhase, TextEditSource, TextEditingValue, TextPosition, TextRange,
    TextSelection, UpdateTextInput, WidgetId,
};
use fission_ir::{Op, WidgetId as IrWidgetId};
use fission_test::{TestDriver, TestHarness};

const FIRST: &str = "text-conformance.first";
const SECOND: &str = "text-conformance.second";
const MULTILINE: &str = "text-conformance.multiline";
const PASSWORD: &str = "text-conformance.password";

#[derive(Clone, Debug, Default)]
struct State {
    first: String,
    second: String,
    multiline: String,
    password: String,
    edits: Vec<ObservedEdit>,
    submissions: Vec<ObservedEdit>,
    completions: Vec<ObservedEdit>,
}

impl GlobalState for State {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ObservedEdit {
    field: String,
    node_id: WidgetId,
    old_value: TextEditingValue,
    new_value: TextEditingValue,
    source: TextEditSource,
    phase: TextEditPhase,
    editing_action: Option<fission_ir::TextInputAction>,
}

impl ObservedEdit {
    fn from_change(field: String, change: &UpdateTextInput) -> Self {
        Self {
            field,
            node_id: change.node_id,
            old_value: change.old_value.clone(),
            new_value: change.new_value.clone(),
            source: change.source,
            phase: change.phase,
            editing_action: change.editing_action,
        }
    }
}

#[fission_macros::fission_action]
struct EditField(String);

#[fission_macros::fission_action]
struct SubmitField(String);

#[fission_macros::fission_action]
struct CompleteField(String);

fn edit_field(state: &mut State, action: EditField, context: &mut ReducerContext<State>) {
    let Some(change) = context.input.text_change() else {
        return;
    };
    match action.0.as_str() {
        FIRST => state.first = change.new_text.clone(),
        SECOND => state.second = change.new_text.clone(),
        MULTILINE => state.multiline = change.new_text.clone(),
        PASSWORD => state.password = change.new_text.clone(),
        unknown => panic!("unexpected field context {unknown}"),
    }
    state
        .edits
        .push(ObservedEdit::from_change(action.0, change));
}

fn submit_field(state: &mut State, action: SubmitField, context: &mut ReducerContext<State>) {
    if let Some(change) = context.input.text_change() {
        state
            .submissions
            .push(ObservedEdit::from_change(action.0, change));
    }
}

fn complete_field(state: &mut State, action: CompleteField, context: &mut ReducerContext<State>) {
    if let Some(change) = context.input.text_change() {
        state
            .completions
            .push(ObservedEdit::from_change(action.0, change));
    }
}

#[derive(Clone)]
struct TextConformanceApp;

impl From<TextConformanceApp> for Widget {
    fn from(_: TextConformanceApp) -> Self {
        let (context, view) = fission_core::build::current::<State>();
        let state = view.state();
        Column {
            children: vec![
                TextInput {
                    id: Some(WidgetId::explicit(FIRST)),
                    semantics_identifier: Some(FIRST.into()),
                    value: state.first.clone(),
                    on_input: Some(
                        context.bind(EditField(FIRST.into()), fission_core::reduce!(edit_field)),
                    ),
                    on_submit: Some(context.bind(
                        SubmitField(FIRST.into()),
                        fission_core::reduce!(submit_field),
                    )),
                    on_editing_complete: Some(context.bind(
                        CompleteField(FIRST.into()),
                        fission_core::reduce!(complete_field),
                    )),
                    width: Some(320.0),
                    text_input_action: fission_ir::TextInputAction::Search,
                    ..Default::default()
                }
                .into(),
                TextInput {
                    id: Some(WidgetId::explicit(SECOND)),
                    semantics_identifier: Some(SECOND.into()),
                    value: state.second.clone(),
                    on_input: Some(
                        context.bind(EditField(SECOND.into()), fission_core::reduce!(edit_field)),
                    ),
                    width: Some(320.0),
                    ..Default::default()
                }
                .into(),
                TextInput {
                    id: Some(WidgetId::explicit(MULTILINE)),
                    semantics_identifier: Some(MULTILINE.into()),
                    value: state.multiline.clone(),
                    on_input: Some(context.bind(
                        EditField(MULTILINE.into()),
                        fission_core::reduce!(edit_field),
                    )),
                    width: Some(320.0),
                    height: Some(88.0),
                    multiline: true,
                    max_lines: Some(4),
                    ..Default::default()
                }
                .into(),
                TextInput {
                    id: Some(WidgetId::explicit(PASSWORD)),
                    semantics_identifier: Some(PASSWORD.into()),
                    value: state.password.clone(),
                    on_input: Some(context.bind(
                        EditField(PASSWORD.into()),
                        fission_core::reduce!(edit_field),
                    )),
                    obscure_text: true,
                    width: Some(320.0),
                    ..Default::default()
                }
                .into(),
            ],
            gap: Some(12.0),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Default)]
struct TestClipboard {
    text: Mutex<String>,
}

impl TestClipboard {
    fn value(&self) -> String {
        self.text.lock().unwrap().clone()
    }
}

impl Clipboard for TestClipboard {
    fn get_text(&self) -> Option<String> {
        Some(self.value())
    }

    fn set_text(&self, text: &str) {
        *self.text.lock().unwrap() = text.to_string();
    }
}

fn driver() -> (TestDriver<State>, Arc<TestClipboard>) {
    let clipboard = Arc::new(TestClipboard::default());
    let mut harness =
        TestHarness::new_with_mock_measurer(State::default()).with_root_widget(TextConformanceApp);
    harness.runtime.clipboard_backend = Some(clipboard.clone());
    (TestDriver::new(harness), clipboard)
}

fn focus(driver: &mut TestDriver<State>, id: &'static str) {
    driver
        .harness
        .runtime
        .runtime_state
        .interaction
        .set_focused(Some(WidgetId::explicit(id)));
}

fn editing_value(driver: &TestDriver<State>, id: &'static str) -> TextEditingValue {
    driver
        .harness
        .runtime
        .runtime_state
        .text_edit
        .get(WidgetId::explicit(id))
        .unwrap_or_else(|| panic!("retained editing value for {id}"))
        .editing_value()
}

fn app_state(driver: &TestDriver<State>) -> &State {
    driver
        .harness
        .runtime
        .get_app_state::<State>()
        .expect("text conformance app state")
}

fn selection(text: &str, base: usize, extent: usize) -> TextSelection {
    TextSelection::new(text, base, extent, TextAffinity::Downstream).unwrap()
}

#[test]
fn complete_edit_sequence_is_one_authoritative_typed_contract() -> Result<()> {
    let (mut driver, clipboard) = driver();
    driver.pump()?;
    focus(&mut driver, FIRST);

    driver.type_text("abé😀")?;
    assert_eq!(app_state(&driver).first, "abé😀");

    let text = "abé😀";
    driver.set_text_selection(selection(text, 1, "abé".len()))?;
    assert_eq!(
        editing_value(&driver, FIRST).selection,
        selection(text, 1, "abé".len())
    );
    driver.replace_text_range(
        TextRange::new(text, 1, "abé".len()).unwrap(),
        "界",
        TextEditSource::Keyboard,
    )?;
    assert_eq!(app_state(&driver).first, "a界😀");

    driver
        .harness
        .send_event(InputEvent::TextEdit(TextEditCommand::MoveSelection {
            direction: TextEditDirection::Backward,
            boundary: TextEditBoundary::Grapheme,
            extend: true,
            source: TextEditSource::Keyboard,
        }))?;
    driver.pump()?;
    let extended = editing_value(&driver, FIRST).selection;
    assert!(!extended.is_collapsed());
    assert_eq!(extended.base.utf8_offset(), "a界".len());
    assert_eq!(extended.extent.utf8_offset(), 1);

    driver
        .harness
        .send_event(InputEvent::TextEdit(TextEditCommand::MoveSelection {
            direction: TextEditDirection::Forward,
            boundary: TextEditBoundary::Grapheme,
            extend: false,
            source: TextEditSource::Keyboard,
        }))?;
    driver.pump()?;
    assert_eq!(
        editing_value(&driver, FIRST).selection,
        TextSelection::collapsed(TextPosition::from_utf8("a界😀", "a界".len()).unwrap())
    );
    driver.set_text_selection(selection("a界😀", "a界".len(), 1))?;

    driver
        .harness
        .send_event(InputEvent::Editing(EditingCommand::Copy))?;
    assert_eq!(clipboard.value(), "界");
    driver
        .harness
        .send_event(InputEvent::Editing(EditingCommand::Cut))?;
    driver.pump()?;
    assert_eq!(app_state(&driver).first, "a😀");
    driver
        .harness
        .send_event(InputEvent::Editing(EditingCommand::Paste("界".into())))?;
    driver.pump()?;
    assert_eq!(app_state(&driver).first, "a界😀");

    driver
        .harness
        .send_event(InputEvent::Editing(EditingCommand::Undo))?;
    driver.pump()?;
    assert_eq!(app_state(&driver).first, "a😀");
    driver
        .harness
        .send_event(InputEvent::Editing(EditingCommand::Redo))?;
    driver.pump()?;
    assert_eq!(app_state(&driver).first, "a界😀");

    let state = app_state(&driver);
    assert!(state.edits.iter().all(|edit| edit.field == FIRST));
    assert!(state
        .edits
        .iter()
        .all(|edit| edit.node_id == WidgetId::explicit(FIRST)));
    assert!(state.edits.iter().any(|edit| {
        edit.old_value.text == "abé😀"
            && edit.new_value.text == "a界😀"
            && edit.source == TextEditSource::Keyboard
    }));
    assert!(state
        .edits
        .iter()
        .any(|edit| edit.source == TextEditSource::Clipboard));

    Ok(())
}

#[test]
fn composition_update_and_cancel_restore_the_committed_value() -> Result<()> {
    let (mut driver, _) = driver();
    driver.pump()?;
    focus(&mut driver, FIRST);
    driver.type_text("cafe")?;
    let committed_edits = app_state(&driver).edits.len();

    let base = editing_value(&driver, FIRST);
    let started_range = TextRange::collapsed(TextPosition::at_end(&base.text));
    driver.start_or_update_composition(started_range)?;
    assert_eq!(editing_value(&driver, FIRST).composing, Some(started_range));
    let composing_text = "café";
    let updated_range = TextRange::new(composing_text, "cafe".len(), composing_text.len()).unwrap();
    driver.set_text_editing_value_with_phase(
        TextEditingValue::new(
            composing_text,
            TextSelection::collapsed(TextPosition::at_end(composing_text)),
            Some(updated_range),
        )?,
        TextEditSource::Ime,
        fission_core::TextValuePhase::CompositionUpdated,
    )?;
    assert_eq!(editing_value(&driver, FIRST).text, composing_text);
    assert_eq!(editing_value(&driver, FIRST).composing, Some(updated_range));
    assert_eq!(
        app_state(&driver).edits.len(),
        committed_edits,
        "composition updates must not dispatch partially committed values"
    );
    driver.cancel_composition()?;
    assert_eq!(editing_value(&driver, FIRST).text, "cafe");
    assert_eq!(app_state(&driver).first, "cafe");

    Ok(())
}

#[test]
fn composition_commit_and_complete_value_replacement_are_atomic() -> Result<()> {
    let (mut driver, _) = driver();
    driver.pump()?;
    focus(&mut driver, FIRST);
    driver.type_text("cafe")?;

    driver.start_or_update_composition(TextRange::new("cafe", 3, 4).unwrap())?;
    driver.commit_composition("é", TextEditSource::Ime)?;
    assert_eq!(app_state(&driver).first, "café");
    let state = app_state(&driver);
    let composition = state.edits.last().expect("composition commit action");
    assert_eq!(composition.source, TextEditSource::Ime);
    assert_eq!(composition.phase, TextEditPhase::CompositionCommitted);
    assert_eq!(composition.old_value.text, "cafe");
    assert_eq!(composition.new_value.text, "café");

    let corrected = "Café";
    driver.set_text_editing_value(
        TextEditingValue::new(
            corrected,
            TextSelection::collapsed(TextPosition::at_end(corrected)),
            None,
        )?,
        TextEditSource::Autocorrect,
    )?;
    let filled = "Café, London";
    driver.set_text_editing_value(
        TextEditingValue::new(filled, selection(filled, filled.len(), "Café".len()), None)?,
        TextEditSource::Autofill,
    )?;
    let state = app_state(&driver);
    assert_eq!(state.first, filled);
    assert_eq!(
        state.edits[state.edits.len() - 2].source,
        TextEditSource::Autocorrect
    );
    assert_eq!(state.edits.last().unwrap().source, TextEditSource::Autofill);
    assert_eq!(
        state.edits.last().unwrap().new_value.selection,
        selection(filled, filled.len(), "Café".len())
    );

    driver
        .harness
        .send_event(InputEvent::TextEdit(TextEditCommand::Submit))?;
    driver
        .harness
        .send_event(InputEvent::TextEdit(TextEditCommand::Complete))?;
    driver.pump()?;
    let state = app_state(&driver);
    assert_eq!(state.submissions.len(), 1);
    assert_eq!(state.submissions[0].field, FIRST);
    assert_eq!(state.submissions[0].phase, TextEditPhase::Submitted);
    assert_eq!(
        state.submissions[0].editing_action,
        Some(fission_ir::TextInputAction::Search)
    );
    assert_eq!(state.submissions[0].new_value.text, filled);
    assert_eq!(state.completions.len(), 1);
    assert_eq!(state.completions[0].phase, TextEditPhase::EditingCompleted);
    assert_eq!(
        state.completions[0].editing_action,
        Some(fission_ir::TextInputAction::Search)
    );

    Ok(())
}

#[test]
fn multiline_complete_value_scrolls_the_caret_into_view() -> Result<()> {
    // This assertion exercises caret geometry, so use Fission's production
    // paragraph measurer rather than the intentionally approximate mock.
    let mut driver =
        TestDriver::new(TestHarness::new(State::default()).with_root_widget(TextConformanceApp));
    driver.set_viewport(420.0, 280.0);
    driver.pump()?;
    focus(&mut driver, MULTILINE);
    let document = (0..30)
        .map(|line| format!("line {line}: mañana 東京 😀"))
        .collect::<Vec<_>>()
        .join("\n");
    driver.set_text_editing_value(
        TextEditingValue::from_text(document.clone()),
        TextEditSource::Keyboard,
    )?;
    assert_eq!(app_state(&driver).multiline, document);
    assert_eq!(
        editing_value(&driver, MULTILINE).selection.extent,
        TextPosition::at_end(&document)
    );
    assert!(
        driver
            .harness
            .runtime
            .runtime_state
            .scroll
            .offsets
            .values()
            .any(|offset| *offset > 0.0),
        "moving the caret to the end of a constrained multiline editor must scroll it into view"
    );

    Ok(())
}

#[test]
fn blur_and_refocus_keep_each_field_isolated() -> Result<()> {
    let (mut driver, _) = driver();
    driver.pump()?;

    focus(&mut driver, FIRST);
    driver.type_text("first")?;
    focus(&mut driver, SECOND);
    driver.type_text("second")?;
    driver
        .harness
        .runtime
        .runtime_state
        .interaction
        .set_focused(None);
    driver.pump()?;
    focus(&mut driver, FIRST);
    driver.type_text("!")?;
    focus(&mut driver, SECOND);
    driver.type_text("?")?;

    let state = app_state(&driver);
    assert_eq!(state.first, "first!");
    assert_eq!(state.second, "second?");
    assert!(state
        .edits
        .iter()
        .filter(|edit| edit.field == FIRST)
        .all(|edit| edit.node_id == WidgetId::explicit(FIRST)));
    assert!(state
        .edits
        .iter()
        .filter(|edit| edit.field == SECOND)
        .all(|edit| edit.node_id == WidgetId::explicit(SECOND)));

    Ok(())
}

#[test]
fn obscured_input_never_paints_the_secret_or_exposes_it_as_visible_text() -> Result<()> {
    let secret = "correct horse battery staple";
    let (mut driver, _) = driver();
    driver.pump()?;
    focus(&mut driver, PASSWORD);
    driver.set_text_editing_value(
        TextEditingValue::from_text(secret),
        TextEditSource::Keyboard,
    )?;

    let ir = driver
        .harness
        .last_ir
        .as_ref()
        .expect("IR after password edit");
    let semantics = ir
        .nodes
        .get(&IrWidgetId::explicit(PASSWORD))
        .and_then(|node| match &node.op {
            Op::Semantics(semantics) => Some(semantics),
            _ => None,
        })
        .expect("password TextInput semantics");
    assert!(semantics.masked);
    assert_eq!(semantics.value.as_deref(), Some(secret));
    assert!(driver
        .get_all_visible_text()
        .iter()
        .all(|painted| !painted.contains(secret)));
    let display = driver
        .harness
        .get_last_display_list()
        .expect("display list");
    assert!(display
        .ops
        .iter()
        .all(|operation| !format!("{operation:?}").contains(secret)));

    let state = app_state(&driver);
    assert_eq!(state.password, secret);
    assert_eq!(state.edits.last().unwrap().field, PASSWORD);

    Ok(())
}
