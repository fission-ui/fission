use super::*;
use anyhow::{bail, Context, Result};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::ObjectCannedAcl;
use reqwest::blocking::{Body, Client};
use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, LOCATION};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const DROPBOX_SIMPLE_UPLOAD_LIMIT: u64 = 150 * 1024 * 1024;
const DROPBOX_CHUNK_SIZE: usize = 8 * 1024 * 1024;
const ONEDRIVE_CHUNK_SIZE: usize = 10 * 1024 * 1024;

struct UploadItem {
    path: PathBuf,
    relative_path: String,
    mime_type: String,
}

struct UploadedFile {
    relative_path: String,
    provider_id: Option<String>,
    url: Option<String>,
}

pub(super) fn publish_s3(
    options: &DistributeOptions,
    config: &PublishManifest,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
    events: &mut Vec<DistributionEvent>,
) -> Result<DistributionReceipt> {
    let cfg = s3_config(config, &options.site)?;
    if options.dry_run {
        let uploaded = s3_upload_plan(&cfg, manifest, artifact_path)?;
        record_upload_plan_events(events, "s3", manifest, artifact_path, &uploaded)?;
        let canonical_url =
            s3_canonical_url(&cfg, uploaded.first().and_then(|file| file.url.as_deref()));
        return Ok(upload_receipt(
            options,
            artifact_path,
            "s3",
            "dry-run",
            canonical_url,
            uploaded,
        ));
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to create S3 upload runtime")?;
    let uploaded = rt.block_on(upload_s3(&cfg, manifest, artifact_path, events))?;
    let canonical_url =
        s3_canonical_url(&cfg, uploaded.first().and_then(|file| file.url.as_deref()));
    Ok(upload_receipt(
        options,
        artifact_path,
        "s3",
        "published",
        canonical_url,
        uploaded,
    ))
}

pub(super) fn publish_google_drive(
    options: &DistributeOptions,
    config: &PublishManifest,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
    events: &mut Vec<DistributionEvent>,
) -> Result<DistributionReceipt> {
    let cfg = google_drive_config(config, &options.site)?;
    if options.dry_run {
        let uploaded: Vec<_> = upload_items(manifest, artifact_path)?
            .into_iter()
            .map(|item| UploadedFile {
                relative_path: item.relative_path.clone(),
                provider_id: Some(drive_name(cfg.name_prefix.as_deref(), &item.relative_path)),
                url: None,
            })
            .collect();
        record_upload_plan_events(events, "google-drive", manifest, artifact_path, &uploaded)?;
        return Ok(upload_receipt(
            options,
            artifact_path,
            "google-drive",
            "dry-run",
            None,
            uploaded,
        ));
    }
    let token = env_secret(&["GOOGLE_DRIVE_ACCESS_TOKEN"])?
        .context("Google Drive upload requires GOOGLE_DRIVE_ACCESS_TOKEN")?;
    let client = Client::new();
    let mut uploaded = Vec::new();
    for item in upload_items(manifest, artifact_path)? {
        uploaded.push(upload_google_drive_item(
            &client, &token, &cfg, &item, events,
        )?);
    }
    Ok(upload_receipt(
        options,
        artifact_path,
        "google-drive",
        "published",
        uploaded.iter().find_map(|file| file.url.clone()),
        uploaded,
    ))
}

pub(super) fn publish_onedrive(
    options: &DistributeOptions,
    config: &PublishManifest,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
    events: &mut Vec<DistributionEvent>,
) -> Result<DistributionReceipt> {
    let cfg = onedrive_config(config, &options.site)?;
    if options.dry_run {
        let uploaded: Vec<_> = upload_items(manifest, artifact_path)?
            .into_iter()
            .map(|item| UploadedFile {
                relative_path: item.relative_path.clone(),
                provider_id: Some(joined_remote_path(
                    cfg.path_prefix.as_deref(),
                    &item.relative_path,
                )),
                url: None,
            })
            .collect();
        record_upload_plan_events(events, "onedrive", manifest, artifact_path, &uploaded)?;
        return Ok(upload_receipt(
            options,
            artifact_path,
            "onedrive",
            "dry-run",
            None,
            uploaded,
        ));
    }
    let token = env_secret(&["ONEDRIVE_ACCESS_TOKEN"])?
        .context("OneDrive upload requires ONEDRIVE_ACCESS_TOKEN")?;
    let client = Client::new();
    let mut uploaded = Vec::new();
    for item in upload_items(manifest, artifact_path)? {
        uploaded.push(upload_onedrive_item(&client, &token, &cfg, &item, events)?);
    }
    Ok(upload_receipt(
        options,
        artifact_path,
        "onedrive",
        "published",
        uploaded.iter().find_map(|file| file.url.clone()),
        uploaded,
    ))
}

pub(super) fn publish_dropbox(
    options: &DistributeOptions,
    config: &PublishManifest,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
    events: &mut Vec<DistributionEvent>,
) -> Result<DistributionReceipt> {
    let cfg = dropbox_config(config, &options.site)?;
    if options.dry_run {
        let uploaded: Vec<_> = upload_items(manifest, artifact_path)?
            .into_iter()
            .map(|item| UploadedFile {
                relative_path: item.relative_path.clone(),
                provider_id: Some(joined_remote_path(
                    cfg.path_prefix.as_deref(),
                    &item.relative_path,
                )),
                url: None,
            })
            .collect();
        record_upload_plan_events(events, "dropbox", manifest, artifact_path, &uploaded)?;
        return Ok(upload_receipt(
            options,
            artifact_path,
            "dropbox",
            "dry-run",
            None,
            uploaded,
        ));
    }
    let token = env_secret(&["DROPBOX_ACCESS_TOKEN"])?
        .context("Dropbox upload requires DROPBOX_ACCESS_TOKEN")?;
    let client = Client::new();
    let mut uploaded = Vec::new();
    for item in upload_items(manifest, artifact_path)? {
        uploaded.push(upload_dropbox_item(&client, &token, &cfg, &item, events)?);
    }
    Ok(upload_receipt(
        options,
        artifact_path,
        "dropbox",
        "published",
        uploaded.iter().find_map(|file| file.url.clone()),
        uploaded,
    ))
}

pub(super) fn s3_status(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = s3_config(config, &options.site)?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to create S3 status runtime")?;
    let value = rt.block_on(s3_status_value(&cfg, options.artifact.as_deref()))?;
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("ok")
        .to_string();
    let mut receipt =
        file_status_receipt(options, "s3", &status, s3_canonical_url(&cfg, None), value);
    receipt.artifact_manifest = options
        .artifact
        .as_ref()
        .map(|path| path.display().to_string());
    Ok(receipt)
}

pub(super) fn google_drive_status(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = google_drive_config(config, &options.site)?;
    let token = env_secret(&["GOOGLE_DRIVE_ACCESS_TOKEN"])?
        .context("Google Drive status requires GOOGLE_DRIVE_ACCESS_TOKEN")?;
    let client = Client::new();
    let value = if let Some(folder_id) = cfg.folder_id.as_deref().filter(|value| !value.is_empty())
    {
        let response = client
            .get(format!("https://www.googleapis.com/drive/v3/files/{folder_id}?fields=id,name,webViewLink,capabilities"))
            .bearer_auth(token.trim())
            .send()
            .context("failed to query Google Drive folder")?;
        json_http_response(response, "Google Drive folder status")?
    } else {
        let response = client
            .get("https://www.googleapis.com/drive/v3/about?fields=user,storageQuota")
            .bearer_auth(token.trim())
            .send()
            .context("failed to query Google Drive account")?;
        json_http_response(response, "Google Drive account status")?
    };
    Ok(file_status_receipt(
        options,
        "google-drive",
        "ok",
        value
            .get("webViewLink")
            .and_then(Value::as_str)
            .map(str::to_string),
        value,
    ))
}

pub(super) fn onedrive_status(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = onedrive_config(config, &options.site)?;
    let token = env_secret(&["ONEDRIVE_ACCESS_TOKEN"])?
        .context("OneDrive status requires ONEDRIVE_ACCESS_TOKEN")?;
    let root = cfg
        .root
        .as_deref()
        .unwrap_or("me/drive/root")
        .trim_matches('/');
    let url = if let Some(prefix) = cfg
        .path_prefix
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        format!(
            "https://graph.microsoft.com/v1.0/{root}:/{}/",
            encode_path(prefix.trim_matches('/'))
        )
    } else {
        format!("https://graph.microsoft.com/v1.0/{root}")
    };
    let response = Client::new()
        .get(url)
        .bearer_auth(token.trim())
        .send()
        .context("failed to query OneDrive destination")?;
    let value = json_http_response(response, "OneDrive destination status")?;
    Ok(file_status_receipt(
        options,
        "onedrive",
        "ok",
        value
            .get("webUrl")
            .and_then(Value::as_str)
            .map(str::to_string),
        value,
    ))
}

