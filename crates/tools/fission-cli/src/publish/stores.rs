use super::*;
use crate::release;
use anyhow::{bail, Context, Result};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

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
                "Would upload {} to Google Play package {package_name} track {track} with release status {release_status}.",
                artifact.display()
            )],
        ));
    }

    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
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
    let api_key_path = env_value("APP_STORE_CONNECT_API_KEY_PATH").or(cfg.api_key_path.clone());
    let ipa = primary_artifact_with_extensions(manifest, &["ipa"])?;
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
                ipa.display()
            )],
        ));
    }

    let mut command = Command::new("xcrun");
    command
        .args([
            "altool",
            "--upload-app",
            "-f",
            ipa.to_string_lossy().as_ref(),
            "-t",
            "ios",
            "--apiKey",
            &key_id,
            "--apiIssuer",
            &issuer_id,
            "--output-format",
            "json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(api_key_path) = api_key_path
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let path = Path::new(api_key_path);
        if let Some(parent) = path.parent() {
            command.env(APP_STORE_API_PRIVATE_KEYS_DIR, parent);
        }
    }
    let output = command
        .output()
        .context("failed to run xcrun altool; install Xcode and App Store Connect upload tools")?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        bail!(
            "App Store Connect upload failed with {}: {}",
            output.status,
            stderr.trim()
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
        manual_follow_up: vec![format!(
            "App Store Connect accepted the upload; wait for build processing, then assign the build to {track} or App Review."
        )],
    })
}

pub(super) fn app_store_status(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = app_store_config(config);
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let url = format!(
        "{APP_STORE_API}/v1/apps/{app_id}/builds?limit=10&sort=-uploadedDate&fields[builds]=version,uploadedDate,processingState,expired,minOsVersion,usesNonExemptEncryption"
    );
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .context("failed to query App Store Connect build status")?;
    let value = json_response(response, "App Store Connect build status")?;
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
        status: "ok".to_string(),
        stdout: Some(serde_json::to_string_pretty(&value)?),
        stderr: None,
        manual_follow_up: Vec::new(),
    })
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
    if cfg.msstore_reconfigure.unwrap_or(false) {
        let (stdout, stderr) = run_msstore_reconfigure(cfg)?;
        if !stdout.trim().is_empty() {
            stdout_parts.push(stdout);
        }
        if !stderr.trim().is_empty() {
            stderr_parts.push(stderr);
        }
    }

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
        status: "ok".to_string(),
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

