use fission_core::authoring::BuildCtx;
use fission_core::{build, GlobalState, View};
use fission_widgets::file_upload::FileUpload;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState;
impl GlobalState for TestState {}

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
        browse_semantics_identifier: None,
    };

    let node = build::enter(&mut ctx, &view, || upload.into());
    // The browse control and the selected filename are one group, so the
    // widget lowers through a semantics region rather than a bare row.
    assert_eq!(
        fission_core::internal::widget_kind_name(&node),
        "SemanticsRegion"
    );
}