pub(super) fn dropbox_status(
    options: &DistributeOptions,
    _config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let token = env_secret(&["DROPBOX_ACCESS_TOKEN"])?
        .context("Dropbox status requires DROPBOX_ACCESS_TOKEN")?;
    let response = Client::new()
        .post("https://api.dropboxapi.com/2/users/get_current_account")
        .bearer_auth(token.trim())
        .send()
        .context("failed to query Dropbox account")?;
    let value = json_http_response(response, "Dropbox account status")?;
    Ok(file_status_receipt(options, "dropbox", "ok", None, value))
}

pub(super) fn readiness_s3(
    site: &str,
    config: &PublishManifest,
    checks: &mut Vec<ReadinessCheck>,
) -> Result<()> {
    let cfg = s3_config(config, site)?;
    checks.push(required_value(
        "release.s3.bucket_configured",
        cfg.bucket.as_deref(),
        "S3 bucket is configured",
        "Set distribution.s3.<site>.bucket.",
    ));
    checks.push(s3_credentials_check());
    checks.push(check(
        "release.s3.overwrite_policy_explicit",
        CheckSeverity::Warning,
        if cfg.overwrite.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        "S3 overwrite policy is explicit",
        Some(format!(
            "overwrite = {}",
            cfg.overwrite
                .map(|value| value.to_string())
                .unwrap_or_else(|| "<missing; defaults to false>".to_string())
        )),
        vec![
            "Set distribution.s3.<site>.overwrite = false to fail safely on existing objects, or true to replace them intentionally.",
        ],
    ));
    checks.push(check(
        "release.s3.direct_rust_backend",
        CheckSeverity::Info,
        CheckStatus::Passed,
        "S3 upload uses the Rust AWS SDK backend",
        Some(format!(
            "endpoint = {}, path_style = {}, visibility = {}, overwrite = {}, cache_control = {}, presign_ttl_seconds = {}",
            cfg.endpoint.as_deref().unwrap_or("<provider default>"),
            cfg.path_style.unwrap_or(false),
            cfg.visibility.as_deref().unwrap_or("private"),
            cfg.overwrite.unwrap_or(false),
            cfg.cache_control.as_deref().unwrap_or("<provider default>"),
            cfg.presign_ttl_seconds
                .map(|value| value.to_string())
                .unwrap_or_else(|| "<none>".to_string())
        )),
        Vec::new(),
    ));
    Ok(())
}

pub(super) fn readiness_google_drive(
    site: &str,
    config: &PublishManifest,
    checks: &mut Vec<ReadinessCheck>,
) -> Result<()> {
    let cfg = google_drive_config(config, site)?;
    checks.push(secret_check(
        "release.google_drive.token_available",
        &["GOOGLE_DRIVE_ACCESS_TOKEN"],
        "Google Drive OAuth token is available",
        "Set GOOGLE_DRIVE_ACCESS_TOKEN from your shell or CI secret store.",
    ));
    checks.push(check(
        "release.google_drive.folder_selected",
        CheckSeverity::Info,
        CheckStatus::Passed,
        "Google Drive folder destination is selected",
        Some(
            cfg.folder_id
                .unwrap_or_else(|| "root drive folder".to_string()),
        ),
        Vec::new(),
    ));
    Ok(())
}

pub(super) fn readiness_onedrive(
    site: &str,
    config: &PublishManifest,
    checks: &mut Vec<ReadinessCheck>,
) -> Result<()> {
    let cfg = onedrive_config(config, site)?;
    checks.push(secret_check(
        "release.onedrive.token_available",
        &["ONEDRIVE_ACCESS_TOKEN"],
        "OneDrive OAuth token is available",
        "Set ONEDRIVE_ACCESS_TOKEN from your shell or CI secret store.",
    ));
    checks.push(check(
        "release.onedrive.path_selected",
        CheckSeverity::Info,
        CheckStatus::Passed,
        "OneDrive upload path is selected",
        Some(
            cfg.path_prefix
                .unwrap_or_else(|| "Fission releases".to_string()),
        ),
        Vec::new(),
    ));
    checks.push(check(
        "release.onedrive.conflict_behavior_explicit",
        CheckSeverity::Warning,
        if cfg.conflict_behavior.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        "OneDrive conflict behavior is explicit",
        Some(format!(
            "conflict_behavior = {}",
            cfg.conflict_behavior
                .as_deref()
                .unwrap_or("<missing; defaults to fail>")
        )),
        vec![
            "Set distribution.onedrive.<site>.conflict_behavior to fail, replace, or rename so republish behavior is reviewable.",
        ],
    ));
    Ok(())
}

