use super::*;
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::blocking::{Body, Client, Response};
use reqwest::header::CONTENT_LENGTH;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod app_store_review;
#[path = "stores/shared.rs"]
mod shared;
use shared::*;

const PLAY_API: &str = "https://androidpublisher.googleapis.com";
const PLAY_UPLOAD_API: &str = "https://androidpublisher.googleapis.com/upload";
const GOOGLE_PLAY_SCOPE: &str = "https://www.googleapis.com/auth/androidpublisher";
const GOOGLE_TOKEN_URI: &str = "https://oauth2.googleapis.com/token";
const APP_STORE_API_PRIVATE_KEYS_DIR: &str = "API_PRIVATE_KEYS_DIR";
const MICROSOFT_STORE_API: &str = "https://api.store.microsoft.com";
const APP_STORE_API: &str = "https://api.appstoreconnect.apple.com";
const MICROSOFT_STORE_SCOPE: &str = "https://api.store.microsoft.com/.default";
const MICROSOFT_STORE_MSIX_TYPES: &[&str] = &["msix", "msixupload"];

#[derive(Debug, Deserialize)]
struct GoogleServiceAccount {
    client_email: String,
    private_key: String,
    #[serde(default)]
    token_uri: Option<String>,
}

#[derive(Debug, Serialize)]
struct GoogleJwtClaims<'a> {
    iss: &'a str,
    scope: &'a str,
    aud: &'a str,
    iat: u64,
    exp: u64,
}

#[derive(Debug, Serialize)]
struct AppStoreJwtClaims<'a> {
    iss: &'a str,
    aud: &'a str,
    iat: u64,
    exp: u64,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseNotesManifest {
    release: Option<ReleaseNotesRoot>,
    #[serde(default)]
    releases: Vec<ReleaseNotesEntry>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseNotesRoot {
    active_release: Option<String>,
    #[serde(default)]
    default_locales: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseNotesEntry {
    id: Option<String>,
    release_notes: Option<String>,
    #[serde(default)]
    locales: Vec<String>,
}

pub(super) fn publish_play_store(
    options: &DistributeOptions,
    config: &PublishManifest,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
) -> Result<DistributionReceipt> {
    let cfg = play_store_config(config);
    let package_name = cfg
        .package_name
        .as_deref()
        .or_else(|| package_name_from_project(manifest))
        .context("distribution.play_store.package_name is required")?;
    let track = options
        .track
        .as_deref()
        .or(cfg.default_track.as_deref())
        .unwrap_or("internal");
    let release_status = cfg.release_status.as_deref().unwrap_or("completed");
    let artifact = primary_artifact_with_extensions(manifest, &["aab", "apk"])?;
    let artifact_kind = artifact
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if is_play_internal_sharing_track(track) {
        return publish_play_internal_sharing(
            options,
            &cfg,
            package_name,
            artifact_path,
            &artifact,
            artifact_kind,
        );
    }
    let release_notes = play_release_notes(&options.project_dir, &options.locales)?;
    if options.dry_run {
        return Ok(store_receipt(
            options,
            "play-store",
            artifact_path,
            "dry-run",
            None,
            Some(format!(
                "https://play.google.com/console/u/0/developers/app/{package_name}/tracks/{track}"
            )),
            vec![format!(
                "Would upload {} to Google Play package {package_name} track {track} with release status {release_status} and {} release note locale(s).",
                artifact.display(),
                release_notes.len()
            )],
        ));
    }

    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    if let Some(version_code) =
        android_version_code_for_provider(&options.project_dir, Some(manifest))?
    {
        ensure_play_version_code_unused(
            &client,
            &token,
            package_name,
            &edit_id,
            track,
            &version_code,
        )?;
    }
    let version_code = upload_play_artifact(
        &client,
        &token,
        package_name,
        &edit_id,
        &artifact,
        artifact_kind,
    )?;
    update_play_track(
        &client,
        &token,
        package_name,
        &edit_id,
        track,
        release_status,
        &version_code,
        &release_notes,
    )?;
    validate_play_edit(&client, &token, package_name, &edit_id)?;
    commit_play_edit(&client, &token, package_name, &edit_id)?;

    Ok(store_receipt(
        options,
        "play-store",
        artifact_path,
        "published",
        Some(format!("edit:{edit_id}/version:{version_code}")),
        Some(format!(
            "https://play.google.com/console/u/0/developers/app/{package_name}/tracks/{track}"
        )),
        vec![format!(
            "Google Play accepted version code {version_code} on track {track}; provider-side review or processing may still apply."
        )],
    ))
}

fn publish_play_internal_sharing(
    options: &DistributeOptions,
    cfg: &PlayStoreConfig,
    package_name: &str,
    artifact_path: &Path,
    artifact: &Path,
    artifact_kind: &str,
) -> Result<DistributionReceipt> {
    if options.dry_run {
        let upload_url = play_internal_sharing_upload_url(package_name, artifact_kind)?;
        return Ok(DistributionReceipt {
            schema_version: 1,
            created_at_unix_seconds: now_unix_seconds(),
            provider: "play-store".to_string(),
            site: options.site.clone(),
            action: "publish".to_string(),
            artifact_manifest: Some(artifact_path.display().to_string()),
            deployment_id: None,
            canonical_url: Some("https://play.google.com/console".to_string()),
            preview_url: None,
            custom_domain: None,
            status: "dry-run".to_string(),
            stdout: Some(serde_json::to_string_pretty(&json!({
                "mode": "internal-sharing",
                "artifact": artifact.display().to_string(),
                "upload_url": upload_url,
            }))?),
            stderr: None,
            manual_follow_up: vec![
                "Would upload this APK/AAB to Google Play internal app sharing and return a generated download URL.".to_string(),
            ],
        });
    }

    let client = http_client()?;
    let token = google_play_access_token(cfg, &client)?;
    let value = upload_play_internal_sharing_artifact(
        &client,
        &token,
        package_name,
        artifact,
        artifact_kind,
    )?;
    let download_url = value
        .get("downloadUrl")
        .and_then(Value::as_str)
        .map(str::to_string);
    let deployment_id = value
        .get("sha256")
        .and_then(Value::as_str)
        .map(|sha| format!("internal-sharing:{sha}"));
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "play-store".to_string(),
        site: options.site.clone(),
        action: "publish".to_string(),
        artifact_manifest: Some(artifact_path.display().to_string()),
        deployment_id,
        canonical_url: download_url,
        preview_url: None,
        custom_domain: None,
        status: "shared".to_string(),
        stdout: Some(serde_json::to_string_pretty(&value)?),
        stderr: None,
        manual_follow_up: vec![
            "Google Play internal app sharing created a download URL for authorized testers; this is separate from Play tracks and production review.".to_string(),
        ],
    })
}

