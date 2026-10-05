use fission_core::authoring::BuildCtx;
use fission_core::{build, GlobalState, View};
use fission_ir::TextFieldValidationState;
use fission_widgets::file_upload::FileUpload;
use serde::{Deserialize, Serialize};
use std::ops::ControlFlow;

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
        error_text: None,
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

#[test]
fn file_upload_renders_and_announces_an_error() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = TestState;
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();
    let node: fission_core::Widget = build::enter(&mut ctx, &view, || {
        FileUpload {
            label: "Browse".into(),
            selected_file: Some("report.csv".into()),
            error_text: Some("The file is too large".into()),
            on_browse: None,
            browse_semantics_identifier: None,
        }
        .into()
    });

    let fission_core::ui::WidgetKind::SemanticsRegion(region) = node.kind() else {
        panic!("file upload should lower through a semantic region");
    };
    assert_eq!(region.validation_state, TextFieldValidationState::Invalid);
    assert_eq!(
        region.validation_message.as_deref(),
        Some("The file is too large")
    );
    assert_eq!(
        region.value.as_deref(),
        Some("Error: The file is too large")
    );
    let mut found_error = false;
    let _ = node.visit(&mut |widget| {
        found_error |= matches!(
            widget.kind(),
            fission_core::ui::WidgetKind::Text(text)
                if matches!(&text.content, fission_core::ui::TextContent::Literal(value)
                    if value == "The file is too large")
        );
        ControlFlow::Continue(())
    });
    assert!(found_error);
}
