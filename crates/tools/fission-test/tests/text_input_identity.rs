//! A text field keeps its focus while content built before it, but placed after it, changes as the user types.

use fission_core::ui::{Button, Column, Text, TextInput, Widget};
use fission_core::{GlobalState, ReducerContext};
use fission_ir::semantics::Role;
use fission_test::{TestDriver, TestHarness};

#[derive(Debug, Default, Clone)]
struct State {
    text: String,
}

impl GlobalState for State {}

#[fission_macros::fission_action]
struct UpdateText;

fn update_text(state: &mut State, _action: UpdateText, ctx: &mut ReducerContext<State>) {
    if let Some(change) = ctx.input.text_change() {
        state.text = change.new_text.clone();
    }
}

#[derive(Clone)]
struct SearchPage;

impl From<SearchPage> for Widget {
    fn from(_: SearchPage) -> Self {
        let (ctx, view) = fission_core::build::current::<State>();
        // The results are built before the field, as a page component often is, but placed after
        // it. They change as the user types: a button only appears once there is a query.
        let results: Widget = if view.state().text.is_empty() {
            Text::new("No query").into()
        } else {
            Button {
                child: Some(Text::new("Clear").into()),
                ..Default::default()
            }
            .into()
        };
        let field: Widget = TextInput {
            value: view.state().text.clone(),
            on_input: Some(ctx.bind(
                UpdateText,
                update_text as fn(&mut State, UpdateText, &mut ReducerContext<State>),
            )),
            width: Some(300.0),
            ..Default::default()
        }
        .into();
        Column {
            children: vec![field, results],
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn a_field_without_an_id_keeps_focus_when_content_built_before_it_changes() {
    let mut driver =
        TestDriver::new(TestHarness::new(State::default()).with_root_widget(SearchPage));
    driver.pump().expect("first frame");

    let field = driver
        .find_role(Role::TextInput)
        .into_iter()
        .next()
        .expect("text field")
        .bounds;
    driver
        .tap_point(field.x() + 20.0, field.y() + field.height() / 2.0)
        .expect("focus the field");
    let focused = driver.harness.runtime.runtime_state.interaction.focused;
    assert!(focused.is_some(), "tapping the field focuses it");

    driver.type_text("ab").expect("type");
    driver.pump().expect("pump after typing");

    assert_eq!(
        driver.harness.runtime.runtime_state.interaction.focused, focused,
        "the field keeps its identity and focus while the results change"
    );
    assert!(
        driver.find_text("ab").is_some(),
        "both characters reach the field"
    );
}