pub(super) fn readiness_dropbox(
    site: &str,
    config: &PublishManifest,
    checks: &mut Vec<ReadinessCheck>,
) -> Result<()> {
    let cfg = dropbox_config(config, site)?;
    checks.push(secret_check(
        "release.dropbox.token_available",
        &["DROPBOX_ACCESS_TOKEN"],
        "Dropbox OAuth token is available",
        "Set DROPBOX_ACCESS_TOKEN from your shell or CI secret store.",
    ));
    checks.push(check(
        "release.dropbox.path_selected",
        CheckSeverity::Info,
        CheckStatus::Passed,
        "Dropbox upload path is selected",
        Some(
            cfg.path_prefix
                .unwrap_or_else(|| "/Fission releases".to_string()),
        ),
        Vec::new(),
    ));
    checks.push(check(
        "release.dropbox.write_mode_explicit",
        CheckSeverity::Warning,
        if cfg.mode.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        "Dropbox write mode is explicit",
        Some(format!(
            "mode = {}, autorename = {}",
            cfg.mode.as_deref().unwrap_or("<missing; defaults to add>"),
            cfg.autorename.unwrap_or(false)
        )),
        vec![
            "Set distribution.dropbox.<site>.mode = \"add\" to fail safely on conflicts, \"overwrite\" to replace intentionally, or another Dropbox-supported mode.",
        ],
    ));
    Ok(())
}

async fn s3_status_value(cfg: &S3Config, artifact: Option<&Path>) -> Result<Value> {
    let bucket = cfg
        .bucket
        .as_deref()
        .context("distribution.s3.<site>.bucket is required")?;
    let mut loader = aws_config::defaults(BehaviorVersion::latest());
    if let Some(region) = cfg
        .region
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        loader = loader.region(Region::new(region.to_string()));
    }
    if let Some(profile) = cfg
        .profile
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        loader = loader.profile_name(profile);
    }
    if let Some(endpoint) = cfg
        .endpoint
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        loader = loader.endpoint_url(endpoint);
    }
    let shared = loader.load().await;
    let mut builder = aws_sdk_s3::config::Builder::from(&shared);
    if cfg.path_style.unwrap_or(false) {
        builder = builder.force_path_style(true);
    }
    let client = aws_sdk_s3::Client::from_conf(builder.build());
    let prefix = normalized_prefix(cfg.prefix.as_deref());
    if let Some(artifact_path) = artifact {
        let manifest = read_artifact_manifest(artifact_path)?;
        let planned = s3_upload_plan(cfg, &manifest, artifact_path)?;
        let mut objects = Vec::new();
        for item in planned {
            let key = format!("{prefix}{}", item.relative_path);
            let object = match client.head_object().bucket(bucket).key(&key).send().await {
                Ok(head) => json!({
                    "key": key,
                    "relative_path": item.relative_path,
                    "provider_id": item.provider_id,
                    "url": item.url,
                    "exists": true,
                    "size": head.content_length(),
                    "etag": head.e_tag(),
                    "version_id": head.version_id(),
                    "content_type": head.content_type(),
                    "cache_control": head.cache_control(),
                }),
                Err(error) => json!({
                    "key": key,
                    "relative_path": item.relative_path,
                    "provider_id": item.provider_id,
                    "url": item.url,
                    "exists": false,
                    "error": redact_sensitive_text(&error.to_string()),
                }),
            };
            objects.push(object);
        }
        let status = s3_status_from_object_values(&objects);
        return Ok(json!({
            "bucket": bucket,
            "prefix": prefix,
            "artifact_manifest": artifact_path.display().to_string(),
            "status": status,
            "objects": objects,
        }));
    }
    let result = client
        .list_objects_v2()
        .bucket(bucket)
        .prefix(prefix.clone())
        .max_keys(10)
        .send()
        .await
        .with_context(|| format!("failed to list s3://{bucket}/{prefix}"))?;
    Ok(json!({
        "bucket": bucket,
        "prefix": prefix,
        "status": "ok",
        "key_count": result.key_count(),
        "objects": result.contents().iter().map(|object| json!({
            "key": object.key(),
            "size": object.size(),
            "etag": object.e_tag(),
        })).collect::<Vec<_>>()
    }))
}

fn s3_status_from_object_values(objects: &[Value]) -> &'static str {
    if objects
        .iter()
        .all(|object| object.get("exists").and_then(Value::as_bool) == Some(true))
    {
        "ok"
    } else if objects
        .iter()
        .any(|object| object.get("exists").and_then(Value::as_bool) == Some(true))
    {
        "partial"
    } else {
        "missing"
    }
}

fn json_http_response(response: reqwest::blocking::Response, operation: &str) -> Result<Value> {
    let status = response.status();
    let text = response.text()?;
    ensure_success(status, text.clone(), operation)?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse {operation} response: {text}"))
}

fn file_status_receipt(
    options: &DistributeOptions,
    provider: &str,
    status: &str,
    canonical_url: Option<String>,
    value: Value,
) -> DistributionReceipt {
    DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: provider.to_string(),
        site: options.site.clone(),
        action: "status".to_string(),
        artifact_manifest: None,
        deployment_id: options.deploy.clone(),
        canonical_url,
        preview_url: None,
        custom_domain: None,
        status: status.to_string(),
        stdout: serde_json::to_string_pretty(&value).ok(),
        stderr: None,
        manual_follow_up: Vec::new(),
    }
}

