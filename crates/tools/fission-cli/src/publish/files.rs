use super::*;
use crate::release;
use anyhow::{bail, Context, Result};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::ObjectCannedAcl;
use reqwest::blocking::Client;
use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, LOCATION};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

const DROPBOX_SIMPLE_UPLOAD_LIMIT: u64 = 150 * 1024 * 1024;
const DROPBOX_CHUNK_SIZE: usize = 8 * 1024 * 1024;

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
) -> Result<DistributionReceipt> {
    let cfg = s3_config(config, &options.site)?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to create S3 upload runtime")?;
    let uploaded = rt.block_on(upload_s3(&cfg, manifest, artifact_path))?;
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
) -> Result<DistributionReceipt> {
    let cfg = google_drive_config(config, &options.site)?;
    let token = release::provider_secret(
        DistributionProvider::GoogleDrive,
        &["GOOGLE_DRIVE_ACCESS_TOKEN"],
    )?
    .context("Google Drive upload requires GOOGLE_DRIVE_ACCESS_TOKEN or a stored google-drive credential")?;
    let client = Client::new();
    let mut uploaded = Vec::new();
    for item in upload_items(manifest, artifact_path)? {
        uploaded.push(upload_google_drive_item(&client, &token, &cfg, &item)?);
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
) -> Result<DistributionReceipt> {
    let cfg = onedrive_config(config, &options.site)?;
    let token =
        release::provider_secret(DistributionProvider::OneDrive, &["ONEDRIVE_ACCESS_TOKEN"])?
            .context(
                "OneDrive upload requires ONEDRIVE_ACCESS_TOKEN or a stored onedrive credential",
            )?;
    let client = Client::new();
    let mut uploaded = Vec::new();
    for item in upload_items(manifest, artifact_path)? {
        uploaded.push(upload_onedrive_item(&client, &token, &cfg, &item)?);
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
) -> Result<DistributionReceipt> {
    let cfg = dropbox_config(config, &options.site)?;
    let token = release::provider_secret(DistributionProvider::Dropbox, &["DROPBOX_ACCESS_TOKEN"])?
        .context("Dropbox upload requires DROPBOX_ACCESS_TOKEN or a stored dropbox credential")?;
    let client = Client::new();
    let mut uploaded = Vec::new();
    for item in upload_items(manifest, artifact_path)? {
        uploaded.push(upload_dropbox_item(&client, &token, &cfg, &item)?);
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
    let value = rt.block_on(s3_status_value(&cfg))?;
    Ok(file_status_receipt(
        options,
        "s3",
        "ok",
        s3_canonical_url(&cfg, None),
        value,
    ))
}

pub(super) fn google_drive_status(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = google_drive_config(config, &options.site)?;
    let token = release::provider_secret(
        DistributionProvider::GoogleDrive,
        &["GOOGLE_DRIVE_ACCESS_TOKEN"],
    )?
    .context("Google Drive status requires GOOGLE_DRIVE_ACCESS_TOKEN or a stored google-drive credential")?;
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
    let token =
        release::provider_secret(DistributionProvider::OneDrive, &["ONEDRIVE_ACCESS_TOKEN"])?
            .context(
                "OneDrive status requires ONEDRIVE_ACCESS_TOKEN or a stored onedrive credential",
            )?;
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
    let token = release::provider_secret(DistributionProvider::Dropbox, &["DROPBOX_ACCESS_TOKEN"])?
        .context("Dropbox status requires DROPBOX_ACCESS_TOKEN or a stored dropbox credential")?;
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
    checks.push(secret_check(
        "release.s3.credentials_available",
        DistributionProvider::S3,
        &["AWS_PROFILE", "AWS_ACCESS_KEY_ID"],
        "AWS/S3 credentials are available",
        "Set AWS_PROFILE, AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY, or import S3 credentials into the Fission vault.",
    ));
    checks.push(check(
        "release.s3.direct_rust_backend",
        CheckSeverity::Info,
        CheckStatus::Passed,
        "S3 upload uses the Rust AWS SDK backend",
        Some(format!(
            "endpoint = {}, path_style = {}, visibility = {}, presign_ttl_seconds = {}",
            cfg.endpoint.as_deref().unwrap_or("<provider default>"),
            cfg.path_style.unwrap_or(false),
            cfg.visibility.as_deref().unwrap_or("private"),
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
        DistributionProvider::GoogleDrive,
        &["GOOGLE_DRIVE_ACCESS_TOKEN"],
        "Google Drive OAuth token is available",
        "Run `fission auth import google-drive --from env:GOOGLE_DRIVE_ACCESS_TOKEN --yes` or set GOOGLE_DRIVE_ACCESS_TOKEN in CI.",
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
        DistributionProvider::OneDrive,
        &["ONEDRIVE_ACCESS_TOKEN"],
        "OneDrive OAuth token is available",
        "Run `fission auth import onedrive --from env:ONEDRIVE_ACCESS_TOKEN --yes` or set ONEDRIVE_ACCESS_TOKEN in CI.",
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
        DistributionProvider::Dropbox,
        &["DROPBOX_ACCESS_TOKEN"],
        "Dropbox OAuth token is available",
        "Run `fission auth import dropbox --from env:DROPBOX_ACCESS_TOKEN --yes` or set DROPBOX_ACCESS_TOKEN in CI.",
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
    Ok(())
}

async fn s3_status_value(cfg: &S3Config) -> Result<Value> {
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
        "key_count": result.key_count(),
        "objects": result.contents().iter().map(|object| json!({
            "key": object.key(),
            "size": object.size(),
            "etag": object.e_tag(),
        })).collect::<Vec<_>>()
    }))
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
        let body = ByteStream::from_path(&item.path)
            .await
            .with_context(|| format!("failed to open {} for S3 upload", item.path.display()))?;
        let mut request = client
            .put_object()
            .bucket(bucket)
            .key(&key)
            .body(body)
            .content_type(item.mime_type.clone());
        if cfg.visibility.as_deref() == Some("public") {
            request = request.acl(ObjectCannedAcl::PublicRead);
        }
        request.send().await.with_context(|| {
            format!(
                "failed to upload {} to s3://{bucket}/{key}",
                item.path.display()
            )
        })?;
        uploaded.push(UploadedFile {
            relative_path: item.relative_path,
            provider_id: Some(format!("s3://{bucket}/{key}")),
            url: s3_object_url(cfg, bucket, &key),
        });
    }
    Ok(uploaded)
}

fn upload_google_drive_item(
    client: &Client,
    token: &str,
    cfg: &GoogleDriveConfig,
    item: &UploadItem,
) -> Result<UploadedFile> {
    let metadata = if let Some(folder_id) =
        cfg.folder_id.as_deref().filter(|value| !value.is_empty())
    {
        json!({ "name": drive_name(cfg.name_prefix.as_deref(), &item.relative_path), "parents": [folder_id] })
    } else {
        json!({ "name": drive_name(cfg.name_prefix.as_deref(), &item.relative_path) })
    };
    let len = fs::metadata(&item.path)?.len();
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
    let bytes = fs::read(&item.path)?;
    let response = client
        .put(location)
        .header(CONTENT_TYPE, item.mime_type.as_str())
        .header(CONTENT_LENGTH, bytes.len().to_string())
        .body(bytes)
        .send()
        .context("failed to upload file bytes to Google Drive")?;
    let status = response.status();
    let text = response.text()?;
    ensure_success(status, text.clone(), "Google Drive upload")?;
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
    Ok(UploadedFile {
        relative_path: item.relative_path.clone(),
        provider_id: id,
        url: value
            .get("webViewLink")
            .or_else(|| value.get("webContentLink"))
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn upload_onedrive_item(
    client: &Client,
    token: &str,
    cfg: &OneDriveConfig,
    item: &UploadItem,
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
    let conflict = cfg.conflict_behavior.as_deref().unwrap_or("replace");
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
    let bytes = fs::read(&item.path)?;
    if bytes.is_empty() {
        bail!(
            "OneDrive upload does not support empty file {} yet",
            item.path.display()
        );
    }
    let range = format!("bytes 0-{}/{}", bytes.len() - 1, bytes.len());
    let response = client
        .put(upload_url)
        .header(CONTENT_LENGTH, bytes.len().to_string())
        .header(CONTENT_RANGE, range)
        .body(bytes)
        .send()
        .context("failed to upload file bytes to OneDrive")?;
    let status = response.status();
    let text = response.text()?;
    ensure_success(status, text.clone(), "OneDrive upload")?;
    let value: Value =
        serde_json::from_str(&text).context("failed to parse OneDrive upload response")?;
    Ok(UploadedFile {
        relative_path: item.relative_path.clone(),
        provider_id: value.get("id").and_then(Value::as_str).map(str::to_string),
        url: value
            .get("webUrl")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn upload_dropbox_item(
    client: &Client,
    token: &str,
    cfg: &DropboxConfig,
    item: &UploadItem,
) -> Result<UploadedFile> {
    let remote_path = joined_remote_path(cfg.path_prefix.as_deref(), &item.relative_path);
    let size = fs::metadata(&item.path)?.len();
    if size <= DROPBOX_SIMPLE_UPLOAD_LIMIT {
        upload_dropbox_simple(client, token, cfg, item, &remote_path)
    } else {
        upload_dropbox_session(client, token, cfg, item, &remote_path)
    }
}

fn upload_dropbox_simple(
    client: &Client,
    token: &str,
    cfg: &DropboxConfig,
    item: &UploadItem,
    remote_path: &str,
) -> Result<UploadedFile> {
    let mode = cfg.mode.as_deref().unwrap_or("overwrite");
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
        .body(fs::read(&item.path)?)
        .send()
        .context("failed to upload file to Dropbox")?;
    let status = response.status();
    let text = response.text()?;
    ensure_success(status, text.clone(), "Dropbox upload")?;
    let value: Value =
        serde_json::from_str(&text).context("failed to parse Dropbox upload response")?;
    Ok(UploadedFile {
        relative_path: item.relative_path.clone(),
        provider_id: value.get("id").and_then(Value::as_str).map(str::to_string),
        url: value
            .get("path_display")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn upload_dropbox_session(
    client: &Client,
    token: &str,
    cfg: &DropboxConfig,
    item: &UploadItem,
    remote_path: &str,
) -> Result<UploadedFile> {
    let bytes = fs::read(&item.path)?;
    let first_len = DROPBOX_CHUNK_SIZE.min(bytes.len());
    let response = client
        .post("https://content.dropboxapi.com/2/files/upload_session/start")
        .bearer_auth(token.trim())
        .header("Dropbox-API-Arg", json!({"close": false}).to_string())
        .header(CONTENT_TYPE, "application/octet-stream")
        .body(bytes[..first_len].to_vec())
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
    let mut offset = first_len;
    while offset + DROPBOX_CHUNK_SIZE < bytes.len() {
        let next = offset + DROPBOX_CHUNK_SIZE;
        let arg = json!({"cursor": {"session_id": session_id, "offset": offset}});
        let response = client
            .post("https://content.dropboxapi.com/2/files/upload_session/append_v2")
            .bearer_auth(token.trim())
            .header("Dropbox-API-Arg", arg.to_string())
            .header(CONTENT_TYPE, "application/octet-stream")
            .body(bytes[offset..next].to_vec())
            .send()
            .context("failed to append Dropbox upload session")?;
        let status = response.status();
        let text = response.text()?;
        ensure_success(status, text, "Dropbox upload session append")?;
        offset = next;
    }
    let mode = cfg.mode.as_deref().unwrap_or("overwrite");
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
    let response = client
        .post("https://content.dropboxapi.com/2/files/upload_session/finish")
        .bearer_auth(token.trim())
        .header("Dropbox-API-Arg", arg.to_string())
        .header(CONTENT_TYPE, "application/octet-stream")
        .body(bytes[offset..].to_vec())
        .send()
        .context("failed to finish Dropbox upload session")?;
    let status = response.status();
    let text = response.text()?;
    ensure_success(status, text.clone(), "Dropbox upload session finish")?;
    let value: Value =
        serde_json::from_str(&text).context("failed to parse Dropbox finish response")?;
    Ok(UploadedFile {
        relative_path: item.relative_path.clone(),
        provider_id: value.get("id").and_then(Value::as_str).map(str::to_string),
        url: value
            .get("path_display")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
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

fn secret_check(
    id: &str,
    provider: DistributionProvider,
    env_names: &[&str],
    summary: &str,
    remediation: &str,
) -> ReadinessCheck {
    let found_env = env_names
        .iter()
        .find(|name| std::env::var_os(name).is_some());
    let found_secret = release::provider_secret(provider, env_names)
        .ok()
        .flatten()
        .is_some();
    check(
        id,
        CheckSeverity::Error,
        if found_env.is_some() || found_secret {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        summary,
        found_env
            .map(|name| format!("environment: {name}"))
            .or_else(|| found_secret.then(|| "vault credential".to_string())),
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