pub(super) fn readiness_play_store(
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
    checks.push(secret_check(
        "release.play_store.credentials_available",
        &["PLAY_STORE_ACCESS_TOKEN", "PLAY_STORE_SERVICE_ACCOUNT_JSON", "GOOGLE_APPLICATION_CREDENTIALS"],
        DistributionProvider::PlayStore,
        "Set PLAY_STORE_SERVICE_ACCOUNT_JSON to a service-account JSON path/value, set PLAY_STORE_ACCESS_TOKEN, or import credentials with `fission auth import play-store --from file:<service-account.json> --yes`.",
    ));
    let selected_track = track.or(cfg.default_track.as_deref()).unwrap_or("internal");
    checks.push(check(
        "release.play_store.track_supported",
        CheckSeverity::Error,
        if matches!(selected_track, "internal" | "closed" | "open" | "production") {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        "Google Play track is supported",
        Some(selected_track.to_string()),
        vec!["Use internal, closed, open, or production. Internal app sharing will be a separate explicit provider mode."],
    ));
    if let Some(path) = artifact.filter(|path| path.exists()) {
        let manifest = read_artifact_manifest(path)?;
        checks.push(artifact_format_check(
            "release.play_store.artifact_format",
            &manifest,
            &["aab", "apk"],
            "Google Play accepts Android App Bundles for production publishing and APKs for legacy/test flows.",
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
    checks.push(required_value(
        "release.app_store.issuer_id_configured",
        cfg.issuer_id
            .as_deref()
            .or_else(|| env_value_ref("APP_STORE_CONNECT_ISSUER_ID")),
        "App Store Connect issuer id is configured",
        "Set distribution.app_store.issuer_id or APP_STORE_CONNECT_ISSUER_ID.",
    ));
    checks.push(required_value(
        "release.app_store.key_id_configured",
        cfg.key_id
            .as_deref()
            .or_else(|| env_value_ref("APP_STORE_CONNECT_KEY_ID")),
        "App Store Connect key id is configured",
        "Set distribution.app_store.key_id or APP_STORE_CONNECT_KEY_ID.",
    ));
    checks.push(secret_check(
        "release.app_store.credentials_available",
        &["APP_STORE_CONNECT_API_KEY", "APP_STORE_CONNECT_API_KEY_PATH"],
        DistributionProvider::AppStore,
        "Set APP_STORE_CONNECT_API_KEY_PATH to AuthKey_<KEYID>.p8, set APP_STORE_CONNECT_API_KEY, or import credentials with `fission auth import app-store`.",
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
        checks.push(artifact_format_check(
            "release.app_store.artifact_format",
            &manifest,
            &["ipa"],
            "App Store Connect binary upload requires an IPA artifact.",
        ));
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
    track: Option<&str>,
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
            "Install Microsoft Store Developer CLI, run `msstore` once to configure it, or set distribution.microsoft_store.msstore_reconfigure = true with Partner Center credentials.",
        ));
        checks.push(check(
            "release.microsoft_store.msix_uses_msstore",
            CheckSeverity::Info,
            CheckStatus::Passed,
            "MSIX submission uses Microsoft Store Developer CLI",
            Some(package_type.clone()),
            vec!["Fission calls `msstore publish --inputFile ... --appId ...`; no durable package_url is required for MSIX/MSIXUPLOAD submissions."],
        ));
        if cfg.msstore_reconfigure.unwrap_or(false) {
            checks.push(required_value(
                "release.microsoft_store.seller_id_configured",
                microsoft_store_seller_id(&cfg).as_deref(),
                "Microsoft Store seller id is configured for msstore reconfigure",
                "Set distribution.microsoft_store.seller_id, MICROSOFT_STORE_SELLER_ID, or PARTNER_CENTER_SELLER_ID.",
            ));
            checks.push(required_value(
                "release.microsoft_store.tenant_id_configured",
                microsoft_store_tenant_id(&cfg).as_deref(),
                "Microsoft Entra tenant id is configured for msstore reconfigure",
                "Set distribution.microsoft_store.tenant_id, AZURE_TENANT_ID, or PARTNER_CENTER_TENANT_ID.",
            ));
            checks.push(required_value(
                "release.microsoft_store.client_id_configured",
                microsoft_store_client_id(&cfg).as_deref(),
                "Microsoft Entra client id is configured for msstore reconfigure",
                "Set distribution.microsoft_store.client_id, AZURE_CLIENT_ID, or PARTNER_CENTER_CLIENT_ID.",
            ));
            checks.push(secret_check(
                "release.microsoft_store.credentials_available",
                &[
                    "MICROSOFT_STORE_CLIENT_SECRET",
                    "PARTNER_CENTER_CLIENT_SECRET",
                ],
                DistributionProvider::MicrosoftStore,
                "Set MICROSOFT_STORE_CLIENT_SECRET, PARTNER_CENTER_CLIENT_SECRET, or import the Partner Center client secret with `fission auth import microsoft-store --from env:MICROSOFT_STORE_CLIENT_SECRET --yes`.",
            ));
        } else {
            checks.push(check(
                "release.microsoft_store.msstore_config_external",
                CheckSeverity::Warning,
                CheckStatus::Warning,
                "Microsoft Store Developer CLI credentials are managed by msstore",
                None,
                vec!["Run `msstore` interactively once, run `msstore reconfigure ...` in CI, or set distribution.microsoft_store.msstore_reconfigure = true so Fission configures msstore before publishing."],
            ));
        }
    } else {
        checks.push(required_value(
            "release.microsoft_store.seller_id_configured",
            microsoft_store_seller_id(&cfg).as_deref(),
            "Microsoft Store seller id is configured",
            "Set distribution.microsoft_store.seller_id, MICROSOFT_STORE_SELLER_ID, or PARTNER_CENTER_SELLER_ID.",
        ));
        checks.push(required_value(
            "release.microsoft_store.tenant_id_configured",
            microsoft_store_tenant_id(&cfg).as_deref(),
            "Microsoft Entra tenant id is configured",
            "Set distribution.microsoft_store.tenant_id, AZURE_TENANT_ID, or PARTNER_CENTER_TENANT_ID.",
        ));
        checks.push(required_value(
            "release.microsoft_store.client_id_configured",
            microsoft_store_client_id(&cfg).as_deref(),
            "Microsoft Entra client id is configured",
            "Set distribution.microsoft_store.client_id, AZURE_CLIENT_ID, or PARTNER_CENTER_CLIENT_ID.",
        ));
        checks.push(secret_check(
            "release.microsoft_store.credentials_available",
            &[
                "MICROSOFT_STORE_CLIENT_SECRET",
                "PARTNER_CENTER_CLIENT_SECRET",
            ],
            DistributionProvider::MicrosoftStore,
            "Set MICROSOFT_STORE_CLIENT_SECRET, PARTNER_CENTER_CLIENT_SECRET, or import the Partner Center client secret with `fission auth import microsoft-store --from env:MICROSOFT_STORE_CLIENT_SECRET --yes`.",
        ));
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

fn create_play_edit(client: &Client, token: &str, package_name: &str) -> Result<String> {
    let url = format!("{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits");
    let response = client
        .post(url)
        .bearer_auth(token)
        .json(&json!({}))
        .send()
        .context("failed to create Google Play edit")?;
    let value = json_response(response, "Google Play edit insert")?;
    value
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .context("Google Play edit insert response did not contain id")
}

fn upload_play_artifact(
    client: &Client,
    token: &str,
    package_name: &str,
    edit_id: &str,
    path: &Path,
    artifact_kind: &str,
) -> Result<String> {
    let endpoint = match artifact_kind {
        "aab" => "bundles",
        "apk" => "apks",
        other => bail!("Google Play upload expected .aab or .apk, got .{other}"),
    };
    let url = format!(
        "{PLAY_UPLOAD_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/{endpoint}?uploadType=media"
    );
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let response = client
        .post(url)
        .bearer_auth(token)
        .header("Content-Type", "application/octet-stream")
        .body(bytes)
        .send()
        .with_context(|| format!("failed to upload {} to Google Play", path.display()))?;
    let value = json_response(response, "Google Play artifact upload")?;
    let version = value
        .get("versionCode")
        .and_then(|value| {
            value
                .as_i64()
                .map(|value| value.to_string())
                .or_else(|| value.as_str().map(str::to_string))
        })
        .context("Google Play upload response did not contain versionCode")?;
    Ok(version)
}

fn update_play_track(
    client: &Client,
    token: &str,
    package_name: &str,
    edit_id: &str,
    track: &str,
    release_status: &str,
    version_code: &str,
) -> Result<()> {
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/tracks/{track}"
    );
    let body = json!({
        "releases": [{
            "status": release_status,
            "versionCodes": [version_code]
        }]
    });
    let response = client
        .put(url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .context("failed to update Google Play track")?;
    json_response(response, "Google Play track update")?;
    Ok(())
}

fn validate_play_edit(
    client: &Client,
    token: &str,
    package_name: &str,
    edit_id: &str,
) -> Result<()> {
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}:validate"
    );
    let response = client
        .post(url)
        .bearer_auth(token)
        .send()
        .context("failed to validate Google Play edit")?;
    json_response(response, "Google Play edit validate")?;
    Ok(())
}

fn commit_play_edit(client: &Client, token: &str, package_name: &str, edit_id: &str) -> Result<()> {
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}:commit"
    );
    let response = client
        .post(url)
        .bearer_auth(token)
        .send()
        .context("failed to commit Google Play edit")?;
    json_response(response, "Google Play edit commit")?;
    Ok(())
}

fn google_play_access_token(cfg: &PlayStoreConfig, client: &Client) -> Result<String> {
    if let Some(token) = env_value("PLAY_STORE_ACCESS_TOKEN") {
        return Ok(token);
    }
    let secret_source = env_value("PLAY_STORE_SERVICE_ACCOUNT_JSON")
        .or_else(|| env_value("GOOGLE_APPLICATION_CREDENTIALS"))
        .or_else(|| cfg.service_account.clone())
        .or_else(|| {
            release::provider_secret(DistributionProvider::PlayStore, &[])
                .ok()
                .flatten()
        });
    let Some(source) = secret_source else {
        bail!("Google Play credentials are missing; set PLAY_STORE_SERVICE_ACCOUNT_JSON, PLAY_STORE_ACCESS_TOKEN, GOOGLE_APPLICATION_CREDENTIALS, or import play-store credentials into the Fission vault")
    };
    if looks_like_bearer_token(&source) {
        return Ok(source);
    }
    let service_account = load_google_service_account(&source)?;
    service_account_access_token(&service_account, client)
}

fn service_account_access_token(account: &GoogleServiceAccount, client: &Client) -> Result<String> {
    let token_uri = account.token_uri.as_deref().unwrap_or(GOOGLE_TOKEN_URI);
    let iat = now_unix_seconds();
    let claims = GoogleJwtClaims {
        iss: &account.client_email,
        scope: GOOGLE_PLAY_SCOPE,
        aud: token_uri,
        iat,
        exp: iat + 3600,
    };
    let key = EncodingKey::from_rsa_pem(account.private_key.as_bytes())
        .context("failed to parse Google service account private_key as RSA PEM")?;
    let jwt = encode(&Header::new(Algorithm::RS256), &claims, &key)
        .context("failed to sign Google service account JWT")?;
    let response = client
        .post(token_uri)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", jwt.as_str()),
        ])
        .send()
        .context("failed to exchange Google service account JWT")?;
    let token: OAuthTokenResponse = response
        .error_for_status()
        .context("Google OAuth token exchange failed")?
        .json()
        .context("failed to parse Google OAuth token response")?;
    let _ = (&token.token_type, token.expires_in);
    Ok(token.access_token)
}

