use fission_core::{AppState, BuildCtx, Node, View, Widget};
use fission_widgets::pagination::Pagination;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState {
    page: usize,
}
impl AppState for TestState {}

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

    let node = fission_core::view::lower_widget_to_node(&pagination, &mut ctx, &view);
    assert!(matches!(node, Node::Row(_))); // It builds a Row (HStack)
}
