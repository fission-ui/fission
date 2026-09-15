//! A tree row's button draws with the row's rounded corners, so its focus ring is not square.

use fission_core::ui::{Container, Widget};
use fission_core::GlobalState;
use fission_ir::op::{Op, PaintOp};
use fission_ir::Role;
use fission_test::{TestDriver, TestHarness};
use fission_widgets::{TreeItem, TreeView};
use std::collections::HashSet;

#[derive(Debug, Default, Clone)]
struct State;

impl GlobalState for State {}

#[derive(Clone)]
struct Page;

impl From<Page> for Widget {
    fn from(_: Page) -> Self {
        Container::new(TreeView {
            selected_id: None,
            expanded_ids: HashSet::new(),
            items: vec![TreeItem {
                id: "inbox".into(),
                label: "Inbox".into(),
                icon: None,
                children: vec![],
                on_toggle: None,
                on_select: None,
            }],
        })
        .width(240.0)
        .into()
    }
}

#[test]
fn a_tree_row_button_uses_the_row_radius() {
    let mut driver = TestDriver::new(TestHarness::new(State).with_root_widget(Page));
    driver.pump().expect("first frame");
    let radius = driver.harness.env.theme.tokens.radii.medium;
    let ir = driver.harness.last_ir.as_ref().expect("ir");
    let row = ir
        .nodes
        .values()
        .find(|node| matches!(&node.op, Op::Semantics(s) if s.role == Role::TreeItem))
        .expect("tree row");

    let mut stack = row.children.clone();
    let mut radii = Vec::new();
    while let Some(id) = stack.pop() {
        let node = &ir.nodes[&id];
        if let Op::Paint(PaintOp::DrawRect { corner_radius, .. }) = &node.op {
            radii.push(*corner_radius);
        }
        if !matches!(&node.op, Op::Semantics(_)) {
            stack.extend(node.children.iter().copied());
        }
    }
    assert!(!radii.is_empty(), "the row paints its surface");
    assert!(
        radii.iter().all(|r| (*r - radius).abs() < 0.01),
        "every surface the row button paints is rounded like the row ({radius}): {radii:?}"
    );
}