fn app_store_access_token(cfg: &AppStoreConfig) -> Result<String> {
    if let Some(token) = env_value("APP_STORE_CONNECT_ACCESS_TOKEN") {
        return Ok(token);
    }
    let issuer_id = env_value("APP_STORE_CONNECT_ISSUER_ID")
        .or(cfg.issuer_id.clone())
        .context("distribution.app_store.issuer_id or APP_STORE_CONNECT_ISSUER_ID is required")?;
    let key_id = env_value("APP_STORE_CONNECT_KEY_ID")
        .or(cfg.key_id.clone())
        .context("distribution.app_store.key_id or APP_STORE_CONNECT_KEY_ID is required")?;
    let key_source = env_value("APP_STORE_CONNECT_API_KEY")
        .or_else(|| env_value("APP_STORE_CONNECT_API_KEY_PATH"))
        .or(cfg.api_key_path.clone())
        .or_else(|| release::provider_secret(DistributionProvider::AppStore, &[]).ok().flatten())
        .context("APP_STORE_CONNECT_API_KEY, APP_STORE_CONNECT_API_KEY_PATH, distribution.app_store.api_key_path, or vault credentials are required")?;
    if looks_like_bearer_token(&key_source) {
        return Ok(key_source);
    }
    let key_text = if key_source.contains("-----BEGIN PRIVATE KEY-----") {
        key_source
    } else {
        fs::read_to_string(&key_source).with_context(|| {
            format!("failed to read App Store Connect API key from {key_source}")
        })?
    };
    let now = now_unix_seconds();
    let claims = AppStoreJwtClaims {
        iss: &issuer_id,
        aud: "appstoreconnect-v1",
        iat: now,
        exp: now + 20 * 60,
    };
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(key_id);
    encode(
        &header,
        &claims,
        &EncodingKey::from_ec_pem(key_text.as_bytes())
            .context("failed to parse App Store Connect .p8 key as EC private key")?,
    )
    .context("failed to sign App Store Connect JWT")
}

