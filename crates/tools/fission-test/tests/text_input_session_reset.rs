use fission_core::event::{InputEvent, KeyEvent};
use fission_core::ui::{Button, Column, Text, TextInput, Widget};
use fission_core::{GlobalState, ReducerContext};
use fission_ir::semantics::{KeyAction, Role};
use fission_ir::{KeyCode, WidgetId};
use fission_test::TestHarness;

const CELL_ID: &str = "sheet-cell-a1";

#[derive(Clone, Debug, Default)]
struct State {
    editing: bool,
    formula_draft: String,
}

impl GlobalState for State {}

#[fission_macros::fission_action]
struct StartEditing;

#[fission_macros::fission_action]
struct CancelEditing;

#[fission_macros::fission_action]
struct UpdateFormula;

fn start_editing(state: &mut State, _action: StartEditing, _ctx: &mut ReducerContext<State>) {
    state.editing = true;
}

fn cancel_editing(state: &mut State, _action: CancelEditing, _ctx: &mut ReducerContext<State>) {
    state.formula_draft.clear();
    state.editing = false;
}

fn update_formula(state: &mut State, _action: UpdateFormula, ctx: &mut ReducerContext<State>) {
    if let Some(change) = ctx.input.text_change() {
        state.formula_draft = change.new_text.clone();
    }
}

#[derive(Clone)]
struct SheetCell;

impl From<SheetCell> for Widget {
    fn from(_: SheetCell) -> Self {
        let (ctx, view) = fission_core::build::current::<State>();
        let state = view.state();
        let cell_id = WidgetId::explicit(CELL_ID);

        let editor_or_button: Widget = if state.editing {
            let cancel = ctx.bind(CancelEditing, fission_core::reduce!(cancel_editing));
            TextInput {
                id: Some(cell_id),
                semantics_identifier: Some(CELL_ID.into()),
                value: state.formula_draft.clone(),
                semantics: Some(fission_ir::Semantics {
                    key_actions: vec![KeyAction::new(KeyCode::Escape, cancel.id.as_u128())
                        .payload(cancel.payload)],
                    ..Default::default()
                }),
                on_input: Some(ctx.bind(UpdateFormula, fission_core::reduce!(update_formula))),
                width: Some(280.0),
                ..Default::default()
            }
            .into()
        } else {
            Button {
                id: Some(cell_id),
                child: Some(Text::new("Edit cell").into()),
                on_press: Some(ctx.bind(StartEditing, fission_core::reduce!(start_editing))),
                width: Some(280.0),
                ..Default::default()
            }
            .into()
        };

        Column {
            children: vec![editor_or_button],
            ..Default::default()
        }
        .into()
    }
}

fn key_down(harness: &mut TestHarness<State>, key_code: KeyCode) {
    harness
        .send_event(InputEvent::Keyboard(KeyEvent::Down {
            key_code,
            modifiers: 0,
        }))
        .expect("send key down");
    harness.pump().expect("pump key down");
}

fn type_char(harness: &mut TestHarness<State>, ch: char) {
    harness
        .send_event(InputEvent::Keyboard(KeyEvent::DownWithText {
            key_code: KeyCode::Char(ch),
            modifiers: 0,
            text: ch.to_string(),
        }))
        .expect("type into the focused cell editor");
    harness.pump().expect("pump typed cell value");
}

#[test]
fn reusing_a_button_id_for_a_controlled_text_input_starts_a_fresh_edit_session() {
    let mut harness = TestHarness::new(State::default()).with_root_widget(SheetCell);
    let cell_id = WidgetId::explicit(CELL_ID);
    harness.pump().expect("initial frame");
    harness
        .runtime
        .runtime_state
        .interaction
        .set_focused(Some(cell_id));
    harness.pump().expect("focus A1");

    key_down(&mut harness, KeyCode::Enter);
    assert_eq!(
        semantics_role(&harness, cell_id),
        Role::TextInput,
        "Enter starts inline editing"
    );
    type_char(&mut harness, '9');
    assert_eq!(app_state(&harness).formula_draft, "9");

    key_down(&mut harness, KeyCode::Escape);
    assert_eq!(
        semantics_role(&harness, cell_id),
        Role::Button,
        "Escape leaves the editor mounted as a button with the same widget ID"
    );
    assert_eq!(app_state(&harness).formula_draft, "");
    assert_eq!(
        harness.runtime.runtime_state.interaction.focused,
        Some(cell_id)
    );

    key_down(&mut harness, KeyCode::Enter);
    assert_eq!(semantics_role(&harness, cell_id), Role::TextInput);
    type_char(&mut harness, '1');
    type_char(&mut harness, '5');

    assert_eq!(
        app_state(&harness).formula_draft,
        "15",
        "a new edit on the same widget ID must not inherit the cancelled buffer"
    );
}

fn app_state(harness: &TestHarness<State>) -> &State {
    harness
        .runtime
        .get_app_state::<State>()
        .expect("sheet app state")
}

fn semantics_role(harness: &TestHarness<State>, id: WidgetId) -> Role {
    match &harness.last_ir.as_ref().expect("pumped IR").nodes[&id].op {
        fission_ir::Op::Semantics(semantics) => semantics.role,
        op => panic!("expected semantics at {id:?}, found {op:?}"),
    }
}