pub(super) fn publish_app_store(
    options: &DistributeOptions,
    config: &PublishManifest,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
) -> Result<DistributionReceipt> {
    let cfg = app_store_config(config);
    let issuer_id = env_value("APP_STORE_CONNECT_ISSUER_ID")
        .or(cfg.issuer_id.clone())
        .context("distribution.app_store.issuer_id or APP_STORE_CONNECT_ISSUER_ID is required")?;
    let key_id = env_value("APP_STORE_CONNECT_KEY_ID")
        .or(cfg.key_id.clone())
        .context("distribution.app_store.key_id or APP_STORE_CONNECT_KEY_ID is required")?;
    let platform = app_store_platform_for_manifest(&cfg, manifest)?;
    let upload = primary_artifact_with_extensions(manifest, &[platform.artifact_extension()])?;
    let track = options
        .track
        .as_deref()
        .or(cfg.default_track.as_deref())
        .unwrap_or("testflight");
    if options.dry_run {
        return Ok(store_receipt(
            options,
            "app-store",
            artifact_path,
            "dry-run",
            None,
            Some("https://appstoreconnect.apple.com/apps".to_string()),
            vec![format!(
                "Would upload {} to App Store Connect with API key {key_id} for track {track}.",
                upload.display()
            )],
        ));
    }

    if let Some(build_number) = manifest.project.build.map(|value| value.to_string()) {
        let client = http_client()?;
        let token = app_store_access_token(&cfg)?;
        let app_id = app_store_app_id(&cfg, &client, &token)?;
        ensure_app_store_build_number_unused(&client, &token, &app_id, &build_number)?;
    }

    let api_key_file = app_store_api_key_file(&key_id, &cfg)?;
    let mut command = Command::new("xcrun");
    command
        .args([
            "altool",
            "--upload-app",
            "-f",
            upload.to_string_lossy().as_ref(),
            "-t",
            platform.altool_type(),
            "--apiKey",
            &key_id,
            "--apiIssuer",
            &issuer_id,
            "--output-format",
            "json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(parent) = api_key_file.path.parent() {
        command.env(APP_STORE_API_PRIVATE_KEYS_DIR, parent);
    }
    let output = command
        .output()
        .context("failed to run xcrun altool; install Xcode and App Store Connect upload tools")?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() || app_store_upload_reported_error(&stdout, &stderr).is_some() {
        let reported_error = app_store_upload_reported_error(&stdout, &stderr)
            .unwrap_or_else(|| stderr.trim().to_string());
        bail!(
            "App Store Connect upload failed with {}: {}",
            output.status,
            reported_error
        );
    }

    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "app-store".to_string(),
        site: options.site.clone(),
        action: "publish".to_string(),
        artifact_manifest: Some(artifact_path.display().to_string()),
        deployment_id: None,
        canonical_url: Some("https://appstoreconnect.apple.com/apps".to_string()),
        preview_url: None,
        custom_domain: None,
        status: "uploaded".to_string(),
        stdout: (!stdout.trim().is_empty()).then_some(stdout),
        stderr: (!stderr.trim().is_empty()).then_some(stderr),
        manual_follow_up: vec![app_store_upload_follow_up(track, artifact_path)],
    })
}

pub(super) fn app_store_upload_reported_error(stdout: &str, stderr: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<Value>(stdout) {
        if app_store_json_has_errors(&value) {
            return Some(stdout.trim().to_string());
        }
    }
    stderr.lines().find_map(|line| {
        let trimmed = line.trim();
        let uppercase = trimmed.to_ascii_uppercase();
        (uppercase.starts_with("ERROR:") || uppercase.contains(" ERROR:"))
            .then(|| trimmed.to_string())
    })
}

fn app_store_json_has_errors(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields.iter().any(|(key, value)| {
            let error_field = key.to_ascii_lowercase().ends_with("errors");
            error_field
                && !matches!(value, Value::Null)
                && !matches!(value, Value::Array(items) if items.is_empty())
                && !matches!(value, Value::Object(fields) if fields.is_empty())
                || app_store_json_has_errors(value)
        }),
        Value::Array(values) => values.iter().any(app_store_json_has_errors),
        _ => false,
    }
}

pub(super) fn app_store_upload_follow_up(track: &str, artifact_path: &Path) -> String {
    match track {
        "testflight" => format!(
            "App Store Connect accepted the upload; wait for build processing, then run `fission beta distribute --provider app-store --artifact {} --group <group> --yes`.",
            artifact_path.display()
        ),
        "app-store-review" | "app-store-release" => format!(
            "App Store Connect accepted the upload; wait for build processing, then run `fission distribute promote --provider app-store --track app-store-review --artifact {} --yes`.",
            artifact_path.display()
        ),
        _ => format!(
            "App Store Connect accepted the upload; wait for build processing, then choose TestFlight or App Review for track {track}."
        ),
    }
}

pub(super) fn app_store_status(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = app_store_config(config);
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let builds_url = app_store_builds_status_url(&app_id);
    let builds_response = client
        .get(builds_url)
        .bearer_auth(&token)
        .send()
        .context("failed to query App Store Connect build status")?;
    let builds = json_response(builds_response, "App Store Connect build status")?;
    let build_uploads_url = app_store_build_uploads_status_url(&app_id);
    let build_uploads_response = client
        .get(build_uploads_url)
        .bearer_auth(&token)
        .send()
        .context("failed to query App Store Connect build upload status")?;
    let build_uploads = json_response(
        build_uploads_response,
        "App Store Connect build upload status",
    )?;
    let review_submissions_url = format!(
        "{APP_STORE_API}/v1/apps/{app_id}/reviewSubmissions?limit=10&fields[reviewSubmissions]=state,platform&include=items&fields[reviewSubmissionItems]=state,appStoreVersion"
    );
    let review_response = client
        .get(review_submissions_url)
        .bearer_auth(&token)
        .send()
        .context("failed to query App Store review submission status")?;
    let review_submissions = json_response(review_response, "App Store review submissions status")?;
    let beta_groups_url = format!(
        "{APP_STORE_API}/v1/apps/{app_id}/betaGroups?limit=200&fields[betaGroups]=name,isInternalGroup,publicLinkEnabled,publicLink"
    );
    let beta_groups_response = client
        .get(beta_groups_url)
        .bearer_auth(&token)
        .send()
        .context("failed to query App Store TestFlight beta group status")?;
    let beta_groups = json_response(beta_groups_response, "App Store beta groups status")?;
    let status = app_store_observed_status(&builds, &review_submissions, &build_uploads);
    let stdout = json!({
        "app_id": app_id,
        "builds": builds,
        "build_uploads": build_uploads,
        "review_submissions": review_submissions,
        "beta_groups": beta_groups,
    });
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "app-store".to_string(),
        site: options.site.clone(),
        action: "status".to_string(),
        artifact_manifest: None,
        deployment_id: Some(app_id),
        canonical_url: Some("https://appstoreconnect.apple.com/apps".to_string()),
        preview_url: None,
        custom_domain: None,
        status,
        stdout: Some(serde_json::to_string_pretty(&stdout)?),
        stderr: None,
        manual_follow_up: Vec::new(),
    })
}

