use super::*;

fn unique_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("fission-store-tests-{name}-{}", std::process::id()));
    fs::remove_dir_all(&dir).ok();
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn options(track: Option<&str>, yes: bool) -> DistributeOptions {
    DistributeOptions {
        project_dir: PathBuf::from("/project"),
        provider: DistributionProvider::MicrosoftStore,
        action: DistributeAction::Publish,
        target: None,
        format: None,
        artifact: None,
        site: "production".to_string(),
        deploy: None,
        track: track.map(str::to_string),
        locales: Vec::new(),
        dry_run: false,
        yes,
        json: false,
    }
}

#[test]
fn store_credentials_honor_configured_env_names() {
    let play = PlayStoreConfig {
        access_token_env: Some("CUSTOM_PLAY_TOKEN".to_string()),
        service_account_json_env: Some("CUSTOM_PLAY_JSON".to_string()),
        service_account_json_base64_env: Some("CUSTOM_PLAY_JSON_B64".to_string()),
        google_application_credentials_env: Some("CUSTOM_GOOGLE_CREDS".to_string()),
        ..Default::default()
    };
    assert_eq!(
        play_credential_env_names(&play),
        vec![
            "CUSTOM_PLAY_TOKEN",
            "CUSTOM_PLAY_JSON",
            "CUSTOM_PLAY_JSON_B64",
            "CUSTOM_GOOGLE_CREDS"
        ]
    );

    let app_store = AppStoreConfig {
        issuer_id: Some("issuer".to_string()),
        key_id: Some("key".to_string()),
        api_key_base64_env: Some("CUSTOM_APP_STORE_KEY_B64".to_string()),
        ..Default::default()
    };
    env::set_var("CUSTOM_APP_STORE_KEY_B64", "not-a-real-key");
    assert!(app_store_credentials_available_for_cfg(&app_store));
    env::remove_var("CUSTOM_APP_STORE_KEY_B64");

    let microsoft_store = MicrosoftStoreConfig {
        token_env: Some("CUSTOM_MICROSOFT_TOKEN".to_string()),
        client_secret_env: Some("CUSTOM_MICROSOFT_SECRET".to_string()),
        ..Default::default()
    };
    assert_eq!(
        microsoft_store_token_env(&microsoft_store),
        "CUSTOM_MICROSOFT_TOKEN"
    );
    assert_eq!(
        microsoft_store_client_secret_env(&microsoft_store),
        "CUSTOM_MICROSOFT_SECRET"
    );
}

#[test]
fn play_internal_sharing_upload_urls_use_media_endpoints() {
    assert_eq!(
        play_internal_sharing_upload_url("com.example.app", "apk").unwrap(),
        "https://androidpublisher.googleapis.com/upload/androidpublisher/v3/applications/internalappsharing/com.example.app/artifacts/apk?uploadType=media"
    );
    assert_eq!(
        play_internal_sharing_upload_url("com.example.app", "aab").unwrap(),
        "https://androidpublisher.googleapis.com/upload/androidpublisher/v3/applications/internalappsharing/com.example.app/artifacts/bundle?uploadType=media"
    );
    assert!(play_internal_sharing_upload_url("com.example.app", "ipa").is_err());
}

