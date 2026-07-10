use super::*;

#[test]
fn workspace_name_is_stable_and_safe() {
    assert_eq!(
        sanitize_workspace_name("Cacydil Frontend"),
        "cacydil-frontend"
    );
    assert_eq!(sanitize_workspace_name("..."), "app");
}

#[test]
fn env_file_upsert_replaces_existing_value() {
    let dir = env::temp_dir().join(format!("fission-publish-shell-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("release.env");
    upsert_env(&path, "ANDROID_KEYSTORE", "/one.jks").unwrap();
    upsert_env(&path, "ANDROID_KEYSTORE", "/two.jks").unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("/two.jks"));
    assert!(!text.contains("/one.jks"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn file_path_prompt_resolves_relative_to_project_dir() {
    let dir = env::temp_dir().join(format!(
        "fission-publish-shell-path-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("play-service-account.json");
    fs::write(&path, "{}").unwrap();

    assert_eq!(
        resolve_input_file_path("play-service-account.json", &dir),
        path
    );
    assert_eq!(
        resolve_input_file_path(trim_path_input("\"play-service-account.json\""), &dir),
        path
    );
    assert_eq!(
        trim_path_input("'play-service-account.json'"),
        "play-service-account.json"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn guided_log_tail_redacts_secret_env_values() {
    env::set_var("FISSION_TEST_SECRET_TOKEN", "super-secret-token-value");
    let mut tail = VecDeque::new();

    record_log_line(
        &mut tail,
        "upload failed with super-secret-token-value".to_string(),
        true,
    );

    env::remove_var("FISSION_TEST_SECRET_TOKEN");
    assert_eq!(
        tail.front().map(String::as_str),
        Some("upload failed with <redacted:FISSION_TEST_SECRET_TOKEN>")
    );
}

#[test]
fn release_plan_json_converts_to_shell_plan() {
    let plan = publish_shell_release_plan(ReleasePlanJson {
        status: "warning".to_string(),
        provider: "play-store".to_string(),
        target: Some("android".to_string()),
        format: Some("aab".to_string()),
        track: Some("internal".to_string()),
        locales: vec!["en-US".to_string()],
        steps: vec![ReleaseStepJson {
            id: "inspect".to_string(),
            title: "Inspect project and release configuration".to_string(),
            status: "ready".to_string(),
        }],
        capabilities: vec![ProviderCapabilityJson {
            id: "provider.capability.artifact_upload".to_string(),
            status: "supported".to_string(),
            summary: "Android APK/AAB upload".to_string(),
        }],
        requirements: vec![ReleaseRequirementJson {
            id: "release_content.play_store.feature_graphic".to_string(),
            level: "fission-recommended".to_string(),
            status: "missing".to_string(),
            summary: "feature graphic exists".to_string(),
            details: None,
            remediation: vec!["Add a feature graphic.".to_string()],
        }],
    });

    assert_eq!(plan.status, "warning");
    assert_eq!(plan.steps[0].id, "inspect");
    assert_eq!(plan.capabilities[0].status, "supported");
    assert_eq!(plan.requirements[0].severity, crate::CheckSeverity::Warning);
    assert_eq!(plan.requirements[0].status, CheckStatus::Missing);
}

#[test]
fn publish_command_args_preserve_overwrite_remote() {
    let args = publish_command_args_from_request(PublishShellWorkflowRequest {
        project_dir: PathBuf::from("."),
        provider: DistributionProvider::PlayStore,
        target: Some(Target::Android),
        format: Some(PackageFormat::Aab),
        artifact: None,
        site: "production".to_string(),
        deploy: None,
        track: Some("internal".to_string()),
        locales: Vec::new(),
        overwrite_remote: true,
        dry_run: true,
        yes: true,
        json: false,
    });

    assert!(args.iter().any(|arg| arg == "--overwrite-remote"));
}

#[test]
fn local_secret_reference_detects_project_tree_paths() {
    let dir = env::temp_dir().join(format!(
        "fission-publish-shell-project-secret-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    let project = dir.join("project");
    let outside = dir.join("outside");
    fs::create_dir_all(project.join("secrets")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let in_project = project.join("secrets/upload.jks");
    let out_project = outside.join("upload.jks");
    fs::write(&in_project, "secret").unwrap();
    fs::write(&out_project, "secret").unwrap();

    assert!(path_is_inside_project(&in_project, &project));
    assert!(!path_is_inside_project(&out_project, &project));

    let _ = fs::remove_dir_all(&dir);
}