pub(super) fn app_store_builds_status_url(app_id: &str) -> String {
    format!(
        "{APP_STORE_API}/v1/builds?filter[app]={app_id}&limit=10&sort=-uploadedDate&fields[builds]=version,uploadedDate,processingState,expired,minOsVersion,usesNonExemptEncryption"
    )
}

pub(super) fn app_store_build_uploads_status_url(app_id: &str) -> String {
    format!(
        "{APP_STORE_API}/v1/apps/{app_id}/buildUploads?limit=10&sort=-uploadedDate&fields[buildUploads]=cfBundleShortVersionString,cfBundleVersion,createdDate,state,platform,uploadedDate,build"
    )
}

pub(super) fn app_store_lifecycle(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    app_store_review::lifecycle(options, config)
}

pub(super) fn publish_microsoft_store(
    options: &DistributeOptions,
    config: &PublishManifest,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
) -> Result<DistributionReceipt> {
    let cfg = microsoft_store_config(config);
    let product_id = cfg
        .product_id
        .as_deref()
        .context("distribution.microsoft_store.product_id is required")?;
    let package_type = microsoft_store_package_type(&cfg, manifest);
    if is_microsoft_store_msix_type(&package_type) {
        return publish_microsoft_store_msix(options, &cfg, product_id, artifact_path, manifest);
    }

    let seller_id = microsoft_store_seller_id(&cfg).context(
        "distribution.microsoft_store.seller_id, MICROSOFT_STORE_SELLER_ID, or PARTNER_CENTER_SELLER_ID is required",
    )?;
    let package_url = options
        .deploy
        .as_deref()
        .filter(|value| value.starts_with("https://") || value.starts_with("http://"))
        .map(str::to_string)
        .or(cfg.package_url.clone())
        .context("Microsoft Store MSI/EXE submission requires a package_url in fission.toml or --deploy <https-url>; publish the artifact to S3/static hosting first")?;
    if !matches!(package_type.as_str(), "exe" | "msi") {
        bail!(
            "Microsoft Store direct package automation supports exe/msi through the Store submission API and msix/msixupload through msstore; package_type `{package_type}` is unsupported"
        );
    }
    if options.dry_run {
        return Ok(store_receipt(
            options,
            "microsoft-store",
            artifact_path,
            "dry-run",
            None,
            Some(format!("https://partner.microsoft.com/dashboard/products/{product_id}")),
            vec![format!(
                "Would update Microsoft Store package metadata for product {product_id} with {package_url}."
            )],
        ));
    }

    let client = http_client()?;
    let token = microsoft_store_access_token(&cfg, &client)?;
    let packages = json!({
        "packages": [{
            "packageUrl": package_url,
            "languages": cfg.languages.clone().unwrap_or_else(|| vec!["en-us".to_string()]),
            "architectures": cfg.architectures.clone().unwrap_or_else(|| vec!["Neutral".to_string()]),
            "isSilentInstall": cfg.is_silent_install.unwrap_or(true),
            "installerParameters": cfg.installer_parameters.clone().unwrap_or_default(),
            "genericDocUrl": cfg.generic_doc_url.clone().unwrap_or_default(),
            "packageType": package_type,
        }]
    });
    let packages_url = format!("{MICROSOFT_STORE_API}/submission/v1/product/{product_id}/packages");
    let package_response = client
        .put(&packages_url)
        .bearer_auth(&token)
        .header("X-Seller-Account-Id", &seller_id)
        .json(&packages)
        .send()
        .context("failed to update Microsoft Store package metadata")?;
    let package_value = json_response(package_response, "Microsoft Store package update")?;
    microsoft_store_success(&package_value, "Microsoft Store package update")?;

    let commit_url =
        format!("{MICROSOFT_STORE_API}/submission/v1/product/{product_id}/packages/commit");
    let commit_response = client
        .post(&commit_url)
        .bearer_auth(&token)
        .header("X-Seller-Account-Id", &seller_id)
        .send()
        .context("failed to commit Microsoft Store package metadata")?;
    let commit_value = json_response(commit_response, "Microsoft Store package commit")?;
    microsoft_store_success(&commit_value, "Microsoft Store package commit")?;
    let polling_url = commit_value
        .pointer("/responseData/pollingUrl")
        .and_then(Value::as_str)
        .map(|value| {
            if value.starts_with("http") {
                value.to_string()
            } else {
                format!("{MICROSOFT_STORE_API}{value}")
            }
        });

    let mut follow_up = vec![
        "Microsoft Store package update was committed; poll Partner Center processing before submitting to certification.".to_string(),
    ];
    let mut status = "package-committed".to_string();
    if cfg.submit.unwrap_or(false) || options.track.as_deref() == Some("public") && options.yes {
        let submit_url = format!("{MICROSOFT_STORE_API}/submission/v1/product/{product_id}/submit");
        let submit_response = client
            .post(&submit_url)
            .bearer_auth(&token)
            .header("X-Seller-Account-Id", &seller_id)
            .send()
            .context("failed to create Microsoft Store submission")?;
        let submit_value = json_response(submit_response, "Microsoft Store submission")?;
        microsoft_store_success(&submit_value, "Microsoft Store submission")?;
        status = "submitted".to_string();
        follow_up.push("Microsoft Store submission was created; certification/review continues in Partner Center.".to_string());
    } else {
        follow_up.push("Set distribution.microsoft_store.submit = true or pass --track public --yes when you are ready to submit the draft to certification.".to_string());
    }

    Ok(store_receipt(
        options,
        "microsoft-store",
        artifact_path,
        &status,
        polling_url,
        Some(format!(
            "https://partner.microsoft.com/dashboard/products/{product_id}"
        )),
        follow_up,
    ))
}