#[test]
fn play_internal_sharing_readiness_skips_track_version_code_preflight() {
    let dir = unique_dir("play-internal-sharing-readiness");
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
name = "Demo"
app_id = "com.example.demo"
version = "1.2.3"
build = 42

[distribution.play_store]
package_name = "com.example.demo"
"#,
    )
    .unwrap();
    let config = super::super::load_publish_manifest(&dir).unwrap();
    let mut checks = Vec::new();

    readiness_play_store(&dir, Some("internal-sharing"), None, &config, &mut checks).unwrap();

    let track = checks
        .iter()
        .find(|check| check.id == "release.play_store.track_supported")
        .unwrap();
    assert_eq!(track.status, CheckStatus::Passed);
    let version = checks
        .iter()
        .find(|check| check.id == "release.play_store.version_code_available")
        .unwrap();
    assert_eq!(version.status, CheckStatus::Skipped);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn base64_env_secret_file_writes_private_temp_and_cleans_up() {
    let env_name = format!("FISSION_TEST_SECRET_B64_{}", std::process::id());
    env::set_var(&env_name, BASE64_STANDARD.encode("secret payload"));
    let path;
    let temp_dir;
    {
        let secret = base64_env_secret_file(&env_name, "secret.json")
            .unwrap()
            .unwrap();
        path = secret.path.clone();
        temp_dir = path.parent().unwrap().to_path_buf();
        assert_eq!(fs::read_to_string(&path).unwrap(), "secret payload");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&temp_dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    assert!(!path.exists());
    assert!(!temp_dir.exists());
    env::remove_var(env_name);
}

#[test]
fn msstore_publish_args_keep_submission_as_draft_by_default() {
    let args = msstore_publish_args(
        Path::new("/project"),
        Path::new("/artifacts/app.msixupload"),
        "9N123",
        None,
        None,
        false,
    );
    assert_eq!(
        args,
        vec![
            "publish",
            "/project",
            "-i",
            "/artifacts/app.msixupload",
            "-id",
            "9N123",
            "-nc"
        ]
    );
}

#[test]
fn msstore_publish_args_include_flight_and_rollout_when_configured() {
    let args = msstore_publish_args(
        Path::new("/project"),
        Path::new("/artifacts/app.msix"),
        "9N123",
        Some("beta"),
        Some(25),
        true,
    );
    assert_eq!(
        args,
        vec![
            "publish",
            "/project",
            "-i",
            "/artifacts/app.msix",
            "-id",
            "9N123",
            "-f",
            "beta",
            "-prp",
            "25"
        ]
    );
}

#[test]
fn microsoft_store_private_track_uses_configured_flight_id() {
    let cfg = MicrosoftStoreConfig {
        flight_id: Some("insiders".to_string()),
        ..Default::default()
    };
    assert_eq!(
        microsoft_store_flight_id(Some("private"), &cfg).unwrap(),
        Some("insiders".to_string())
    );
    assert_eq!(
        microsoft_store_flight_id(Some("preview"), &cfg).unwrap(),
        Some("preview".to_string())
    );
    assert!(microsoft_store_flight_id(Some("public"), &cfg)
        .unwrap()
        .is_none());
}

#[test]
fn microsoft_store_submit_requires_explicit_commit_intent() {
    let cfg = MicrosoftStoreConfig::default();
    assert!(!microsoft_store_should_submit(&options(None, false), &cfg));
    assert!(!microsoft_store_should_submit(
        &options(Some("public"), false),
        &cfg
    ));
    assert!(microsoft_store_should_submit(
        &options(Some("public"), true),
        &cfg
    ));
    let cfg = MicrosoftStoreConfig {
        submit: Some(true),
        ..Default::default()
    };
    assert!(microsoft_store_should_submit(&options(None, false), &cfg));
}

#[test]
fn microsoft_store_rollout_rejects_out_of_range_percentages() {
    let cfg = MicrosoftStoreConfig {
        package_rollout_percentage: Some(101),
        ..Default::default()
    };
    assert!(microsoft_store_rollout_percentage(&cfg).is_err());
}

#[test]
fn play_track_version_detection_accepts_string_or_number_codes() {
    let track = serde_json::json!({
        "releases": [
            { "versionCodes": ["41"] },
            { "versionCodes": [42] }
        ]
    });
    assert!(play_track_contains_version_code(&track, "41"));
    assert!(play_track_contains_version_code(&track, "42"));
    assert!(!play_track_contains_version_code(&track, "43"));
}

#[test]
fn play_track_status_uses_latest_release_status() {
    let track = serde_json::json!({
        "track": "internal",
        "releases": [
            { "status": "inProgress", "versionCodes": ["42"] }
        ]
    });
    assert_eq!(play_track_status(&track), "inprogress");
}

#[test]
fn play_artifact_inventory_detection_checks_uploaded_artifacts() {
    let bundles = serde_json::json!({
        "bundles": [
            { "versionCode": 42 }
        ]
    });
    assert!(play_artifact_inventory_contains_version_code(
        &bundles, "bundles", "42"
    ));
    assert!(!play_artifact_inventory_contains_version_code(
        &bundles, "bundles", "43"
    ));
}

#[test]
fn play_release_notes_load_active_release_locale_files() {
    let dir =
        std::env::temp_dir().join(format!("fission-play-release-notes-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("release-content/metadata/1.2.3+4/notes")).unwrap();
    fs::write(
        dir.join("release-content/metadata/1.2.3+4/notes/en-US.md"),
        "New Android release.",
    )
    .unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"
[release]
active_release = "1.2.3+4"
default_locales = ["en-US"]

[[releases]]
id = "1.2.3+4"
release_notes = "release-content/metadata/1.2.3+4/notes"
"#,
    )
    .unwrap();

    let notes = play_release_notes(&dir, &[]).unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["language"], "en-US");
    assert_eq!(notes[0]["text"], "New Android release.");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn configured_android_version_code_uses_package_then_app_then_release() {
    let dir =
        std::env::temp_dir().join(format!("fission-play-version-code-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
build = 7

[release]
active_release = "1.2.3+8"

[[releases]]
id = "1.2.3+8"
build = 8

[package.android]
version_code = 9
"#,
    )
    .unwrap();

    assert_eq!(
        configured_android_version_code(&dir).unwrap(),
        Some("9".to_string())
    );
}

#[test]
fn play_provider_version_code_prefers_artifact_manifest_build() {
    let dir = std::env::temp_dir().join(format!(
        "fission-play-artifact-version-code-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[package.android]
version_code = 9
"#,
    )
    .unwrap();
    let manifest = ArtifactManifest {
        schema_version: 1,
        created_at_unix_seconds: 0,
        project: ArtifactProject {
            app_id: "com.example.demo".to_string(),
            name: "Demo".to_string(),
            build: Some(42),
            version: Some("1.2.3".to_string()),
        },
        target: "android".to_string(),
        format: "aab".to_string(),
        profile: "release".to_string(),
        variant: None,
        root_dir: dir.display().to_string(),
        source_config: Vec::new(),
        artifacts: Vec::new(),
        icon_manifest: None,
        signing: None,
        notarization: None,
        validation: ArtifactValidation {
            state: "passed".to_string(),
            checks: Vec::new(),
        },
    };

    assert_eq!(
        android_version_code_for_provider(&dir, Some(&manifest)).unwrap(),
        Some("42".to_string())
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn configured_ios_build_number_ignores_android_version_code() {
    let dir = std::env::temp_dir().join(format!("fission-ios-build-number-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
build = 7

[package.android]
version_code = 99

[package.ios]
build_number = "42"
"#,
    )
    .unwrap();

    assert_eq!(
        configured_ios_build_number(&dir).unwrap(),
        Some("42".to_string())
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn app_store_provider_build_number_prefers_artifact_manifest_build() {
    let dir = std::env::temp_dir().join(format!(
        "fission-app-store-artifact-build-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[package.ios]
build_number = "9"
"#,
    )
    .unwrap();
    let manifest = ArtifactManifest {
        schema_version: 1,
        created_at_unix_seconds: 0,
        project: ArtifactProject {
            app_id: "com.example.demo".to_string(),
            name: "Demo".to_string(),
            build: Some(43),
            version: Some("1.2.3".to_string()),
        },
        target: "ios".to_string(),
        format: "ipa".to_string(),
        profile: "release".to_string(),
        variant: None,
        root_dir: dir.display().to_string(),
        source_config: Vec::new(),
        artifacts: Vec::new(),
        icon_manifest: None,
        signing: None,
        notarization: None,
        validation: ArtifactValidation {
            state: "passed".to_string(),
            checks: Vec::new(),
        },
    };

    assert_eq!(
        app_store_build_number_for_provider(&dir, Some(&manifest)).unwrap(),
        Some("43".to_string())
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn microsoft_store_package_version_uses_artifact_manifest_release() {
    let manifest = ArtifactManifest {
        schema_version: 1,
        created_at_unix_seconds: 0,
        project: ArtifactProject {
            app_id: "com.example.demo".to_string(),
            name: "Demo".to_string(),
            build: Some(44),
            version: Some("1.2.3".to_string()),
        },
        target: "windows".to_string(),
        format: "msix".to_string(),
        profile: "release".to_string(),
        variant: None,
        root_dir: "/tmp/fission-msix".to_string(),
        source_config: Vec::new(),
        artifacts: Vec::new(),
        icon_manifest: None,
        signing: None,
        notarization: None,
        validation: ArtifactValidation {
            state: "passed".to_string(),
            checks: Vec::new(),
        },
    };

    let check = microsoft_store_package_version_check(Some(&manifest), Path::new("."));

    assert_eq!(check.id, "release.microsoft_store.package_version_valid");
    assert_eq!(check.status, CheckStatus::Passed);
    assert_eq!(
        check.details.as_deref(),
        Some("1.2.3.44 from artifact manifest")
    );
}

#[test]
fn microsoft_store_package_version_uses_config_before_artifact_exists() {
    let dir = unique_dir("microsoft-store-config-version");
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
version = "2.3.4"
build = 45
"#,
    )
    .unwrap();

    let check = microsoft_store_package_version_check(None, &dir);

    assert_eq!(check.id, "release.microsoft_store.package_version_valid");
    assert_eq!(check.status, CheckStatus::Passed);
    assert_eq!(check.details.as_deref(), Some("2.3.4.45 from fission.toml"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn microsoft_store_package_version_rejects_invalid_config_before_artifact_exists() {
    let dir = unique_dir("microsoft-store-invalid-config-version");
    fs::write(
        dir.join("fission.toml"),
        r#"[package.windows]
version = "2.preview.4"
"#,
    )
    .unwrap();

    let check = microsoft_store_package_version_check(None, &dir);

    assert_eq!(check.id, "release.microsoft_store.package_version_valid");
    assert_eq!(check.status, CheckStatus::Failed);
    assert!(check
        .details
        .as_deref()
        .unwrap_or_default()
        .contains("must be numeric"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn microsoft_store_readiness_uses_requested_msix_format_before_artifact_exists() {
    let dir = unique_dir("microsoft-store-msix-requested-format");
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
version = "2.3.4"
build = 45

[distribution.microsoft_store]
product_id = "9N123"
package_identity_name = "Example.App"
"#,
    )
    .unwrap();
    let config = super::super::load_publish_manifest(&dir).unwrap();
    let mut checks = Vec::new();

    readiness_microsoft_store(
        &dir,
        Some("private"),
        Some(PackageFormat::Msix),
        None,
        &config,
        &mut checks,
    )
    .unwrap();

    assert!(checks
        .iter()
        .any(|check| check.id == "release.microsoft_store.msix_uses_msstore"));
    assert!(!checks
        .iter()
        .any(|check| check.id == "release.microsoft_store.package_url_configured"));
    assert!(checks
        .iter()
        .any(|check| check.id == "release.microsoft_store.flight_id_configured"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn app_store_build_number_detection_checks_build_attributes() {
    let value = serde_json::json!({
        "data": [
            { "id": "1", "attributes": { "version": "41", "processingState": "VALID" } },
            { "id": "2", "attributes": { "version": "42", "processingState": "PROCESSING" } }
        ]
    });
    assert!(app_store_builds_contain_build_number(&value, "42"));
    assert!(!app_store_builds_contain_build_number(&value, "43"));
}

#[test]
fn app_store_latest_build_status_uses_processing_state() {
    let value = serde_json::json!({
        "data": [
            { "id": "1", "attributes": { "processingState": "PROCESSING" } }
        ]
    });
    assert_eq!(app_store_latest_build_status(&value), "processing");
}
