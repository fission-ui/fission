use fission_core::{AppState, BuildCtx, View, Widget, Node};
use fission_widgets::empty_state::EmptyState;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState;
impl AppState for TestState {}

#[test]
fn test_empty_state_structure() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = TestState::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();

    let empty = EmptyState {
        icon: None,
        title: "Nothing here".into(),
        description: None,
        action: None,
    };

    let node = empty.build(&mut ctx, &view);
    assert!(matches!(node, Node::Align(_) | Node::Container(_)));
}