fn publish_microsoft_store_msix(
    options: &DistributeOptions,
    cfg: &MicrosoftStoreConfig,
    product_id: &str,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
) -> Result<DistributionReceipt> {
    let artifact = primary_artifact_with_extensions(manifest, MICROSOFT_STORE_MSIX_TYPES)?;
    let flight_id = microsoft_store_flight_id(options.track.as_deref(), cfg)?;
    let rollout = microsoft_store_rollout_percentage(cfg)?;
    let should_submit = microsoft_store_should_submit(options, cfg);
    let project_path = microsoft_store_msstore_project(options, cfg);
    let publish_args = msstore_publish_args(
        &project_path,
        &artifact,
        product_id,
        flight_id.as_deref(),
        rollout,
        should_submit,
    );

    if options.dry_run {
        return Ok(store_receipt(
            options,
            "microsoft-store",
            artifact_path,
            "dry-run",
            None,
            Some(format!(
                "https://partner.microsoft.com/dashboard/products/{product_id}"
            )),
            vec![format!(
                "Would run `{}` to publish {} through Microsoft Store Developer CLI.",
                command_line("msstore", &publish_args),
                artifact.display()
            )],
        ));
    }

    let mut stdout_parts = Vec::new();
    let mut stderr_parts = Vec::new();

    let (stdout, stderr) = run_msstore(&publish_args, "Microsoft Store MSIX publish")?;
    if !stdout.trim().is_empty() {
        stdout_parts.push(stdout);
    }
    if !stderr.trim().is_empty() {
        stderr_parts.push(stderr);
    }

    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "microsoft-store".to_string(),
        site: options.site.clone(),
        action: "publish".to_string(),
        artifact_manifest: Some(artifact_path.display().to_string()),
        deployment_id: flight_id
            .map(|flight| format!("product:{product_id}/flight:{flight}"))
            .or_else(|| Some(format!("product:{product_id}"))),
        canonical_url: Some(format!(
            "https://partner.microsoft.com/dashboard/products/{product_id}"
        )),
        preview_url: None,
        custom_domain: None,
        status: if should_submit {
            "submitted".to_string()
        } else {
            "draft-updated".to_string()
        },
        stdout: (!stdout_parts.is_empty()).then(|| stdout_parts.join("\n")),
        stderr: (!stderr_parts.is_empty()).then(|| stderr_parts.join("\n")),
        manual_follow_up: if should_submit {
            vec!["Microsoft Store Developer CLI committed the submission; certification/review continues in Partner Center.".to_string()]
        } else {
            vec!["The MSIX submission remains a Partner Center draft because --noCommit was used. Set distribution.microsoft_store.submit = true or pass --track public --yes when you are ready to commit it.".to_string()]
        },
    })
}

pub(super) fn play_store_status(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = play_store_config(config);
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required")?;
    let track = options
        .track
        .as_deref()
        .or(cfg.default_track.as_deref())
        .unwrap_or("internal");
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/tracks/{track}"
    );
    let response = client
        .get(url)
        .bearer_auth(&token)
        .send()
        .with_context(|| format!("failed to read Google Play track {track}"))?;
    let value = json_response(response, "Google Play track get")?;
    let status = play_track_status(&value);
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "play-store".to_string(),
        site: options.site.clone(),
        action: "status".to_string(),
        artifact_manifest: None,
        deployment_id: Some(format!("edit:{edit_id}/track:{track}")),
        canonical_url: Some(format!(
            "https://play.google.com/console/u/0/developers/app/{package_name}/tracks/{track}"
        )),
        preview_url: None,
        custom_domain: None,
        status,
        stdout: Some(serde_json::to_string_pretty(&value)?),
        stderr: None,
        manual_follow_up: Vec::new(),
    })
}

pub(super) fn microsoft_store_status(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = microsoft_store_config(config);
    let product_id = cfg
        .product_id
        .as_deref()
        .context("distribution.microsoft_store.product_id is required")?;
    if cfg
        .package_type
        .as_deref()
        .map(|value| is_microsoft_store_msix_type(&value.to_ascii_lowercase()))
        .unwrap_or(false)
    {
        return microsoft_store_msix_status(options, &cfg, product_id);
    }

    let seller_id = microsoft_store_seller_id(&cfg).context(
        "distribution.microsoft_store.seller_id, MICROSOFT_STORE_SELLER_ID, or PARTNER_CENTER_SELLER_ID is required",
    )?;
    let client = http_client()?;
    let token = microsoft_store_access_token(&cfg, &client)?;
    let url = options
        .deploy
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            if value.starts_with("http") {
                value.to_string()
            } else if value.starts_with('/') {
                format!("{MICROSOFT_STORE_API}{value}")
            } else {
                format!("{MICROSOFT_STORE_API}/submission/v1/product/{product_id}/submission/{value}/status")
            }
        })
        .unwrap_or_else(|| format!("{MICROSOFT_STORE_API}/submission/v1/product/{product_id}/status"));
    let response = client
        .get(url)
        .bearer_auth(&token)
        .header("X-Seller-Account-Id", &seller_id)
        .send()
        .context("failed to query Microsoft Store submission status")?;
    let value = json_response(response, "Microsoft Store status")?;
    microsoft_store_success(&value, "Microsoft Store status")?;
    let status = value
        .pointer("/responseData/publishingStatus")
        .or_else(|| value.pointer("/responseData/packageUploadStatus"))
        .and_then(Value::as_str)
        .unwrap_or("ok")
        .to_ascii_lowercase();
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "microsoft-store".to_string(),
        site: options.site.clone(),
        action: "status".to_string(),
        artifact_manifest: None,
        deployment_id: options.deploy.clone(),
        canonical_url: Some(format!(
            "https://partner.microsoft.com/dashboard/products/{product_id}"
        )),
        preview_url: None,
        custom_domain: None,
        status,
        stdout: Some(serde_json::to_string_pretty(&value)?),
        stderr: None,
        manual_follow_up: Vec::new(),
    })
}

pub(super) fn app_store_observed_status(
    builds: &Value,
    review_submissions: &Value,
    build_uploads: &Value,
) -> String {
    app_store_latest_review_submission_status(review_submissions)
        .or_else(|| {
            let status = app_store_latest_build_status(builds);
            (status != "no-builds").then_some(status)
        })
        .or_else(|| app_store_latest_build_upload_status(build_uploads))
        .unwrap_or_else(|| "no-builds".to_string())
}

fn app_store_latest_build_upload_status(value: &Value) -> Option<String> {
    let state = value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("attributes"))
        .and_then(|attributes| attributes.get("state"))?;
    state
        .as_str()
        .or_else(|| state.get("state").and_then(Value::as_str))
        .map(|state| state.to_ascii_lowercase())
}

