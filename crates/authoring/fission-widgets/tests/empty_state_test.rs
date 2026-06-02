use fission_core::internal::BuildCtx;
use fission_core::{build, GlobalState, View};
use fission_widgets::empty_state::EmptyState;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState;
impl GlobalState for TestState {}

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

    let node = build::enter(&mut ctx, &view, || empty.into());
    assert!(matches!(
        fission_core::internal::widget_kind_name(&node),
        "Align" | "Container"
    ));
}
