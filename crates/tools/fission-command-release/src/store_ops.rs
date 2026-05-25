use super::*;
use anyhow::{bail, Context, Result};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;
use std::time::Duration;

const PLAY_API: &str = "https://androidpublisher.googleapis.com";
const GOOGLE_PLAY_SCOPE: &str = "https://www.googleapis.com/auth/androidpublisher";
const GOOGLE_TOKEN_URI: &str = "https://oauth2.googleapis.com/token";
const APP_STORE_API: &str = "https://api.appstoreconnect.apple.com";

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
    service_account: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct AppStoreConfig {
    app_id: Option<String>,
    bundle_id: Option<String>,
    issuer_id: Option<String>,
    key_id: Option<String>,
    api_key_path: Option<String>,
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

pub(super) fn reviews_list(
    provider: DistributionProvider,
    since: Option<String>,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    match provider {
        DistributionProvider::PlayStore => play_reviews_list(project_dir, since, json_output),
        DistributionProvider::AppStore => app_store_reviews_list(project_dir, since, json_output),
        _ => unsupported_reviews(provider, "list"),
    }
}

pub(super) fn reviews_reply(
    provider: DistributionProvider,
    review: &str,
    message_file: &Path,
    project_dir: &Path,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    match provider {
        DistributionProvider::PlayStore => {
            play_reviews_reply(project_dir, review, message_file, dry_run, json_output)
        }
        DistributionProvider::AppStore => {
            app_store_reviews_reply(project_dir, review, message_file, dry_run, json_output)
        }
        _ => unsupported_reviews(provider, "reply"),
    }
}

pub(super) fn beta_groups_list(
    provider: DistributionProvider,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    match provider {
        DistributionProvider::PlayStore => play_beta_groups_list(project_dir, json_output),
        DistributionProvider::AppStore => app_store_beta_groups_list(project_dir, json_output),
        _ => unsupported_beta(provider, "groups list"),
    }
}

pub(super) fn beta_groups_sync(
    provider: DistributionProvider,
    from: &Path,
    project_dir: &Path,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    match provider {
        DistributionProvider::PlayStore => {
            play_beta_groups_sync(project_dir, from, dry_run, json_output)
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
    json_output: bool,
) -> Result<()> {
    match provider {
        DistributionProvider::PlayStore => {
            play_beta_testers_import(project_dir, track, csv, dry_run, json_output)
        }
        DistributionProvider::AppStore => {
            app_store_beta_testers_import(project_dir, group, csv, dry_run, json_output)
        }
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
            play_beta_testers_export(project_dir, track, output, json_output)
        }
        DistributionProvider::AppStore => {
            app_store_beta_testers_export(project_dir, group, output, json_output)
        }
        _ => unsupported_beta(provider, "testers export"),
    }
}

pub(super) fn release_config_import(
    provider: DistributionProvider,
    locales: Option<String>,
    yes: bool,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    match provider {
        DistributionProvider::PlayStore => {
            play_release_config_import(project_dir, locales.as_deref(), yes, json_output)
        }
        DistributionProvider::AppStore => {
            app_store_release_config_import(project_dir, locales.as_deref(), yes, json_output)
        }
        DistributionProvider::MicrosoftStore => super::microsoft_store_ops::release_config_import(
            project_dir,
            locales.as_deref(),
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
    dry_run: bool,
    yes: bool,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    match provider {
        DistributionProvider::PlayStore => {
            play_release_config_push(project_dir, locales.as_deref(), dry_run, yes, json_output)
        }
        DistributionProvider::AppStore => app_store_release_config_push(
            project_dir,
            locales.as_deref(),
            dry_run,
            yes,
            json_output,
        ),
        DistributionProvider::MicrosoftStore => super::microsoft_store_ops::release_config_push(
            project_dir,
            locales.as_deref(),
            dry_run,
            yes,
            json_output,
        ),
        _ => unsupported_release_config(provider, "push"),
    }
}

fn app_store_release_config_import(
    project_dir: &Path,
    locales: Option<&str>,
    yes: bool,
    json_output: bool,
) -> Result<()> {
    if !yes {
        bail!("release-config import mutates fission.toml/release metadata; pass --yes after reviewing the provider and locales");
    }
    let root = read_release_provider_toml(project_dir)?;
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let version_id = app_store_version_id(&root, &client, &token, &app_id)?;
    let remote = fetch_app_store_version_localizations(&client, &token, &version_id)?;
    write_imported_app_store_localizations(project_dir, &root, locales, &remote)?;
    let summary = json!({
        "provider": "app-store",
        "app_id": app_id,
        "version_id": version_id,
        "imported_locales": remote.iter().map(|item| item.locale.as_str()).collect::<Vec<_>>(),
        "status": "imported"
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!("Imported {} App Store localization(s)", remote.len());
    }
    Ok(())
}

fn app_store_release_config_diff(project_dir: &Path, json_output: bool) -> Result<()> {
    let root = read_release_provider_toml(project_dir)?;
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let version_id = app_store_version_id(&root, &client, &token, &app_id)?;
    let locales = resolve_release_locales(&root, None)?;
    let local = resolve_app_store_localizations(project_dir, &root, &locales)?;
    let remote = fetch_app_store_version_localizations(&client, &token, &version_id)?;
    let diff = app_store_localization_diff(&local, &remote);
    if json_output {
        println!("{}", serde_json::to_string_pretty(&diff)?);
    } else if diff.as_array().is_some_and(Vec::is_empty) {
        println!(
            "App Store metadata is in sync for {} locale(s)",
            locales.len()
        );
    } else {
        println!("App Store metadata differences:");
        for item in diff.as_array().into_iter().flatten() {
            println!(
                "{} {}: local={:?} remote={:?}",
                item.get("locale")
                    .and_then(Value::as_str)
                    .unwrap_or("<locale>"),
                item.get("field")
                    .and_then(Value::as_str)
                    .unwrap_or("<field>"),
                item.get("local"),
                item.get("remote")
            );
        }
    }
    Ok(())
}

fn app_store_release_config_push(
    project_dir: &Path,
    locales_arg: Option<&str>,
    dry_run: bool,
    yes: bool,
    json_output: bool,
) -> Result<()> {
    if !dry_run && !yes {
        bail!("release-config push mutates provider metadata; pass --yes after reviewing `release-config diff`");
    }
    let root = read_release_provider_toml(project_dir)?;
    let cfg = app_store_config(project_dir)?;
    let locales = resolve_release_locales(&root, locales_arg)?;
    let localizations = resolve_app_store_localizations(project_dir, &root, &locales)?;
    if dry_run {
        let value = json!({
            "provider": "app-store",
            "locales": locales,
            "localizations": localizations,
            "status": "dry-run"
        });
        if json_output {
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!(
                "Would push {} App Store localization(s)",
                localizations.len()
            );
        }
        return Ok(());
    }
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let version_id = app_store_version_id(&root, &client, &token, &app_id)?;
    let remote = fetch_app_store_version_localizations(&client, &token, &version_id)?;
    let mut responses = Vec::new();
    for localization in &localizations {
        if let Some(existing) = remote
            .iter()
            .find(|item| item.locale == localization.locale)
        {
            let response = client
                .patch(format!(
                    "{APP_STORE_API}/v1/appStoreVersionLocalizations/{}",
                    existing
                        .id
                        .as_deref()
                        .context("remote localization missing id")?
                ))
                .bearer_auth(&token)
                .json(&app_store_localization_update_payload(
                    existing.id.as_deref().unwrap(),
                    localization,
                ))
                .send()
                .with_context(|| {
                    format!(
                        "failed to update App Store localization {}",
                        localization.locale
                    )
                })?;
            responses.push(json_response(response, "App Store localization update")?);
        } else {
            let response = client
                .post(format!("{APP_STORE_API}/v1/appStoreVersionLocalizations"))
                .bearer_auth(&token)
                .json(&app_store_localization_create_payload(
                    &version_id,
                    localization,
                ))
                .send()
                .with_context(|| {
                    format!(
                        "failed to create App Store localization {}",
                        localization.locale
                    )
                })?;
            responses.push(json_response(response, "App Store localization create")?);
        }
    }
    let value = json!({
        "provider": "app-store",
        "app_id": app_id,
        "version_id": version_id,
        "updated_locales": localizations.iter().map(|item| item.locale.as_str()).collect::<Vec<_>>(),
        "responses": responses,
        "status": "pushed"
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("Pushed {} App Store localization(s)", localizations.len());
    }
    Ok(())
}

fn play_release_config_import(
    project_dir: &Path,
    locales: Option<&str>,
    yes: bool,
    json_output: bool,
) -> Result<()> {
    if !yes {
        bail!("release-config import mutates fission.toml/release metadata; pass --yes after reviewing the provider and locales");
    }
    let mut root = read_release_provider_toml(project_dir)?;
    let cfg = root
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.play_store.clone())
        .unwrap_or_default();
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play metadata import")?;
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let remote = fetch_play_listings(&client, &token, package_name, &edit_id, locales)?;
    write_imported_play_listings(project_dir, &mut root, &remote)?;
    let summary = json!({
        "provider": "play-store",
        "package_name": package_name,
        "imported_locales": remote.iter().map(|listing| listing.locale.as_str()).collect::<Vec<_>>(),
        "status": "imported"
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!(
            "Imported {} Google Play listing locale(s) into fission.toml/release metadata",
            remote.len()
        );
    }
    Ok(())
}

fn play_release_config_diff(project_dir: &Path, json_output: bool) -> Result<()> {
    let root = read_release_provider_toml(project_dir)?;
    let cfg = root
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.play_store.clone())
        .unwrap_or_default();
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play metadata diff")?;
    let locales = resolve_release_locales(&root, None)?;
    let local = resolve_play_listings(project_dir, &root, &locales)?;
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let remote = fetch_play_listings(
        &client,
        &token,
        package_name,
        &edit_id,
        Some(&locales.join(",")),
    )?;
    let diff = play_listing_diff(&local, &remote);
    if json_output {
        println!("{}", serde_json::to_string_pretty(&diff)?);
    } else if diff.as_array().is_some_and(Vec::is_empty) {
        println!(
            "Google Play listing metadata is in sync for {} locale(s)",
            locales.len()
        );
    } else {
        println!("Google Play listing metadata differences:");
        for item in diff.as_array().into_iter().flatten() {
            println!(
                "{} {}: local={:?} remote={:?}",
                item.get("locale")
                    .and_then(Value::as_str)
                    .unwrap_or("<locale>"),
                item.get("field")
                    .and_then(Value::as_str)
                    .unwrap_or("<field>"),
                item.get("local"),
                item.get("remote")
            );
        }
    }
    Ok(())
}

fn play_release_config_push(
    project_dir: &Path,
    locales_arg: Option<&str>,
    dry_run: bool,
    yes: bool,
    json_output: bool,
) -> Result<()> {
    if !dry_run && !yes {
        bail!("release-config push mutates provider metadata; pass --yes after reviewing `release-config diff`");
    }
    let root = read_release_provider_toml(project_dir)?;
    let cfg = root
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.play_store.clone())
        .unwrap_or_default();
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play metadata push")?;
    let locales = resolve_release_locales(&root, locales_arg)?;
    let listings = resolve_play_listings(project_dir, &root, &locales)?;
    if dry_run {
        let value = json!({
            "provider": "play-store",
            "package_name": package_name,
            "locales": locales,
            "listings": listings,
            "status": "dry-run"
        });
        if json_output {
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!(
                "Would push {} Google Play listing locale(s) for {package_name}",
                listings.len()
            );
        }
        return Ok(());
    }
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let mut responses = Vec::new();
    for listing in &listings {
        let url = format!(
            "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/listings/{}",
            listing.locale
        );
        let response = client
            .put(url)
            .bearer_auth(&token)
            .json(&json!({
                "title": listing.title,
                "shortDescription": listing.short_description,
                "fullDescription": listing.full_description,
                "video": listing.video,
            }))
            .send()
            .with_context(|| format!("failed to update Google Play listing {}", listing.locale))?;
        responses.push(json_response(response, "Google Play listing update")?);
    }
    validate_play_edit(&client, &token, package_name, &edit_id)?;
    commit_play_edit(&client, &token, package_name, &edit_id)?;
    let value = json!({
        "provider": "play-store",
        "package_name": package_name,
        "updated_locales": listings.iter().map(|listing| listing.locale.as_str()).collect::<Vec<_>>(),
        "responses": responses,
        "status": "pushed"
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!(
            "Pushed {} Google Play listing locale(s) for {package_name}",
            listings.len()
        );
    }
    Ok(())
}

fn app_store_reviews_list(
    project_dir: &Path,
    since: Option<String>,
    json_output: bool,
) -> Result<()> {
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let url = format!(
        "{APP_STORE_API}/v1/apps/{app_id}/customerReviews?limit=200&sort=-createdDate&fields[customerReviews]=rating,title,body,reviewerNickname,createdDate,territory,response"
    );
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .context("failed to list App Store customer reviews")?;
    let value = json_response(response, "App Store customer reviews list")?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    println!("App Store reviews for app {app_id}");
    if let Some(since) = since {
        println!("Requested window: {since} (App Store Connect returned newest-first; filter locally if needed)");
    }
    for review in value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = review.get("id").and_then(Value::as_str).unwrap_or("<id>");
        let attrs = review.get("attributes").unwrap_or(&Value::Null);
        let rating = attrs
            .get("rating")
            .and_then(Value::as_i64)
            .map(|rating| rating.to_string())
            .unwrap_or_else(|| "?".to_string());
        let title = attrs.get("title").and_then(Value::as_str).unwrap_or("");
        let body = attrs
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or("")
            .replace('\n', " ");
        println!("{id} [{rating}/5] {title}: {body}");
    }
    Ok(())
}

fn app_store_reviews_reply(
    project_dir: &Path,
    review: &str,
    message_file: &Path,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    let reply = fs::read_to_string(message_file)
        .with_context(|| format!("failed to read {}", message_file.display()))?;
    let payload = app_store_review_response_payload(review, reply.trim());
    if dry_run {
        let value = json!({
            "provider": "app-store",
            "review": review,
            "reply_text_bytes": reply.len(),
            "payload": payload,
            "status": "dry-run"
        });
        if json_output {
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!("Would reply to App Store review {review}");
        }
        return Ok(());
    }
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let response = client
        .post(format!("{APP_STORE_API}/v1/customerReviewResponses"))
        .bearer_auth(token)
        .json(&payload)
        .send()
        .context("failed to reply to App Store review")?;
    let value = json_response(response, "App Store review reply")?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("Replied to App Store review {review}");
    }
    Ok(())
}

fn app_store_review_response_payload(review: &str, response_body: &str) -> Value {
    json!({
        "data": {
            "type": "customerReviewResponses",
            "attributes": {
                "responseBody": response_body,
            },
            "relationships": {
                "review": {
                    "data": {
                        "type": "customerReviews",
                        "id": review,
                    }
                }
            }
        }
    })
}

fn play_reviews_list(project_dir: &Path, since: Option<String>, json_output: bool) -> Result<()> {
    let cfg = play_config(project_dir)?;
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play reviews")?;
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let url =
        format!("{PLAY_API}/androidpublisher/v3/applications/{package_name}/reviews?maxResults=50");
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .context("failed to list Google Play reviews")?;
    let value = json_response(response, "Google Play reviews list")?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    println!("Google Play reviews for {package_name}");
    if let Some(since) = since {
        println!("Requested window: {since} (Google Play API pagination is returned newest-first; filter locally if needed)");
    }
    for review in value
        .get("reviews")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = review
            .get("reviewId")
            .and_then(Value::as_str)
            .unwrap_or("<id>");
        let author = review
            .get("authorName")
            .and_then(Value::as_str)
            .unwrap_or("<anonymous>");
        let user = latest_user_comment(review);
        let rating = user
            .and_then(|comment| comment.get("starRating"))
            .and_then(Value::as_i64)
            .map(|rating| rating.to_string())
            .unwrap_or_else(|| "?".to_string());
        let text = user
            .and_then(|comment| comment.get("text"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .replace('\n', " ");
        println!("{id} [{rating}/5] {author}: {text}");
    }
    Ok(())
}

fn play_reviews_reply(
    project_dir: &Path,
    review: &str,
    message_file: &Path,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    let cfg = play_config(project_dir)?;
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play reviews")?;
    let reply = fs::read_to_string(message_file)
        .with_context(|| format!("failed to read {}", message_file.display()))?;
    if dry_run {
        let value = json!({
            "provider": "play-store",
            "package_name": package_name,
            "review": review,
            "reply_text_bytes": reply.len(),
            "status": "dry-run"
        });
        if json_output {
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!("Would reply to Google Play review {review} for {package_name}");
        }
        return Ok(());
    }
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/reviews/{review}:reply"
    );
    let response = client
        .post(url)
        .bearer_auth(token)
        .json(&json!({ "replyText": reply.trim() }))
        .send()
        .context("failed to reply to Google Play review")?;
    let value = json_response(response, "Google Play review reply")?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("Replied to Google Play review {review}");
    }
    Ok(())
}

fn play_beta_groups_list(project_dir: &Path, json_output: bool) -> Result<()> {
    let cfg = play_config(project_dir)?;
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play beta groups")?;
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let mut tracks = Vec::new();
    for track in ["internal", "closed", "open"] {
        let url = format!(
            "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/testers/{track}"
        );
        let response = client
            .get(url)
            .bearer_auth(&token)
            .send()
            .with_context(|| format!("failed to get Google Play testers for {track}"))?;
        let value = json_response(response, "Google Play testers get")?;
        tracks.push(json!({
            "track": track,
            "googleGroups": value.get("googleGroups").cloned().unwrap_or_else(|| json!([]))
        }));
    }
    let value = json!({ "package_name": package_name, "tracks": tracks });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("Google Play tester groups for {package_name}");
        for track in value
            .get("tracks")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = track
                .get("track")
                .and_then(Value::as_str)
                .unwrap_or("<track>");
            let groups = track
                .get("googleGroups")
                .and_then(Value::as_array)
                .map(|groups| {
                    groups
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            println!("{name}: {groups}");
        }
    }
    Ok(())
}

fn app_store_beta_groups_list(project_dir: &Path, json_output: bool) -> Result<()> {
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let value = app_store_beta_groups(&client, &token, &app_id)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("App Store TestFlight groups for app {app_id}");
        for group in value
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let id = group.get("id").and_then(Value::as_str).unwrap_or("<id>");
            let attrs = group.get("attributes").unwrap_or(&Value::Null);
            let name = attrs
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("<name>");
            let public_link = attrs
                .get("publicLink")
                .and_then(Value::as_str)
                .unwrap_or("");
            println!("{id} {name} {public_link}");
        }
    }
    Ok(())
}

