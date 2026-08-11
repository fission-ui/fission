use super::*;

#[test]
fn app_store_platform_supports_ios_and_macos() {
    assert_eq!(
        app_store_platform_api_value(&AppStoreConfig::default()).unwrap(),
        "IOS"
    );
    assert_eq!(
        app_store_platform_api_value(&AppStoreConfig {
            platform: Some("macos".to_string()),
            ..Default::default()
        })
        .unwrap(),
        "MAC_OS"
    );
    assert!(app_store_platform_api_value(&AppStoreConfig {
        platform: Some("windows".to_string()),
        ..Default::default()
    })
    .is_err());
}

#[test]
fn latest_user_comment_uses_newest_user_comment() {
    let value = json!({
        "comments": [
            {"userComment": {"text": "old", "starRating": 2}},
            {"developerComment": {"text": "reply"}},
            {"userComment": {"text": "new", "starRating": 4}}
        ]
    });
    let comment = latest_user_comment(&value).unwrap();
    assert_eq!(comment.get("text").and_then(Value::as_str), Some("new"));
}

#[test]
fn google_group_csv_reader_deduplicates_group_emails() {
    let path = std::env::temp_dir().join(format!("fission-play-groups-{}.csv", std::process::id()));
    fs::write(
        &path,
        "email\nclosed-testers@example.com\nclosed-testers@example.com,other@example.com\n",
    )
    .unwrap();
    let groups = read_google_group_csv(&path).unwrap();
    assert_eq!(
        groups,
        vec![
            "closed-testers@example.com".to_string(),
            "other@example.com".to_string()
        ]
    );
}

