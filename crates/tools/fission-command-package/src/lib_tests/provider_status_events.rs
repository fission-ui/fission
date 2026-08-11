use super::support::{unique_dir, write_minimal_site};
use super::*;

#[test]
fn app_store_status_prefers_review_submission_state_over_build_processing() {
    let status = stores::app_store_observed_status(
        &json!({
            "data": [{
                "attributes": { "processingState": "VALID" }
            }]
        }),
        &json!({
            "data": [{
                "attributes": { "state": "WAITING_FOR_REVIEW" }
            }]
        }),
        &json!({ "data": [] }),
    );
    assert_eq!(status, "waiting_for_review");
}

#[test]
fn app_store_status_queries_the_sortable_global_builds_endpoint() {
    let url = stores::app_store_builds_status_url("6794005791");

    assert!(url.contains("/v1/builds?filter[app]=6794005791"));
    assert!(url.contains("sort=-uploadedDate"));
    assert!(!url.contains("/v1/apps/6794005791/builds"));
}

#[test]
fn app_store_status_reports_build_upload_failures_before_a_build_exists() {
    let status = stores::app_store_observed_status(
        &json!({ "data": [] }),
        &json!({ "data": [] }),
        &json!({
            "data": [{
                "attributes": {
                    "state": {
                        "state": "FAILED",
                        "errors": [{ "code": "91109" }]
                    }
                }
            }]
        }),
    );

    assert_eq!(status, "failed");
}

#[test]
fn app_store_build_upload_status_uses_the_app_relationship_endpoint() {
    let url = stores::app_store_build_uploads_status_url("6794005791");

    assert!(url.contains("/v1/apps/6794005791/buildUploads"));
    assert!(url.contains("sort=-uploadedDate"));
    assert!(url.contains("fields[buildUploads]"));
}

#[test]
fn app_store_upload_follow_up_points_to_review_submission_command() {
    let msg = stores::app_store_upload_follow_up(
        "app-store-review",
        Path::new("target/fission/release/ios/ipa/artifact-manifest.json"),
    );
    assert!(
        msg.contains("fission distribute promote --provider app-store --track app-store-review")
    );
}

#[test]
fn provider_stdio_line_events_are_bounded_and_structured() {
    let mut events = Vec::new();
    push_provider_stdio_line_events(&mut events, "provider.stdout.line", "one\n\ntwo\n");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].id, "provider.stdout.line");
    assert_eq!(events[0].details.as_deref(), Some("one"));
    assert_eq!(events[1].details.as_deref(), Some("two"));
}

#[test]
fn distribute_publish_outcome_records_structured_events() {
    let dir = unique_dir("publish-events");
    write_minimal_site(&dir);
    let artifact = package_silent(PackageOptions {
        project_dir: dir.clone(),
        target: Target::Site,
        format: PackageFormat::Static,
        release: true,
        variant: None,
        json: false,
    })
    .unwrap();
    let config = load_publish_manifest(&dir).unwrap();
    setup_github_pages(
        &DistributeOptions {
            project_dir: dir.clone(),
            provider: DistributionProvider::GithubPages,
            action: DistributeAction::Setup,
            target: None,
            format: None,
            artifact: None,
            site: "production".to_string(),
            deploy: None,
            track: None,
            locales: Vec::new(),
            dry_run: false,
            yes: true,
            json: true,
        },
        &config,
    )
    .unwrap();

    let outcome = distribute_publish_outcome(DistributeOptions {
        project_dir: dir,
        provider: DistributionProvider::GithubPages,
        action: DistributeAction::Publish,
        target: Some(Target::Site),
        format: Some(PackageFormat::Static),
        artifact: Some(artifact),
        site: "production".to_string(),
        deploy: None,
        track: None,
        locales: Vec::new(),
        dry_run: true,
        yes: true,
        json: true,
    })
    .unwrap();

    let event_ids = outcome
        .events
        .iter()
        .map(|event| event.id.as_str())
        .collect::<Vec<_>>();
    assert!(event_ids.contains(&"distribution.config"));
    assert!(event_ids.contains(&"distribution.artifact_manifest"));
    assert!(event_ids.contains(&"distribution.readiness"));
    assert!(event_ids.contains(&"provider.publish"));
    assert!(event_ids.contains(&"provider.request"));
    assert!(event_ids.contains(&"provider.response"));
    assert!(event_ids.contains(&"provider.upload_plan"));
    assert!(event_ids.contains(&"provider.uploaded_asset"));
    assert!(event_ids.contains(&"distribution.receipt"));
    let receipt_event_ids = outcome.receipt["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|event| event.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(receipt_event_ids.contains(&"provider.request"));
    assert!(receipt_event_ids.contains(&"provider.response"));
    assert!(receipt_event_ids.contains(&"provider.uploaded_asset"));
    assert!(receipt_event_ids.contains(&"distribution.receipt"));
    assert_eq!(outcome.receipt["status"], "workflow-required");
}

#[test]
fn distribute_setup_writes_action_receipt() {
    let dir = unique_dir("setup-receipt");
    write_minimal_site(&dir);

    distribute(DistributeOptions {
        project_dir: dir.clone(),
        provider: DistributionProvider::GithubPages,
        action: DistributeAction::Setup,
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
    })
    .unwrap();

    let receipt_dir = dir.join("target/fission/distribution/github-pages/production");
    let receipt_path = fs::read_dir(&receipt_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("setup-") && name.ends_with(".json"))
        })
        .unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(receipt_path).unwrap()).unwrap();

    assert_eq!(
        value.get("status").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(value.get("action").and_then(Value::as_str), Some("setup"));
    let event_ids = value
        .get("events")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|event| event.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(event_ids.contains(&"distribution.config"));
    assert!(event_ids.contains(&"distribution.setup"));
    assert!(event_ids.contains(&"distribution.receipt"));

    let _ = fs::remove_dir_all(&dir);
}
