use fission_core::internal::BuildCtx;
use fission_core::ui::widgets::spacer::Spacer;
use fission_core::{build, GlobalState, View};
use fission_widgets::dropzone::Dropzone;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState;
impl GlobalState for TestState {}

#[test]
fn test_dropzone_structure() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = TestState::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();

    let dropzone = Dropzone {
        child: Spacer::default().into(),
        on_drop: None,
        on_drag_enter: None,
        on_drag_leave: None,
    };

    let node = build::enter(&mut ctx, &view, || dropzone.into());
    assert_eq!(
        fission_core::internal::widget_kind_name(&node),
        "GestureDetector"
    );
}
