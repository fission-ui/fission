use super::support::unique_dir;
use super::*;

#[test]
fn report_status_keeps_recommended_missing_checks_visible() {
    let checks = vec![ReadinessCheck {
        id: "release_content.play_store.feature_graphic".to_string(),
        severity: CheckSeverity::Warning,
        status: CheckStatus::Missing,
        summary: "feature graphic exists".to_string(),
        details: None,
        remediation: Vec::new(),
    }];

    assert_eq!(report_status(&checks), "warning");
}

#[test]
fn failed_distribution_receipt_records_stage_error() {
    let dir = unique_dir("failed-distribution-receipt");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
name = "site-demo"
app_id = "com.example.site_demo"
version = "1.2.3"
build = 4
"#,
    )
    .unwrap();
    let options = DistributeOptions {
        project_dir: dir.clone(),
        provider: DistributionProvider::S3,
        action: DistributeAction::Publish,
        target: Some(Target::Site),
        format: Some(PackageFormat::Static),
        artifact: None,
        site: "production".to_string(),
        deploy: None,
        track: None,
        locales: vec!["en-US".to_string()],
        dry_run: false,
        yes: true,
        json: true,
    };
    let mut events = Vec::new();

    let path = write_failed_distribution_receipt(
        &options,
        &mut events,
        "distribution.readiness",
        "S3 bucket is missing",
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();

    assert_eq!(value.get("status").and_then(Value::as_str), Some("failed"));
    assert_eq!(value.get("provider").and_then(Value::as_str), Some("s3"));
    assert_eq!(
        value.get("stderr").and_then(Value::as_str),
        Some("S3 bucket is missing")
    );
    let event_ids = value
        .get("events")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|event| event.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert_eq!(
        event_ids,
        vec![
            "distribution.readiness",
            "distribution.failed",
            "distribution.receipt"
        ]
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn distribution_receipt_paths_do_not_overwrite_same_second_receipts() {
    let dir = unique_dir("receipt-path-unique");
    let receipt = DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: 123,
        provider: "s3".to_string(),
        site: "production".to_string(),
        action: "publish".to_string(),
        artifact_manifest: None,
        deployment_id: None,
        canonical_url: None,
        preview_url: None,
        custom_domain: None,
        status: "published".to_string(),
        stdout: None,
        stderr: None,
        manual_follow_up: Vec::new(),
    };
    let first = receipt_output_path(&dir, &receipt);
    fs::create_dir_all(first.parent().unwrap()).unwrap();
    fs::write(&first, "{}").unwrap();

    let second = receipt_output_path(&dir, &receipt);

    assert_ne!(first, second);
    assert!(second
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "publish-123-2.json"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn distribute_status_failure_writes_failed_receipt() {
    let dir = unique_dir("failed-distribution-status");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
name = "site-demo"
app_id = "com.example.site_demo"
version = "1.2.3"
build = 4
"#,
    )
    .unwrap();
    let options = DistributeOptions {
        project_dir: dir.clone(),
        provider: DistributionProvider::S3,
        action: DistributeAction::Status,
        target: Some(Target::Site),
        format: Some(PackageFormat::Static),
        artifact: None,
        site: "production".to_string(),
        deploy: None,
        track: None,
        locales: vec!["en-US".to_string()],
        dry_run: false,
        yes: true,
        json: true,
    };

    let error = distribute_status_outcome(options).unwrap_err().to_string();

    assert!(error.contains("distribution status failed"));
    assert!(error.contains("distribution receipt:"));
    let receipt_dir = dir.join("target/fission/distribution/s3/production");
    let receipt_path = fs::read_dir(&receipt_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("status-") && name.ends_with(".json"))
        })
        .unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(receipt_path).unwrap()).unwrap();

    assert_eq!(value.get("status").and_then(Value::as_str), Some("failed"));
    assert_eq!(value.get("action").and_then(Value::as_str), Some("status"));
    let event_ids = value
        .get("events")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|event| event.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(event_ids.contains(&"distribution.config"));
    assert!(event_ids.contains(&"provider.status"));
    assert!(event_ids.contains(&"distribution.failed"));
    assert!(event_ids.contains(&"distribution.receipt"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn distribute_lifecycle_failure_writes_failed_receipt() {
    let dir = unique_dir("failed-distribution-lifecycle");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
name = "site-demo"
app_id = "com.example.site_demo"
version = "1.2.3"
build = 4
"#,
    )
    .unwrap();
    let options = DistributeOptions {
        project_dir: dir.clone(),
        provider: DistributionProvider::S3,
        action: DistributeAction::Promote,
        target: Some(Target::Site),
        format: Some(PackageFormat::Static),
        artifact: None,
        site: "production".to_string(),
        deploy: None,
        track: None,
        locales: vec!["en-US".to_string()],
        dry_run: false,
        yes: true,
        json: true,
    };

    let error = distribute(options).unwrap_err().to_string();

    assert!(error.contains("promote failed"));
    assert!(error.contains("distribution receipt:"));
    let receipt_dir = dir.join("target/fission/distribution/s3/production");
    let receipt_path = fs::read_dir(&receipt_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("promote-") && name.ends_with(".json"))
        })
        .unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(receipt_path).unwrap()).unwrap();

    assert_eq!(value.get("status").and_then(Value::as_str), Some("failed"));
    assert_eq!(value.get("action").and_then(Value::as_str), Some("promote"));
    let event_ids = value
        .get("events")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|event| event.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(event_ids.contains(&"distribution.config"));
    assert!(event_ids.contains(&"provider.lifecycle"));
    assert!(event_ids.contains(&"distribution.failed"));

    let _ = fs::remove_dir_all(&dir);
}
