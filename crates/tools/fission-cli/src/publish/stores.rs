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
const MICROSOFT_STORE_SCOPE: &str = "https://api.store.microsoft.com/.default";

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
    let seller_id = env_value("MICROSOFT_STORE_SELLER_ID")
        .or(cfg.seller_id.clone())
        .context(
            "distribution.microsoft_store.seller_id or MICROSOFT_STORE_SELLER_ID is required",
        )?;
    let package_url = options
        .deploy
        .as_deref()
        .filter(|value| value.starts_with("https://") || value.starts_with("http://"))
        .map(str::to_string)
        .or(cfg.package_url.clone())
        .context("Microsoft Store MSI/EXE submission requires a package_url in fission.toml or --deploy <https-url>; publish the artifact to S3/static hosting first")?;
    let package_type = cfg.package_type.clone().unwrap_or_else(|| {
        primary_artifact_extension(manifest)
            .unwrap_or("exe")
            .to_ascii_lowercase()
    });
    if !matches!(package_type.as_str(), "exe" | "msi") {
        bail!(
            "Microsoft Store direct package automation currently supports the Store MSI/EXE submission API; package_type `{package_type}` requires the MSIX Partner Center path"
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
    checks.push(required_value(
        "release.microsoft_store.seller_id_configured",
        cfg.seller_id
            .as_deref()
            .or_else(|| env_value_ref("MICROSOFT_STORE_SELLER_ID")),
        "Microsoft Store seller id is configured",
        "Set distribution.microsoft_store.seller_id or MICROSOFT_STORE_SELLER_ID.",
    ));
    checks.push(required_value(
        "release.microsoft_store.tenant_id_configured",
        cfg.tenant_id
            .as_deref()
            .or_else(|| env_value_ref("AZURE_TENANT_ID")),
        "Microsoft Entra tenant id is configured",
        "Set distribution.microsoft_store.tenant_id or AZURE_TENANT_ID.",
    ));
    checks.push(required_value(
        "release.microsoft_store.client_id_configured",
        cfg.client_id
            .as_deref()
            .or_else(|| env_value_ref("AZURE_CLIENT_ID")),
        "Microsoft Entra client id is configured",
        "Set distribution.microsoft_store.client_id or AZURE_CLIENT_ID.",
    ));
    checks.push(secret_check(
        "release.microsoft_store.credentials_available",
        &["MICROSOFT_STORE_CLIENT_SECRET"],
        DistributionProvider::MicrosoftStore,
        "Set MICROSOFT_STORE_CLIENT_SECRET or import the Partner Center client secret with `fission auth import microsoft-store --from env:MICROSOFT_STORE_CLIENT_SECRET --yes`.",
    ));
    checks.push(required_value(
        "release.microsoft_store.package_url_configured",
        cfg.package_url.as_deref(),
        "Microsoft Store package URL is configured for MSI/EXE submissions",
        "Upload the package to a durable HTTPS URL first, then set distribution.microsoft_store.package_url or pass --deploy <https-url>.",
    ));
    let selected_track = track.unwrap_or("public");
    checks.push(check(
        "release.microsoft_store.track_supported",
        CheckSeverity::Warning,
        if matches!(selected_track, "public" | "private") {
            CheckStatus::Passed
        } else {
            CheckStatus::Warning
        },
        "Microsoft Store destination is understood",
        Some(selected_track.to_string()),
        vec!["Use public, private, or a Partner Center flight once flight support is configured."],
    ));
    if let Some(path) = artifact.filter(|path| path.exists()) {
        let manifest = read_artifact_manifest(path)?;
        checks.push(artifact_format_check(
            "release.microsoft_store.artifact_format",
            &manifest,
            &["exe", "msi", "msix"],
            "Microsoft Store accepts configured installer/package formats; MSI/EXE automation requires package_url.",
        ));
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

fn microsoft_store_access_token(cfg: &MicrosoftStoreConfig, client: &Client) -> Result<String> {
    if let Some(token) = env_value("MICROSOFT_STORE_TOKEN") {
        return Ok(token);
    }
    let tenant_id = env_value("AZURE_TENANT_ID")
        .or(cfg.tenant_id.clone())
        .context("distribution.microsoft_store.tenant_id or AZURE_TENANT_ID is required")?;
    let client_id = env_value("AZURE_CLIENT_ID")
        .or(cfg.client_id.clone())
        .context("distribution.microsoft_store.client_id or AZURE_CLIENT_ID is required")?;
    let client_secret = env_value("MICROSOFT_STORE_CLIENT_SECRET")
        .or_else(|| {
            release::provider_secret(DistributionProvider::MicrosoftStore, &[])
                .ok()
                .flatten()
        })
        .context("MICROSOFT_STORE_CLIENT_SECRET or vault credentials are required")?;
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
