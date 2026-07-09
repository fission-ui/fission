use super::*;

#[cfg(test)]
#[test]
fn publish_ui_renders_android_board() {
    let state = PublishUiState::load(PublishUiOptions {
        project_dir: PathBuf::from("."),
        provider: DistributionProvider::PlayStore,
        target: Some(Target::Android),
        format: Some(PackageFormat::Aab),
        artifact: None,
        site: "production".to_string(),
        deploy: None,
        track: Some("internal".to_string()),
        locales: vec!["en-US".to_string()],
        screenshot: None,
        exit_after_render: false,
        width: None,
        height: None,
        native_file_dialog: false,
    });
    let mut app = fission::terminal::TerminalApp::with_state(PublishApp, state);
    let frame = app.render_frame(160, 80).expect("publish UI renders");
    let text = frame.as_plain_text();
    assert!(text.contains("Fission Local Publish"));
    assert!(text.contains("Android-specific preflight"));
    assert!(text.contains("Step 1 of 7"));
}

#[test]
fn publish_ui_file_picker_lists_project_entries() {
    let root = env::temp_dir().join(format!(
        "fission-publish-picker-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("nested")).unwrap();
    fs::write(root.join("fixture.json"), "{}").unwrap();

    let mut state = PublishUiState::load(PublishUiOptions {
        project_dir: root.clone(),
        provider: DistributionProvider::PlayStore,
        target: Some(Target::Android),
        format: Some(PackageFormat::Aab),
        artifact: None,
        site: "production".to_string(),
        deploy: None,
        track: Some("internal".to_string()),
        locales: vec!["en-US".to_string()],
        screenshot: None,
        exit_after_render: false,
        width: None,
        height: None,
        native_file_dialog: false,
    });
    state.current_step = 3;
    state.open_file_picker(FilePurpose::PlayServiceJson);

    let mut app = fission::terminal::TerminalApp::with_state(PublishApp, state);
    let frame = app.render_frame(160, 100).expect("publish picker renders");
    let text = frame.as_plain_text();

    assert!(text.contains("Select service account JSON"));
    assert!(text.contains("nested/"));
    assert!(text.contains("fixture.json"));

    let _ = fs::remove_dir_all(root);
}
