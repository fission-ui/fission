//! A widget's implicit identity does not shift when a widget of another kind, or a button with
//! another action, is built before it.

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

#[derive(Debug, Clone)]
struct ListState {
    rows: usize,
    filtered: bool,
}

impl Default for ListState {
    fn default() -> Self {
        Self {
            rows: 4,
            filtered: false,
        }
    }
}

impl GlobalState for ListState {}

#[fission_macros::fission_action]
struct OpenRow(usize);

fn open_row(_state: &mut ListState, _action: OpenRow, _ctx: &mut ReducerContext<ListState>) {}

#[fission_macros::fission_action]
struct ToggleFilter;

fn toggle_filter(
    state: &mut ListState,
    _action: ToggleFilter,
    _ctx: &mut ReducerContext<ListState>,
) {
    state.filtered = !state.filtered;
    state.rows = if state.filtered { 1 } else { 4 };
}

#[derive(Clone)]
struct FilteredList;

impl From<FilteredList> for Widget {
    fn from(_: FilteredList) -> Self {
        let (ctx, view) = fission_core::build::current::<ListState>();
        let state = view.state();
        // The rows are built first, as a list built before a sidebar is, and their count changes
        // when the filter is toggled.
        let mut children: Vec<Widget> = (0..state.rows)
            .map(|row| {
                Button {
                    child: Some(Text::new(format!("Row {row}")).into()),
                    on_press: Some(ctx.bind(
                        OpenRow(row),
                        open_row as fn(&mut ListState, OpenRow, &mut ReducerContext<ListState>),
                    )),
                    ..Default::default()
                }
                .into()
            })
            .collect();
        children.push(
            Button {
                child: Some(Text::new("Filter").into()),
                on_press: Some(ctx.bind(
                    ToggleFilter,
                    toggle_filter
                        as fn(&mut ListState, ToggleFilter, &mut ReducerContext<ListState>),
                )),
                ..Default::default()
            }
            .semantics_identifier("identity.filter")
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
fn a_button_keeps_its_identity_and_focus_when_a_list_built_before_it_shrinks() {
    let mut driver =
        TestDriver::new(TestHarness::new(ListState::default()).with_root_widget(FilteredList));
    driver.pump().expect("first frame");
    let filter = driver
        .find_semantics_identifier("identity.filter")
        .expect("filter button");
    driver
        .tap_point(
            filter.bounds.x() + filter.bounds.width() / 2.0,
            filter.bounds.y() + filter.bounds.height() / 2.0,
        )
        .expect("press the filter");
    driver.pump().expect("frame with one row");
    assert!(
        driver
            .harness
            .runtime
            .get_app_state::<ListState>()
            .expect("state")
            .filtered,
        "the filter is on"
    );

    let after = driver
        .find_semantics_identifier("identity.filter")
        .expect("filter button")
        .node_id;
    assert_eq!(
        filter.node_id, after,
        "the filter keeps its identity when the rows before it shrink"
    );
    assert_eq!(
        driver.harness.runtime.runtime_state.interaction.focused,
        Some(after),
        "focus stays on the pressed filter instead of moving to whatever took its old identity"
    );
}