fn app_store_beta_testers_import(
    project_dir: &Path,
    group: Option<&str>,
    csv: &Path,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    let group = group.context("App Store tester import requires --group <group-id-or-name>")?;
    let testers = read_app_store_tester_csv(csv)?;
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let group_id = resolve_app_store_beta_group(&client, &token, &app_id, group)?;
    if dry_run {
        let value = json!({
            "provider": "app-store",
            "app_id": app_id,
            "group": group,
            "group_id": group_id,
            "testers": testers,
            "status": "dry-run"
        });
        if json_output {
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!(
                "Would import {} App Store TestFlight tester(s) into group {group}",
                testers.len()
            );
        }
        return Ok(());
    }
    let mut responses = Vec::new();
    for tester in &testers {
        let response = client
            .post(format!("{APP_STORE_API}/v1/betaTesters"))
            .bearer_auth(&token)
            .json(&app_store_beta_tester_payload(tester, &group_id))
            .send()
            .with_context(|| format!("failed to create App Store beta tester {}", tester.email))?;
        responses.push(json_response(response, "App Store beta tester create")?);
    }
    let value = json!({
        "provider": "app-store",
        "app_id": app_id,
        "group": group,
        "group_id": group_id,
        "created": responses.len(),
        "responses": responses,
        "status": "imported"
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!(
            "Imported {} App Store TestFlight tester(s) into group {group}",
            responses.len()
        );
    }
    Ok(())
}

fn app_store_beta_testers_export(
    project_dir: &Path,
    group: Option<&str>,
    output: &Path,
    json_output: bool,
) -> Result<()> {
    let group = group.context("App Store tester export requires --group <group-id-or-name>")?;
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let group_id = resolve_app_store_beta_group(&client, &token, &app_id, group)?;
    let url = format!(
        "{APP_STORE_API}/v1/betaGroups/{group_id}/betaTesters?limit=200&fields[betaTesters]=email,firstName,lastName,inviteType,state"
    );
    let response = client
        .get(url)
        .bearer_auth(&token)
        .send()
        .context("failed to list App Store beta testers")?;
    let value = json_response(response, "App Store beta testers list")?;
    let testers = value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(app_store_tester_from_value)
        .collect::<Vec<_>>();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut csv = String::from("email,first_name,last_name\n");
    for tester in &testers {
        csv.push_str(&format!(
            "{},{},{}\n",
            csv_cell(&tester.email),
            csv_cell(tester.first_name.as_deref().unwrap_or("")),
            csv_cell(tester.last_name.as_deref().unwrap_or(""))
        ));
    }
    fs::write(output, csv)?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "provider": "app-store",
                "app_id": app_id,
                "group": group,
                "group_id": group_id,
                "output": output,
                "count": testers.len()
            }))?
        );
    } else {
        println!(
            "Exported {} App Store TestFlight tester(s) to {}",
            testers.len(),
            output.display()
        );
    }
    Ok(())
}

