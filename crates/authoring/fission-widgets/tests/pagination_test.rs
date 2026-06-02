use fission_core::internal::BuildCtx;
use fission_core::{build, GlobalState, View};
use fission_widgets::pagination::Pagination;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState {
    page: usize,
}
impl GlobalState for TestState {}

#[test]
fn test_pagination_structure() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = TestState::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();

    let pagination = Pagination {
        current_page: 1,
        total_pages: 5,
        on_change: None,
    };

    let node = build::enter(&mut ctx, &view, || pagination.into());
    assert_eq!(fission_core::internal::widget_kind_name(&node), "Row"); // It builds a Row (HStack)
}
