use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct AppStoreReviewBuild {
    pub id: String,
    pub version: String,
    pub processing_state: Option<String>,
    pub uploaded_date: Option<String>,
    pub expired: Option<bool>,
}

pub(super) fn lifecycle(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    if options.action != DistributeAction::Promote {
        bail!(
            "app-store currently supports App Review submission through `fission distribute promote --provider app-store --track app-store-review --artifact <manifest>`"
        );
    }
    let cfg = app_store_config(config);
    let track = options
        .track
        .as_deref()
        .or(cfg.default_track.as_deref())
        .unwrap_or("app-store-review");
    if track != "app-store-review" {
        bail!("App Store lifecycle promote requires --track app-store-review");
    }
    let artifact_path = options
        .artifact
        .as_deref()
        .context("App Store App Review submission requires --artifact <artifact-manifest.json>")?;
    let manifest = read_artifact_manifest(artifact_path)?;
    let version = manifest
        .project
        .version
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .context("artifact manifest project.version is required before App Review submission")?;
    let build_number = manifest
        .project
        .build
        .map(|value| value.to_string())
        .context("artifact manifest project.build is required before App Review submission")?;

    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let version_id = resolve_app_store_version_id(&client, &token, &app_id, version)?;
    let build = resolve_app_store_review_build(&client, &token, &app_id, &build_number)?;
    let attach_payload = app_store_version_build_payload(&version_id, &build.id);
    let create_payload = app_store_review_submission_create_payload(&app_id, "IOS");

    if options.dry_run {
        let stdout = serde_json::to_string_pretty(&json!({
            "provider": "app-store",
            "app_id": app_id,
            "version": version,
            "version_id": version_id,
            "build": build,
            "attach_build_payload": attach_payload,
            "create_review_submission_payload": create_payload,
            "status": "dry-run"
        }))?;
        return Ok(DistributionReceipt {
            schema_version: 1,
            created_at_unix_seconds: now_unix_seconds(),
            provider: "app-store".to_string(),
            site: options.site.clone(),
            action: "promote".to_string(),
            artifact_manifest: Some(artifact_path.display().to_string()),
            deployment_id: Some(format!("app:{app_id}/version:{version_id}/build:{}", build.id)),
            canonical_url: Some("https://appstoreconnect.apple.com/apps".to_string()),
            preview_url: None,
            custom_domain: None,
            status: "dry-run".to_string(),
            stdout: Some(stdout),
            stderr: None,
            manual_follow_up: vec![format!(
                "Would attach build {} to App Store version {version} and submit it for App Review.",
                build.version
            )],
        });
    }

    ensure_review_build_assignable(&build)?;
    let attach_response = client
        .patch(format!("{APP_STORE_API}/v1/appStoreVersions/{version_id}"))
        .bearer_auth(&token)
        .json(&attach_payload)
        .send()
        .context("failed to attach App Store build to version")?;
    let attach_value = json_response(attach_response, "App Store version build assignment")?;

    let review_submission_response = client
        .post(format!("{APP_STORE_API}/v1/reviewSubmissions"))
        .bearer_auth(&token)
        .json(&create_payload)
        .send()
        .context("failed to create App Store review submission")?;
    let review_submission = json_response(
        review_submission_response,
        "App Store review submission create",
    )?;
    let submission_id = json_id(&review_submission, "App Store review submission")?;
    let item_payload = app_store_review_submission_item_payload(&submission_id, &version_id);
    let item_response = client
        .post(format!("{APP_STORE_API}/v1/reviewSubmissionItems"))
        .bearer_auth(&token)
        .json(&item_payload)
        .send()
        .context("failed to add App Store version to review submission")?;
    let item_value = json_response(item_response, "App Store review submission item create")?;
    let submit_payload = app_store_review_submission_submit_payload(&submission_id);
    let submit_response = client
        .patch(format!(
            "{APP_STORE_API}/v1/reviewSubmissions/{submission_id}"
        ))
        .bearer_auth(&token)
        .json(&submit_payload)
        .send()
        .context("failed to submit App Store review submission")?;
    let submit_value = json_response(submit_response, "App Store review submission submit")?;
    let status = submit_value
        .pointer("/data/attributes/state")
        .and_then(Value::as_str)
        .unwrap_or("submitted-for-review")
        .to_ascii_lowercase();

    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "app-store".to_string(),
        site: options.site.clone(),
        action: "promote".to_string(),
        artifact_manifest: Some(artifact_path.display().to_string()),
        deployment_id: Some(submission_id),
        canonical_url: Some("https://appstoreconnect.apple.com/apps".to_string()),
        preview_url: None,
        custom_domain: None,
        status,
        stdout: Some(serde_json::to_string_pretty(&json!({
            "attach_build": attach_value,
            "review_submission": review_submission,
            "review_submission_item": item_value,
            "submit": submit_value,
        }))?),
        stderr: None,
        manual_follow_up: vec![
            "App Store version was submitted for App Review; monitor review status in App Store Connect or with `fission distribute status --provider app-store`.".to_string(),
        ],
    })
}

fn resolve_app_store_version_id(
    client: &Client,
    token: &str,
    app_id: &str,
    version: &str,
) -> Result<String> {
    let url = format!(
        "{APP_STORE_API}/v1/apps/{app_id}/appStoreVersions?filter[versionString]={}&filter[platform]=IOS&limit=1&fields[appStoreVersions]=versionString,appStoreState,platform",
        encode_query_component(version)
    );
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .context("failed to resolve App Store version for review submission")?;
    let value = json_response(response, "App Store version lookup")?;
    value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .with_context(|| format!("App Store version {version} was not found for app {app_id}"))
}

