use fission_core::{AppState, BuildCtx, View, Widget, Node, ActionId, ActionEnvelope, Action, Handler, ReducerContext, Lower};
use fission_widgets::calendar::Calendar;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState {
    date: Option<NaiveDate>,
}
impl AppState for TestState {}

#[test]
fn test_calendar_build() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = TestState::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut reg = fission_core::ActionRegistry::<TestState>::new();
    let mut ctx = BuildCtx::new();

    let calendar = Calendar {
        year: 2025,
        month: 12,
        selected_date: None,
        on_select: None,
        on_navigate: None,
    };

    let node = calendar.build(&mut ctx, &view);
    
    // Verify it builds a Container wrapping a VStack
    if let Node::Container(c) = node {
        assert!(c.child.is_some());
    } else {
        panic!("Calendar should return a Container root");
    }
}
