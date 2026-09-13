//! A data table's header cells line up with the body cells beneath them.

use fission_core::ui::{Container, Widget};
use fission_core::{GlobalState, ReducerContext, WidgetId};
use fission_ir::op::Op;
use fission_test::{TestDriver, TestHarness};
use fission_widgets::{DataTable, TableColumn, TableRow};
use std::sync::Arc;

#[derive(Debug, Default, Clone)]
struct State;

impl GlobalState for State {}

#[fission_macros::fission_action]
struct Noop;

fn noop(_state: &mut State, _action: Noop, _ctx: &mut ReducerContext<State>) {}

#[derive(Clone)]
struct Page;

impl From<Page> for Widget {
    fn from(_: Page) -> Self {
        let (ctx, _) = fission_core::build::current::<State>();
        let action = ctx.bind(
            Noop,
            noop as fn(&mut State, Noop, &mut ReducerContext<State>),
        );
        let row_action = action.clone();
        Container::new(DataTable {
            id: WidgetId::explicit("table"),
            columns: vec![
                TableColumn {
                    id: "name".into(),
                    title: "Name".into(),
                    width: 160.0,
                    sortable: true,
                    on_sort: Some(action.clone()),
                    sorted_ascending: Some(true),
                },
                TableColumn {
                    id: "email".into(),
                    title: "Email".into(),
                    width: 220.0,
                    sortable: false,
                    on_sort: None,
                    sorted_ascending: None,
                },
            ],
            rows: vec![
                TableRow {
                    id: "1".into(),
                    cells: vec!["Alice".into(), "alice@example.com".into()],
                },
                TableRow {
                    id: "2".into(),
                    cells: vec!["Bob".into(), "bob@example.com".into()],
                },
            ],
            selected_ids: Vec::new(),
            on_selection_change: Some(Arc::new(move |_| row_action.clone())),
            on_select_all: Some(action),
            label: Some("Contacts".into()),
        })
        .width(520.0)
        .height(300.0)
        .into()
    }
}

#[test]
fn header_cells_line_up_with_body_cells() {
    let mut driver = TestDriver::new(TestHarness::new(State).with_root_widget(Page));
    driver.pump().expect("first frame");
    let ir = driver.harness.last_ir.as_ref().expect("ir");
    let snapshot = driver.harness.last_snapshot.as_ref().expect("snapshot");
    let text_x = |needle: &str| {
        ir.nodes
            .iter()
            .find(|(_, node)| node.op.text().as_deref() == Some(needle))
            .and_then(|(id, _)| snapshot.get_node_rect(*id))
            .unwrap_or_else(|| panic!("text {needle:?} is laid out"))
            .x()
    };
    let labelled_x = |label: &str| {
        ir.nodes
            .iter()
            .find(|(_, node)| {
                matches!(&node.op, Op::Semantics(semantics) if semantics.label.as_deref() == Some(label))
            })
            .and_then(|(id, _)| snapshot.get_node_rect(*id))
            .unwrap_or_else(|| panic!("{label:?} is laid out"))
            .x()
    };

    let (name, alice) = (text_x("Name"), text_x("Alice"));
    assert!(
        (name - alice).abs() < 1.0,
        "the Name header starts where its cells do: header {name}, cell {alice}"
    );
    let (email, alice_email) = (text_x("Email"), text_x("alice@example.com"));
    assert!(
        (email - alice_email).abs() < 1.0,
        "the Email header starts where its cells do: header {email}, cell {alice_email}"
    );
    let (select_all, select_row) = (labelled_x("Select all rows"), labelled_x("Select row 1"));
    assert!(
        (select_all - select_row).abs() < 1.0,
        "the select-all checkbox lines up with the row checkboxes: {select_all} vs {select_row}"
    );
}
