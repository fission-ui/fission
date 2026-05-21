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

#[derive(Debug, Deserialize, Default)]
struct ReleaseProviderToml {
    distribution: Option<DistributionToml>,
    release: Option<ReleaseRootToml>,
    #[serde(default)]
    releases: Vec<ReleaseEntryToml>,
}

#[derive(Debug, Deserialize, Default)]
struct DistributionToml {
    play_store: Option<PlayStoreConfig>,
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
    #[serde(default)]
    locales: Vec<String>,
    metadata: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default, Serialize, PartialEq, Eq)]
struct StoreListingToml {
    title: Option<String>,
    name: Option<String>,
    short_description: Option<String>,
    subtitle: Option<String>,
    video: Option<String>,
    video_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct ReleaseMetadataToml {
    #[serde(default)]
    play_store: BTreeMap<String, PlayReleaseMetadataToml>,
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

#[derive(Clone, Debug, Deserialize, Default)]
struct PlayStoreConfig {
    package_name: Option<String>,
    service_account: Option<String>,
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

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
}

pub(super) fn reviews_list(
    provider: publish::DistributionProvider,
    since: Option<String>,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    match provider {
        publish::DistributionProvider::PlayStore => {
            play_reviews_list(project_dir, since, json_output)
        }
        _ => unsupported_reviews(provider, "list"),
    }
}

pub(super) fn reviews_reply(
    provider: publish::DistributionProvider,
    review: &str,
    message_file: &Path,
    project_dir: &Path,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    match provider {
        publish::DistributionProvider::PlayStore => {
            play_reviews_reply(project_dir, review, message_file, dry_run, json_output)
        }
        _ => unsupported_reviews(provider, "reply"),
    }
}

pub(super) fn beta_groups_list(
    provider: publish::DistributionProvider,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    match provider {
        publish::DistributionProvider::PlayStore => play_beta_groups_list(project_dir, json_output),
        _ => unsupported_beta(provider, "groups list"),
    }
}

pub(super) fn beta_testers_import(
    provider: publish::DistributionProvider,
    track: Option<&str>,
    csv: &Path,
    project_dir: &Path,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    match provider {
        publish::DistributionProvider::PlayStore => {
            play_beta_testers_import(project_dir, track, csv, dry_run, json_output)
        }
        _ => unsupported_beta(provider, "testers import"),
    }
}

pub(super) fn beta_testers_export(
    provider: publish::DistributionProvider,
    track: Option<&str>,
    output: &Path,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    match provider {
        publish::DistributionProvider::PlayStore => {
            play_beta_testers_export(project_dir, track, output, json_output)
        }
        _ => unsupported_beta(provider, "testers export"),
    }
}

pub(super) fn release_config_import(
    provider: publish::DistributionProvider,
    locales: Option<String>,
    yes: bool,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    match provider {
        publish::DistributionProvider::PlayStore => {
            play_release_config_import(project_dir, locales.as_deref(), yes, json_output)
        }
        _ => unsupported_release_config(provider, "import"),
    }
}

pub(super) fn release_config_diff(
    provider: publish::DistributionProvider,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    match provider {
        publish::DistributionProvider::PlayStore => {
            play_release_config_diff(project_dir, json_output)
        }
        _ => unsupported_release_config(provider, "diff"),
    }
}

pub(super) fn release_config_push(
    provider: publish::DistributionProvider,
    locales: Option<String>,
    dry_run: bool,
    yes: bool,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    match provider {
        publish::DistributionProvider::PlayStore => {
            play_release_config_push(project_dir, locales.as_deref(), dry_run, yes, json_output)
        }
        _ => unsupported_release_config(provider, "push"),
    }
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
    let mut doc: toml::Value = toml::from_str(&data)
        .with_context(|| format!("failed to parse {}", fission_path.display()))?;
    for listing in listings {
        set_toml_path(
            &mut doc,
            &format!("release.store_listing.play_store.{}.title", listing.locale),
            toml::Value::String(listing.title.clone()),
        )?;
        set_toml_path(
            &mut doc,
            &format!(
                "release.store_listing.play_store.{}.short_description",
                listing.locale
            ),
            toml::Value::String(listing.short_description.clone()),
        )?;
        if let Some(video) = &listing.video {
            set_toml_path(
                &mut doc,
                &format!("release.store_listing.play_store.{}.video", listing.locale),
                toml::Value::String(video.clone()),
            )?;
        }
    }
    fs::write(&fission_path, toml::to_string_pretty(&doc)? + "\n")
        .with_context(|| format!("failed to write {}", fission_path.display()))?;

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
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
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

fn unsupported_release_config(provider: publish::DistributionProvider, action: &str) -> Result<()> {
    bail!(
        "{} release-config {} is not exposed by the current provider API backend; Google Play listing import/diff/push is implemented",
        provider.as_str(),
        action
    )
}

fn unsupported_reviews(provider: publish::DistributionProvider, action: &str) -> Result<()> {
    bail!(
        "{} review {} is not exposed by the current provider API backend; Google Play review list/reply is implemented",
        provider.as_str(),
        action
    )
}

fn unsupported_beta(provider: publish::DistributionProvider, action: &str) -> Result<()> {
    bail!(
        "{} beta {} is not exposed by the current provider API backend; Google Play Google Group tester management is implemented",
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
            provider_secret(publish::DistributionProvider::PlayStore, &[])
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
        .user_agent("fission-cli-release/0.1")
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
}