#[test]
fn base64_env_secret_file_writes_private_temp_and_cleans_up() {
    let env_name = format!("FISSION_RELEASE_TEST_SECRET_B64_{}", std::process::id());
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
fn resolved_play_listing_merges_root_listing_and_release_metadata() {
    let root: ReleaseProviderToml = toml::from_str(
        r#"
[release]
active_release = "1.0.0+1"
default_locales = ["en-US"]

[release.store_listing.play_store.en-US]
title = "Todo"
short_description = "Plan work"
video = "https://example.com/video"

[[releases]]
id = "1.0.0+1"
metadata = "release-content/metadata/1.0.0+1/release.toml"
"#,
    )
    .unwrap();
    let metadata: ReleaseMetadataToml = toml::from_str(
        r#"
[play_store.en-US]
full_description = "A focused task manager."
"#,
    )
    .unwrap();
    let listing = resolve_play_listing(&root, &metadata, "en-US").unwrap();
    assert_eq!(listing.title, "Todo");
    assert_eq!(listing.short_description, "Plan work");
    assert_eq!(listing.full_description, "A focused task manager.");
    assert_eq!(listing.video.as_deref(), Some("https://example.com/video"));
}

#[test]
fn beta_play_store_tracks_parse_group_and_groups() {
    let root: ReleaseProviderToml = toml::from_str(
        r#"
[beta.play_store.tracks.closed]
tester_source = "google_group"
group = "closed@example.com"
groups = ["qa@example.com"]
"#,
    )
    .unwrap();
    let tracks = root.beta.unwrap().play_store.unwrap().tracks;
    let closed = tracks.get("closed").unwrap();
    assert_eq!(closed.group.as_deref(), Some("closed@example.com"));
    assert_eq!(closed.groups, vec!["qa@example.com".to_string()]);
}

#[test]
fn app_store_review_response_payload_targets_review() {
    let payload =
        review_beta_ops::app_store_review_response_payload("review-123", "Thanks for the report.");
    assert_eq!(
        payload.pointer("/data/type").and_then(Value::as_str),
        Some("customerReviewResponses")
    );
    assert_eq!(
        payload
            .pointer("/data/attributes/responseBody")
            .and_then(Value::as_str),
        Some("Thanks for the report.")
    );
    assert_eq!(
        payload
            .pointer("/data/relationships/review/data/id")
            .and_then(Value::as_str),
        Some("review-123")
    );
}

#[test]
fn reviews_reply_requires_yes_before_provider_mutation() {
    let dir = std::env::temp_dir().join(format!("fission-review-reply-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let message = dir.join("reply.txt");
    fs::write(&message, "Thanks for the report.").unwrap();

    let err = reviews_reply(
        DistributionProvider::AppStore,
        "review-123",
        &message,
        &dir,
        false,
        false,
        false,
    )
    .unwrap_err();
    assert!(err.to_string().contains("pass --yes"));
}

#[test]
fn app_store_beta_tester_payload_assigns_group() {
    let tester = AppStoreTester {
        email: "person@example.com".to_string(),
        first_name: Some("Test".to_string()),
        last_name: Some("User".to_string()),
    };
    let payload = app_store_beta_tester_payload(&tester, "group-123");
    assert_eq!(
        payload.pointer("/data/type").and_then(Value::as_str),
        Some("betaTesters")
    );
    assert_eq!(
        payload
            .pointer("/data/attributes/email")
            .and_then(Value::as_str),
        Some("person@example.com")
    );
    assert_eq!(
        payload
            .pointer("/data/relationships/betaGroups/data/0/id")
            .and_then(Value::as_str),
        Some("group-123")
    );
}

#[test]
fn app_store_beta_build_assignment_payload_targets_build() {
    let payload = app_store_beta_build_assignment_payload("build-123");
    assert_eq!(
        payload.pointer("/data/0/type").and_then(Value::as_str),
        Some("builds")
    );
    assert_eq!(
        payload.pointer("/data/0/id").and_then(Value::as_str),
        Some("build-123")
    );
}

#[test]
fn app_store_build_lookup_filters_artifact_build() {
    let url = app_store_builds_url("app-123", Some("42"));
    assert!(url.contains("/v1/builds?"));
    assert!(url.contains("filter[app]=app-123"));
    assert!(url.contains("filter[version]=42"));
    assert!(url.contains("sort=-uploadedDate"));
}

#[test]
fn app_store_build_from_response_reports_processing_state() {
    let value = json!({
        "data": [{
            "id": "build-123",
            "attributes": {
                "version": "42",
                "processingState": "VALID",
                "uploadedDate": "2026-07-10T10:00:00Z",
                "expired": false
            }
        }]
    });
    let build = app_store_build_from_response(&value).unwrap();
    assert_eq!(build.id, "build-123");
    assert_eq!(build.version, "42");
    assert_eq!(build.processing_state.as_deref(), Some("VALID"));
    ensure_app_store_build_assignable(&build).unwrap();
}

#[test]
fn app_store_processing_build_is_not_assignable() {
    let build = AppStoreBuild {
        id: "build-123".to_string(),
        version: "42".to_string(),
        processing_state: Some("PROCESSING".to_string()),
        uploaded_date: None,
        expired: None,
    };
    assert!(ensure_app_store_build_assignable(&build).is_err());
}

#[test]
fn app_store_localization_payload_uses_version_level_fields() {
    let localization = AppStoreLocalization {
        id: None,
        locale: "en-US".to_string(),
        description: "A focused task manager.".to_string(),
        keywords: Some("todo,tasks".to_string()),
        marketing_url: Some("https://example.com".to_string()),
        promotional_text: Some("Better planning.".to_string()),
        support_url: Some("https://example.com/support".to_string()),
        whats_new: Some("New editor.".to_string()),
    };
    let payload = app_store_localization_create_payload("version-123", &localization);
    assert_eq!(
        payload.pointer("/data/type").and_then(Value::as_str),
        Some("appStoreVersionLocalizations")
    );
    assert_eq!(
        payload
            .pointer("/data/attributes/locale")
            .and_then(Value::as_str),
        Some("en-US")
    );
    assert_eq!(
        payload
            .pointer("/data/relationships/appStoreVersion/data/id")
            .and_then(Value::as_str),
        Some("version-123")
    );
}

#[test]
fn app_store_localization_update_omits_immutable_locale() {
    let localization = AppStoreLocalization {
        id: Some("localization-123".to_string()),
        locale: "en-US".to_string(),
        description: "A focused task manager.".to_string(),
        keywords: Some("todo,tasks".to_string()),
        marketing_url: Some("https://example.com".to_string()),
        promotional_text: Some("Better planning.".to_string()),
        support_url: Some("https://example.com/support".to_string()),
        whats_new: Some("New editor.".to_string()),
    };

    let payload = app_store_localization_update_payload("localization-123", &localization);

    assert!(
        payload.pointer("/data/attributes/locale").is_none(),
        "App Store Connect rejects locale on localization updates"
    );
    assert_eq!(
        payload
            .pointer("/data/attributes/description")
            .and_then(Value::as_str),
        Some("A focused task manager.")
    );
}

#[test]
fn release_config_lock_round_trips_provider_revision() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-config-lock-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("fission.toml"), "[app]\nname = \"Demo\"\n").unwrap();

    let state = remote_state(
        DistributionProvider::PlayStore,
        "com.example.demo".to_string(),
        vec!["en-US".to_string()],
        json!({"listing": {"title": "Demo"}}),
    )
    .unwrap();
    write_release_config_lock(&dir, DistributionProvider::PlayStore, &state).unwrap();

    let lock = read_release_config_lock(&dir, DistributionProvider::PlayStore)
        .unwrap()
        .unwrap();
    ensure_release_config_lock_matches(DistributionProvider::PlayStore, &lock, &state).unwrap();

    let text = fs::read_to_string(dir.join("fission.toml")).unwrap();
    assert!(text.contains("[release.provider_locks.play_store]"));
    assert!(text.contains("remote_revision = \"sha256:"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_config_lock_rejects_changed_remote_revision() {
    let old_state = remote_state(
        DistributionProvider::AppStore,
        "app:version".to_string(),
        vec!["en-US".to_string()],
        json!({"description": "old"}),
    )
    .unwrap();
    let new_state = remote_state(
        DistributionProvider::AppStore,
        "app:version".to_string(),
        vec!["en-US".to_string()],
        json!({"description": "new"}),
    )
    .unwrap();
    let lock = ReleaseConfigLock {
        provider: Some(DistributionProvider::AppStore.as_str().to_string()),
        subject: Some(old_state.subject),
        remote_revision: Some(old_state.remote_revision),
        locales: old_state.locales,
        locked_at_unix_seconds: Some(1),
    };

    let err = ensure_release_config_lock_matches(DistributionProvider::AppStore, &lock, &new_state)
        .unwrap_err()
        .to_string();
    assert!(err.contains("metadata changed since the last import/lock"));
}

#[test]
fn release_content_manifest_freshness_rejects_changed_referenced_files() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-content-fresh-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    let notes_dir = dir.join("release-content/metadata/1.0.0+1/notes");
    fs::create_dir_all(&notes_dir).unwrap();
    let notes = notes_dir.join("en-US.md");
    fs::write(&notes, "Initial notes").unwrap();
    let relative = "release-content/metadata/1.0.0+1/notes/en-US.md";
    fs::write(
        dir.join("release-content/content-manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "referenced_files": [{
                "path": relative,
                "sha256": sha256_file(&notes).unwrap(),
            }]
        }))
        .unwrap(),
    )
    .unwrap();

    ensure_release_config_content_manifest_fresh(&dir).unwrap();
    fs::write(&notes, "Changed notes").unwrap();
    let err = ensure_release_config_content_manifest_fresh(&dir)
        .unwrap_err()
        .to_string();
    assert!(err.contains("changed since release-content/content-manifest.json"));

    let _ = fs::remove_dir_all(&dir);
}