fn app_store_app_id(cfg: &AppStoreConfig, client: &Client, token: &str) -> Result<String> {
    if let Some(app_id) = cfg
        .app_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(app_id.to_string());
    }
    let bundle_id = cfg.bundle_id.as_deref().context(
        "distribution.app_store.app_id or bundle_id is required for App Store Connect status",
    )?;
    let url = format!("{APP_STORE_API}/v1/apps?filter[bundleId]={bundle_id}&limit=1");
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .context("failed to resolve App Store Connect app id from bundle id")?;
    let value = json_response(response, "App Store app lookup")?;
    value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .with_context(|| {
            format!("App Store Connect did not return an app for bundle id {bundle_id}")
        })
}

fn microsoft_store_access_token(cfg: &MicrosoftStoreConfig, client: &Client) -> Result<String> {
    if let Some(token) = env_value("MICROSOFT_STORE_TOKEN") {
        return Ok(token);
    }
    let tenant_id = microsoft_store_tenant_id(cfg).context(
        "distribution.microsoft_store.tenant_id, AZURE_TENANT_ID, or PARTNER_CENTER_TENANT_ID is required",
    )?;
    let client_id = microsoft_store_client_id(cfg).context(
        "distribution.microsoft_store.client_id, AZURE_CLIENT_ID, or PARTNER_CENTER_CLIENT_ID is required",
    )?;
    let client_secret = microsoft_store_client_secret().context(
        "MICROSOFT_STORE_CLIENT_SECRET, PARTNER_CENTER_CLIENT_SECRET, or vault credentials are required",
    )?;
    let url = format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token");
    let response = client
        .post(url)
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("scope", MICROSOFT_STORE_SCOPE),
        ])
        .send()
        .context("failed to request Microsoft Store access token")?;
    let token: OAuthTokenResponse = response
        .error_for_status()
        .context("Microsoft Store access token request failed")?
        .json()
        .context("failed to parse Microsoft Store access token response")?;
    Ok(token.access_token)
}

