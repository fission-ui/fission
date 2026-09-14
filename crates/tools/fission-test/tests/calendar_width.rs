//! A calendar is as wide as its month grid, even when far more width is offered.

use fission_core::ui::{Column, Widget};
use fission_core::GlobalState;
use fission_ir::Role;
use fission_test::{TestDriver, TestHarness};
use fission_widgets::Calendar;

#[derive(Debug, Default, Clone)]
struct State;

impl GlobalState for State {}

#[derive(Clone)]
struct Page;

impl From<Page> for Widget {
    fn from(_: Page) -> Self {
        Column {
            children: vec![Calendar {
                year: 2026,
                month: 9,
                selected_date: None,
                on_select: None,
                on_navigate: None,
                cell_size: None,
                padding: None,
            }
            .into()],
            align_items: fission_core::op::AlignItems::Start,
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn a_calendar_hugs_its_seven_day_columns() {
    let mut driver = TestDriver::new(TestHarness::new(State).with_root_widget(Page));
    driver.pump().expect("first frame");
    let calendar = driver
        .find_role(Role::Table)
        .into_iter()
        .next()
        .expect("calendar")
        .bounds;
    let expected = 36.0 * 7.0 + 16.0 * 2.0;
    assert!(
        (calendar.width() - expected).abs() < 1.0,
        "the calendar is {expected} wide, got {calendar:?}"
    );
}