async fn upload_s3(
    cfg: &S3Config,
    manifest: &ArtifactManifest,
    artifact_path: &Path,
    events: &mut Vec<DistributionEvent>,
) -> Result<Vec<UploadedFile>> {
    let bucket = cfg
        .bucket
        .as_deref()
        .context("distribution.s3.<site>.bucket is required")?;
    let mut loader = aws_config::defaults(BehaviorVersion::latest());
    if let Some(region) = cfg
        .region
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        loader = loader.region(Region::new(region.to_string()));
    }
    if let Some(profile) = cfg
        .profile
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        loader = loader.profile_name(profile);
    }
    if let Some(endpoint) = cfg
        .endpoint
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        loader = loader.endpoint_url(endpoint);
    }
    let shared = loader.load().await;
    let mut builder = aws_sdk_s3::config::Builder::from(&shared);
    if cfg.path_style.unwrap_or(false) {
        builder = builder.force_path_style(true);
    }
    let client = aws_sdk_s3::Client::from_conf(builder.build());
    let prefix = normalized_prefix(cfg.prefix.as_deref());
    let mut uploaded = Vec::new();
    for item in upload_items(manifest, artifact_path)? {
        let key = format!("{prefix}{}", item.relative_path);
        let size = file_size(&item.path)?;
        record_upload_item_event(events, "s3", "started", &item, size, None);
        if !cfg.overwrite.unwrap_or(false)
            && client
                .head_object()
                .bucket(bucket)
                .key(&key)
                .send()
                .await
                .is_ok()
        {
            bail!(
                "S3 object s3://{bucket}/{key} already exists; set distribution.s3.<site>.overwrite = true or choose a new prefix"
            );
        }
        let body = ByteStream::from_path(&item.path)
            .await
            .with_context(|| format!("failed to open {} for S3 upload", item.path.display()))?;
        let mut request = client
            .put_object()
            .bucket(bucket)
            .key(&key)
            .body(body)
            .content_type(item.mime_type.clone());
        if let Some(cache_control) = cfg
            .cache_control
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            request = request.cache_control(cache_control);
        }
        if cfg.visibility.as_deref() == Some("public") {
            request = request.acl(ObjectCannedAcl::PublicRead);
        }
        request.send().await.with_context(|| {
            format!(
                "failed to upload {} to s3://{bucket}/{key}",
                item.path.display()
            )
        })?;
        let uploaded_file = UploadedFile {
            relative_path: item.relative_path.clone(),
            provider_id: Some(format!("s3://{bucket}/{key}")),
            url: s3_object_url(cfg, bucket, &key),
        };
        record_upload_item_event(events, "s3", "uploaded", &item, size, Some(&uploaded_file));
        uploaded.push(uploaded_file);
    }
    Ok(uploaded)
}

fn s3_upload_plan(
    cfg: &S3Config,
    manifest: &ArtifactManifest,
    artifact_path: &Path,
) -> Result<Vec<UploadedFile>> {
    let bucket = cfg
        .bucket
        .as_deref()
        .context("distribution.s3.<site>.bucket is required")?;
    let prefix = normalized_prefix(cfg.prefix.as_deref());
    Ok(upload_items(manifest, artifact_path)?
        .into_iter()
        .map(|item| {
            let key = format!("{prefix}{}", item.relative_path);
            UploadedFile {
                relative_path: item.relative_path,
                provider_id: Some(format!("s3://{bucket}/{key}")),
                url: s3_object_url(cfg, bucket, &key),
            }
        })
        .collect())
}