fn run_msstore_reconfigure(cfg: &MicrosoftStoreConfig) -> Result<(String, String)> {
    let tenant_id = microsoft_store_tenant_id(cfg).context(
        "distribution.microsoft_store.tenant_id, AZURE_TENANT_ID, or PARTNER_CENTER_TENANT_ID is required for msstore reconfigure",
    )?;
    let seller_id = microsoft_store_seller_id(cfg).context(
        "distribution.microsoft_store.seller_id, MICROSOFT_STORE_SELLER_ID, or PARTNER_CENTER_SELLER_ID is required for msstore reconfigure",
    )?;
    let client_id = microsoft_store_client_id(cfg).context(
        "distribution.microsoft_store.client_id, AZURE_CLIENT_ID, or PARTNER_CENTER_CLIENT_ID is required for msstore reconfigure",
    )?;
    let client_secret = microsoft_store_client_secret().context(
        "MICROSOFT_STORE_CLIENT_SECRET, PARTNER_CENTER_CLIENT_SECRET, or vault credentials are required for msstore reconfigure",
    )?;
    let args = vec![
        "reconfigure".to_string(),
        "--tenantId".to_string(),
        tenant_id,
        "--sellerId".to_string(),
        seller_id,
        "--clientId".to_string(),
        client_id,
        "--clientSecret".to_string(),
        client_secret,
    ];
    run_msstore(&args, "Microsoft Store Developer CLI reconfigure")
}

fn run_msstore(args: &[String], operation: &str) -> Result<(String, String)> {
    let output = Command::new("msstore")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| {
            format!(
                "failed to run msstore; install Microsoft Store Developer CLI before {operation}"
            )
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        bail!("{operation} failed with {}: {}", output.status, detail);
    }
    Ok((stdout, stderr))
}

fn load_google_service_account(source: &str) -> Result<GoogleServiceAccount> {
    let text = if source.trim_start().starts_with('{') {
        source.to_string()
    } else {
        fs::read_to_string(source)
            .with_context(|| format!("failed to read Google service account JSON from {source}"))?
    };
    serde_json::from_str(&text).context("failed to parse Google service account JSON")
}

fn json_response(response: Response, operation: &str) -> Result<Value> {
    let status = response.status();
    let text = response
        .text()
        .with_context(|| format!("failed to read {operation} response"))?;
    if !status.is_success() {
        bail!("{operation} failed with {status}: {text}");
    }
    if text.trim().is_empty() {
        Ok(Value::Null)
    } else {
        serde_json::from_str(&text)
            .with_context(|| format!("failed to parse {operation} JSON response: {text}"))
    }
}

fn microsoft_store_success(value: &Value, operation: &str) -> Result<()> {
    if value
        .get("isSuccess")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        Ok(())
    } else {
        bail!("{operation} returned an unsuccessful response: {value}")
    }
}

fn http_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(300))
        .user_agent("fission-cli-release/0.1")
        .build()
        .context("failed to build release HTTP client")
}