fn app_store_latest_review_submission_status(value: &Value) -> Option<String> {
    value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("attributes"))
        .and_then(|attributes| attributes.get("state"))
        .and_then(Value::as_str)
        .map(|state| state.to_ascii_lowercase())
}

fn play_track_status(value: &Value) -> String {
    value
        .get("releases")
        .and_then(Value::as_array)
        .and_then(|releases| releases.first())
        .and_then(|release| release.get("status"))
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("track")
                .and_then(Value::as_str)
                .map(|_| "track-active")
        })
        .unwrap_or("ok")
        .to_ascii_lowercase()
}

fn is_play_internal_sharing_track(track: &str) -> bool {
    matches!(track, "internal-sharing" | "internal-app-sharing")
}

fn app_store_latest_build_status(value: &Value) -> String {
    value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("attributes"))
        .and_then(|attributes| attributes.get("processingState"))
        .and_then(Value::as_str)
        .unwrap_or("no-builds")
        .to_ascii_lowercase()
}

fn microsoft_store_msix_status(
    options: &DistributeOptions,
    cfg: &MicrosoftStoreConfig,
    product_id: &str,
) -> Result<DistributionReceipt> {
    let flight_id = microsoft_store_flight_id(options.track.as_deref(), cfg)?;
    let args = if let Some(flight_id) = flight_id.as_deref() {
        vec![
            "flights".to_string(),
            "submission".to_string(),
            "status".to_string(),
            product_id.to_string(),
            flight_id.to_string(),
        ]
    } else {
        vec![
            "submission".to_string(),
            "status".to_string(),
            product_id.to_string(),
        ]
    };
    let (stdout, stderr) = run_msstore(&args, "Microsoft Store MSIX submission status")?;
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "microsoft-store".to_string(),
        site: options.site.clone(),
        action: "status".to_string(),
        artifact_manifest: None,
        deployment_id: flight_id
            .map(|flight| format!("product:{product_id}/flight:{flight}"))
            .or_else(|| Some(format!("product:{product_id}"))),
        canonical_url: Some(format!(
            "https://partner.microsoft.com/dashboard/products/{product_id}"
        )),
        preview_url: None,
        custom_domain: None,
        status: "ok".to_string(),
        stdout: (!stdout.trim().is_empty()).then_some(stdout),
        stderr: (!stderr.trim().is_empty()).then_some(stderr),
        manual_follow_up: Vec::new(),
    })
}

fn play_version_code_state_check(
    project_dir: &Path,
    selected_track: &str,
    cfg: &PlayStoreConfig,
    artifact_version_code: Option<String>,
) -> ReadinessCheck {
    let Some(package_name) = cfg
        .package_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return check(
            "release.play_store.version_code_available",
            CheckSeverity::Info,
            CheckStatus::Skipped,
            "Google Play version code availability was not checked",
            Some("package name is not configured".to_string()),
            vec!["Set distribution.play_store.package_name."],
        );
    };
    let version_code = match artifact_version_code {
        Some(version_code) => Ok(Some(version_code)),
        None => configured_android_version_code(project_dir),
    };
    let Ok(version_code) = version_code else {
        return check(
            "release.play_store.version_code_available",
            CheckSeverity::Error,
            CheckStatus::Failed,
            "Google Play version code availability could not be checked",
            None,
            vec!["Fix fission.toml so the Android version code can be read."],
        );
    };
    let Some(version_code) = version_code else {
        return check(
            "release.play_store.version_code_available",
            CheckSeverity::Error,
            CheckStatus::Missing,
            "Android version code is configured before Google Play upload",
            None,
            vec!["Set [package.android].version_code or [app].build before publishing to Google Play."],
        );
    };
    if !play_credentials_env_available(cfg) {
        return check(
            "release.play_store.version_code_available",
            CheckSeverity::Info,
            CheckStatus::Skipped,
            "Google Play version code availability was not checked",
            Some("provider credentials are not available".to_string()),
            vec!["Set Play credentials to let readiness query existing Play tracks and artifact inventories before packaging."],
        );
    }

    let client = match http_client() {
        Ok(client) => client,
        Err(error) => {
            return check(
                "release.play_store.version_code_available",
                CheckSeverity::Error,
                CheckStatus::Failed,
                "Google Play version code availability could not be checked",
                Some(error.to_string()),
                vec!["Fix the HTTP client environment and rerun readiness."],
            )
        }
    };
    let token = match google_play_access_token(cfg, &client) {
        Ok(token) => token,
        Err(error) => {
            return check(
                "release.play_store.version_code_available",
                CheckSeverity::Error,
                CheckStatus::Failed,
                "Google Play version code availability could not be checked",
                Some(error.to_string()),
                vec!["Fix Play credentials and rerun readiness."],
            )
        }
    };
    let edit_id = match create_play_edit(&client, &token, package_name) {
        Ok(edit_id) => edit_id,
        Err(error) => {
            return check(
                "release.play_store.version_code_available",
                CheckSeverity::Error,
                CheckStatus::Failed,
                "Google Play app state could not be queried",
                Some(error.to_string()),
                vec!["Verify the service account has access to the Play Console app and rerun readiness."],
            )
        }
    };
    match ensure_play_version_code_unused(
        &client,
        &token,
        package_name,
        &edit_id,
        selected_track,
        &version_code,
    ) {
        Ok(()) => check(
            "release.play_store.version_code_available",
            CheckSeverity::Error,
            CheckStatus::Passed,
            "Google Play version code has not been used",
            Some(format!("{package_name} versionCode {version_code}")),
            vec!["Keep this version code for the next package build."],
        ),
        Err(error) => check(
            "release.play_store.version_code_available",
            CheckSeverity::Error,
            CheckStatus::Failed,
            "Google Play version code has already been used",
            Some(error.to_string()),
            vec![
                "Run `fission release-config bump-build --target android --yes`, rebuild the Android artifact, then publish again.",
            ],
        ),
    }
}

fn android_version_code_for_provider(
    project_dir: &Path,
    artifact_manifest: Option<&ArtifactManifest>,
) -> Result<Option<String>> {
    if let Some(build) = artifact_manifest.and_then(|manifest| manifest.project.build) {
        return Ok(Some(build.to_string()));
    }
    configured_android_version_code(project_dir)
}