fn upload_google_drive_item(
    client: &Client,
    token: &str,
    cfg: &GoogleDriveConfig,
    item: &UploadItem,
    events: &mut Vec<DistributionEvent>,
) -> Result<UploadedFile> {
    let metadata = if let Some(folder_id) =
        cfg.folder_id.as_deref().filter(|value| !value.is_empty())
    {
        json!({ "name": drive_name(cfg.name_prefix.as_deref(), &item.relative_path), "parents": [folder_id] })
    } else {
        json!({ "name": drive_name(cfg.name_prefix.as_deref(), &item.relative_path) })
    };
    let len = file_size(&item.path)?;
    record_upload_item_event(events, "google-drive", "started", item, len, None);
    let response = client
        .post("https://www.googleapis.com/upload/drive/v3/files?uploadType=resumable&fields=id,name,webViewLink,webContentLink")
        .bearer_auth(token.trim())
        .header(CONTENT_TYPE, "application/json; charset=utf-8")
        .header("X-Upload-Content-Type", item.mime_type.as_str())
        .header("X-Upload-Content-Length", len.to_string())
        .json(&metadata)
        .send()
        .context("failed to start Google Drive resumable upload")?;
    let status = response.status();
    let location = response_location(&response)?;
    if !status.is_success() {
        bail!(
            "Google Drive upload start failed with {status}: {}",
            response.text()?
        );
    }
    record_upload_stage_event(
        events,
        "google-drive",
        "provider.upload.session",
        "created",
        item,
        len,
        Some("resumable"),
    );
    let file = fs::File::open(&item.path).with_context(|| {
        format!(
            "failed to open {} for Google Drive upload",
            item.path.display()
        )
    })?;
    let response = client
        .put(location)
        .header(CONTENT_TYPE, item.mime_type.as_str())
        .header(CONTENT_LENGTH, len.to_string())
        .body(Body::new(file))
        .send()
        .context("failed to upload file bytes to Google Drive")?;
    let status = response.status();
    let text = response.text()?;
    ensure_success(status, text.clone(), "Google Drive upload")?;
    record_upload_chunk_event(
        events,
        "google-drive",
        item,
        "uploaded",
        0,
        len,
        len,
        Some("resumable-complete"),
    );
    let value: Value =
        serde_json::from_str(&text).context("failed to parse Google Drive upload response")?;
    let id = value.get("id").and_then(Value::as_str).map(str::to_string);
    if cfg.share.unwrap_or(false) {
        if let Some(id) = id.as_deref() {
            let response = client
                .post(format!(
                    "https://www.googleapis.com/drive/v3/files/{id}/permissions"
                ))
                .bearer_auth(token.trim())
                .json(&json!({ "type": "anyone", "role": "reader" }))
                .send()
                .context("failed to create Google Drive sharing permission")?;
            let status = response.status();
            let text = response.text()?;
            ensure_success(status, text, "Google Drive sharing permission")?;
        }
    }
    let uploaded = UploadedFile {
        relative_path: item.relative_path.clone(),
        provider_id: id,
        url: value
            .get("webViewLink")
            .or_else(|| value.get("webContentLink"))
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    record_upload_item_event(
        events,
        "google-drive",
        "uploaded",
        item,
        len,
        Some(&uploaded),
    );
    Ok(uploaded)
}

fn upload_onedrive_item(
    client: &Client,
    token: &str,
    cfg: &OneDriveConfig,
    item: &UploadItem,
    events: &mut Vec<DistributionEvent>,
) -> Result<UploadedFile> {
    let root = cfg
        .root
        .as_deref()
        .unwrap_or("me/drive/root")
        .trim_matches('/');
    let upload_path = joined_remote_path(cfg.path_prefix.as_deref(), &item.relative_path)
        .trim_start_matches('/')
        .to_string();
    let url = format!(
        "https://graph.microsoft.com/v1.0/{root}:/{}/createUploadSession",
        encode_path(&upload_path)
    );
    let conflict = onedrive_conflict_behavior(cfg);
    let response = client
        .post(url)
        .bearer_auth(token.trim())
        .json(&json!({ "item": { "@microsoft.graph.conflictBehavior": conflict } }))
        .send()
        .context("failed to create OneDrive upload session")?;
    let status = response.status();
    let text = response.text()?;
    ensure_success(status, text.clone(), "OneDrive upload session")?;
    let value: Value =
        serde_json::from_str(&text).context("failed to parse OneDrive upload session")?;
    let upload_url = value
        .get("uploadUrl")
        .and_then(Value::as_str)
        .context("OneDrive upload session response did not contain uploadUrl")?;
    let size = file_size(&item.path)?;
    if size == 0 {
        bail!(
            "OneDrive upload does not support empty file {} yet",
            item.path.display()
        );
    }
    record_upload_item_event(events, "onedrive", "started", item, size, None);
    record_upload_stage_event(
        events,
        "onedrive",
        "provider.upload.session",
        "created",
        item,
        size,
        Some("upload-session"),
    );
    let mut file = fs::File::open(&item.path)
        .with_context(|| format!("failed to open {} for OneDrive upload", item.path.display()))?;
    let mut offset = 0_u64;
    let mut final_value = None;
    while offset < size {
        let chunk = read_next_chunk(&mut file, ONEDRIVE_CHUNK_SIZE)?;
        if chunk.is_empty() {
            bail!(
                "unexpected EOF while uploading {} to OneDrive",
                item.path.display()
            );
        }
        let chunk_len = chunk.len();
        let range = content_range(offset, chunk_len, size);
        let response = client
            .put(upload_url)
            .header(CONTENT_LENGTH, chunk_len.to_string())
            .header(CONTENT_RANGE, range)
            .body(chunk)
            .send()
            .context("failed to upload file bytes to OneDrive")?;
        let status = response.status();
        let text = response.text()?;
        offset += chunk_len as u64;
        record_upload_chunk_event(
            events,
            "onedrive",
            item,
            if status.as_u16() == 202 {
                "accepted"
            } else {
                "uploaded"
            },
            offset - chunk_len as u64,
            chunk_len as u64,
            size,
            Some("upload-session"),
        );
        if status.as_u16() == 202 {
            if offset == 0 {
                bail!("OneDrive upload did not advance after an accepted chunk");
            }
            continue;
        }
        ensure_success(status, text.clone(), "OneDrive upload")?;
        if offset < size {
            bail!(
                "OneDrive upload completed before all bytes were sent for {}",
                item.path.display()
            );
        }
        let value: Value =
            serde_json::from_str(&text).context("failed to parse OneDrive upload response")?;
        final_value = Some(value);
        break;
    }
    let value = final_value.context("OneDrive upload did not return a completed file response")?;
    let uploaded = UploadedFile {
        relative_path: item.relative_path.clone(),
        provider_id: value.get("id").and_then(Value::as_str).map(str::to_string),
        url: value
            .get("webUrl")
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    record_upload_item_event(events, "onedrive", "uploaded", item, size, Some(&uploaded));
    Ok(uploaded)
}

fn onedrive_conflict_behavior(cfg: &OneDriveConfig) -> &str {
    cfg.conflict_behavior.as_deref().unwrap_or("fail")
}

fn upload_dropbox_item(
    client: &Client,
    token: &str,
    cfg: &DropboxConfig,
    item: &UploadItem,
    events: &mut Vec<DistributionEvent>,
) -> Result<UploadedFile> {
    let remote_path = joined_remote_path(cfg.path_prefix.as_deref(), &item.relative_path);
    let size = file_size(&item.path)?;
    record_upload_item_event(events, "dropbox", "started", item, size, None);
    if size <= DROPBOX_SIMPLE_UPLOAD_LIMIT {
        upload_dropbox_simple(client, token, cfg, item, &remote_path, events)
    } else {
        upload_dropbox_session(client, token, cfg, item, &remote_path, events)
    }
}

fn upload_dropbox_simple(
    client: &Client,
    token: &str,
    cfg: &DropboxConfig,
    item: &UploadItem,
    remote_path: &str,
    events: &mut Vec<DistributionEvent>,
) -> Result<UploadedFile> {
    let mode = dropbox_write_mode(cfg);
    let len = file_size(&item.path)?;
    let file = fs::File::open(&item.path)
        .with_context(|| format!("failed to open {}", item.path.display()))?;
    let arg = json!({
        "path": remote_path,
        "mode": mode,
        "autorename": cfg.autorename.unwrap_or(false),
        "mute": false,
        "strict_conflict": false
    });
    let response = client
        .post("https://content.dropboxapi.com/2/files/upload")
        .bearer_auth(token.trim())
        .header("Dropbox-API-Arg", arg.to_string())
        .header(CONTENT_TYPE, "application/octet-stream")
        .body(Body::sized(file, len))
        .send()
        .context("failed to upload file to Dropbox")?;
    let status = response.status();
    let text = response.text()?;
    ensure_success(status, text.clone(), "Dropbox upload")?;
    record_upload_chunk_event(
        events,
        "dropbox",
        item,
        "uploaded",
        0,
        len,
        len,
        Some("simple"),
    );
    let value: Value =
        serde_json::from_str(&text).context("failed to parse Dropbox upload response")?;
    let uploaded = UploadedFile {
        relative_path: item.relative_path.clone(),
        provider_id: value.get("id").and_then(Value::as_str).map(str::to_string),
        url: value
            .get("path_display")
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    record_upload_item_event(events, "dropbox", "uploaded", item, len, Some(&uploaded));
    Ok(uploaded)
}

fn upload_dropbox_session(
    client: &Client,
    token: &str,
    cfg: &DropboxConfig,
    item: &UploadItem,
    remote_path: &str,
    events: &mut Vec<DistributionEvent>,
) -> Result<UploadedFile> {
    let mut file = fs::File::open(&item.path)
        .with_context(|| format!("failed to open {} for Dropbox upload", item.path.display()))?;
    let first = read_next_chunk(&mut file, DROPBOX_CHUNK_SIZE)?;
    if first.is_empty() {
        bail!("Dropbox session upload requires a non-empty file");
    }
    let first_len = first.len();
    let response = client
        .post("https://content.dropboxapi.com/2/files/upload_session/start")
        .bearer_auth(token.trim())
        .header("Dropbox-API-Arg", json!({"close": false}).to_string())
        .header(CONTENT_TYPE, "application/octet-stream")
        .body(first)
        .send()
        .context("failed to start Dropbox upload session")?;
    let status = response.status();
    let text = response.text()?;
    ensure_success(status, text.clone(), "Dropbox upload session start")?;
    let value: Value =
        serde_json::from_str(&text).context("failed to parse Dropbox session response")?;
    let session_id = value
        .get("session_id")
        .and_then(Value::as_str)
        .context("Dropbox upload session did not return session_id")?;
    let size = file_size(&item.path)?;
    record_upload_stage_event(
        events,
        "dropbox",
        "provider.upload.session",
        "created",
        item,
        size,
        Some("upload-session"),
    );
    record_upload_chunk_event(
        events,
        "dropbox",
        item,
        "accepted",
        0,
        first_len as u64,
        size,
        Some("session-start"),
    );
    let mut offset = first_len as u64;
    while size.saturating_sub(offset) > DROPBOX_CHUNK_SIZE as u64 {
        let chunk = read_next_chunk(&mut file, DROPBOX_CHUNK_SIZE)?;
        if chunk.is_empty() {
            bail!(
                "unexpected EOF while uploading {} to Dropbox",
                item.path.display()
            );
        }
        let arg = json!({"cursor": {"session_id": session_id, "offset": offset}});
        let chunk_len = chunk.len();
        let response = client
            .post("https://content.dropboxapi.com/2/files/upload_session/append_v2")
            .bearer_auth(token.trim())
            .header("Dropbox-API-Arg", arg.to_string())
            .header(CONTENT_TYPE, "application/octet-stream")
            .body(chunk)
            .send()
            .context("failed to append Dropbox upload session")?;
        let status = response.status();
        let text = response.text()?;
        ensure_success(status, text, "Dropbox upload session append")?;
        record_upload_chunk_event(
            events,
            "dropbox",
            item,
            "accepted",
            offset,
            chunk_len as u64,
            size,
            Some("session-append"),
        );
        offset += chunk_len as u64;
    }
    let remaining = read_next_chunk(&mut file, DROPBOX_CHUNK_SIZE)?;
    let mode = dropbox_write_mode(cfg);
    let arg = json!({
        "cursor": {"session_id": session_id, "offset": offset},
        "commit": {
            "path": remote_path,
            "mode": mode,
            "autorename": cfg.autorename.unwrap_or(false),
            "mute": false,
            "strict_conflict": false
        }
    });
    let remaining_len = remaining.len();
    let response = client
        .post("https://content.dropboxapi.com/2/files/upload_session/finish")
        .bearer_auth(token.trim())
        .header("Dropbox-API-Arg", arg.to_string())
        .header(CONTENT_TYPE, "application/octet-stream")
        .body(remaining)
        .send()
        .context("failed to finish Dropbox upload session")?;
    let status = response.status();
    let text = response.text()?;
    ensure_success(status, text.clone(), "Dropbox upload session finish")?;
    record_upload_chunk_event(
        events,
        "dropbox",
        item,
        "uploaded",
        offset,
        remaining_len as u64,
        size,
        Some("session-finish"),
    );
    let value: Value =
        serde_json::from_str(&text).context("failed to parse Dropbox finish response")?;
    let uploaded = UploadedFile {
        relative_path: item.relative_path.clone(),
        provider_id: value.get("id").and_then(Value::as_str).map(str::to_string),
        url: value
            .get("path_display")
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    record_upload_item_event(events, "dropbox", "uploaded", item, size, Some(&uploaded));
    Ok(uploaded)
}

fn dropbox_write_mode(cfg: &DropboxConfig) -> &str {
    cfg.mode.as_deref().unwrap_or("add")
}

fn upload_receipt(
    options: &DistributeOptions,
    artifact_path: &Path,
    provider: &str,
    status: &str,
    canonical_url: Option<String>,
    uploaded: Vec<UploadedFile>,
) -> DistributionReceipt {
    let stdout = serde_json::to_string_pretty(&json!({
        "uploaded": uploaded.iter().map(|file| json!({
            "relative_path": file.relative_path,
            "provider_id": file.provider_id,
            "url": file.url,
        })).collect::<Vec<_>>()
    }))
    .ok();
    DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: provider.to_string(),
        site: options.site.clone(),
        action: "publish".to_string(),
        artifact_manifest: Some(artifact_path.display().to_string()),
        deployment_id: options.deploy.clone(),
        canonical_url,
        preview_url: uploaded.iter().find_map(|file| file.url.clone()),
        custom_domain: None,
        status: status.to_string(),
        stdout,
        stderr: None,
        manual_follow_up: Vec::new(),
    }
}

fn record_upload_plan_events(
    events: &mut Vec<DistributionEvent>,
    provider: &str,
    manifest: &ArtifactManifest,
    artifact_path: &Path,
    uploaded: &[UploadedFile],
) -> Result<()> {
    let uploaded_by_relative = uploaded
        .iter()
        .map(|file| (file.relative_path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    for item in upload_items(manifest, artifact_path)? {
        let size = file_size(&item.path)?;
        record_upload_item_event(
            events,
            provider,
            "planned",
            &item,
            size,
            uploaded_by_relative
                .get(item.relative_path.as_str())
                .copied(),
        );
    }
    Ok(())
}

fn record_upload_item_event(
    events: &mut Vec<DistributionEvent>,
    provider: &str,
    status: &str,
    item: &UploadItem,
    size_bytes: u64,
    uploaded: Option<&UploadedFile>,
) {
    let details = json!({
        "provider": provider,
        "relative_path": item.relative_path,
        "path": item.path.display().to_string(),
        "mime_type": item.mime_type,
        "size_bytes": size_bytes,
        "provider_id": uploaded.and_then(|file| file.provider_id.clone()),
        "url": uploaded.and_then(|file| file.url.clone()),
    });
    push_distribution_event(
        events,
        "provider.upload.item",
        status,
        Some(details.to_string()),
    );
}

fn record_upload_stage_event(
    events: &mut Vec<DistributionEvent>,
    provider: &str,
    id: &str,
    status: &str,
    item: &UploadItem,
    size_bytes: u64,
    stage: Option<&str>,
) {
    let details = json!({
        "provider": provider,
        "relative_path": item.relative_path,
        "path": item.path.display().to_string(),
        "size_bytes": size_bytes,
        "stage": stage,
    });
    push_distribution_event(events, id, status, Some(details.to_string()));
}

fn record_upload_chunk_event(
    events: &mut Vec<DistributionEvent>,
    provider: &str,
    item: &UploadItem,
    status: &str,
    byte_start: u64,
    chunk_bytes: u64,
    total_bytes: u64,
    stage: Option<&str>,
) {
    let uploaded_bytes = byte_start.saturating_add(chunk_bytes);
    let details = json!({
        "provider": provider,
        "relative_path": item.relative_path,
        "path": item.path.display().to_string(),
        "stage": stage,
        "byte_start": byte_start,
        "byte_end_exclusive": uploaded_bytes,
        "chunk_bytes": chunk_bytes,
        "uploaded_bytes": uploaded_bytes,
        "total_bytes": total_bytes,
    });
    push_distribution_event(
        events,
        "provider.upload.chunk",
        status,
        Some(details.to_string()),
    );
}

fn upload_items(manifest: &ArtifactManifest, artifact_path: &Path) -> Result<Vec<UploadItem>> {
    let mut items = manifest
        .artifacts
        .iter()
        .map(|file| UploadItem {
            path: PathBuf::from(&file.path),
            relative_path: file.relative_path.clone(),
            mime_type: file.mime_type.clone(),
        })
        .collect::<Vec<_>>();
    items.push(UploadItem {
        path: artifact_path.to_path_buf(),
        relative_path: ARTIFACT_MANIFEST.to_string(),
        mime_type: content_type(artifact_path).to_string(),
    });
    Ok(items)
}

fn file_size(path: &Path) -> Result<u64> {
    Ok(fs::metadata(path)
        .with_context(|| format!("failed to stat {}", path.display()))?
        .len())
}

fn s3_credentials_check() -> ReadinessCheck {
    s3_credentials_check_with(|name| std::env::var_os(name).is_some())
}

fn s3_credentials_check_with(has_env: impl Fn(&str) -> bool) -> ReadinessCheck {
    let found = if has_env("AWS_PROFILE") {
        Some("environment variable AWS_PROFILE".to_string())
    } else if has_env("AWS_WEB_IDENTITY_TOKEN_FILE") {
        Some("environment variable AWS_WEB_IDENTITY_TOKEN_FILE".to_string())
    } else if has_env("AWS_ACCESS_KEY_ID") && has_env("AWS_SECRET_ACCESS_KEY") {
        Some("environment variables AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY".to_string())
    } else {
        None
    };
    check(
        "release.s3.credentials_available",
        CheckSeverity::Error,
        if found.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        "AWS/S3 credentials are available",
        found,
        vec!["Set AWS_PROFILE, AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY, or AWS_WEB_IDENTITY_TOKEN_FILE from your shell or CI secret store."],
    )
}

fn secret_check(id: &str, env_names: &[&str], summary: &str, remediation: &str) -> ReadinessCheck {
    let found_env = env_names
        .iter()
        .find(|name| std::env::var_os(name).is_some());
    check(
        id,
        CheckSeverity::Error,
        if found_env.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        summary,
        found_env.map(|name| format!("environment variable {name}")),
        vec![remediation],
    )
}

fn ensure_success(status: reqwest::StatusCode, body: String, context: &str) -> Result<()> {
    if status.is_success() {
        Ok(())
    } else {
        bail!("{context} failed with {status}: {body}")
    }
}

fn read_next_chunk(file: &mut fs::File, max_len: usize) -> Result<Vec<u8>> {
    let mut buffer = vec![0; max_len];
    let len = file.read(&mut buffer)?;
    buffer.truncate(len);
    Ok(buffer)
}

fn content_range(offset: u64, chunk_len: usize, total_len: u64) -> String {
    let end = offset + chunk_len as u64 - 1;
    format!("bytes {offset}-{end}/{total_len}")
}

fn response_location(response: &reqwest::blocking::Response) -> Result<String> {
    response
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .context("resumable upload response did not include Location header")
}

fn normalized_prefix(prefix: Option<&str>) -> String {
    prefix
        .unwrap_or("")
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
        .pipe(|value| {
            if value.is_empty() {
                value
            } else {
                format!("{value}/")
            }
        })
}

fn joined_remote_path(prefix: Option<&str>, relative: &str) -> String {
    let prefix = prefix.unwrap_or("").trim_matches('/');
    let relative = relative.trim_start_matches('/');
    if prefix.is_empty() {
        format!("/{relative}")
    } else {
        format!("/{prefix}/{relative}")
    }
}

fn drive_name(prefix: Option<&str>, relative: &str) -> String {
    let name = relative.replace('/', "__");
    match prefix.map(str::trim).filter(|value| !value.is_empty()) {
        Some(prefix) => format!("{prefix}-{name}"),
        None => name,
    }
}

fn encode_path(path: &str) -> String {
    path.split('/')
        .map(percent_encode_segment)
        .collect::<Vec<_>>()
        .join("/")
}

fn percent_encode_segment(segment: &str) -> String {
    let mut out = String::new();
    for byte in segment.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn s3_object_url(cfg: &S3Config, bucket: &str, key: &str) -> Option<String> {
    if cfg.visibility.as_deref() != Some("public") {
        return None;
    }
    if let Some(endpoint) = cfg
        .endpoint
        .as_deref()
        .filter(|value| value.starts_with("http"))
    {
        if cfg.path_style.unwrap_or(false) {
            Some(format!(
                "{}/{}/{}",
                endpoint.trim_end_matches('/'),
                bucket,
                key
            ))
        } else {
            let endpoint = endpoint
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .trim_end_matches('/');
            Some(format!("https://{bucket}.{endpoint}/{key}"))
        }
    } else {
        let region = cfg.region.as_deref().unwrap_or("us-east-1");
        Some(format!("https://{bucket}.s3.{region}.amazonaws.com/{key}"))
    }
}

fn s3_canonical_url(cfg: &S3Config, fallback: Option<&str>) -> Option<String> {
    cfg.bucket.as_deref().and_then(|bucket| {
        let prefix = normalized_prefix(cfg.prefix.as_deref());
        if prefix.is_empty() {
            fallback.map(str::to_string)
        } else {
            s3_object_url(cfg, bucket, &prefix)
        }
    })
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_with_artifact(path: &Path) -> ArtifactManifest {
        ArtifactManifest {
            schema_version: 1,
            created_at_unix_seconds: 1,
            project: ArtifactProject {
                app_id: "com.example.demo".to_string(),
                name: "Demo".to_string(),
                build: Some(1),
                version: Some("1.0.0".to_string()),
            },
            target: "static-site".to_string(),
            format: "static".to_string(),
            profile: "release".to_string(),
            root_dir: path.parent().unwrap().display().to_string(),
            source_config: Vec::new(),
            artifacts: vec![ArtifactFile {
                kind: "archive".to_string(),
                purpose: Some("site".to_string()),
                platform: Some("site".to_string()),
                upload_provider: Some("s3".to_string()),
                path: path.display().to_string(),
                relative_path: "app.zip".to_string(),
                sha256: "abc".to_string(),
                size_bytes: 3,
                mime_type: "application/zip".to_string(),
            }],
            icon_manifest: None,
            signing: None,
            notarization: None,
            validation: ArtifactValidation {
                state: "passed".to_string(),
                checks: Vec::new(),
            },
        }
    }

    #[test]
    fn s3_dry_run_plan_includes_manifest_and_public_urls() {
        let dir = std::env::temp_dir().join(format!("fission-s3-plan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let artifact = dir.join("app.zip");
        fs::write(&artifact, b"zip").unwrap();
        let manifest_path = dir.join("artifact-manifest.json");
        fs::write(&manifest_path, b"{}").unwrap();
        let manifest = manifest_with_artifact(&artifact);
        let cfg = S3Config {
            bucket: Some("downloads".to_string()),
            region: Some("eu-west-2".to_string()),
            prefix: Some("app/releases/1".to_string()),
            visibility: Some("public".to_string()),
            ..Default::default()
        };

        let plan = s3_upload_plan(&cfg, &manifest, &manifest_path).unwrap();
        assert_eq!(plan.len(), 2);
        assert_eq!(
            plan[0].provider_id.as_deref(),
            Some("s3://downloads/app/releases/1/app.zip")
        );
        assert_eq!(
            plan[0].url.as_deref(),
            Some("https://downloads.s3.eu-west-2.amazonaws.com/app/releases/1/app.zip")
        );
        assert_eq!(
            plan[1].provider_id.as_deref(),
            Some("s3://downloads/app/releases/1/artifact-manifest.json")
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn chunk_helpers_preserve_ranges_without_loading_full_file() {
        let dir =
            std::env::temp_dir().join(format!("fission-upload-chunks-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("payload.bin");
        fs::write(&path, b"abcdef").unwrap();
        let mut file = fs::File::open(&path).unwrap();

        assert_eq!(read_next_chunk(&mut file, 4).unwrap(), b"abcd");
        assert_eq!(read_next_chunk(&mut file, 4).unwrap(), b"ef");
        assert_eq!(content_range(4, 2, 6), "bytes 4-5/6");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn s3_dry_run_records_per_item_upload_plan_events() {
        let dir = std::env::temp_dir().join(format!("fission-s3-events-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let artifact = dir.join("app.zip");
        fs::write(&artifact, b"zip").unwrap();
        let manifest_path = dir.join("artifact-manifest.json");
        fs::write(&manifest_path, b"{}").unwrap();
        let manifest = manifest_with_artifact(&artifact);
        let options = DistributeOptions {
            project_dir: dir.clone(),
            provider: DistributionProvider::S3,
            action: DistributeAction::Publish,
            target: Some(Target::Site),
            format: Some(PackageFormat::Static),
            artifact: Some(manifest_path.clone()),
            site: "production".to_string(),
            deploy: None,
            track: None,
            locales: Vec::new(),
            dry_run: true,
            yes: false,
            json: false,
        };
        let config = PublishManifest {
            distribution: Some(DistributionManifest {
                s3: BTreeMap::from([(
                    "production".to_string(),
                    S3Config {
                        bucket: Some("downloads".to_string()),
                        prefix: Some("app/releases/1".to_string()),
                        visibility: Some("public".to_string()),
                        ..Default::default()
                    },
                )]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut events = Vec::new();

        let receipt =
            publish_s3(&options, &config, &manifest_path, &manifest, &mut events).unwrap();

        assert_eq!(receipt.status, "dry-run");
        let upload_events = events
            .iter()
            .filter(|event| event.id == "provider.upload.item")
            .collect::<Vec<_>>();
        assert_eq!(upload_events.len(), 2);
        assert!(upload_events.iter().all(|event| event.status == "planned"));
        assert!(upload_events.iter().any(|event| event
            .details
            .as_deref()
            .unwrap_or("")
            .contains("app.zip")));
        assert!(upload_events.iter().any(|event| event
            .details
            .as_deref()
            .unwrap_or("")
            .contains("artifact-manifest.json")));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn s3_status_reports_partial_object_visibility() {
        let objects = vec![
            json!({
                "key": "app/releases/app.zip",
                "exists": true,
                "etag": "\"abc\"",
            }),
            json!({
                "key": "app/releases/artifact-manifest.json",
                "exists": false,
                "error": "not found",
            }),
        ];

        assert_eq!(s3_status_from_object_values(&objects), "partial");
        assert_eq!(s3_status_from_object_values(&objects[1..]), "missing");
    }

    #[test]
    fn s3_credentials_require_access_key_pair_when_no_profile_or_web_identity() {
        let missing_secret = s3_credentials_check_with(|name| name == "AWS_ACCESS_KEY_ID");
        assert_eq!(missing_secret.status, CheckStatus::Missing);

        let access_pair = s3_credentials_check_with(|name| {
            matches!(name, "AWS_ACCESS_KEY_ID" | "AWS_SECRET_ACCESS_KEY")
        });
        assert_eq!(access_pair.status, CheckStatus::Passed);
        assert_eq!(
            access_pair.details.as_deref(),
            Some("environment variables AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY")
        );
    }

    #[test]
    fn file_provider_conflict_defaults_fail_safely() {
        assert_eq!(
            onedrive_conflict_behavior(&OneDriveConfig::default()),
            "fail"
        );
        assert_eq!(dropbox_write_mode(&DropboxConfig::default()), "add");

        assert_eq!(
            onedrive_conflict_behavior(&OneDriveConfig {
                conflict_behavior: Some("replace".to_string()),
                ..Default::default()
            }),
            "replace"
        );
        assert_eq!(
            dropbox_write_mode(&DropboxConfig {
                mode: Some("overwrite".to_string()),
                ..Default::default()
            }),
            "overwrite"
        );
    }
}