fn play_store_config(config: &PublishManifest) -> PlayStoreConfig {
    config
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.play_store.clone())
        .unwrap_or_default()
}

fn app_store_config(config: &PublishManifest) -> AppStoreConfig {
    config
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.app_store.clone())
        .unwrap_or_default()
}

fn microsoft_store_config(config: &PublishManifest) -> MicrosoftStoreConfig {
    config
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.microsoft_store.clone())
        .unwrap_or_default()
}

fn primary_artifact_with_extensions(manifest: &ArtifactManifest, exts: &[&str]) -> Result<PathBuf> {
    manifest
        .artifacts
        .iter()
        .map(|file| PathBuf::from(&file.path))
        .find(|path| {
            path.extension()
                .and_then(|value| value.to_str())
                .is_some_and(|ext| {
                    exts.iter()
                        .any(|candidate| ext.eq_ignore_ascii_case(candidate))
                })
        })
        .with_context(|| {
            format!(
                "artifact manifest does not contain one of: {}",
                exts.join(", ")
            )
        })
}

fn primary_artifact_extension(manifest: &ArtifactManifest) -> Option<&str> {
    manifest
        .artifacts
        .iter()
        .map(|file| Path::new(&file.path))
        .find_map(|path| path.extension().and_then(|value| value.to_str()))
}

fn has_artifact_with_extension(manifest: &ArtifactManifest, exts: &[&str]) -> bool {
    manifest.artifacts.iter().any(|file| {
        Path::new(&file.path)
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|ext| {
                exts.iter()
                    .any(|candidate| ext.eq_ignore_ascii_case(candidate))
            })
    })
}

fn microsoft_store_package_type(cfg: &MicrosoftStoreConfig, manifest: &ArtifactManifest) -> String {
    cfg.package_type
        .as_deref()
        .or_else(|| primary_artifact_extension(manifest))
        .unwrap_or("exe")
        .to_ascii_lowercase()
}

fn is_microsoft_store_msix_type(package_type: &str) -> bool {
    MICROSOFT_STORE_MSIX_TYPES
        .iter()
        .any(|candidate| package_type.eq_ignore_ascii_case(candidate))
}

fn microsoft_store_msstore_project(
    options: &DistributeOptions,
    cfg: &MicrosoftStoreConfig,
) -> PathBuf {
    cfg.msstore_project
        .as_deref()
        .map(|path| {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                path
            } else {
                options.project_dir.join(path)
            }
        })
        .unwrap_or_else(|| options.project_dir.clone())
}

fn msstore_publish_args(
    project_path: &Path,
    artifact: &Path,
    product_id: &str,
    flight_id: Option<&str>,
    rollout: Option<u8>,
    should_submit: bool,
) -> Vec<String> {
    let mut args = vec![
        "publish".to_string(),
        project_path.display().to_string(),
        "-i".to_string(),
        artifact.display().to_string(),
        "-id".to_string(),
        product_id.to_string(),
    ];
    if !should_submit {
        args.push("-nc".to_string());
    }
    if let Some(flight_id) = flight_id.filter(|value| !value.trim().is_empty()) {
        args.push("-f".to_string());
        args.push(flight_id.to_string());
    }
    if let Some(rollout) = rollout {
        args.push("-prp".to_string());
        args.push(rollout.to_string());
    }
    args
}

fn microsoft_store_should_submit(options: &DistributeOptions, cfg: &MicrosoftStoreConfig) -> bool {
    cfg.submit.unwrap_or(false) || options.track.as_deref() == Some("public") && options.yes
}

fn microsoft_store_flight_id(
    track: Option<&str>,
    cfg: &MicrosoftStoreConfig,
) -> Result<Option<String>> {
    match track.map(str::trim).filter(|value| !value.is_empty()) {
        Some("public") | None => Ok(None),
        Some("private") => cfg.flight_id.clone().map(Some).context(
            "distribution.microsoft_store.flight_id is required when --track private is used for MSIX publishing",
        ),
        Some(flight_id) => Ok(Some(flight_id.to_string())),
    }
}