fn play_beta_groups_sync(
    project_dir: &Path,
    source: &Path,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    let source = if source.is_absolute() {
        source.to_path_buf()
    } else {
        project_dir.join(source)
    };
    let root = read_release_provider_toml_from_path(&source)?;
    let tracks = root
        .beta
        .and_then(|beta| beta.play_store)
        .map(|play| play.tracks)
        .unwrap_or_default();
    if tracks.is_empty() {
        bail!(
            "{} does not contain [beta.play_store.tracks.<track>] entries",
            source.display()
        );
    }
    let updates = tracks
        .into_iter()
        .map(|(track, config)| {
            let mut groups = config.groups;
            if let Some(group) = config.group {
                groups.push(group);
            }
            groups.retain(|group| !group.trim().is_empty());
            groups.sort();
            groups.dedup();
            if groups.is_empty() {
                bail!("beta.play_store.tracks.{track} must set group or groups");
            }
            if config
                .tester_source
                .as_deref()
                .is_some_and(|source| source != "google_group")
            {
                bail!("Google Play beta group sync supports tester_source = \"google_group\" for track {track}");
            }
            Ok((track, groups))
        })
        .collect::<Result<Vec<_>>>()?;
    let cfg = play_config(project_dir)?;
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play beta group sync")?;
    if dry_run {
        let value = json!({
            "provider": "play-store",
            "package_name": package_name,
            "tracks": updates.iter().map(|(track, groups)| json!({"track": track, "googleGroups": groups})).collect::<Vec<_>>(),
            "status": "dry-run"
        });
        if json_output {
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!(
                "Would sync Google Play tester groups for {} track(s)",
                updates.len()
            );
        }
        return Ok(());
    }
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let mut responses = Vec::new();
    for (track, groups) in &updates {
        let url = format!(
            "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/testers/{track}"
        );
        let response = client
            .put(url)
            .bearer_auth(&token)
            .json(&json!({ "googleGroups": groups }))
            .send()
            .with_context(|| format!("failed to update Google Play testers for {track}"))?;
        responses.push(json_response(response, "Google Play testers update")?);
    }
    validate_play_edit(&client, &token, package_name, &edit_id)?;
    commit_play_edit(&client, &token, package_name, &edit_id)?;
    let value = json!({
        "provider": "play-store",
        "package_name": package_name,
        "tracks": updates.iter().map(|(track, groups)| json!({"track": track, "googleGroups": groups})).collect::<Vec<_>>(),
        "responses": responses,
        "status": "synced"
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!(
            "Synced Google Play tester groups for {} track(s)",
            updates.len()
        );
    }
    Ok(())
}

fn play_beta_testers_import(
    project_dir: &Path,
    track: Option<&str>,
    csv: &Path,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    let track = track.context("Google Play tester import requires --track internal|closed|open")?;
    let groups = read_google_group_csv(csv)?;
    let cfg = play_config(project_dir)?;
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play beta testers")?;
    if dry_run {
        let value = json!({
            "provider": "play-store",
            "package_name": package_name,
            "track": track,
            "googleGroups": groups,
            "status": "dry-run"
        });
        if json_output {
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!(
                "Would set {} Google Groups on Play track {track}",
                value
                    .get("googleGroups")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0)
            );
        }
        return Ok(());
    }
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/testers/{track}"
    );
    let response = client
        .put(url)
        .bearer_auth(&token)
        .json(&json!({ "googleGroups": groups }))
        .send()
        .context("failed to update Google Play testers")?;
    let value = json_response(response, "Google Play testers update")?;
    validate_play_edit(&client, &token, package_name, &edit_id)?;
    commit_play_edit(&client, &token, package_name, &edit_id)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("Updated Google Play tester groups for {package_name} track {track}");
    }
    Ok(())
}

