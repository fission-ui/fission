use super::*;
use serde_json::json;
use std::fs;

#[test]
fn auth_setup_documents_provider_credentials_without_secrets() {
    let report =
        auth_ops::auth_setup_report("auth.setup", Some(DistributionProvider::CloudflarePages));
    assert_eq!(report.status, "ready");
    assert!(report.checks.iter().any(|check| {
        check.id == "auth.cloudflare_pages.env"
            && check
                .details
                .as_deref()
                .is_some_and(|details| details.contains("CLOUDFLARE_API_TOKEN"))
    }));
    assert!(report.checks.iter().any(|check| {
        check.id == "auth.cloudflare_pages.scopes"
            && check
                .details
                .as_deref()
                .is_some_and(|details| details.contains("Pages"))
    }));
}

#[test]
fn auth_import_reports_env_policy_without_storing_secret() {
    let report = auth_ops::auth_import_report(
        DistributionProvider::PlayStore,
        Some("service-account.json"),
    );
    assert_eq!(report.area, "auth.import");
    assert!(report.checks.iter().any(|check| {
        check.id == "auth.play_store.import_policy"
            && check
                .remediation
                .iter()
                .any(|item| item.contains("PLAY_STORE_SERVICE_ACCOUNT_JSON_BASE64"))
    }));
}

#[test]
fn auth_logout_and_rotate_are_provider_owned() {
    let logout = auth_ops::auth_logout_report(Some(DistributionProvider::GithubReleases));
    assert!(logout.checks.iter().any(|check| {
        check.id == "auth.github_releases.logout"
            && check
                .details
                .as_deref()
                .is_some_and(|details| details.contains("gh auth logout"))
    }));
    let rotate = auth_ops::auth_rotate_report(Some(DistributionProvider::AppStore));
    assert!(rotate.checks.iter().any(|check| {
        check.id == "auth.app_store.rotate"
            && check
                .details
                .as_deref()
                .is_some_and(|details| details.contains("replacement App Store Connect API key"))
    }));
}