fn resolve_app_store_review_build(
    client: &Client,
    token: &str,
    app_id: &str,
    build_number: &str,
) -> Result<AppStoreReviewBuild> {
    let url = format!(
        "{APP_STORE_API}/v1/apps/{app_id}/builds?filter[version]={}&limit=1&fields[builds]=version,processingState,uploadedDate,expired",
        encode_query_component(build_number)
    );
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .context("failed to resolve App Store build for review submission")?;
    let value = json_response(response, "App Store build lookup")?;
    app_store_review_build_from_response(&value)
        .with_context(|| format!("App Store build {build_number} was not found for app {app_id}"))
}

fn app_store_review_build_from_response(value: &Value) -> Option<AppStoreReviewBuild> {
    let item = value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())?;
    let attrs = item.get("attributes").unwrap_or(&Value::Null);
    Some(AppStoreReviewBuild {
        id: item.get("id")?.as_str()?.to_string(),
        version: attrs.get("version")?.as_str()?.to_string(),
        processing_state: attrs
            .get("processingState")
            .and_then(Value::as_str)
            .map(str::to_string),
        uploaded_date: attrs
            .get("uploadedDate")
            .and_then(Value::as_str)
            .map(str::to_string),
        expired: attrs.get("expired").and_then(Value::as_bool),
    })
}

fn ensure_review_build_assignable(build: &AppStoreReviewBuild) -> Result<()> {
    if build.expired == Some(true) {
        bail!(
            "App Store build {} is expired and cannot be submitted for review",
            build.version
        );
    }
    if build
        .processing_state
        .as_deref()
        .is_some_and(|state| state != "VALID")
    {
        bail!(
            "App Store build {} is not ready for review submission; processingState={}",
            build.version,
            build.processing_state.as_deref().unwrap_or("<unknown>")
        );
    }
    Ok(())
}

pub(super) fn app_store_version_build_payload(version_id: &str, build_id: &str) -> Value {
    json!({
        "data": {
            "type": "appStoreVersions",
            "id": version_id,
            "relationships": {
                "build": {
                    "data": {
                        "type": "builds",
                        "id": build_id
                    }
                }
            }
        }
    })
}

pub(super) fn app_store_review_submission_create_payload(app_id: &str, platform: &str) -> Value {
    json!({
        "data": {
            "type": "reviewSubmissions",
            "attributes": {
                "platform": platform
            },
            "relationships": {
                "app": {
                    "data": {
                        "type": "apps",
                        "id": app_id
                    }
                }
            }
        }
    })
}

pub(super) fn app_store_review_submission_item_payload(
    submission_id: &str,
    version_id: &str,
) -> Value {
    json!({
        "data": {
            "type": "reviewSubmissionItems",
            "relationships": {
                "reviewSubmission": {
                    "data": {
                        "type": "reviewSubmissions",
                        "id": submission_id
                    }
                },
                "appStoreVersion": {
                    "data": {
                        "type": "appStoreVersions",
                        "id": version_id
                    }
                }
            }
        }
    })
}

pub(super) fn app_store_review_submission_submit_payload(submission_id: &str) -> Value {
    json!({
        "data": {
            "type": "reviewSubmissions",
            "id": submission_id,
            "attributes": {
                "submitted": true
            }
        }
    })
}

fn json_id(value: &Value, context: &str) -> Result<String> {
    value
        .pointer("/data/id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .with_context(|| format!("{context} response did not include data.id"))
}

fn encode_query_component(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod app_store_review_tests {
    use super::*;

    #[test]
    fn app_store_review_payloads_attach_version_and_submit() {
        assert_eq!(
            app_store_version_build_payload("version-1", "build-1"),
            json!({
                "data": {
                    "type": "appStoreVersions",
                    "id": "version-1",
                    "relationships": {
                        "build": { "data": { "type": "builds", "id": "build-1" } }
                    }
                }
            })
        );
        assert_eq!(
            app_store_review_submission_create_payload("app-1", "IOS"),
            json!({
                "data": {
                    "type": "reviewSubmissions",
                    "attributes": { "platform": "IOS" },
                    "relationships": { "app": { "data": { "type": "apps", "id": "app-1" } } }
                }
            })
        );
        assert_eq!(
            app_store_review_submission_item_payload("submission-1", "version-1"),
            json!({
                "data": {
                    "type": "reviewSubmissionItems",
                    "relationships": {
                        "reviewSubmission": { "data": { "type": "reviewSubmissions", "id": "submission-1" } },
                        "appStoreVersion": { "data": { "type": "appStoreVersions", "id": "version-1" } }
                    }
                }
            })
        );
        assert_eq!(
            app_store_review_submission_submit_payload("submission-1"),
            json!({
                "data": {
                    "type": "reviewSubmissions",
                    "id": "submission-1",
                    "attributes": { "submitted": true }
                }
            })
        );
    }

    #[test]
    fn app_store_review_build_from_response_keeps_processing_state() {
        let build = app_store_review_build_from_response(&json!({
            "data": [{
                "id": "build-1",
                "attributes": {
                    "version": "42",
                    "processingState": "VALID",
                    "uploadedDate": "2026-07-10T00:00:00Z",
                    "expired": false
                }
            }]
        }))
        .unwrap();
        assert_eq!(build.id, "build-1");
        assert_eq!(build.version, "42");
        assert_eq!(build.processing_state.as_deref(), Some("VALID"));
        assert_eq!(build.expired, Some(false));
    }

    #[test]
    fn non_valid_review_build_is_rejected() {
        let build = AppStoreReviewBuild {
            id: "build-1".to_string(),
            version: "42".to_string(),
            processing_state: Some("PROCESSING".to_string()),
            uploaded_date: None,
            expired: None,
        };
        assert!(ensure_review_build_assignable(&build).is_err());
    }
}