fn play_beta_testers_export(
    project_dir: &Path,
    track: Option<&str>,
    output: &Path,
    json_output: bool,
) -> Result<()> {
    let track = track.context("Google Play tester export requires --track internal|closed|open")?;
    let cfg = play_config(project_dir)?;
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play beta testers")?;
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/testers/{track}"
    );
    let response = client
        .get(url)
        .bearer_auth(&token)
        .send()
        .context("failed to get Google Play testers")?;
    let value = json_response(response, "Google Play testers get")?;
    let groups = value
        .get("googleGroups")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, groups.join("\n") + "\n")?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "provider": "play-store",
                "package_name": package_name,
                "track": track,
                "output": output,
                "googleGroups": groups
            }))?
        );
    } else {
        println!(
            "Exported {} Google Groups to {}",
            groups.len(),
            output.display()
        );
    }
    Ok(())
}

fn fetch_app_store_version_localizations(
    client: &Client,
    token: &str,
    version_id: &str,
) -> Result<Vec<AppStoreLocalization>> {
    let response = client
        .get(format!(
            "{APP_STORE_API}/v1/appStoreVersions/{version_id}/appStoreVersionLocalizations?limit=200"
        ))
        .bearer_auth(token)
        .send()
        .context("failed to list App Store version localizations")?;
    let value = json_response(response, "App Store localizations list")?;
    value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(app_store_localization_from_value)
        .collect()
}