#[test]
fn release_config_set_preserves_existing_comments_and_formatting() {
    let dir =
        std::env::temp_dir().join(format!("fission-release-config-set-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fission.toml");
    fs::write(&path, "# keep this comment\n[app]\nname = \"Todo\"\n").unwrap();

    set_release_field(&dir, "app.version", "1.2.3", true).unwrap();

    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("# keep this comment"));
    assert!(text.contains("version = \"1.2.3\""));
    assert!(text.contains("name = \"Todo\""));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_config_set_accepts_quoted_toml_dotted_keys() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-config-set-quoted-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fission.toml");
    fs::write(
        &path,
        "[distribution.\"github-releases\"]\n\"release.notes\" = \"old\"\n",
    )
    .unwrap();

    assert_eq!(
        toml_field_value(&dir, "distribution.\"github-releases\".\"release.notes\"")
            .unwrap()
            .as_deref(),
        Some("old")
    );
    set_release_field(
        &dir,
        "distribution.\"github-releases\".\"release.notes\"",
        "new",
        true,
    )
    .unwrap();

    let value: toml::Value = fs::read_to_string(&path).unwrap().parse().unwrap();
    assert_eq!(
        value
            .get("distribution")
            .and_then(|value| value.get("github-releases"))
            .and_then(|value| value.get("release.notes"))
            .and_then(toml::Value::as_str),
        Some("new")
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn toml_field_path_parser_reports_malformed_quoted_keys() {
    assert!(toml_path_segments("app.\"version").is_err());
    assert!(toml_path_segments("app.\"version\"suffix").is_err());
    assert!(toml_path_segments("app..version").is_err());
}

#[test]
fn release_config_set_dry_run_validates_typed_scalars() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-config-typed-dry-run-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("fission.toml"), "[app]\nbuild = 1\n").unwrap();

    let err = set_release_field_command(&dir, "app.build", "banana", true, false, true)
        .unwrap_err()
        .to_string();
    assert!(err.contains("app.build expects an integer value"));

    let text = fs::read_to_string(dir.join("fission.toml")).unwrap();
    assert!(text.contains("build = 1"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_config_set_writes_typed_scalars_for_release_fields() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-config-typed-set-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        "[app]\nbuild = 1\n[distribution.s3.production]\noverwrite = false\n",
    )
    .unwrap();

    set_release_field(&dir, "app.build", "42", true).unwrap();
    set_release_field(&dir, "distribution.s3.production.overwrite", "true", true).unwrap();
    set_release_field(&dir, "package.android.version_code", "43", true).unwrap();

    let value: toml::Value =
        toml::from_str(&fs::read_to_string(dir.join("fission.toml")).unwrap()).unwrap();
    assert_eq!(
        value
            .get("app")
            .and_then(|table| table.get("build"))
            .and_then(toml::Value::as_integer),
        Some(42)
    );
    assert_eq!(
        value
            .get("distribution")
            .and_then(|table| table.get("s3"))
            .and_then(|table| table.get("production"))
            .and_then(|table| table.get("overwrite"))
            .and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value
            .get("package")
            .and_then(|table| table.get("android"))
            .and_then(|table| table.get("version_code"))
            .and_then(toml::Value::as_integer),
        Some(43)
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_config_set_allows_windows_certificate_thumbprint() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-config-thumbprint-set-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("fission.toml"), "[package.windows]\n").unwrap();

    set_release_field(
        &dir,
        "package.windows.certificate_thumbprint",
        "ABCDEF123456",
        true,
    )
    .unwrap();

    let text = fs::read_to_string(dir.join("fission.toml")).unwrap();
    assert!(text.contains("certificate_thumbprint = \"ABCDEF123456\""));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_config_set_rejects_secret_fields() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-config-secret-set-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("fission.toml"), "[app]\nname = \"Todo\"\n").unwrap();

    let err = set_release_field(
        &dir,
        "distribution.play_store.service_account_json",
        "{\"private_key\":\"secret\"}",
        true,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("refuses to write it to fission.toml"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_config_set_dry_run_does_not_mutate_manifest() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-config-set-dry-run-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fission.toml");
    fs::write(&path, "[app]\nname = \"Todo\"\nversion = \"1.0.0\"\n").unwrap();

    set_release_field_command(&dir, "app.version", "2.0.0", true, false, true).unwrap();

    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("version = \"1.0.0\""));
    assert!(!text.contains("version = \"2.0.0\""));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_config_skip_requirement_appends_once() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-config-skip-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fission.toml");
    fs::write(&path, "[app]\nname = \"Todo\"\n").unwrap();

    skip_release_requirement(&dir, "release_content.play_store.feature_graphic", true).unwrap();
    skip_release_requirement(&dir, "release_content.play_store.feature_graphic", true).unwrap();

    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("[release]"));
    assert_eq!(
        text.matches("release_content.play_store.feature_graphic")
            .count(),
        1
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_config_add_release_dry_run_does_not_append() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-config-add-dry-run-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fission.toml");
    fs::write(&path, "[app]\nname = \"Todo\"\n").unwrap();

    add_release_command(&dir, "1.2.3", 4, None, true, false, true).unwrap();

    let text = fs::read_to_string(&path).unwrap();
    assert!(!text.contains("[[releases]]"));
    assert!(!text.contains("1.2.3+4"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_config_write_file_dry_run_does_not_create_file() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-file-dry-run-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    write_release_file_command(
        &dir,
        "1.2.3+4",
        "notes",
        Some(DistributionProvider::PlayStore),
        Some("en-US"),
        Some("Release notes"),
        None,
        true,
        false,
        true,
    )
    .unwrap();

    assert!(!dir
        .join("release-content/metadata/1.2.3+4/notes/en-US.md")
        .exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_config_write_file_addresses_release_kind_and_locale() {
    let dir =
        std::env::temp_dir().join(format!("fission-release-file-write-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    write_release_file_command(
        &dir,
        "1.2.3+4",
        "notes",
        Some(DistributionProvider::AppStore),
        Some("fr-FR"),
        Some("Notes de version"),
        None,
        false,
        true,
        true,
    )
    .unwrap();

    assert_eq!(
        fs::read_to_string(dir.join("release-content/metadata/1.2.3+4/notes/fr-FR.md")).unwrap(),
        "Notes de version"
    );
    assert!(write_release_file_command(
        &dir,
        "../escape",
        "notes",
        None,
        Some("en-US"),
        Some("bad"),
        None,
        false,
        true,
        true,
    )
    .is_err());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_config_bump_build_updates_android_version_code() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-config-bump-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fission.toml");
    fs::write(
        &path,
        r#"[app]
name = "Demo"
app_id = "com.example.demo"
build = 7

[package.android]
version_code = 9
"#,
    )
    .unwrap();

    bump_release_build(&dir, Some(Target::Android), 1, true).unwrap();

    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("version_code = 10"));
    assert!(text.contains("build = 7"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_config_bump_build_updates_macos_build_number() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-config-bump-macos-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fission.toml");
    fs::write(
        &path,
        r#"[app]
name = "Demo"
app_id = "com.example.demo"
build = 7

[package.macos]
marketing_version = "1.2.3"
build_number = "9"
"#,
    )
    .unwrap();

    bump_release_build(&dir, Some(Target::Macos), 1, true).unwrap();

    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("build_number = \"10\""));
    assert!(text.contains("build = 7"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_config_bump_build_updates_windows_package_version() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-config-bump-windows-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fission.toml");
    fs::write(
        &path,
        r#"[app]
name = "Demo"
app_id = "com.example.demo"
version = "1.2.3"
build = 7

[package.windows]
identity_name = "Example.Demo"
"#,
    )
    .unwrap();

    bump_release_build(&dir, Some(Target::Windows), 1, true).unwrap();

    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("version = \"1.2.3.8\""));
    assert!(text.contains("build = 7"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_config_version_state_extracts_play_track_builds() {
    let provider_state = play_store_version_state(&json!({
        "track": "internal",
        "releases": [
            {
                "status": "completed",
                "versionCodes": ["41", "42"]
            }
        ]
    }));

    assert_eq!(provider_state.latest_released_build, Some(42));
    assert_eq!(provider_state.latest_uploaded_build, None);
    assert_eq!(provider_state.latest_status.as_deref(), Some("completed"));

    let blocked =
        version_state_monotonic(DistributionProvider::PlayStore, Some(42), &provider_state);
    assert_eq!(blocked.status, "failed");
    assert_eq!(version_state_next_action(&blocked, None), "bump-build");

    let passed =
        version_state_monotonic(DistributionProvider::PlayStore, Some(43), &provider_state);
    assert_eq!(passed.status, "passed");
    assert_eq!(version_state_next_action(&passed, None), "upload-new-build");
}

#[test]
fn release_config_version_state_extracts_app_store_builds() {
    let provider_state = app_store_version_state(&json!({
        "builds": {
            "data": [
                {
                    "attributes": {
                        "version": "9",
                        "processingState": "VALID"
                    }
                },
                {
                    "attributes": {
                        "version": "10",
                        "processingState": "PROCESSING"
                    }
                }
            ]
        },
        "review_submissions": {
            "data": [
                {
                    "attributes": {
                        "state": "IN_REVIEW"
                    }
                }
            ]
        }
    }));

    assert_eq!(provider_state.latest_uploaded_build, Some(10));
    assert_eq!(provider_state.latest_released_build, None);
    assert_eq!(provider_state.latest_status.as_deref(), Some("VALID"));
    assert_eq!(provider_state.review_status.as_deref(), Some("IN_REVIEW"));

    let blocked =
        version_state_monotonic(DistributionProvider::AppStore, Some(10), &provider_state);
    assert_eq!(blocked.status, "failed");

    let passed = version_state_monotonic(DistributionProvider::AppStore, Some(11), &provider_state);
    assert_eq!(passed.status, "passed");
}
