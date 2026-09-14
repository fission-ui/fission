//! A sized control in a stretching column reports its own bounds and stays where it is drawn.

use fission_core::ui::{Column, Container, Widget};
use fission_core::{GlobalState, WidgetId};
use fission_ir::Role;
use fission_test::{TestDriver, TestHarness};
use fission_widgets::{Select, SelectItem};

#[derive(Debug, Default, Clone)]
struct State;

impl GlobalState for State {}

#[derive(Clone)]
struct Page;

impl From<Page> for Widget {
    fn from(_: Page) -> Self {
        Container::new(Column {
            children: vec![Select {
                id: WidgetId::explicit("bounds.select"),
                selected_label: Some("Light".into()),
                items: vec![SelectItem {
                    label: "Light".into(),
                    icon: None,
                    on_select: fission_core::ActionEnvelope {
                        id: fission_core::ActionId::from_name("bounds.noop"),
                        payload: Vec::new(),
                    },
                    semantics_identifier: None,
                }],
                width: Some(200.0),
                ..Default::default()
            }
            .into()],
            ..Default::default()
        })
        .width(600.0)
        .into()
    }
}

#[test]
fn a_select_in_a_stretching_column_reports_its_drawn_width() {
    let mut driver = TestDriver::new(TestHarness::new(State).with_root_widget(Page));
    driver.pump().expect("first frame");
    let combo = driver
        .find_role(Role::ComboBox)
        .into_iter()
        .next()
        .expect("select trigger")
        .bounds;
    assert!(
        (combo.width() - 200.0).abs() < 1.0,
        "the trigger reports the 200 wide control it draws, got {combo:?}"
    );
    assert!(
        combo.x().abs() < 1.0,
        "the trigger stays at the start of the column rather than moving to its centre, got {combo:?}"
    );
}
