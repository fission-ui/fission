//! An open selection region menu opens above all content instead of inside the region.

use fission_core::ui::widgets::SelectionRegion;
use fission_core::ui::{Column, Container, Text, Widget};
use fission_core::{GlobalState, WidgetId};
use fission_layout::{LayoutPoint, LayoutSize};
use fission_test::{TestDriver, TestHarness};

#[derive(Clone, Debug, Default)]
struct State;

impl GlobalState for State {}

const REGION: &str = "menu.region";

#[derive(Clone)]
struct Page;

impl From<Page> for Widget {
    fn from(_: Page) -> Self {
        Column {
            children: vec![Container::new(SelectionRegion {
                id: Some(WidgetId::explicit(REGION)),
                ..SelectionRegion::new(Column {
                    children: vec![
                        Text::new("First selectable line").into(),
                        Text::new("Second selectable line").into(),
                    ],
                    ..Default::default()
                })
            })
            .height(24.0)
            .into()],
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn an_open_selection_region_menu_is_lifted_out_of_the_region() {
    let mut driver = TestDriver::new(TestHarness::new(State).with_root_widget(Page));
    driver.harness.env.viewport_size = LayoutSize::new(600.0, 400.0);
    driver.pump().expect("first frame");

    let region = WidgetId::explicit(REGION);
    driver
        .harness
        .runtime
        .runtime_state
        .context_menu
        .open(region, LayoutPoint::new(40.0, 12.0));
    driver.pump().expect("frame with the menu open");

    let ir = driver.harness.last_ir.as_ref().expect("ir");
    let rows: Vec<_> = ir
        .nodes
        .iter()
        .filter(|(_, node)| {
            let op = format!("{:?}", node.op);
            op.contains("\"Copy\"") || op.contains("\"Select All\"")
        })
        .map(|(id, _)| *id)
        .collect();
    assert!(!rows.is_empty(), "the region menu should be built");
    for row in rows {
        let mut current = Some(row);
        while let Some(id) = current {
            assert_ne!(
                id, region,
                "the menu should open above all content, not inside the region"
            );
            current = ir.nodes.get(&id).and_then(|node| node.parent);
        }
    }
}
