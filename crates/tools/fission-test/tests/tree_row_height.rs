//! A tree row grows to fit a label that wraps, and the next row starts below it.

use fission_core::ui::{Container, Widget};
use fission_core::GlobalState;
use fission_ir::Role;
use fission_test::{TestDriver, TestHarness};
use fission_widgets::{TreeItem, TreeView};
use std::collections::HashSet;

#[derive(Debug, Default, Clone)]
struct State;

impl GlobalState for State {}

#[derive(Clone)]
struct Page;

fn item(id: &str, label: &str) -> TreeItem {
    TreeItem {
        id: id.into(),
        icon: None,
        label: label.into(),
        children: Vec::new(),
        on_toggle: None,
        on_select: None,
    }
}

impl From<Page> for Widget {
    fn from(_: Page) -> Self {
        Container::new(TreeView {
            items: vec![
                item("inbox", "Bandeja de entrada principal"),
                item("starred", "Destacados"),
            ],
            expanded_ids: HashSet::new(),
            selected_id: None,
        })
        .width(130.0)
        .into()
    }
}

#[test]
fn a_wrapping_label_grows_its_row_and_pushes_the_next_one_down() {
    let mut driver = TestDriver::new(TestHarness::new(State).with_root_widget(Page));
    driver.pump().expect("first frame");
    let mut rows: Vec<_> = driver
        .find_role(Role::TreeItem)
        .into_iter()
        .map(|found| (found.label, found.bounds))
        .collect();
    rows.sort_by(|a, b| a.1.y().total_cmp(&b.1.y()));
    assert_eq!(rows.len(), 2, "two rows, got {rows:?}");
    let (first, second) = (rows[0].1, rows[1].1);
    assert!(
        first.height() > 40.0,
        "the wrapping label makes its row taller than the minimum, got {first:?}"
    );
    assert!(
        second.y() >= first.y() + first.height() - 0.5,
        "the next row starts below the grown row: {first:?} then {second:?}"
    );
}
