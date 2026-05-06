use fission_core::{AppState, BuildCtx, Node, View, Widget};
use fission_widgets::file_upload::FileUpload;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState;
impl AppState for TestState {}

#[test]
fn test_file_upload_structure() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = TestState::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();

    let upload = FileUpload {
        label: "Browse".into(),
        selected_file: None,
        on_browse: None,
    };

    let node = upload.build(&mut ctx, &view);
    assert!(matches!(node, Node::Row(_)));
}
