use chrono::NaiveDate;
use fission_core::internal::BuildCtx;
use fission_core::{build, GlobalState, View};
use fission_widgets::calendar::Calendar;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState {
    date: Option<NaiveDate>,
}
impl GlobalState for TestState {}

#[test]
fn test_calendar_build() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = TestState::default();
    let view = View::new(&state, &runtime, &env, None);
    let _reg = fission_core::ActionRegistry::<TestState>::new();
    let mut ctx = BuildCtx::<TestState>::new();

    let calendar = Calendar {
        year: 2025,
        month: 12,
        selected_date: None,
        on_select: None,
        on_navigate: None,
        cell_size: None,
        padding: None,
    };

    let node = build::enter(&mut ctx, &view, || calendar.into());

    let c = fission_core::internal::widget_as_container(&node)
        .expect("Calendar should return a Container root");
    assert!(c.child.is_some());
}
