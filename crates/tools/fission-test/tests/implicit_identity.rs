//! A widget's implicit identity does not shift when a widget of another kind is built before it.

use fission_core::ui::{Button, Checkbox, Column, Text, Widget};
use fission_core::{GlobalState, ReducerContext};
use fission_test::{TestDriver, TestHarness};

#[derive(Debug, Default, Clone)]
struct State {
    extra: bool,
}

impl GlobalState for State {}

#[fission_macros::fission_action]
struct ShowExtra;

fn show_extra(state: &mut State, _action: ShowExtra, _ctx: &mut ReducerContext<State>) {
    state.extra = true;
}

#[derive(Clone)]
struct Page;

impl From<Page> for Widget {
    fn from(_: Page) -> Self {
        let (ctx, view) = fission_core::build::current::<State>();
        let reveal = ctx.bind(
            ShowExtra,
            show_extra as fn(&mut State, ShowExtra, &mut ReducerContext<State>),
        );
        let mut children: Vec<Widget> = Vec::new();
        // Built before the button only once revealed, like a dialog or banner that appears.
        if view.state().extra {
            children.push(Checkbox::default().into());
        }
        children.push(
            Button {
                child: Some(Text::new("Stable").into()),
                on_press: Some(reveal),
                ..Default::default()
            }
            .semantics_identifier("identity.stable")
            .into(),
        );
        Column {
            children,
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn a_button_keeps_its_identity_when_another_kind_of_widget_appears_before_it() {
    let mut driver = TestDriver::new(TestHarness::new(State::default()).with_root_widget(Page));
    driver.pump().expect("first frame");
    let before = driver
        .find_semantics_identifier("identity.stable")
        .expect("button")
        .node_id;

    driver
        .harness
        .dispatch(ShowExtra)
        .expect("reveal the checkbox");
    driver.pump().expect("frame with the checkbox");
    assert!(
        driver
            .harness
            .runtime
            .get_app_state::<State>()
            .expect("state")
            .extra,
        "the checkbox is shown"
    );

    let after = driver
        .find_semantics_identifier("identity.stable")
        .expect("button")
        .node_id;
    assert_eq!(
        before, after,
        "the button keeps its identity, and with it hover, focus and motion"
    );
}
