use fission_core::ui::widgets::spacer::Spacer;
use fission_core::{AppState, BuildCtx, Node, View, Widget};
use fission_widgets::dropzone::Dropzone;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState;
impl AppState for TestState {}

#[test]
fn test_dropzone_structure() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = TestState::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();

    let dropzone = Dropzone {
        child: Box::new(Spacer::default().into_node()),
        on_drop: None,
        on_drag_enter: None,
        on_drag_leave: None,
    };

    let node = dropzone.build(&mut ctx, &view);
    assert!(matches!(node, Node::GestureDetector(_)));
}
