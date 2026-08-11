use super::*;
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::blocking::{Client, Response};
use reqwest::header::CONTENT_LENGTH;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const PLAY_API: &str = "https://androidpublisher.googleapis.com";
const PLAY_UPLOAD_API: &str = "https://androidpublisher.googleapis.com/upload";
const GOOGLE_PLAY_SCOPE: &str = "https://www.googleapis.com/auth/androidpublisher";
const GOOGLE_TOKEN_URI: &str = "https://oauth2.googleapis.com/token";
const APP_STORE_API: &str = "https://api.appstoreconnect.apple.com";

mod play_content;
mod provider_content;
mod release_config_ops;
mod review_beta_ops;

pub(crate) use release_config_ops::remote_state;
use release_config_ops::*;

#[derive(Debug, Deserialize, Default)]
struct ReleaseProviderToml {
    distribution: Option<DistributionToml>,
    beta: Option<BetaRootToml>,
    release: Option<ReleaseRootToml>,
    #[serde(default)]
    releases: Vec<ReleaseEntryToml>,
}

#[derive(Debug, Deserialize, Default)]
struct DistributionToml {
    play_store: Option<PlayStoreConfig>,
    app_store: Option<AppStoreConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct BetaRootToml {
    play_store: Option<PlayBetaToml>,
}

#[derive(Debug, Deserialize, Default)]
struct PlayBetaToml {
    #[serde(default)]
    tracks: BTreeMap<String, PlayBetaTrackToml>,
}

#[derive(Debug, Deserialize, Default)]
struct PlayBetaTrackToml {
    tester_source: Option<String>,
    group: Option<String>,
    #[serde(default)]
    groups: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseRootToml {
    active_release: Option<String>,
    #[serde(default)]
    default_locales: Vec<String>,
    #[serde(default)]
    store_listing: BTreeMap<String, BTreeMap<String, StoreListingToml>>,
    assets: Option<ReleaseAssetsToml>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseAssetsToml {
    play_store: Option<PlayStoreAssetsToml>,
}

#[derive(Debug, Deserialize, Default)]
struct PlayStoreAssetsToml {
    screenshot_sets_dir: Option<String>,
    feature_graphic: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseEntryToml {
    id: Option<String>,
    version: Option<String>,
    #[serde(default)]
    locales: Vec<String>,
    metadata: Option<String>,
    release_notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default, Serialize, PartialEq, Eq)]
struct StoreListingToml {
    title: Option<String>,
    name: Option<String>,
    short_description: Option<String>,
    subtitle: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    support_url: Option<String>,
    marketing_url: Option<String>,
    privacy_url: Option<String>,
    video: Option<String>,
    video_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct ReleaseMetadataToml {
    #[serde(default)]
    play_store: BTreeMap<String, PlayReleaseMetadataToml>,
    #[serde(default)]
    app_store: BTreeMap<String, AppStoreReleaseMetadataToml>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct AppStoreReleaseMetadataToml {
    description: Option<String>,
    promotional_text: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct PlayReleaseMetadataToml {
    full_description: Option<String>,
    description: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct PlayListing {
    locale: String,
    title: String,
    short_description: String,
    full_description: String,
    video: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct AppStoreLocalization {
    id: Option<String>,
    locale: String,
    description: String,
    keywords: Option<String>,
    marketing_url: Option<String>,
    promotional_text: Option<String>,
    support_url: Option<String>,
    whats_new: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct PlayStoreConfig {
    package_name: Option<String>,
    access_token_env: Option<String>,
    service_account_json_env: Option<String>,
    service_account_json_base64_env: Option<String>,
    google_application_credentials_env: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct AppStoreConfig {
    app_id: Option<String>,
    bundle_id: Option<String>,
    platform: Option<String>,
    issuer_id: Option<String>,
    key_id: Option<String>,
    access_token_env: Option<String>,
    issuer_id_env: Option<String>,
    key_id_env: Option<String>,
    api_key_env: Option<String>,
    api_key_base64_env: Option<String>,
    api_key_path_env: Option<String>,
}

fn app_store_platform_api_value(cfg: &AppStoreConfig) -> Result<&'static str> {
    match cfg
        .platform
        .as_deref()
        .unwrap_or("ios")
        .trim()
        .to_ascii_lowercase()
        .replace('_', "-")
        .as_str()
    {
        "ios" => Ok("IOS"),
        "macos" | "mac-os" => Ok("MAC_OS"),
        other => bail!("distribution.app_store.platform must be `ios` or `macos`, got `{other}`"),
    }
}

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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ReleaseConfigRemoteState {
    provider: String,
    subject: String,
    locales: Vec<String>,
    remote_revision: String,
    snapshot: Value,
}

#[derive(Clone, Debug, Deserialize)]
struct ReleaseConfigLock {
    provider: Option<String>,
    subject: Option<String>,
    remote_revision: Option<String>,
    #[serde(default)]
    locales: Vec<String>,
    locked_at_unix_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseConfigLocksToml {
    release: Option<ReleaseConfigLocksRoot>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseConfigLocksRoot {
    #[serde(default)]
    provider_locks: BTreeMap<String, ReleaseConfigLock>,
}

pub(super) fn reviews_list(
    provider: DistributionProvider,
    since: Option<String>,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    match provider {
        DistributionProvider::PlayStore => {
            review_beta_ops::play_reviews_list(project_dir, since, json_output)
        }
        DistributionProvider::AppStore => {
            review_beta_ops::app_store_reviews_list(project_dir, since, json_output)
        }
        _ => unsupported_reviews(provider, "list"),
    }
}

pub(super) fn reviews_reply(
    provider: DistributionProvider,
    review: &str,
    message_file: &Path,
    project_dir: &Path,
    dry_run: bool,
    yes: bool,
    json_output: bool,
) -> Result<()> {
    if !dry_run && !yes {
        bail!("replying to a provider review mutates provider state; pass --yes after reviewing the dry run");
    }
    match provider {
        DistributionProvider::PlayStore => review_beta_ops::play_reviews_reply(
            project_dir,
            review,
            message_file,
            dry_run,
            json_output,
        ),
        DistributionProvider::AppStore => review_beta_ops::app_store_reviews_reply(
            project_dir,
            review,
            message_file,
            dry_run,
            json_output,
        ),
        _ => unsupported_reviews(provider, "reply"),
    }
}

pub(super) fn beta_groups_list(
    provider: DistributionProvider,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    match provider {
        DistributionProvider::PlayStore => {
            review_beta_ops::play_beta_groups_list(project_dir, json_output)
        }
        DistributionProvider::AppStore => {
            review_beta_ops::app_store_beta_groups_list(project_dir, json_output)
        }
        _ => unsupported_beta(provider, "groups list"),
    }
}

pub(super) fn beta_groups_sync(
    provider: DistributionProvider,
    from: &Path,
    project_dir: &Path,
    dry_run: bool,
    yes: bool,
    json_output: bool,
) -> Result<()> {
    match provider {
        DistributionProvider::PlayStore => {
            review_beta_ops::play_beta_groups_sync(project_dir, from, dry_run, yes, json_output)
        }
        _ => unsupported_beta(provider, "groups sync"),
    }
}

pub(super) fn beta_testers_import(
    provider: DistributionProvider,
    group: Option<&str>,
    track: Option<&str>,
    csv: &Path,
    project_dir: &Path,
    dry_run: bool,
    yes: bool,
    json_output: bool,
) -> Result<()> {
    match provider {
        DistributionProvider::PlayStore => review_beta_ops::play_beta_testers_import(
            project_dir,
            track,
            csv,
            dry_run,
            yes,
            json_output,
        ),
        DistributionProvider::AppStore => review_beta_ops::app_store_beta_testers_import(
            project_dir,
            group,
            csv,
            dry_run,
            yes,
            json_output,
        ),
        _ => unsupported_beta(provider, "testers import"),
    }
}

pub(super) fn beta_testers_export(
    provider: DistributionProvider,
    group: Option<&str>,
    track: Option<&str>,
    output: &Path,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    match provider {
        DistributionProvider::PlayStore => {
            review_beta_ops::play_beta_testers_export(project_dir, track, output, json_output)
        }
        DistributionProvider::AppStore => {
            review_beta_ops::app_store_beta_testers_export(project_dir, group, output, json_output)
        }
        _ => unsupported_beta(provider, "testers export"),
    }
}

pub(super) fn app_store_beta_distribute(
    project_dir: &Path,
    artifact: &Path,
    group: Option<&str>,
    dry_run: bool,
    yes: bool,
    json_output: bool,
) -> Result<()> {
    review_beta_ops::app_store_beta_distribute(
        project_dir,
        artifact,
        group,
        dry_run,
        yes,
        json_output,
    )
}

pub(super) fn release_config_import(
    provider: DistributionProvider,
    locales: Option<String>,
    dry_run: bool,
    yes: bool,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    match provider {
        DistributionProvider::PlayStore => {
            play_release_config_import(project_dir, locales.as_deref(), dry_run, yes, json_output)
        }
        DistributionProvider::AppStore => app_store_release_config_import(
            project_dir,
            locales.as_deref(),
            dry_run,
            yes,
            json_output,
        ),
        DistributionProvider::MicrosoftStore => super::microsoft_store_ops::release_config_import(
            project_dir,
            locales.as_deref(),
            dry_run,
            yes,
            json_output,
        ),
        _ => unsupported_release_config(provider, "import"),
    }
}

pub(super) fn release_config_diff(
    provider: DistributionProvider,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    match provider {
        DistributionProvider::PlayStore => play_release_config_diff(project_dir, json_output),
        DistributionProvider::AppStore => app_store_release_config_diff(project_dir, json_output),
        DistributionProvider::MicrosoftStore => {
            super::microsoft_store_ops::release_config_diff(project_dir, json_output)
        }
        _ => unsupported_release_config(provider, "diff"),
    }
}

pub(super) fn release_config_push(
    provider: DistributionProvider,
    locales: Option<String>,
    overwrite_remote: bool,
    dry_run: bool,
    yes: bool,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    let summary = release_config_push_value(
        provider,
        locales.as_deref(),
        overwrite_remote,
        dry_run,
        yes,
        project_dir,
    )?;
    print_release_config_push_summary(provider, &summary, json_output)
}

pub(crate) fn release_config_push_value(
    provider: DistributionProvider,
    locales: Option<&str>,
    overwrite_remote: bool,
    dry_run: bool,
    yes: bool,
    project_dir: &Path,
) -> Result<Value> {
    if !dry_run {
        ensure_release_config_content_manifest_fresh(project_dir)?;
        ensure_release_config_remote_lock(project_dir, provider, locales, overwrite_remote)?;
    }
    let mut value = match provider {
        DistributionProvider::PlayStore => {
            play_release_config_push(project_dir, locales, dry_run, yes)
        }
        DistributionProvider::AppStore => {
            app_store_release_config_push(project_dir, locales, dry_run, yes)
        }
        DistributionProvider::MicrosoftStore => {
            super::microsoft_store_ops::release_config_push_value(project_dir, locales, dry_run, yes)
        }
        _ => bail!(
            "{} release-config push is not exposed by the current provider API backend; Google Play, App Store, and Microsoft Store metadata import/diff/push are implemented",
            provider.as_str()
        ),
    }?;
    if !dry_run {
        let state = release_config_remote_state(provider, project_dir, locales)?;
        write_release_config_lock(project_dir, provider, &state)?;
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "remote_lock".to_string(),
                json!({
                    "subject": state.subject,
                    "locales": state.locales,
                    "remote_revision": state.remote_revision,
                    "locked_at_unix_seconds": now_unix_seconds(),
                }),
            );
        }
    }
    Ok(value)
}

pub(super) fn release_config_lock(
    provider: DistributionProvider,
    locales: Option<String>,
    dry_run: bool,
    yes: bool,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    if !dry_run && !yes {
        bail!("release-config lock rewrites fission.toml; pass --yes after reviewing the provider and locales");
    }
    let state = release_config_remote_state(provider, project_dir, locales.as_deref())?;
    let value = json!({
        "schema_version": 1,
        "action": "release-config.lock",
        "status": if dry_run { "dry-run" } else { "applied" },
        "provider": provider.as_str(),
        "subject": state.subject,
        "locales": state.locales,
        "remote_revision": state.remote_revision,
        "project_dir": project_dir.display().to_string(),
    });
    if !dry_run {
        write_release_config_lock(project_dir, provider, &state)?;
    }
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!(
            "release-config.lock: {} {} {}",
            value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("applied"),
            provider.as_str(),
            value
                .get("remote_revision")
                .and_then(Value::as_str)
                .unwrap_or("<revision>")
        );
    }
    Ok(())
}

pub(super) fn release_config_lock_check(
    project_dir: &Path,
    provider: DistributionProvider,
    overwrite_remote: bool,
) -> LifecycleCheck {
    if overwrite_remote {
        return LifecycleCheck {
            id: format!("release_config.{}.remote_lock", provider.as_str()),
            status: "skipped".to_string(),
            summary: "provider metadata baseline lock is intentionally bypassed".to_string(),
            details: Some("--overwrite-remote was supplied".to_string()),
            remediation: vec![
                "Use --overwrite-remote only after reviewing release-config diff.".to_string(),
            ],
        };
    }
    match read_release_config_lock(project_dir, provider) {
        Ok(Some(lock)) if lock.remote_revision.as_deref().is_some() => LifecycleCheck {
            id: format!("release_config.{}.remote_lock", provider.as_str()),
            status: "passed".to_string(),
            summary: "provider metadata baseline lock exists".to_string(),
            details: Some(format!(
                "{} locked at {:?}",
                lock.remote_revision.as_deref().unwrap_or("<revision>"),
                lock.locked_at_unix_seconds
            )),
            remediation: Vec::new(),
        },
        Ok(_) => LifecycleCheck {
            id: format!("release_config.{}.remote_lock", provider.as_str()),
            status: "missing".to_string(),
            summary: "provider metadata baseline lock is missing".to_string(),
            details: None,
            remediation: vec![format!(
                "Run `fission release-config lock --provider {} --yes` after reviewing remote metadata, or pass --overwrite-remote to intentionally replace provider metadata.",
                provider.as_str()
            )],
        },
        Err(error) => LifecycleCheck {
            id: format!("release_config.{}.remote_lock", provider.as_str()),
            status: "failed".to_string(),
            summary: "provider metadata baseline lock could not be read".to_string(),
            details: Some(error.to_string()),
            remediation: vec!["Fix fission.toml release.provider_locks and rerun readiness.".to_string()],
        },
    }
}

fn ensure_release_config_remote_lock(
    project_dir: &Path,
    provider: DistributionProvider,
    locales: Option<&str>,
    overwrite_remote: bool,
) -> Result<()> {
    if overwrite_remote {
        return Ok(());
    }
    let Some(lock) = read_release_config_lock(project_dir, provider)? else {
        bail!(
            "release-config push requires a provider baseline lock; run `fission release-config lock --provider {} --yes` after reviewing remote state, or pass --overwrite-remote to intentionally replace provider metadata",
            provider.as_str()
        );
    };
    let state = release_config_remote_state(provider, project_dir, locales)?;
    ensure_release_config_lock_matches(provider, &lock, &state)
}

fn ensure_release_config_lock_matches(
    provider: DistributionProvider,
    lock: &ReleaseConfigLock,
    state: &ReleaseConfigRemoteState,
) -> Result<()> {
    if lock.provider.as_deref() != Some(provider.as_str()) {
        bail!(
            "release-config lock provider mismatch: lock={:?}, current={}",
            lock.provider,
            provider.as_str()
        );
    }
    if lock.subject.as_deref() != Some(state.subject.as_str()) {
        bail!(
            "release-config lock subject mismatch: lock={:?}, current={}",
            lock.subject,
            state.subject
        );
    }
    if lock.remote_revision.as_deref() != Some(state.remote_revision.as_str()) {
        bail!(
            "remote {} metadata changed since the last import/lock; run `fission release-config diff --provider {}` and then `fission release-config lock --provider {} --yes`, or pass --overwrite-remote to intentionally replace provider metadata",
            provider.as_str(),
            provider.as_str(),
            provider.as_str()
        );
    }
    let mut locked_locales = lock.locales.clone();
    locked_locales.sort();
    if locked_locales != state.locales {
        bail!(
            "release-config lock locales mismatch: lock={:?}, current={:?}; lock the same locale set or pass --overwrite-remote",
            locked_locales,
            state.locales
        );
    }
    Ok(())
}

fn release_config_remote_state(
    provider: DistributionProvider,
    project_dir: &Path,
    locales_arg: Option<&str>,
) -> Result<ReleaseConfigRemoteState> {
    match provider {
        DistributionProvider::PlayStore => play_release_config_remote_state(project_dir, locales_arg),
        DistributionProvider::AppStore => app_store_release_config_remote_state(project_dir, locales_arg),
        DistributionProvider::MicrosoftStore => {
            super::microsoft_store_ops::release_config_remote_state(project_dir, locales_arg)
        }
        _ => bail!(
            "{} release-config lock is not exposed by the current provider API backend; Google Play, App Store, and Microsoft Store metadata locks are implemented",
            provider.as_str()
        ),
    }
}

fn read_release_config_lock(
    project_dir: &Path,
    provider: DistributionProvider,
) -> Result<Option<ReleaseConfigLock>> {
    let path = project_dir.join("fission.toml");
    if !path.exists() {
        return Ok(None);
    }
    let config: ReleaseConfigLocksToml = toml::from_str(
        &fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(config
        .release
        .and_then(|mut release| release.provider_locks.remove(&provider_lock_key(provider))))
}

pub(crate) fn write_release_config_lock(
    project_dir: &Path,
    provider: DistributionProvider,
    state: &ReleaseConfigRemoteState,
) -> Result<()> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut doc = parse_toml_edit_document(&data, &path)?;
    let prefix = format!("release.provider_locks.{}", provider_lock_key(provider));
    set_toml_edit_path(
        &mut doc,
        &format!("{prefix}.provider"),
        toml_edit::value(provider.as_str()),
    )?;
    set_toml_edit_path(
        &mut doc,
        &format!("{prefix}.subject"),
        toml_edit::value(state.subject.clone()),
    )?;
    set_toml_edit_path(
        &mut doc,
        &format!("{prefix}.remote_revision"),
        toml_edit::value(state.remote_revision.clone()),
    )?;
    set_toml_edit_path(
        &mut doc,
        &format!("{prefix}.locked_at_unix_seconds"),
        toml_edit::value(
            i64::try_from(now_unix_seconds()).context("current timestamp is too large")?,
        ),
    )?;
    set_toml_edit_path(
        &mut doc,
        &format!("{prefix}.locales"),
        toml_edit_string_array(state.locales.clone()),
    )?;
    write_toml_edit_document(&path, &doc)
}

fn provider_lock_key(provider: DistributionProvider) -> String {
    provider.as_str().replace('-', "_")
}

fn ensure_release_config_content_manifest_fresh(project_dir: &Path) -> Result<()> {
    let path = project_dir.join("release-content/content-manifest.json");
    if !path.exists() {
        return Ok(());
    }
    let value: Value = serde_json::from_slice(
        &fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))?;
    for file in value
        .get("referenced_files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let relative = file
            .get("path")
            .and_then(Value::as_str)
            .context("content manifest referenced file is missing path")?;
        let expected = file
            .get("sha256")
            .and_then(Value::as_str)
            .context("content manifest referenced file is missing sha256")?;
        let actual = sha256_file(&project_dir.join(relative))?;
        if actual != expected {
            bail!(
                "release metadata file `{relative}` changed since release-content/content-manifest.json was materialized; rerun `fission release-content validate` or the publish workflow to refresh the content manifest before pushing metadata"
            );
        }
    }
    Ok(())
}

fn sha256_json(value: &Value) -> Result<String> {
    let bytes = serde_json::to_vec(value)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

pub(super) fn release_content_push(
    provider: DistributionProvider,
    locales: Option<String>,
    dry_run: bool,
    yes: bool,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    let summary =
        release_content_push_value(provider, locales.as_deref(), dry_run, yes, project_dir)?;
    print_release_content_push_summary(provider, &summary, json_output)
}

pub(crate) fn release_content_push_value(
    provider: DistributionProvider,
    locales: Option<&str>,
    dry_run: bool,
    yes: bool,
    project_dir: &Path,
) -> Result<Value> {
    match provider {
        DistributionProvider::PlayStore => {
            play_content::play_release_content_push(project_dir, locales, dry_run, yes)
        }
        DistributionProvider::AppStore => {
            provider_content::app_store_release_content_push(project_dir, locales, dry_run, yes)
        }
        DistributionProvider::MicrosoftStore => {
            provider_content::microsoft_store_release_content_push(project_dir, locales, dry_run, yes)
        }
        _ => bail!(
            "{} release-content push is not exposed by the current provider API backend; Google Play image upload, App Store screenshot/preview/review-attachment upload, and Microsoft Store screenshot/logo upload are implemented",
            provider.as_str()
        ),
    }
}

fn print_release_config_push_summary(
    provider: DistributionProvider,
    value: &Value,
    json_output: bool,
) -> Result<()> {
    if json_output {
        println!("{}", serde_json::to_string_pretty(value)?);
        return Ok(());
    }
    let count = value
        .get("updated_locales")
        .or_else(|| value.get("pushed_locales"))
        .or_else(|| value.get("locales"))
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("complete");
    if status == "dry-run" {
        println!(
            "Would push {count} {} release metadata locale(s)",
            provider.as_str()
        );
    } else {
        println!(
            "Pushed {count} {} release metadata locale(s)",
            provider.as_str()
        );
    }
    Ok(())
}

fn print_release_content_push_summary(
    provider: DistributionProvider,
    value: &Value,
    json_output: bool,
) -> Result<()> {
    if json_output {
        println!("{}", serde_json::to_string_pretty(value)?);
        return Ok(());
    }
    let count = value
        .get("assets")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("complete");
    match status {
        "dry-run" => println!(
            "Would push {count} {} release-content asset(s)",
            provider.as_str()
        ),
        "staged" => println!(
            "Staged {count} {} release-content asset(s) for provider handoff",
            provider.as_str()
        ),
        "partial" => println!(
            "Pushed {count} {} release-content asset(s); staged remaining assets for provider handoff",
            provider.as_str()
        ),
        "skipped" => println!("No {} release-content assets to push", provider.as_str()),
        _ => println!(
            "Pushed {count} {} release-content asset(s)",
            provider.as_str()
        ),
    }
    Ok(())
}

fn read_release_provider_toml(project_dir: &Path) -> Result<ReleaseProviderToml> {
    read_release_provider_toml_from_path(&project_dir.join("fission.toml"))
}

fn read_release_provider_toml_from_path(path: &Path) -> Result<ReleaseProviderToml> {
    let data =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))
}

fn read_release_metadata(project_dir: &Path, relative: &str) -> Result<ReleaseMetadataToml> {
    let path = project_dir.join(relative);
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))
}

fn active_release(root: &ReleaseProviderToml) -> Option<&ReleaseEntryToml> {
    let active = root.release.as_ref()?.active_release.as_deref()?;
    root.releases
        .iter()
        .find(|release| release.id.as_deref() == Some(active))
}

fn parse_locale_list(locales: &str) -> Result<Vec<String>> {
    let mut values = locales
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if values.is_empty() {
        bail!("locale list is empty")
    }
    values.sort();
    Ok(values)
}

fn unsupported_release_config(provider: DistributionProvider, action: &str) -> Result<()> {
    bail!(
        "{} release-config {} is not exposed by the current provider API backend; Google Play, App Store, and Microsoft Store metadata import/diff/push are implemented",
        provider.as_str(),
        action
    )
}

fn unsupported_reviews(provider: DistributionProvider, action: &str) -> Result<()> {
    bail!(
        "{} review {} is not exposed by the current provider API backend; Google Play and App Store review list/reply are implemented",
        provider.as_str(),
        action
    )
}

fn unsupported_beta(provider: DistributionProvider, action: &str) -> Result<()> {
    bail!(
        "{} beta {} is not exposed by the current provider API backend; Google Play group management and App Store TestFlight group/tester management are implemented",
        provider.as_str(),
        action
    )
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
        .header(CONTENT_LENGTH, "0")
        .body(Vec::new())
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
        .header(CONTENT_LENGTH, "0")
        .body(Vec::new())
        .send()
        .context("failed to commit Google Play edit")?;
    json_response(response, "Google Play edit commit")?;
    Ok(())
}

fn read_google_group_csv(path: &Path) -> Result<Vec<String>> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut groups = Vec::new();
    for line in text.lines() {
        for cell in line.split(',') {
            let value = cell.trim().trim_matches('"');
            if value.contains('@') && !value.eq_ignore_ascii_case("email") && !value.is_empty() {
                groups.push(value.to_string());
            }
        }
    }
    groups.sort();
    groups.dedup();
    if groups.is_empty() {
        bail!(
            "{} did not contain any Google Group email addresses; Play API tester updates do not support individual email lists",
            path.display()
        );
    }
    Ok(groups)
}

fn latest_user_comment(review: &Value) -> Option<&Value> {
    review
        .get("comments")
        .and_then(Value::as_array)
        .and_then(|comments| {
            comments
                .iter()
                .rev()
                .find_map(|comment| comment.get("userComment"))
        })
}

fn app_store_config(project_dir: &Path) -> Result<AppStoreConfig> {
    Ok(read_release_provider_toml(project_dir)?
        .distribution
        .and_then(|distribution| distribution.app_store)
        .unwrap_or_default())
}

fn app_store_access_token(cfg: &AppStoreConfig) -> Result<String> {
    let access_token_env = app_store_access_token_env(cfg);
    if let Some(token) = env_value(&access_token_env) {
        return Ok(token);
    }
    let issuer_id_env = app_store_issuer_id_env(cfg);
    let key_id_env = app_store_key_id_env(cfg);
    let api_key_env = app_store_api_key_env(cfg);
    let api_key_base64_env = app_store_api_key_base64_env(cfg);
    let api_key_path_env = app_store_api_key_path_env(cfg);
    let issuer_id = env_value(&issuer_id_env)
        .or(cfg.issuer_id.clone())
        .with_context(|| {
            format!("distribution.app_store.issuer_id or {issuer_id_env} is required")
        })?;
    let key_id = env_value(&key_id_env)
        .or(cfg.key_id.clone())
        .with_context(|| format!("distribution.app_store.key_id or {key_id_env} is required"))?;
    let (key_source, _temp_key) =
        app_store_api_key_source_with_temp(&api_key_env, &api_key_base64_env, &api_key_path_env)?;
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
    let bundle_id = cfg
        .bundle_id
        .as_deref()
        .context("distribution.app_store.app_id or bundle_id is required for App Store Connect review operations")?;
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

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct AppStoreTester {
    email: String,
    first_name: Option<String>,
    last_name: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct AppStoreBuild {
    id: String,
    version: String,
    processing_state: Option<String>,
    uploaded_date: Option<String>,
    expired: Option<bool>,
}

fn app_store_beta_groups(client: &Client, token: &str, app_id: &str) -> Result<Value> {
    let url = format!(
        "{APP_STORE_API}/v1/apps/{app_id}/betaGroups?limit=200&fields[betaGroups]=name,createdDate,isInternalGroup,hasAccessToAllBuilds,publicLinkEnabled,publicLink,feedbackEnabled"
    );
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .context("failed to list App Store beta groups")?;
    json_response(response, "App Store beta groups list")
}

fn resolve_app_store_beta_build(
    client: &Client,
    token: &str,
    app_id: &str,
    build_number: Option<&str>,
) -> Result<AppStoreBuild> {
    let response = client
        .get(app_store_builds_url(app_id, build_number))
        .bearer_auth(token)
        .send()
        .context("failed to list App Store builds")?;
    let value = json_response(response, "App Store builds list")?;
    app_store_build_from_response(&value).with_context(|| match build_number {
        Some(build) => format!("App Store build {build} was not found for app {app_id}"),
        None => format!("No App Store builds were found for app {app_id}"),
    })
}

fn app_store_builds_url(app_id: &str, build_number: Option<&str>) -> String {
    let mut url = format!(
        "{APP_STORE_API}/v1/builds?filter[app]={}&limit=10&sort=-uploadedDate&fields[builds]=version,uploadedDate,processingState,expired,minOsVersion,usesNonExemptEncryption",
        encode_query_component(app_id)
    );
    if let Some(build_number) = build_number.filter(|value| !value.trim().is_empty()) {
        url.push_str("&filter[version]=");
        url.push_str(&encode_query_component(build_number.trim()));
    }
    url
}

fn app_store_build_from_response(value: &Value) -> Option<AppStoreBuild> {
    let item = value.get("data")?.as_array()?.first()?;
    let id = item.get("id")?.as_str()?.to_string();
    let attributes = item.get("attributes").unwrap_or(&Value::Null);
    let version = attributes.get("version")?.as_str()?.to_string();
    Some(AppStoreBuild {
        id,
        version,
        processing_state: attributes
            .get("processingState")
            .and_then(Value::as_str)
            .map(str::to_string),
        uploaded_date: attributes
            .get("uploadedDate")
            .and_then(Value::as_str)
            .map(str::to_string),
        expired: attributes.get("expired").and_then(Value::as_bool),
    })
}

fn ensure_app_store_build_assignable(build: &AppStoreBuild) -> Result<()> {
    if build.expired == Some(true) {
        bail!(
            "App Store build {} is expired and cannot be assigned to TestFlight groups",
            build.version
        );
    }
    if let Some(state) = build.processing_state.as_deref() {
        if !state.eq_ignore_ascii_case("valid") {
            bail!(
                "App Store build {} is not ready for TestFlight assignment; processingState={state}",
                build.version
            );
        }
    }
    Ok(())
}

fn app_store_artifact_build_number(path: &Path) -> Result<Option<String>> {
    let value: Value = serde_json::from_slice(
        &fs::read(path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse artifact manifest {}", path.display()))?;
    Ok(value
        .pointer("/project/build")
        .and_then(|value| {
            value
                .as_u64()
                .map(|value| value.to_string())
                .or_else(|| value.as_str().map(str::to_string))
        })
        .filter(|value| !value.trim().is_empty()))
}

fn app_store_beta_build_assignment_payload(build_id: &str) -> Value {
    json!({
        "data": [{
            "type": "builds",
            "id": build_id
        }]
    })
}

fn encode_query_component(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn resolve_app_store_beta_group(
    client: &Client,
    token: &str,
    app_id: &str,
    group: &str,
) -> Result<String> {
    let groups = app_store_beta_groups(client, token, app_id)?;
    groups
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find_map(|item| {
            let id = item.get("id").and_then(Value::as_str)?;
            let name = item
                .pointer("/attributes/name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            (id == group || name == group).then(|| id.to_string())
        })
        .with_context(|| {
            format!("App Store TestFlight group `{group}` was not found for app {app_id}")
        })
}

fn read_app_store_tester_csv(path: &Path) -> Result<Vec<AppStoreTester>> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut testers = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let cells = split_csv_line(line);
        if cells.is_empty() || cells[0].eq_ignore_ascii_case("email") {
            continue;
        }
        let email = cells[0].trim().to_string();
        if !email.contains('@') {
            bail!(
                "{} line {} does not start with a tester email address",
                path.display(),
                index + 1
            );
        }
        testers.push(AppStoreTester {
            email,
            first_name: cells
                .get(1)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            last_name: cells
                .get(2)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
        });
    }
    testers.sort_by(|left, right| left.email.cmp(&right.email));
    testers.dedup_by(|left, right| left.email == right.email);
    if testers.is_empty() {
        bail!(
            "{} did not contain any tester email addresses",
            path.display()
        );
    }
    Ok(testers)
}

fn app_store_beta_tester_payload(tester: &AppStoreTester, group_id: &str) -> Value {
    let mut attributes = serde_json::Map::new();
    attributes.insert("email".to_string(), Value::String(tester.email.clone()));
    if let Some(first_name) = &tester.first_name {
        attributes.insert("firstName".to_string(), Value::String(first_name.clone()));
    }
    if let Some(last_name) = &tester.last_name {
        attributes.insert("lastName".to_string(), Value::String(last_name.clone()));
    }
    json!({
        "data": {
            "type": "betaTesters",
            "attributes": attributes,
            "relationships": {
                "betaGroups": {
                    "data": [{"type": "betaGroups", "id": group_id}]
                }
            }
        }
    })
}

fn app_store_tester_from_value(value: &Value) -> AppStoreTester {
    let attrs = value.get("attributes").unwrap_or(&Value::Null);
    AppStoreTester {
        email: attrs
            .get("email")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        first_name: attrs
            .get("firstName")
            .and_then(Value::as_str)
            .map(str::to_string),
        last_name: attrs
            .get("lastName")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

fn split_csv_line(line: &str) -> Vec<String> {
    line.split(',')
        .map(|cell| cell.trim().trim_matches('"').to_string())
        .collect()
}

fn csv_cell(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn play_config(project_dir: &Path) -> Result<PlayStoreConfig> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let root: ReleaseProviderToml =
        toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(root
        .distribution
        .and_then(|distribution| distribution.play_store)
        .unwrap_or_default())
}

fn google_play_access_token(cfg: &PlayStoreConfig, client: &Client) -> Result<String> {
    let access_token_env = play_access_token_env(cfg);
    if let Some(token) = env_value(&access_token_env) {
        return Ok(token);
    }
    let service_account_json_env = play_service_account_json_env(cfg);
    let service_account_json_base64_env = play_service_account_json_base64_env(cfg);
    let google_application_credentials_env = play_google_application_credentials_env(cfg);
    let service_account_base64 = base64_env_secret_file(
        &service_account_json_base64_env,
        "play-service-account.json",
    )?;
    let secret_source = env_value(&service_account_json_env)
        .or_else(|| {
            service_account_base64
                .as_ref()
                .map(|file| file.path.display().to_string())
        })
        .or_else(|| env_value(&google_application_credentials_env));
    let Some(source) = secret_source else {
        bail!(
            "Google Play credentials are missing; set {service_account_json_env}, {service_account_json_base64_env}, {access_token_env}, or {google_application_credentials_env}"
        )
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
    let text = response.text()?;
    if !status.is_success() {
        bail!("{operation} failed with {status}: {text}");
    }
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse {operation} response: {text}"))
}

fn http_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(300))
        .user_agent("cargo-fission-release/0.1")
        .build()
        .context("failed to build release HTTP client")
}

fn looks_like_bearer_token(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.starts_with('{') && !Path::new(trimmed).exists() && trimmed.matches('.').count() >= 2
}

fn play_access_token_env(cfg: &PlayStoreConfig) -> String {
    configured_env_name(cfg.access_token_env.as_deref(), "PLAY_STORE_ACCESS_TOKEN")
}

fn play_service_account_json_env(cfg: &PlayStoreConfig) -> String {
    configured_env_name(
        cfg.service_account_json_env.as_deref(),
        "PLAY_STORE_SERVICE_ACCOUNT_JSON",
    )
}

fn play_service_account_json_base64_env(cfg: &PlayStoreConfig) -> String {
    configured_env_name(
        cfg.service_account_json_base64_env.as_deref(),
        "PLAY_STORE_SERVICE_ACCOUNT_JSON_BASE64",
    )
}

fn play_google_application_credentials_env(cfg: &PlayStoreConfig) -> String {
    configured_env_name(
        cfg.google_application_credentials_env.as_deref(),
        "GOOGLE_APPLICATION_CREDENTIALS",
    )
}

fn app_store_access_token_env(cfg: &AppStoreConfig) -> String {
    configured_env_name(
        cfg.access_token_env.as_deref(),
        "APP_STORE_CONNECT_ACCESS_TOKEN",
    )
}

fn app_store_issuer_id_env(cfg: &AppStoreConfig) -> String {
    configured_env_name(cfg.issuer_id_env.as_deref(), "APP_STORE_CONNECT_ISSUER_ID")
}

fn app_store_key_id_env(cfg: &AppStoreConfig) -> String {
    configured_env_name(cfg.key_id_env.as_deref(), "APP_STORE_CONNECT_KEY_ID")
}

fn app_store_api_key_env(cfg: &AppStoreConfig) -> String {
    configured_env_name(cfg.api_key_env.as_deref(), "APP_STORE_CONNECT_API_KEY")
}

fn app_store_api_key_base64_env(cfg: &AppStoreConfig) -> String {
    configured_env_name(
        cfg.api_key_base64_env.as_deref(),
        "APP_STORE_CONNECT_API_KEY_BASE64",
    )
}

fn app_store_api_key_path_env(cfg: &AppStoreConfig) -> String {
    configured_env_name(
        cfg.api_key_path_env.as_deref(),
        "APP_STORE_CONNECT_API_KEY_PATH",
    )
}

fn configured_env_name(configured: Option<&str>, default: &str) -> String {
    configured
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_string()
}

fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

struct TemporarySecretFile {
    path: PathBuf,
    temp_dir: PathBuf,
}

impl Drop for TemporarySecretFile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temp_dir);
    }
}

fn app_store_api_key_source_with_temp(
    api_key_env: &str,
    api_key_base64_env: &str,
    api_key_path_env: &str,
) -> Result<(String, Option<TemporarySecretFile>)> {
    if let Some(value) = env_value(api_key_env) {
        return Ok((value, None));
    }
    if let Some(file) = base64_env_secret_file(api_key_base64_env, "AuthKey.p8")? {
        return Ok((file.path.display().to_string(), Some(file)));
    }
    if let Some(path) = env_value(api_key_path_env) {
        return Ok((path, None));
    }
    bail!("{api_key_env}, {api_key_base64_env}, or {api_key_path_env} is required")
}

fn base64_env_secret_file(name: &str, file_name: &str) -> Result<Option<TemporarySecretFile>> {
    let Some(value) = env_value(name) else {
        return Ok(None);
    };
    let bytes = BASE64_STANDARD
        .decode(value.trim())
        .with_context(|| format!("failed to decode base64 environment variable {name}"))?;
    temporary_secret_file(name, file_name, &bytes).map(Some)
}

fn temporary_secret_file(
    kind: &str,
    file_name: &str,
    contents: &[u8],
) -> Result<TemporarySecretFile> {
    let temp_dir = env::temp_dir().join(format!(
        "fission-secret-{}-{}-{}",
        sanitize_temp_component(kind),
        std::process::id(),
        temp_nanos()
    ));
    fs::create_dir_all(&temp_dir).with_context(|| {
        format!(
            "failed to create temporary secret directory {}",
            temp_dir.display()
        )
    })?;
    set_private_dir_permissions(&temp_dir)?;
    let path = temp_dir.join(sanitize_temp_component(file_name));
    fs::write(&path, contents)
        .with_context(|| format!("failed to write temporary secret file {}", path.display()))?;
    set_private_file_permissions(&path)?;
    Ok(TemporarySecretFile { path, temp_dir })
}

fn temp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn sanitize_temp_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
#[path = "store_ops_tests.rs"]
mod tests;