fn microsoft_store_rollout_percentage(cfg: &MicrosoftStoreConfig) -> Result<Option<u8>> {
    if let Some(rollout) = cfg.package_rollout_percentage {
        if rollout > 100 {
            bail!(
                "distribution.microsoft_store.package_rollout_percentage must be between 0 and 100"
            );
        }
    }
    Ok(cfg.package_rollout_percentage)
}

fn microsoft_store_seller_id(cfg: &MicrosoftStoreConfig) -> Option<String> {
    env_value("MICROSOFT_STORE_SELLER_ID")
        .or_else(|| env_value("PARTNER_CENTER_SELLER_ID"))
        .or_else(|| cfg.seller_id.clone())
}

fn microsoft_store_tenant_id(cfg: &MicrosoftStoreConfig) -> Option<String> {
    env_value("AZURE_TENANT_ID")
        .or_else(|| env_value("PARTNER_CENTER_TENANT_ID"))
        .or_else(|| cfg.tenant_id.clone())
}

fn microsoft_store_client_id(cfg: &MicrosoftStoreConfig) -> Option<String> {
    env_value("AZURE_CLIENT_ID")
        .or_else(|| env_value("PARTNER_CENTER_CLIENT_ID"))
        .or_else(|| cfg.client_id.clone())
}

fn microsoft_store_client_secret() -> Option<String> {
    env_value("MICROSOFT_STORE_CLIENT_SECRET")
        .or_else(|| env_value("PARTNER_CENTER_CLIENT_SECRET"))
        .or_else(|| {
            release::provider_secret(DistributionProvider::MicrosoftStore, &[])
                .ok()
                .flatten()
        })
}

fn command_line(program: &str, args: &[String]) -> String {
    std::iter::once(program.to_string())
        .chain(args.iter().map(|arg| shell_word(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_word(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':' | '\\'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn artifact_format_check(
    id: &str,
    manifest: &ArtifactManifest,
    accepted: &[&str],
    remediation: &str,
) -> ReadinessCheck {
    check(
        id,
        CheckSeverity::Error,
        if accepted.iter().any(|format| manifest.format == *format) {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        format!("artifact format is one of {}", accepted.join(", ")),
        Some(format!("manifest format: {}", manifest.format)),
        vec![remediation],
    )
}

fn secret_check(
    id: &str,
    env_names: &[&str],
    provider: DistributionProvider,
    remediation: &str,
) -> ReadinessCheck {
    let env_name = env_names.iter().find(|name| env::var_os(name).is_some());
    let vault_present = release::provider_secret(provider, &[])
        .ok()
        .flatten()
        .is_some();
    check(
        id,
        CheckSeverity::Error,
        if env_name.is_some() || vault_present {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        "provider credentials are available",
        env_name
            .map(|name| format!("environment variable {name}"))
            .or_else(|| vault_present.then(|| "Fission release vault".to_string())),
        vec![remediation],
    )
}

fn store_receipt(
    options: &DistributeOptions,
    provider: &str,
    artifact_path: &Path,
    status: &str,
    deployment_id: Option<String>,
    canonical_url: Option<String>,
    manual_follow_up: Vec<String>,
) -> DistributionReceipt {
    DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: provider.to_string(),
        site: options.site.clone(),
        action: "publish".to_string(),
        artifact_manifest: Some(artifact_path.display().to_string()),
        deployment_id,
        canonical_url,
        preview_url: None,
        custom_domain: None,
        status: status.to_string(),
        stdout: None,
        stderr: None,
        manual_follow_up,
    }
}

fn package_name_from_project(manifest: &ArtifactManifest) -> Option<&str> {
    (!manifest.project.app_id.trim().is_empty()).then_some(manifest.project.app_id.as_str())
}

fn looks_like_bearer_token(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.starts_with('{') && !Path::new(trimmed).exists() && trimmed.matches('.').count() >= 2
}

fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn env_value_ref(name: &str) -> Option<&'static str> {
    if env::var_os(name).is_some() {
        Some("set")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(track: Option<&str>, yes: bool) -> DistributeOptions {
        DistributeOptions {
            project_dir: PathBuf::from("/project"),
            provider: DistributionProvider::MicrosoftStore,
            action: DistributeAction::Publish,
            artifact: None,
            site: "production".to_string(),
            deploy: None,
            track: track.map(str::to_string),
            dry_run: false,
            yes,
            json: false,
        }
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
}
