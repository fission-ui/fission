use super::*;
use std::ffi::OsStr;
use std::fs;

fn test_context() -> ReleaseContextReport {
    ReleaseContextReport {
        project_dir: ".".to_string(),
        app_name: Some("Demo".to_string()),
        target: Some("android".to_string()),
        format: Some("aab".to_string()),
        provider: "play-store".to_string(),
        release_id: Some("1.0.0+1".to_string()),
        track: Some("internal".to_string()),
        locales: vec!["en-US".to_string()],
        interactive: false,
        ci: true,
    }
}

#[test]
fn artifact_manifest_supplies_missing_build_number() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-workflow-artifact-build-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
name = "Demo"
app_id = "com.example.demo"
version = "1.2.3"
"#,
    )
    .unwrap();
    let artifact = dir.join("artifact-manifest.json");
    fs::write(
        &artifact,
        r#"{
  "project": { "version": "1.2.3", "build": 42 },
  "target": "android",
  "format": "apk",
  "artifacts": []
}"#,
    )
    .unwrap();
    let requirements = version_requirements(
        &PublishWorkflowOptions {
            project_dir: dir.clone(),
            provider: DistributionProvider::PlayStore,
            target: Some(Target::Android),
            format: Some(PackageFormat::Apk),
            artifact: Some(artifact.clone()),
            site: "production".to_string(),
            deploy: None,
            track: Some("internal".to_string()),
            locales: Vec::new(),
            overwrite_remote: false,
            dry_run: true,
            yes: true,
            json: true,
        },
        Some(&artifact),
    )
    .unwrap();
    let build = requirements
        .iter()
        .find(|req| req.id == "release.build.resolved")
        .unwrap();
    assert_eq!(build.status, RequirementStatus::Passed);
    assert_eq!(build.details.as_deref(), Some("42"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_metadata_sync_requires_complete_release_config() {
    let ready = ReleasePlanReport {
        context: test_context(),
        project_dir: ".".to_string(),
        provider: "play-store".to_string(),
        target: Some("android".to_string()),
        format: Some("aab".to_string()),
        artifact: None,
        track: Some("internal".to_string()),
        locales: vec!["en-US".to_string()],
        status: "ready".to_string(),
        steps: Vec::new(),
        capabilities: Vec::new(),
        requirements: vec![ReleaseRequirement {
            id: "release_config.active_release".to_string(),
            level: RequirementLevel::FissionRecommended,
            status: RequirementStatus::Passed,
            summary: "active release is set".to_string(),
            details: None,
            remediation: Vec::new(),
            can_fix_interactively: false,
        }],
    };
    assert!(release_config_ready(&ready));

    let blocked = ReleasePlanReport {
        requirements: vec![ReleaseRequirement {
            id: "release_config.default_locales".to_string(),
            level: RequirementLevel::FissionRecommended,
            status: RequirementStatus::Missing,
            summary: "locales missing".to_string(),
            details: None,
            remediation: Vec::new(),
            can_fix_interactively: true,
        }],
        ..ready
    };
    assert!(!release_config_ready(&blocked));
}

#[test]
fn workflow_events_and_json_redact_secret_env_values() {
    std::env::set_var("FISSION_TEST_CLIENT_SECRET", "workflow-secret-value");
    let mut events = Vec::new();
    record_event(
        &mut events,
        "provider.response",
        "failed",
        Some("provider returned workflow-secret-value".to_string()),
    );
    let mut value = serde_json::json!({
        "stdout": "raw workflow-secret-value",
        "events": events,
    });
    redact_json_value(&mut value);
    std::env::remove_var("FISSION_TEST_CLIENT_SECRET");

    let text = serde_json::to_string(&value).unwrap();
    assert!(!text.contains("workflow-secret-value"));
    assert!(text.contains("<redacted:FISSION_TEST_CLIENT_SECRET>"));
}

#[test]
fn provider_capabilities_report_direct_release_content_support() {
    let app_store = provider_capabilities(DistributionProvider::AppStore);
    let app_assets = app_store
        .iter()
        .find(|capability| capability.id == "provider.capability.release_content_assets")
        .unwrap();
    assert_eq!(app_assets.status, ProviderCapabilityStatus::Supported);
    let app_review = app_store
        .iter()
        .find(|capability| capability.id == "provider.capability.app_review_submission")
        .unwrap();
    assert_eq!(app_review.status, ProviderCapabilityStatus::Supported);
    assert!(app_store.iter().all(|capability| {
        capability.id != "provider.capability.supplemental_release_content_assets"
    }));

    let microsoft_store = provider_capabilities(DistributionProvider::MicrosoftStore);
    let microsoft_assets = microsoft_store
        .iter()
        .find(|capability| capability.id == "provider.capability.release_content_assets")
        .unwrap();
    assert_eq!(microsoft_assets.status, ProviderCapabilityStatus::Supported);
}

#[test]
fn release_plan_snapshot_exposes_context_steps_capabilities_and_requirements() {
    let snapshot = release_plan_snapshot_from_report(ReleasePlanReport {
        context: test_context(),
        project_dir: ".".to_string(),
        provider: "play-store".to_string(),
        target: Some("android".to_string()),
        format: Some("aab".to_string()),
        artifact: Some("target/fission/release/android/aab/artifact-manifest.json".to_string()),
        track: Some("internal".to_string()),
        locales: vec!["en-US".to_string()],
        status: "warning".to_string(),
        steps: vec![ReleaseStep {
            id: "inspect".to_string(),
            title: "Inspect project and release configuration".to_string(),
            status: StepStatus::Warning,
            jobs: vec![ReleaseJob {
                id: "inspect.fission_toml".to_string(),
                title: "Read fission.toml".to_string(),
                status: StepStatus::Ready,
            }],
        }],
        capabilities: vec![capability(
            "artifact_upload",
            ProviderCapabilityStatus::Supported,
            "Android APK/AAB upload through Google Play edits",
            None,
        )],
        requirements: vec![ReleaseRequirement {
            id: "release_content.play_store.feature_graphic".to_string(),
            level: RequirementLevel::FissionRecommended,
            status: RequirementStatus::Missing,
            summary: "feature graphic exists".to_string(),
            details: None,
            remediation: vec!["Add feature graphic.".to_string()],
            can_fix_interactively: true,
        }],
    });

    assert_eq!(snapshot.context.provider, "play-store");
    assert_eq!(snapshot.steps[0].status, "warning");
    assert_eq!(snapshot.steps[0].jobs[0].status, "ready");
    assert_eq!(snapshot.capabilities[0].status, "supported");
    assert_eq!(snapshot.requirements[0].level, "fission-recommended");
    assert_eq!(snapshot.requirements[0].status, "missing");
}

#[test]
fn provider_required_release_assets_are_not_skippable_recommendations() {
    assert_eq!(
        release_content_requirement_level(
            DistributionProvider::AppStore,
            "release_content.app-store.required_assets"
        ),
        RequirementLevel::ProviderRequired
    );
    assert_eq!(
        release_content_requirement_level(
            DistributionProvider::PlayStore,
            "release_content.play_store.feature_graphic"
        ),
        RequirementLevel::FissionRecommended
    );
}

#[test]
fn publish_options_default_from_provider_when_no_artifact_is_supplied() {
    let options = publish_options_with_defaults(PublishWorkflowOptions {
        project_dir: std::path::PathBuf::from("."),
        provider: DistributionProvider::PlayStore,
        target: None,
        format: None,
        artifact: None,
        site: "production".to_string(),
        deploy: None,
        track: None,
        locales: Vec::new(),
        overwrite_remote: false,
        dry_run: true,
        yes: true,
        json: true,
    })
    .unwrap();

    assert_eq!(options.target, Some(Target::Android));
    assert_eq!(options.format, Some(PackageFormat::Aab));
    assert_eq!(options.track.as_deref(), Some("internal"));
}

#[test]
fn release_plan_uses_release_package_profile() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-workflow-package-profile-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
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
"#,
    )
    .unwrap();

    let report = build_release_plan(
        &PublishWorkflowOptions {
            project_dir: dir.clone(),
            provider: DistributionProvider::PlayStore,
            target: Some(Target::Android),
            format: Some(PackageFormat::Aab),
            artifact: None,
            site: "production".to_string(),
            deploy: None,
            track: Some("internal".to_string()),
            locales: Vec::new(),
            overwrite_remote: false,
            dry_run: true,
            yes: true,
            json: true,
        },
        None,
    )
    .unwrap();

    let profile = report
        .requirements
        .iter()
        .find(|req| req.id == "release.package.profile_selected")
        .expect("release plan should include package profile");
    assert_eq!(profile.details.as_deref(), Some("release"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn publish_options_default_from_artifact_manifest_when_available() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-workflow-artifact-defaults-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let artifact = dir.join("artifact-manifest.json");
    fs::write(
        &artifact,
        r#"{
  "project": { "version": "3.0.0", "build": 8 },
  "target": "windows",
  "format": "msix",
  "artifacts": []
}"#,
    )
    .unwrap();

    let options = publish_options_with_defaults(PublishWorkflowOptions {
        project_dir: dir.clone(),
        provider: DistributionProvider::MicrosoftStore,
        target: None,
        format: None,
        artifact: Some(artifact),
        site: "production".to_string(),
        deploy: None,
        track: None,
        locales: Vec::new(),
        overwrite_remote: false,
        dry_run: true,
        yes: true,
        json: true,
    })
    .unwrap();

    assert_eq!(options.target, Some(Target::Windows));
    assert_eq!(options.format, Some(PackageFormat::Msix));
    assert_eq!(options.track.as_deref(), Some("private"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn publish_options_accept_static_site_and_ssr_artifact_targets() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-workflow-target-defaults-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    for (name, target, expected_target, expected_format) in [
        (
            "static-site",
            "static-site",
            Target::Site,
            PackageFormat::Static,
        ),
        ("legacy-site", "site", Target::Site, PackageFormat::Static),
        ("ssr", "ssr", Target::Server, PackageFormat::DockerImage),
        (
            "legacy-server",
            "server",
            Target::Server,
            PackageFormat::DockerImage,
        ),
    ] {
        let artifact = dir.join(format!("{name}.json"));
        let format = expected_format.as_str();
        fs::write(
            &artifact,
            format!(
                r#"{{
  "project": {{ "version": "3.0.0", "build": 8 }},
  "target": "{target}",
  "format": "{format}",
  "artifacts": []
}}"#
            ),
        )
        .unwrap();

        let options = publish_options_with_defaults(PublishWorkflowOptions {
            project_dir: dir.clone(),
            provider: DistributionProvider::DockerRegistry,
            target: None,
            format: None,
            artifact: Some(artifact),
            site: "production".to_string(),
            deploy: None,
            track: None,
            locales: Vec::new(),
            overwrite_remote: false,
            dry_run: true,
            yes: true,
            json: true,
        })
        .unwrap();

        assert_eq!(options.target, Some(expected_target));
        assert_eq!(options.format, Some(expected_format));
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_skip_requirements_only_skips_non_required_items() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-workflow-skip-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[release]
skip_requirements = [
  "release_content.play_store.feature_graphic",
  "release.version.resolved",
  "release_content.stale",
]
"#,
    )
    .unwrap();
    let mut requirements = vec![
        ReleaseRequirement {
            id: "release_content.play_store.feature_graphic".to_string(),
            level: RequirementLevel::FissionRecommended,
            status: RequirementStatus::Missing,
            summary: "feature graphic exists".to_string(),
            details: None,
            remediation: Vec::new(),
            can_fix_interactively: true,
        },
        ReleaseRequirement {
            id: "release.version.resolved".to_string(),
            level: RequirementLevel::ProviderRequired,
            status: RequirementStatus::Missing,
            summary: "version exists".to_string(),
            details: None,
            remediation: Vec::new(),
            can_fix_interactively: true,
        },
    ];

    apply_skipped_requirements(&dir, &mut requirements).unwrap();

    let skipped = requirements
        .iter()
        .find(|req| req.id == "release_content.play_store.feature_graphic")
        .unwrap();
    assert_eq!(skipped.status, RequirementStatus::Skipped);
    let required = requirements
        .iter()
        .find(|req| req.id == "release.version.resolved")
        .unwrap();
    assert_eq!(required.status, RequirementStatus::Missing);
    assert!(requirements
        .iter()
        .any(|req| req.id.starts_with("release.skip.provider_required.")));
    assert!(requirements
        .iter()
        .any(|req| req.id == "release.skip.release_content_stale"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn workflow_receipt_carries_release_artifact_and_omission_summary() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-workflow-receipt-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let artifact = dir.join("artifact-manifest.json");
    fs::write(
        &artifact,
        r#"{
  "project": { "version": "2.0.0", "build": 7 },
  "target": "android",
  "format": "aab",
  "artifacts": [
    { "sha256": "abc123", "path": "app.aab" }
  ]
}"#,
    )
    .unwrap();
    let report = ReleasePlanReport {
        context: test_context(),
        project_dir: dir.display().to_string(),
        provider: "play-store".to_string(),
        target: Some("android".to_string()),
        format: Some("aab".to_string()),
        artifact: Some(artifact.display().to_string()),
        track: Some("internal".to_string()),
        locales: vec!["en-US".to_string()],
        status: "warning".to_string(),
        steps: Vec::new(),
        capabilities: Vec::new(),
        requirements: vec![ReleaseRequirement {
            id: "release_content.play_store.feature_graphic".to_string(),
            level: RequirementLevel::FissionRecommended,
            status: RequirementStatus::Missing,
            summary: "feature graphic exists".to_string(),
            details: None,
            remediation: Vec::new(),
            can_fix_interactively: true,
        }],
    };
    let options = PublishWorkflowOptions {
        project_dir: dir.clone(),
        provider: DistributionProvider::PlayStore,
        target: Some(Target::Android),
        format: Some(PackageFormat::Aab),
        artifact: Some(artifact.clone()),
        site: "production".to_string(),
        deploy: None,
        track: Some("internal".to_string()),
        locales: vec!["en-US".to_string()],
        overwrite_remote: false,
        dry_run: false,
        yes: true,
        json: true,
    };
    let distribution = json!({
        "status": "published",
        "deployment_id": "edit:123/version:7",
        "canonical_url": "https://play.google.com/console",
        "uploaded_assets": [{"path": "app.aab", "sha256": "abc123", "size_bytes": 10}]
    });
    let release_content = json!({
        "status": "pushed",
        "assets": [{"path": "feature.png", "sha256": "content123", "size_bytes": 5}],
        "manual_follow_up": ["Check provider processing state."]
    });
    let provider_status = json!({
        "status": "processing",
        "deployment_id": "edit:123/status"
    });

    let path = write_publish_workflow_receipt(
        &options,
        &report,
        "published",
        None,
        Some(release_content),
        Some(distribution),
        Some(provider_status),
        &[],
    )
    .unwrap();
    let value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();

    assert_eq!(
        value.get("release_id").and_then(Value::as_str),
        Some("2.0.0+7")
    );
    assert_eq!(value.get("version").and_then(Value::as_str), Some("2.0.0"));
    assert_eq!(value.get("build").and_then(Value::as_u64), Some(7));
    assert_eq!(
        value.get("artifact_hash").and_then(Value::as_str),
        Some("abc123")
    );
    assert_eq!(
        value.get("provider_deployment_id").and_then(Value::as_str),
        Some("edit:123/version:7")
    );
    assert_eq!(
        value
            .get("uploaded_assets")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        value.get("uploaded_bytes").and_then(Value::as_u64),
        Some(15)
    );
    assert_eq!(
        value
            .get("release_content_assets")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        value
            .get("manual_follow_up")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        value
            .get("omitted_requirements")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        value
            .get("provider_status")
            .and_then(|status| status.get("status"))
            .and_then(Value::as_str),
        Some("processing")
    );

    let _ = fs::remove_dir_all(&dir);
}
#[test]
fn workflow_receipt_records_failed_distribution_diagnostics() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-workflow-failed-distribution-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let artifact = dir.join("artifact-manifest.json");
    fs::write(
        &artifact,
        r#"{
  "project": { "version": "2.0.0", "build": 7 },
  "target": "android",
  "format": "aab",
  "artifacts": [
    { "sha256": "abc123", "path": "app.aab" }
  ]
}"#,
    )
    .unwrap();
    let report = ReleasePlanReport {
        context: test_context(),
        project_dir: dir.display().to_string(),
        provider: "play-store".to_string(),
        target: Some("android".to_string()),
        format: Some("aab".to_string()),
        artifact: Some(artifact.display().to_string()),
        track: Some("internal".to_string()),
        locales: vec!["en-US".to_string()],
        status: "ready".to_string(),
        steps: Vec::new(),
        capabilities: Vec::new(),
        requirements: Vec::new(),
    };
    let options = PublishWorkflowOptions {
        project_dir: dir.clone(),
        provider: DistributionProvider::PlayStore,
        target: Some(Target::Android),
        format: Some(PackageFormat::Aab),
        artifact: Some(artifact.clone()),
        site: "production".to_string(),
        deploy: None,
        track: Some("internal".to_string()),
        locales: vec!["en-US".to_string()],
        overwrite_remote: false,
        dry_run: false,
        yes: true,
        json: true,
    };
    let events = vec![PublishWorkflowEvent {
        at_unix_seconds: 1,
        id: "workflow.failed".to_string(),
        status: "failed".to_string(),
        details: Some("Google Play artifact upload failed with 403 Forbidden".to_string()),
    }];
    let distribution = json!({
        "provider": "play-store",
        "status": "failed",
        "error": "Google Play artifact upload failed with 403 Forbidden"
    });

    let path = write_publish_workflow_receipt(
        &options,
        &report,
        "failed",
        None,
        None,
        Some(distribution),
        None,
        &events,
    )
    .unwrap();
    let value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();

    assert_eq!(value.get("status").and_then(Value::as_str), Some("failed"));
    assert_eq!(
        value
            .get("distribution")
            .and_then(|distribution| distribution.get("error"))
            .and_then(Value::as_str),
        Some("Google Play artifact upload failed with 403 Forbidden")
    );
    assert_eq!(
        value
            .get("events")
            .and_then(Value::as_array)
            .and_then(|events| events.first())
            .and_then(|event| event.get("id"))
            .and_then(Value::as_str),
        Some("workflow.failed")
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn workflow_failure_helper_writes_stage_receipt() {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-workflow-stage-failure-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let report = ReleasePlanReport {
        context: test_context(),
        project_dir: dir.display().to_string(),
        provider: "play-store".to_string(),
        target: Some("android".to_string()),
        format: Some("aab".to_string()),
        artifact: None,
        track: Some("internal".to_string()),
        locales: vec!["en-US".to_string()],
        status: "ready".to_string(),
        steps: Vec::new(),
        capabilities: Vec::new(),
        requirements: Vec::new(),
    };
    let options = PublishWorkflowOptions {
        project_dir: dir.clone(),
        provider: DistributionProvider::PlayStore,
        target: Some(Target::Android),
        format: Some(PackageFormat::Aab),
        artifact: None,
        site: "production".to_string(),
        deploy: None,
        track: Some("internal".to_string()),
        locales: vec!["en-US".to_string()],
        overwrite_remote: false,
        dry_run: false,
        yes: true,
        json: true,
    };
    let metadata = json!({
        "provider": "play-store",
        "status": "failed",
        "stage": "release.metadata",
        "error": "remote metadata changed"
    });
    let mut events = Vec::new();

    let error = fail_publish_workflow(
        &options,
        &report,
        &mut events,
        "release.metadata",
        "remote metadata changed".to_string(),
        Some(metadata),
        None,
        None,
        None,
    )
    .unwrap_err();

    assert!(error.to_string().contains("workflow receipt"));
    let receipt_dir = dir.join("target/fission/release-workflows/publish/play-store");
    let receipt_path = fs::read_dir(receipt_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let value: Value = serde_json::from_slice(&fs::read(receipt_path).unwrap()).unwrap();

    assert_eq!(value.get("status").and_then(Value::as_str), Some("failed"));
    assert_eq!(
        value
            .get("release_metadata")
            .and_then(|metadata| metadata.get("error"))
            .and_then(Value::as_str),
        Some("remote metadata changed")
    );
    let event_ids = value
        .get("events")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|event| event.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert_eq!(event_ids, vec!["release.metadata", "workflow.failed"]);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn publish_workflow_receipts_do_not_overwrite_same_second_runs() {
    let dir = std::env::temp_dir().join(format!(
        "fission-publish-workflow-unique-receipts-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let report = ReleasePlanReport {
        context: test_context(),
        project_dir: dir.display().to_string(),
        provider: "play-store".to_string(),
        target: Some("android".to_string()),
        format: Some("aab".to_string()),
        artifact: None,
        track: Some("internal".to_string()),
        locales: vec!["en-US".to_string()],
        status: "blocked".to_string(),
        steps: Vec::new(),
        capabilities: Vec::new(),
        requirements: Vec::new(),
    };
    let options = PublishWorkflowOptions {
        project_dir: dir.clone(),
        provider: DistributionProvider::PlayStore,
        target: Some(Target::Android),
        format: Some(PackageFormat::Aab),
        artifact: None,
        site: "production".to_string(),
        deploy: None,
        track: Some("internal".to_string()),
        locales: vec!["en-US".to_string()],
        overwrite_remote: false,
        dry_run: true,
        yes: false,
        json: true,
    };

    let first =
        write_publish_workflow_receipt(&options, &report, "blocked", None, None, None, None, &[])
            .unwrap();
    let second =
        write_publish_workflow_receipt(&options, &report, "blocked", None, None, None, None, &[])
            .unwrap();

    assert_ne!(first, second);
    let receipt_count =
        fs::read_dir(dir.join("target/fission/release-workflows/publish/play-store"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(OsStr::to_str) == Some("json"))
            .count();
    assert_eq!(receipt_count, 2);

    let _ = fs::remove_dir_all(&dir);
}
