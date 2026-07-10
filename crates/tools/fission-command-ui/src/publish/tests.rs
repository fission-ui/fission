use super::*;

fn test_options(project_dir: PathBuf) -> PublishUiOptions {
    PublishUiOptions {
        project_dir,
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
    }
}

fn write_android_release_project(root: &Path) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("fission.toml"),
        r#"targets = ["android"]

[app]
name = "Demo"
app_id = "com.example.demo"
version = "1.0.0"
build = 1

[package.android]
package_name = "com.example.demo"
version_name = "1.0.0"
version_code = 1
keystore_alias = "upload"

[distribution.play_store]
package_name = "com.example.demo"
default_track = "internal"

[release]
default_locales = ["en-US"]
active_release = "1.0.0+1"

[[releases]]
id = "1.0.0+1"
version = "1.0.0"
build = 1
locales = ["en-US"]
"#,
    )
    .unwrap();
}

#[cfg(test)]
#[test]
fn publish_ui_renders_android_board() {
    let state = PublishUiState::load(test_options(PathBuf::from(".")));
    let mut app = fission::terminal::TerminalApp::with_state(PublishApp, state);
    let frame = app.render_frame(160, 80).expect("publish UI renders");
    let text = frame.as_plain_text();
    assert!(text.contains("Fission Local Publish"));
    assert!(text.contains("Android-specific preflight"));
    assert!(text.contains("Step 1 of 7"));
}

#[test]
fn publish_ui_terminal_screenshot_writes_png() {
    let root = env::temp_dir().join(format!(
        "fission-publish-screenshot-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let screenshot = root.join("publish-ui.png");

    let state = PublishUiState::load(test_options(PathBuf::from(".")));
    let mut app = fission::terminal::TerminalApp::with_state(PublishApp, state);
    let frame = app.render_frame(160, 80).expect("publish UI renders");
    fission::terminal::write_frame_png(
        &frame,
        &screenshot,
        fission::terminal::ScreenshotOptions::default(),
    )
    .expect("terminal screenshot writes");

    let bytes = fs::read(&screenshot).expect("read screenshot");
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(bytes.len() > 1024);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn publish_ui_gate_includes_release_plan() {
    let root = env::temp_dir().join(format!(
        "fission-publish-release-plan-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    write_android_release_project(&root);

    let mut state = PublishUiState::load(test_options(root.clone()));
    state.current_step = 7;
    assert!(state.release_plan.is_some());

    let mut app = fission::terminal::TerminalApp::with_state(PublishApp, state);
    let frame = app.render_frame(160, 80).expect("publish UI renders");
    let text = frame.as_plain_text();

    assert!(text.contains("Release plan"));
    assert!(text.contains("Plan status"));
    assert!(text.contains("Step inspect"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn publish_ui_does_not_unlock_publish_before_checks_load() {
    let mut state = PublishUiState::load(test_options(PathBuf::from(".")));
    state.publish_confirmation = state.app_id.clone();

    assert!(!state.is_ready_to_publish());
}

#[test]
fn publish_ui_arrow_keys_move_between_wizard_steps() {
    let mut state = PublishUiState::load(test_options(PathBuf::from(".")));

    assert!(state.handle_key(&fission::KeyCode::Right));
    assert_eq!(state.current_step, 2);

    assert!(state.handle_key(&fission::KeyCode::Down));
    assert_eq!(state.current_step, 3);

    assert!(state.handle_key(&fission::KeyCode::Left));
    assert_eq!(state.current_step, 2);

    assert!(state.handle_key(&fission::KeyCode::Up));
    assert_eq!(state.current_step, 1);
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

    let mut state = PublishUiState::load(test_options(root.clone()));
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

#[test]
fn publish_ui_file_picker_supports_arrow_key_selection() {
    let root = env::temp_dir().join(format!(
        "fission-publish-picker-keys-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let fixture = root.join("fixture.json");
    fs::write(&fixture, "{}").unwrap();

    let mut state = PublishUiState::load(test_options(root.clone()));
    state.open_file_picker(FilePurpose::PlayServiceJson);

    assert!(state.handle_key(&fission::KeyCode::Down));
    assert_eq!(state.file_picker.as_ref().unwrap().selected_index, 1);
    assert!(state.handle_key(&fission::KeyCode::Enter));
    assert_eq!(state.selected_file.as_ref().unwrap().path, fixture);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn publish_ui_refuses_to_reference_project_secret_files() {
    let root = env::temp_dir().join(format!(
        "fission-publish-reference-secret-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    write_android_release_project(&root);
    let selected = root.join("upload-key.jks");
    fs::write(&selected, "secret").unwrap();

    let mut state = PublishUiState::load(test_options(root.clone()));
    state.workspace = root.join("local-release-workspace");
    state.selected_file = Some(FileSelection {
        purpose: FilePurpose::AndroidKeystore,
        path: selected,
    });

    state.apply_selected_file(FileAction::Reference);

    assert!(state.android_jks_path.is_empty());
    assert!(state
        .status_message
        .contains("refusing to reference a secret file inside the project tree"));
    assert!(!state.workspace.join("release.env").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn publish_ui_task_output_redacts_secret_env_values() {
    env::set_var("FISSION_UI_TEST_SECRET_TOKEN", "super-secret-token");

    assert_eq!(
        redact_output_lines("provider returned super-secret-token"),
        vec!["provider returned <redacted:FISSION_UI_TEST_SECRET_TOKEN>".to_string()]
    );

    env::remove_var("FISSION_UI_TEST_SECRET_TOKEN");
}

#[test]
fn publish_ui_save_values_updates_current_process_env() {
    let root = env::temp_dir().join(format!(
        "fission-publish-save-env-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    write_android_release_project(&root);
    let mut state = PublishUiState::load(test_options(root.clone()));
    state.workspace = root.join("local-release-workspace");

    env::remove_var("FISSION_UI_TEST_SAVE_SECRET");
    state.save_values(&[(
        "FISSION_UI_TEST_SAVE_SECRET",
        "saved-secret-value".to_string(),
    )]);

    assert_eq!(
        env::var("FISSION_UI_TEST_SAVE_SECRET").as_deref(),
        Ok("saved-secret-value")
    );
    let env_file = fs::read_to_string(state.workspace.join("release.env")).unwrap();
    assert!(env_file.contains("FISSION_UI_TEST_SAVE_SECRET"));

    env::remove_var("FISSION_UI_TEST_SAVE_SECRET");
    let _ = fs::remove_dir_all(root);
}