fn app_store_build_number_state_check(
    project_dir: &Path,
    cfg: &AppStoreConfig,
    artifact_manifest: Option<&ArtifactManifest>,
) -> ReadinessCheck {
    if cfg
        .app_id
        .as_deref()
        .or(cfg.bundle_id.as_deref())
        .filter(|value| !value.trim().is_empty())
        .is_none()
    {
        return check(
            "release.app_store.build_number_available",
            CheckSeverity::Info,
            CheckStatus::Skipped,
            "App Store build number availability was not checked",
            Some("app id or bundle id is not configured".to_string()),
            vec!["Set distribution.app_store.app_id or distribution.app_store.bundle_id."],
        );
    }
    let build_number = match app_store_build_number_for_provider(project_dir, artifact_manifest) {
        Ok(value) => value,
        Err(error) => {
            return check(
                "release.app_store.build_number_available",
                CheckSeverity::Error,
                CheckStatus::Failed,
                "App Store build number availability could not be checked",
                Some(error.to_string()),
                vec!["Fix fission.toml so the iOS build number can be read."],
            );
        }
    };
    let Some(build_number) = build_number else {
        return check(
            "release.app_store.build_number_available",
            CheckSeverity::Error,
            CheckStatus::Missing,
            "iOS build number is configured before App Store upload",
            None,
            vec!["Set [package.ios].build_number or [app].build before publishing to App Store Connect."],
        );
    };
    if !app_store_credentials_available_for_cfg(cfg) {
        return check(
            "release.app_store.build_number_available",
            CheckSeverity::Info,
            CheckStatus::Skipped,
            "App Store build number availability was not checked",
            Some("provider credentials are not available".to_string()),
            vec!["Set App Store Connect credentials to let readiness query existing builds before packaging."],
        );
    }

    let client = match http_client() {
        Ok(client) => client,
        Err(error) => {
            return check(
                "release.app_store.build_number_available",
                CheckSeverity::Error,
                CheckStatus::Failed,
                "App Store build number availability could not be checked",
                Some(error.to_string()),
                vec!["Fix the HTTP client environment and rerun readiness."],
            );
        }
    };
    let token = match app_store_access_token(cfg) {
        Ok(token) => token,
        Err(error) => {
            return check(
                "release.app_store.build_number_available",
                CheckSeverity::Error,
                CheckStatus::Failed,
                "App Store build number availability could not be checked",
                Some(error.to_string()),
                vec!["Fix App Store Connect credentials and rerun readiness."],
            );
        }
    };
    let app_id = match app_store_app_id(cfg, &client, &token) {
        Ok(app_id) => app_id,
        Err(error) => {
            return check(
                "release.app_store.build_number_available",
                CheckSeverity::Error,
                CheckStatus::Failed,
                "App Store app state could not be queried",
                Some(error.to_string()),
                vec![
                    "Verify the API key can access the App Store Connect app and rerun readiness.",
                ],
            );
        }
    };
    match ensure_app_store_build_number_unused(&client, &token, &app_id, &build_number) {
        Ok(()) => check(
            "release.app_store.build_number_available",
            CheckSeverity::Error,
            CheckStatus::Passed,
            "App Store build number has not been used",
            Some(format!("app {app_id} build {build_number}")),
            vec!["Keep this build number for the next IPA build."],
        ),
        Err(error) => check(
            "release.app_store.build_number_available",
            CheckSeverity::Error,
            CheckStatus::Failed,
            "App Store build number has already been used",
            Some(error.to_string()),
            vec![
                "Run `fission release-config bump-build --target ios --yes`, rebuild the IPA, then publish again.",
            ],
        ),
    }
}

fn app_store_build_number_for_provider(
    project_dir: &Path,
    artifact_manifest: Option<&ArtifactManifest>,
) -> Result<Option<String>> {
    if let Some(build) = artifact_manifest.and_then(|manifest| manifest.project.build) {
        return Ok(Some(build.to_string()));
    }
    configured_ios_build_number(project_dir)
}