fn app_store_localization_from_value(value: &Value) -> Result<AppStoreLocalization> {
    let attrs = value.get("attributes").unwrap_or(&Value::Null);
    Ok(AppStoreLocalization {
        id: value.get("id").and_then(Value::as_str).map(str::to_string),
        locale: attrs
            .get("locale")
            .and_then(Value::as_str)
            .context("App Store localization missing locale")?
            .to_string(),
        description: attrs
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        keywords: attrs
            .get("keywords")
            .and_then(Value::as_str)
            .map(str::to_string),
        marketing_url: attrs
            .get("marketingUrl")
            .and_then(Value::as_str)
            .map(str::to_string),
        promotional_text: attrs
            .get("promotionalText")
            .and_then(Value::as_str)
            .map(str::to_string),
        support_url: attrs
            .get("supportUrl")
            .and_then(Value::as_str)
            .map(str::to_string),
        whats_new: attrs
            .get("whatsNew")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn resolve_app_store_localizations(
    project_dir: &Path,
    root: &ReleaseProviderToml,
    locales: &[String],
) -> Result<Vec<AppStoreLocalization>> {
    let metadata = active_release(root)
        .and_then(|release| release.metadata.as_deref())
        .map(|metadata| read_release_metadata(project_dir, metadata))
        .transpose()?
        .unwrap_or_default();
    locales
        .iter()
        .map(|locale| resolve_app_store_localization(project_dir, root, &metadata, locale))
        .collect()
}

fn resolve_app_store_localization(
    project_dir: &Path,
    root: &ReleaseProviderToml,
    metadata: &ReleaseMetadataToml,
    locale: &str,
) -> Result<AppStoreLocalization> {
    let listing = root
        .release
        .as_ref()
        .and_then(|release| release.store_listing.get("app_store"))
        .and_then(|listings| listings.get(locale))
        .cloned()
        .unwrap_or_default();
    let meta = metadata.app_store.get(locale).cloned().unwrap_or_default();
    let description = meta.description.with_context(|| {
        format!("active release metadata [app_store.{locale}].description is required")
    })?;
    let whats_new = active_release(root)
        .and_then(|release| release.release_notes.as_deref())
        .map(|notes| project_dir.join(notes).join(format!("{locale}.md")))
        .filter(|path| path.exists())
        .map(fs::read_to_string)
        .transpose()?;
    Ok(AppStoreLocalization {
        id: None,
        locale: locale.to_string(),
        description,
        keywords: (!listing.keywords.is_empty()).then(|| listing.keywords.join(",")),
        marketing_url: listing.marketing_url,
        promotional_text: meta.promotional_text,
        support_url: listing.support_url,
        whats_new: whats_new
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    })
}

fn app_store_version_id(
    root: &ReleaseProviderToml,
    client: &Client,
    token: &str,
    app_id: &str,
) -> Result<String> {
    let version = active_release(root)
        .and_then(|release| release.version.as_deref())
        .or_else(|| {
            root.release
                .as_ref()?
                .active_release
                .as_deref()
                .and_then(|id| id.split('+').next())
        })
        .context("active [[releases]].version is required for App Store metadata sync")?;
    let response = client
        .get(format!(
            "{APP_STORE_API}/v1/apps/{app_id}/appStoreVersions?filter[versionString]={version}&limit=1"
        ))
        .bearer_auth(token)
        .send()
        .context("failed to list App Store versions")?;
    let value = json_response(response, "App Store versions list")?;
    value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .with_context(|| format!("App Store version {version} was not found for app {app_id}"))
}

fn app_store_localization_update_payload(id: &str, localization: &AppStoreLocalization) -> Value {
    json!({
        "data": {
            "type": "appStoreVersionLocalizations",
            "id": id,
            "attributes": app_store_localization_attributes(localization)
        }
    })
}

fn app_store_localization_create_payload(
    version_id: &str,
    localization: &AppStoreLocalization,
) -> Value {
    json!({
        "data": {
            "type": "appStoreVersionLocalizations",
            "attributes": app_store_localization_attributes(localization),
            "relationships": {
                "appStoreVersion": {
                    "data": {"type": "appStoreVersions", "id": version_id}
                }
            }
        }
    })
}

fn app_store_localization_attributes(localization: &AppStoreLocalization) -> Value {
    let mut attrs = serde_json::Map::new();
    attrs.insert(
        "locale".to_string(),
        Value::String(localization.locale.clone()),
    );
    attrs.insert(
        "description".to_string(),
        Value::String(localization.description.clone()),
    );
    if let Some(value) = &localization.keywords {
        attrs.insert("keywords".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = &localization.marketing_url {
        attrs.insert("marketingUrl".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = &localization.promotional_text {
        attrs.insert("promotionalText".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = &localization.support_url {
        attrs.insert("supportUrl".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = &localization.whats_new {
        attrs.insert("whatsNew".to_string(), Value::String(value.clone()));
    }
    Value::Object(attrs)
}

fn write_imported_app_store_localizations(
    project_dir: &Path,
    root: &ReleaseProviderToml,
    locales_arg: Option<&str>,
    remote: &[AppStoreLocalization],
) -> Result<()> {
    let selected = locales_arg.map(parse_locale_list).transpose()?;
    let selected = selected.as_ref();
    let fission_path = project_dir.join("fission.toml");
    let metadata_path = active_release(root)
        .and_then(|release| release.metadata.as_deref())
        .map(|metadata| project_dir.join(metadata))
        .context("active release metadata path is required for App Store metadata import")?;
    let mut metadata_doc: toml::Value = if metadata_path.exists() {
        toml::from_str(&fs::read_to_string(&metadata_path)?)?
    } else {
        toml::Value::Table(Default::default())
    };
    let mut fission_doc =
        parse_toml_edit_document(&fs::read_to_string(&fission_path)?, &fission_path)?;
    for item in remote {
        if selected.is_some_and(|selected| !selected.contains(&item.locale)) {
            continue;
        }
        set_toml_path(
            &mut metadata_doc,
            &format!("app_store.{}.description", item.locale),
            toml::Value::String(item.description.clone()),
        )?;
        if let Some(value) = &item.promotional_text {
            set_toml_path(
                &mut metadata_doc,
                &format!("app_store.{}.promotional_text", item.locale),
                toml::Value::String(value.clone()),
            )?;
        }
        if let Some(value) = &item.support_url {
            set_toml_edit_path(
                &mut fission_doc,
                &format!(
                    "release.store_listing.app_store.{}.support_url",
                    item.locale
                ),
                toml_edit::value(value.clone()),
            )?;
        }
        if let Some(value) = &item.marketing_url {
            set_toml_edit_path(
                &mut fission_doc,
                &format!(
                    "release.store_listing.app_store.{}.marketing_url",
                    item.locale
                ),
                toml_edit::value(value.clone()),
            )?;
        }
        if let Some(value) = &item.keywords {
            set_toml_edit_path(
                &mut fission_doc,
                &format!("release.store_listing.app_store.{}.keywords", item.locale),
                toml_edit_string_array(
                    value
                        .split(',')
                        .map(|item| item.trim().to_string())
                        .collect::<Vec<_>>(),
                ),
            )?;
        }
    }
    if let Some(parent) = metadata_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &metadata_path,
        toml::to_string_pretty(&metadata_doc)? + "\n",
    )?;
    write_toml_edit_document(&fission_path, &fission_doc)?;
    Ok(())
}

fn app_store_localization_diff(
    local: &[AppStoreLocalization],
    remote: &[AppStoreLocalization],
) -> Value {
    let mut remote_by_locale = remote
        .iter()
        .map(|item| (item.locale.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let mut diffs = Vec::new();
    for local_item in local {
        match remote_by_locale.remove(local_item.locale.as_str()) {
            Some(remote_item) => {
                push_field_diff(&mut diffs, &local_item.locale, "description", &local_item.description, &remote_item.description);
                push_option_diff(&mut diffs, &local_item.locale, "keywords", &local_item.keywords, &remote_item.keywords);
                push_option_diff(&mut diffs, &local_item.locale, "marketing_url", &local_item.marketing_url, &remote_item.marketing_url);
                push_option_diff(&mut diffs, &local_item.locale, "promotional_text", &local_item.promotional_text, &remote_item.promotional_text);
                push_option_diff(&mut diffs, &local_item.locale, "support_url", &local_item.support_url, &remote_item.support_url);
                push_option_diff(&mut diffs, &local_item.locale, "whats_new", &local_item.whats_new, &remote_item.whats_new);
            }
            None => diffs.push(json!({"locale": local_item.locale, "field": "localization", "local": "present", "remote": "missing"})),
        }
    }
    Value::Array(diffs)
}

fn push_option_diff(
    diffs: &mut Vec<Value>,
    locale: &str,
    field: &str,
    local: &Option<String>,
    remote: &Option<String>,
) {
    if local != remote {
        diffs.push(json!({"locale": locale, "field": field, "local": local, "remote": remote}));
    }
}

fn fetch_play_listings(
    client: &Client,
    token: &str,
    package_name: &str,
    edit_id: &str,
    locales: Option<&str>,
) -> Result<Vec<PlayListing>> {
    let locale_list = locales.map(parse_locale_list).transpose()?;
    if let Some(locales) = locale_list {
        return locales
            .into_iter()
            .map(|locale| fetch_play_listing(client, token, package_name, edit_id, &locale))
            .collect();
    }
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/listings"
    );
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .context("failed to list Google Play listings")?;
    let value = json_response(response, "Google Play listings list")?;
    value
        .get("listings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(play_listing_from_value)
        .collect()
}

fn fetch_play_listing(
    client: &Client,
    token: &str,
    package_name: &str,
    edit_id: &str,
    locale: &str,
) -> Result<PlayListing> {
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/listings/{locale}"
    );
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .with_context(|| format!("failed to get Google Play listing {locale}"))?;
    play_listing_from_value(&json_response(response, "Google Play listing get")?)
}

fn play_listing_from_value(value: &Value) -> Result<PlayListing> {
    let locale = value
        .get("language")
        .or_else(|| value.get("locale"))
        .and_then(Value::as_str)
        .context("Google Play listing response did not contain language")?
        .to_string();
    Ok(PlayListing {
        locale,
        title: value
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        short_description: value
            .get("shortDescription")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        full_description: value
            .get("fullDescription")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        video: value
            .get("video")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn resolve_release_locales(
    root: &ReleaseProviderToml,
    locales_arg: Option<&str>,
) -> Result<Vec<String>> {
    if let Some(locales) = locales_arg {
        return parse_locale_list(locales);
    }
    let active = active_release(root);
    if let Some(release) = active
        .as_ref()
        .filter(|release| !release.locales.is_empty())
    {
        return Ok(release.locales.clone());
    }
    if let Some(release) = root
        .release
        .as_ref()
        .filter(|release| !release.default_locales.is_empty())
    {
        return Ok(release.default_locales.clone());
    }
    let listing_locales = root
        .release
        .as_ref()
        .and_then(|release| release.store_listing.get("play_store"))
        .map(|listings| listings.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    if listing_locales.is_empty() {
        bail!("no release locales configured; set release.default_locales, [[releases]].locales, or pass --locales")
    }
    Ok(listing_locales)
}

fn resolve_play_listings(
    project_dir: &Path,
    root: &ReleaseProviderToml,
    locales: &[String],
) -> Result<Vec<PlayListing>> {
    let metadata = active_release(root)
        .and_then(|release| release.metadata.as_deref())
        .map(|metadata| read_release_metadata(project_dir, metadata))
        .transpose()?
        .unwrap_or_default();
    locales
        .iter()
        .map(|locale| resolve_play_listing(root, &metadata, locale))
        .collect()
}

fn resolve_play_listing(
    root: &ReleaseProviderToml,
    metadata: &ReleaseMetadataToml,
    locale: &str,
) -> Result<PlayListing> {
    let listing = root
        .release
        .as_ref()
        .and_then(|release| release.store_listing.get("play_store"))
        .and_then(|listings| listings.get(locale))
        .cloned()
        .unwrap_or_default();
    let meta = metadata.play_store.get(locale).cloned().unwrap_or_default();
    let title = listing.title.or(listing.name).with_context(|| {
        format!("release.store_listing.play_store.{locale}.title or .name is required")
    })?;
    let short_description = listing
        .short_description
        .or(listing.subtitle)
        .with_context(|| {
            format!("release.store_listing.play_store.{locale}.short_description is required")
        })?;
    let full_description = meta
        .full_description
        .or(meta.description)
        .with_context(|| {
            format!("active release metadata [play_store.{locale}].full_description is required")
        })?;
    Ok(PlayListing {
        locale: locale.to_string(),
        title,
        short_description,
        full_description,
        video: listing.video.or(listing.video_url),
    })
}

fn write_imported_play_listings(
    project_dir: &Path,
    root: &mut ReleaseProviderToml,
    listings: &[PlayListing],
) -> Result<()> {
    let fission_path = project_dir.join("fission.toml");
    let data = fs::read_to_string(&fission_path)
        .with_context(|| format!("failed to read {}", fission_path.display()))?;
    let mut doc = parse_toml_edit_document(&data, &fission_path)?;
    for listing in listings {
        set_toml_edit_path(
            &mut doc,
            &format!("release.store_listing.play_store.{}.title", listing.locale),
            toml_edit::value(listing.title.clone()),
        )?;
        set_toml_edit_path(
            &mut doc,
            &format!(
                "release.store_listing.play_store.{}.short_description",
                listing.locale
            ),
            toml_edit::value(listing.short_description.clone()),
        )?;
        if let Some(video) = &listing.video {
            set_toml_edit_path(
                &mut doc,
                &format!("release.store_listing.play_store.{}.video", listing.locale),
                toml_edit::value(video.clone()),
            )?;
        }
    }
    write_toml_edit_document(&fission_path, &doc)?;

    let metadata_path = active_release(root)
        .and_then(|release| release.metadata.as_deref())
        .map(|metadata| project_dir.join(metadata));
    if let Some(metadata_path) = metadata_path {
        let mut metadata_doc: toml::Value = if metadata_path.exists() {
            toml::from_str(&fs::read_to_string(&metadata_path)?)?
        } else {
            toml::Value::Table(Default::default())
        };
        for listing in listings {
            set_toml_path(
                &mut metadata_doc,
                &format!("play_store.{}.full_description", listing.locale),
                toml::Value::String(listing.full_description.clone()),
            )?;
        }
        if let Some(parent) = metadata_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            &metadata_path,
            toml::to_string_pretty(&metadata_doc)? + "\n",
        )
        .with_context(|| format!("failed to write {}", metadata_path.display()))?;
    }
    Ok(())
}

fn play_listing_diff(local: &[PlayListing], remote: &[PlayListing]) -> Value {
    let mut remote_by_locale = remote
        .iter()
        .map(|listing| (listing.locale.as_str(), listing))
        .collect::<BTreeMap<_, _>>();
    let mut diffs = Vec::new();
    for local_listing in local {
        let remote_listing = remote_by_locale.remove(local_listing.locale.as_str());
        match remote_listing {
            Some(remote_listing) => {
                push_field_diff(&mut diffs, &local_listing.locale, "title", &local_listing.title, &remote_listing.title);
                push_field_diff(&mut diffs, &local_listing.locale, "short_description", &local_listing.short_description, &remote_listing.short_description);
                push_field_diff(&mut diffs, &local_listing.locale, "full_description", &local_listing.full_description, &remote_listing.full_description);
                if local_listing.video != remote_listing.video {
                    diffs.push(json!({"locale": local_listing.locale, "field": "video", "local": local_listing.video, "remote": remote_listing.video}));
                }
            }
            None => diffs.push(json!({"locale": local_listing.locale, "field": "listing", "local": "present", "remote": "missing"})),
        }
    }
    for remote_listing in remote_by_locale.values() {
        diffs.push(json!({"locale": remote_listing.locale, "field": "listing", "local": "missing", "remote": "present"}));
    }
    Value::Array(diffs)
}

fn push_field_diff(diffs: &mut Vec<Value>, locale: &str, field: &str, local: &str, remote: &str) {
    if local != remote {
        diffs.push(json!({"locale": locale, "field": field, "local": local, "remote": remote}));
    }
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
        .or_else(|| provider_secret(DistributionProvider::AppStore, &[]).ok().flatten())
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
    if let Some(token) = env_value("PLAY_STORE_ACCESS_TOKEN") {
        return Ok(token);
    }
    let secret_source = env_value("PLAY_STORE_SERVICE_ACCOUNT_JSON")
        .or_else(|| env_value("GOOGLE_APPLICATION_CREDENTIALS"))
        .or_else(|| cfg.service_account.clone())
        .or_else(|| {
            provider_secret(DistributionProvider::PlayStore, &[])
                .ok()
                .flatten()
        });
    let Some(source) = secret_source else {
        bail!("Google Play credentials are missing; set PLAY_STORE_SERVICE_ACCOUNT_JSON, PLAY_STORE_ACCESS_TOKEN, GOOGLE_APPLICATION_CREDENTIALS, or import play-store credentials")
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

fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_user_comment_uses_newest_user_comment() {
        let value = json!({
            "comments": [
                {"userComment": {"text": "old", "starRating": 2}},
                {"developerComment": {"text": "reply"}},
                {"userComment": {"text": "new", "starRating": 4}}
            ]
        });
        let comment = latest_user_comment(&value).unwrap();
        assert_eq!(comment.get("text").and_then(Value::as_str), Some("new"));
    }

    #[test]
    fn google_group_csv_reader_deduplicates_group_emails() {
        let path =
            std::env::temp_dir().join(format!("fission-play-groups-{}.csv", std::process::id()));
        fs::write(
            &path,
            "email\nclosed-testers@example.com\nclosed-testers@example.com,other@example.com\n",
        )
        .unwrap();
        let groups = read_google_group_csv(&path).unwrap();
        assert_eq!(
            groups,
            vec![
                "closed-testers@example.com".to_string(),
                "other@example.com".to_string()
            ]
        );
    }

    #[test]
    fn resolved_play_listing_merges_root_listing_and_release_metadata() {
        let root: ReleaseProviderToml = toml::from_str(
            r#"
[release]
active_release = "1.0.0+1"
default_locales = ["en-US"]

[release.store_listing.play_store.en-US]
title = "Todo"
short_description = "Plan work"
video = "https://example.com/video"

[[releases]]
id = "1.0.0+1"
metadata = "release-content/metadata/1.0.0+1/release.toml"
"#,
        )
        .unwrap();
        let metadata: ReleaseMetadataToml = toml::from_str(
            r#"
[play_store.en-US]
full_description = "A focused task manager."
"#,
        )
        .unwrap();
        let listing = resolve_play_listing(&root, &metadata, "en-US").unwrap();
        assert_eq!(listing.title, "Todo");
        assert_eq!(listing.short_description, "Plan work");
        assert_eq!(listing.full_description, "A focused task manager.");
        assert_eq!(listing.video.as_deref(), Some("https://example.com/video"));
    }

    #[test]
    fn beta_play_store_tracks_parse_group_and_groups() {
        let root: ReleaseProviderToml = toml::from_str(
            r#"
[beta.play_store.tracks.closed]
tester_source = "google_group"
group = "closed@example.com"
groups = ["qa@example.com"]
"#,
        )
        .unwrap();
        let tracks = root.beta.unwrap().play_store.unwrap().tracks;
        let closed = tracks.get("closed").unwrap();
        assert_eq!(closed.group.as_deref(), Some("closed@example.com"));
        assert_eq!(closed.groups, vec!["qa@example.com".to_string()]);
    }

    #[test]
    fn app_store_review_response_payload_targets_review() {
        let payload = app_store_review_response_payload("review-123", "Thanks for the report.");
        assert_eq!(
            payload.pointer("/data/type").and_then(Value::as_str),
            Some("customerReviewResponses")
        );
        assert_eq!(
            payload
                .pointer("/data/attributes/responseBody")
                .and_then(Value::as_str),
            Some("Thanks for the report.")
        );
        assert_eq!(
            payload
                .pointer("/data/relationships/review/data/id")
                .and_then(Value::as_str),
            Some("review-123")
        );
    }

    #[test]
    fn app_store_beta_tester_payload_assigns_group() {
        let tester = AppStoreTester {
            email: "person@example.com".to_string(),
            first_name: Some("Test".to_string()),
            last_name: Some("User".to_string()),
        };
        let payload = app_store_beta_tester_payload(&tester, "group-123");
        assert_eq!(
            payload.pointer("/data/type").and_then(Value::as_str),
            Some("betaTesters")
        );
        assert_eq!(
            payload
                .pointer("/data/attributes/email")
                .and_then(Value::as_str),
            Some("person@example.com")
        );
        assert_eq!(
            payload
                .pointer("/data/relationships/betaGroups/data/0/id")
                .and_then(Value::as_str),
            Some("group-123")
        );
    }

    #[test]
    fn app_store_localization_payload_uses_version_level_fields() {
        let localization = AppStoreLocalization {
            id: None,
            locale: "en-US".to_string(),
            description: "A focused task manager.".to_string(),
            keywords: Some("todo,tasks".to_string()),
            marketing_url: Some("https://example.com".to_string()),
            promotional_text: Some("Better planning.".to_string()),
            support_url: Some("https://example.com/support".to_string()),
            whats_new: Some("New editor.".to_string()),
        };
        let payload = app_store_localization_create_payload("version-123", &localization);
        assert_eq!(
            payload.pointer("/data/type").and_then(Value::as_str),
            Some("appStoreVersionLocalizations")
        );
        assert_eq!(
            payload
                .pointer("/data/attributes/locale")
                .and_then(Value::as_str),
            Some("en-US")
        );
        assert_eq!(
            payload
                .pointer("/data/relationships/appStoreVersion/data/id")
                .and_then(Value::as_str),
            Some("version-123")
        );
    }
}
