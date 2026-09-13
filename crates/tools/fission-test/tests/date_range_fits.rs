//! A date range in a narrow container keeps both fields inside it.

use fission_core::ui::{Container, Widget};
use fission_core::GlobalState;
use fission_core::WidgetId;
use fission_ir::Role;
use fission_test::{TestDriver, TestHarness};
use fission_widgets::DateRangePicker;

#[derive(Debug, Default, Clone)]
struct State;

impl GlobalState for State {}

#[derive(Clone)]
struct Page;

impl From<Page> for Widget {
    fn from(_: Page) -> Self {
        Container::new(DateRangePicker {
            id_start: WidgetId::explicit("range_start"),
            id_end: WidgetId::explicit("range_end"),
            start: None,
            end: None,
            is_start_open: false,
            is_end_open: false,
            on_change: None,
            on_toggle_start: None,
            on_toggle_end: None,
            on_close_start: None,
            on_close_end: None,
        })
        .width(288.0)
        .into()
    }
}

#[test]
fn both_date_fields_stay_inside_a_narrow_container() {
    let mut driver = TestDriver::new(TestHarness::new(State).with_root_widget(Page));
    driver.pump().expect("first frame");
    let fields: Vec<_> = driver
        .find_role(Role::ComboBox)
        .into_iter()
        .map(|node| node.bounds)
        .collect();
    assert_eq!(fields.len(), 2, "a start and an end field");
    for field in fields {
        assert!(
            field.x() + field.width() <= 288.5,
            "a date field ends at {}, past the 288 wide container",
            field.x() + field.width()
        );
    }
}