pub(super) fn readiness_play_store(
    project_dir: &Path,
    track: Option<&str>,
    artifact: Option<&Path>,
    config: &PublishManifest,
    checks: &mut Vec<ReadinessCheck>,
) -> Result<()> {
    let cfg = play_store_config(config);
    checks.push(required_value(
        "release.play_store.package_name_configured",
        cfg.package_name.as_deref(),
        "Google Play package name is configured",
        "Set distribution.play_store.package_name to the Android application id registered in Play Console.",
    ));
    let play_env_names = play_credential_env_names(&cfg);
    checks.push(secret_check_env_names(
        "release.play_store.credentials_available",
        &play_env_names,
        &format!(
            "Set {} or {} from CI secrets, set {}, or set {}.",
            play_env_names[1], play_env_names[2], play_env_names[0], play_env_names[3]
        ),
    ));
    let selected_track = track.or(cfg.default_track.as_deref()).unwrap_or("internal");
    checks.push(check(
        "release.play_store.track_supported",
        CheckSeverity::Error,
        if matches!(
            selected_track,
            "internal"
                | "closed"
                | "open"
                | "production"
                | "internal-sharing"
                | "internal-app-sharing"
        ) {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        "Google Play track is supported",
        Some(selected_track.to_string()),
        vec!["Use internal, closed, open, production, or internal-sharing."],
    ));
    let mut artifact_version_code = None;
    if let Some(path) = artifact.filter(|path| path.exists()) {
        let manifest = read_artifact_manifest(path)?;
        artifact_version_code = manifest.project.build.map(|build| build.to_string());
        checks.push(artifact_format_check(
            "release.play_store.artifact_format",
            &manifest,
            &["aab", "apk"],
            "Google Play accepts Android App Bundles for production publishing and APKs for legacy/test flows.",
        ));
    }
    if is_play_internal_sharing_track(selected_track) {
        checks.push(check(
            "release.play_store.version_code_available",
            CheckSeverity::Info,
            CheckStatus::Skipped,
            "Google Play version code availability is not required for internal app sharing",
            Some(selected_track.to_string()),
            vec!["Internal app sharing uploads do not mutate a Play release track."],
        ));
    } else {
        checks.push(play_version_code_state_check(
            project_dir,
            selected_track,
            &cfg,
            artifact_version_code,
        ));
    }
    checks.push(check(
        "release.play_store.first_setup_manual_steps",
        CheckSeverity::Warning,
        CheckStatus::Warning,
        "first Google Play setup may require Play Console work",
        cfg.package_name.clone(),
        vec!["Create the Play Console app, configure Play App Signing, complete policy/listing/data-safety requirements, and grant the service account access before first automation."],
    ));
    Ok(())
}

pub(super) fn readiness_app_store(
    project_dir: &Path,
    track: Option<&str>,
    artifact: Option<&Path>,
    config: &PublishManifest,
    checks: &mut Vec<ReadinessCheck>,
) -> Result<()> {
    let cfg = app_store_config(config);
    checks.push(required_value(
        "release.app_store.bundle_id_configured",
        cfg.bundle_id.as_deref(),
        "App Store bundle id is configured",
        "Set distribution.app_store.bundle_id to the Bundle ID registered in App Store Connect.",
    ));
    let issuer_id_env = app_store_issuer_id_env(&cfg);
    let key_id_env = app_store_key_id_env(&cfg);
    checks.push(required_value(
        "release.app_store.issuer_id_configured",
        cfg.issuer_id
            .as_deref()
            .or_else(|| env_value_ref(&issuer_id_env)),
        "App Store Connect issuer id is configured",
        &format!("Set distribution.app_store.issuer_id or {issuer_id_env}."),
    ));
    checks.push(required_value(
        "release.app_store.key_id_configured",
        cfg.key_id.as_deref().or_else(|| env_value_ref(&key_id_env)),
        "App Store Connect key id is configured",
        &format!("Set distribution.app_store.key_id or {key_id_env}."),
    ));
    let app_store_key_env_names = app_store_api_key_env_names(&cfg);
    checks.push(secret_check_env_names(
        "release.app_store.credentials_available",
        &app_store_key_env_names,
        &format!(
            "Set {} or {} from CI secrets, or set {} locally.",
            app_store_key_env_names[0], app_store_key_env_names[1], app_store_key_env_names[2]
        ),
    ));
    checks.push(check_tool(
        "release.app_store.xcrun_available",
        "xcrun",
        "Install Xcode and select it with xcode-select before uploading IPA files.",
    ));
    let selected_track = track
        .or(cfg.default_track.as_deref())
        .unwrap_or("testflight");
    checks.push(check(
        "release.app_store.track_supported",
        CheckSeverity::Error,
        if matches!(
            selected_track,
            "testflight" | "app-store-review" | "app-store-release"
        ) {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        "App Store destination is supported",
        Some(selected_track.to_string()),
        vec!["Use testflight, app-store-review, or app-store-release."],
    ));
    if let Some(path) = artifact.filter(|path| path.exists()) {
        let manifest = read_artifact_manifest(path)?;
        let platform = app_store_platform_for_manifest(&cfg, &manifest);
        checks.push(check(
            "release.app_store.artifact_format",
            CheckSeverity::Error,
            if platform.is_ok() {
                CheckStatus::Passed
            } else {
                CheckStatus::Failed
            },
            "artifact is an iOS .ipa or macOS .pkg",
            Some(
                platform
                    .map(|platform| {
                        format!(
                            "target={} format={} platform={}",
                            manifest.target,
                            manifest.format,
                            platform.target()
                        )
                    })
                    .unwrap_or_else(|error| error.to_string()),
            ),
            vec![
                "Build an iOS .ipa or macOS .pkg and configure distribution.app_store.platform to match.",
            ],
        ));
        checks.push(app_store_build_number_state_check(
            project_dir,
            &cfg,
            Some(&manifest),
        ));
    } else {
        checks.push(app_store_build_number_state_check(project_dir, &cfg, None));
    }
    checks.push(check(
        "release.app_store.first_setup_manual_steps",
        CheckSeverity::Warning,
        CheckStatus::Warning,
        "first App Store setup may require App Store Connect work",
        cfg.bundle_id.clone(),
        vec!["Create the Bundle ID, certificates, provisioning profiles, App Store Connect app record, metadata, privacy, pricing, and beta groups before first automation."],
    ));
    Ok(())
}

pub(super) fn readiness_microsoft_store(
    project_dir: &Path,
    track: Option<&str>,
    format: Option<PackageFormat>,
    artifact: Option<&Path>,
    config: &PublishManifest,
    checks: &mut Vec<ReadinessCheck>,
) -> Result<()> {
    let cfg = microsoft_store_config(config);
    let artifact_manifest = artifact
        .filter(|path| path.exists())
        .map(read_artifact_manifest)
        .transpose()?;
    let package_type = artifact_manifest
        .as_ref()
        .map(|manifest| microsoft_store_package_type(&cfg, manifest))
        .or_else(|| artifact.and_then(microsoft_store_package_type_from_artifact_path))
        .or_else(|| format.and_then(microsoft_store_package_type_from_format))
        .or_else(|| {
            cfg.package_type
                .clone()
                .map(|value| value.to_ascii_lowercase())
        })
        .unwrap_or_else(|| "exe".to_string());
    let uses_msix = is_microsoft_store_msix_type(&package_type);

    checks.push(required_value(
        "release.microsoft_store.product_id_configured",
        cfg.product_id.as_deref(),
        "Microsoft Store product id is configured",
        "Set distribution.microsoft_store.product_id after reserving the product in Partner Center.",
    ));
    checks.push(required_value(
        "release.microsoft_store.package_identity_configured",
        cfg.package_identity_name.as_deref(),
        "Microsoft Store package identity name is configured",
        "Set distribution.microsoft_store.package_identity_name to the Partner Center package identity.",
    ));

    if uses_msix {
        checks.push(check_tool(
            "release.microsoft_store.msstore_available",
            "msstore",
            "Install Microsoft Store Developer CLI and configure it with `msstore` or CI-managed msstore credentials before publishing MSIX/MSIXUPLOAD artifacts.",
        ));
        checks.push(check(
            "release.microsoft_store.msix_uses_msstore",
            CheckSeverity::Info,
            CheckStatus::Passed,
            "MSIX submission uses Microsoft Store Developer CLI",
            Some(package_type.clone()),
            vec!["Fission calls `msstore publish --inputFile ... --appId ...`; no durable package_url is required for MSIX/MSIXUPLOAD submissions."],
        ));
        checks.push(check(
            "release.microsoft_store.msstore_config_external",
            CheckSeverity::Warning,
            CheckStatus::Warning,
            "Microsoft Store Developer CLI credentials are managed outside fission.toml",
            None,
            vec!["Run `msstore` interactively once or configure the msstore CLI in CI; Fission does not pass Partner Center client secrets on the command line."],
        ));
    } else {
        let token_env = microsoft_store_token_env(&cfg);
        let token_available = env::var_os(&token_env).is_some();
        let seller_id_env = microsoft_store_seller_id_env(&cfg);
        let tenant_id_env = microsoft_store_tenant_id_env(&cfg);
        let client_id_env = microsoft_store_client_id_env(&cfg);
        let client_secret_env = microsoft_store_client_secret_env(&cfg);
        checks.push(required_value(
            "release.microsoft_store.seller_id_configured",
            microsoft_store_seller_id(&cfg).as_deref(),
            "Microsoft Store seller id is configured",
            &format!(
                "Set distribution.microsoft_store.seller_id, {seller_id_env}, or PARTNER_CENTER_SELLER_ID."
            ),
        ));
        if token_available {
            checks.push(check(
                "release.microsoft_store.credentials_available",
                CheckSeverity::Error,
                CheckStatus::Passed,
                "Microsoft Store access token is available",
                Some(format!("environment variable {token_env}")),
                vec!["Rotate short-lived access tokens through your shell or CI secret store."],
            ));
        } else {
            checks.push(required_value(
                "release.microsoft_store.tenant_id_configured",
                microsoft_store_tenant_id(&cfg).as_deref(),
                "Microsoft Entra tenant id is configured",
                &format!(
                    "Set distribution.microsoft_store.tenant_id, {tenant_id_env}, or PARTNER_CENTER_TENANT_ID."
                ),
            ));
            checks.push(required_value(
                "release.microsoft_store.client_id_configured",
                microsoft_store_client_id(&cfg).as_deref(),
                "Microsoft Entra client id is configured",
                &format!(
                    "Set distribution.microsoft_store.client_id, {client_id_env}, or PARTNER_CENTER_CLIENT_ID."
                ),
            ));
            let microsoft_secret_env_names = vec![
                client_secret_env,
                "PARTNER_CENTER_CLIENT_SECRET".to_string(),
            ];
            checks.push(secret_check_env_names(
                "release.microsoft_store.credentials_available",
                &microsoft_secret_env_names,
                &format!(
                    "Set {} or {} from your shell or CI secret store, or set {token_env} to a short-lived access token.",
                    microsoft_secret_env_names[0], microsoft_secret_env_names[1]
                ),
            ));
        }
        checks.push(required_value(
            "release.microsoft_store.package_url_configured",
            cfg.package_url.as_deref(),
            "Microsoft Store package URL is configured for MSI/EXE submissions",
            "Upload the package to a durable HTTPS URL first, then set distribution.microsoft_store.package_url or pass --deploy <https-url>.",
        ));
    }

    let selected_track = track.unwrap_or("public");
    if uses_msix && selected_track == "private" {
        checks.push(required_value(
            "release.microsoft_store.flight_id_configured",
            cfg.flight_id.as_deref(),
            "Microsoft Store package flight id is configured",
            "Set distribution.microsoft_store.flight_id or pass the Partner Center package-flight id directly with --track <flight-id>.",
        ));
    }
    checks.push(check(
        "release.microsoft_store.track_supported",
        CheckSeverity::Warning,
        if selected_track == "public"
            || selected_track == "private"
            || uses_msix && !selected_track.trim().is_empty()
        {
            CheckStatus::Passed
        } else {
            CheckStatus::Warning
        },
        "Microsoft Store destination is understood",
        Some(selected_track.to_string()),
        vec!["Use public, private, or an MSIX package-flight id when publishing through Microsoft Store Developer CLI."],
    ));

    if let Some(manifest) = artifact_manifest.as_ref() {
        checks.push(artifact_format_check(
            "release.microsoft_store.artifact_format",
            manifest,
            if uses_msix {
                &["msix", "msixupload"]
            } else {
                &["exe", "msi"]
            },
            if uses_msix {
                "Build a Windows MSIX artifact before publishing to the Microsoft Store MSIX path."
            } else {
                "Build a Windows MSI or EXE artifact before using the Store MSI/EXE submission API."
            },
        ));
        checks.push(microsoft_store_package_version_check(
            Some(manifest),
            project_dir,
        ));
        if uses_msix {
            checks.push(check(
                "release.microsoft_store.msix_upload_artifact_present",
                CheckSeverity::Error,
                if has_artifact_with_extension(manifest, MICROSOFT_STORE_MSIX_TYPES) {
                    CheckStatus::Passed
                } else {
                    CheckStatus::Missing
                },
                "artifact manifest contains an MSIX/MSIXUPLOAD file",
                Some(format!("checked: {}", MICROSOFT_STORE_MSIX_TYPES.join(", "))),
                vec!["Rebuild the Windows MSIX package and ensure the artifact manifest includes the .msix or .msixupload file."],
            ));
        }
    } else {
        checks.push(microsoft_store_package_version_check(None, project_dir));
    }
    checks.push(check(
        "release.microsoft_store.first_setup_manual_steps",
        CheckSeverity::Warning,
        CheckStatus::Warning,
        "first Microsoft Store setup may require Partner Center work",
        cfg.product_id.clone(),
        vec!["Reserve the app, complete first submission/ratings/pricing, associate the Entra app with Partner Center, and verify package identity before first automation."],
    ));
    Ok(())
}

fn microsoft_store_package_version_check(
    manifest: Option<&ArtifactManifest>,
    project_dir: &Path,
) -> ReadinessCheck {
    let (version, build, source) = if let Some(manifest) = manifest {
        (
            manifest.project.version.clone(),
            manifest.project.build,
            "artifact manifest",
        )
    } else {
        match resolve_release_version_config(project_dir, Some(Target::Windows)) {
            Ok(release) => (release.version, release.build, "fission.toml"),
            Err(error) => {
                return check(
                    "release.microsoft_store.package_version_valid",
                    CheckSeverity::Error,
                    CheckStatus::Failed,
                    "Microsoft Store package version could not be resolved",
                    Some(error.to_string()),
                    vec!["Fix fission.toml so the Windows package version/build can be resolved before packaging."],
                )
            }
        }
    };

    match normalize_windows_package_version(version.as_deref(), build) {
        Ok(version) => check(
            "release.microsoft_store.package_version_valid",
            CheckSeverity::Error,
            CheckStatus::Passed,
            "Microsoft Store package version is valid",
            Some(format!("{version} from {source}")),
            vec!["Keep this package version for the next Windows package build."],
        ),
        Err(error) => check(
            "release.microsoft_store.package_version_valid",
            CheckSeverity::Error,
            CheckStatus::Failed,
            "Microsoft Store package version is invalid",
            Some(format!("{source}: {error}")),
            vec!["Set [package.windows].version or [app].version/build to a numeric Windows package version, then rebuild the artifact."],
        ),
    }
}

fn microsoft_store_package_type_from_artifact_path(path: &Path) -> Option<String> {
    path.parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|value| matches!(value.as_str(), "msix" | "msixupload" | "msi" | "exe"))
}

fn microsoft_store_package_type_from_format(format: PackageFormat) -> Option<String> {
    match format {
        PackageFormat::Msix => Some("msix".to_string()),
        PackageFormat::Msi => Some("msi".to_string()),
        PackageFormat::Exe => Some("exe".to_string()),
        _ => None,
    }
}

#[cfg(test)]
#[path = "stores_tests.rs"]
mod tests;
