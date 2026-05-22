use super::*;
use anyhow::{bail, Context, Result};
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;
use std::time::Duration;

const MICROSOFT_STORE_API: &str = "https://api.store.microsoft.com";
const MICROSOFT_STORE_SCOPE: &str = "https://api.store.microsoft.com/.default";

#[derive(Debug, Deserialize, Default)]
struct ReleaseProviderToml {
    distribution: Option<DistributionToml>,
    release: Option<ReleaseRootToml>,
    #[serde(default)]
    releases: Vec<ReleaseEntryToml>,
}

#[derive(Debug, Deserialize, Default)]
struct DistributionToml {
    microsoft_store: Option<MicrosoftStoreConfig>,
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
    privacy_url: Option<String>,
    support_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct ReleaseMetadataToml {
    #[serde(default)]
    microsoft_store: BTreeMap<String, MicrosoftReleaseMetadataToml>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct MicrosoftReleaseMetadataToml {
    description: Option<String>,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    search_terms: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct MicrosoftStoreConfig {
    product_id: Option<String>,
    tenant_id: Option<String>,
    client_id: Option<String>,
    seller_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct MicrosoftListing {
    language: String,
    title: Option<String>,
    short_description: Option<String>,
    description: String,
    keywords: Vec<String>,
    privacy_url: Option<String>,
    support_url: Option<String>,
    release_notes: Option<String>,
    features: Vec<String>,
    search_terms: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
}

pub(super) fn release_config_import(
    project_dir: &Path,
    locales: Option<&str>,
    yes: bool,
    json_output: bool,
) -> Result<()> {
    if !yes {
        bail!("release-config import mutates fission.toml/release metadata; pass --yes after reviewing the provider and locales");
    }
    let root = read_release_provider_toml(project_dir)?;
    let cfg = microsoft_store_config(project_dir)?;
    let product_id = product_id(&cfg)?;
    let seller_id = seller_id(&cfg)?;
    let client = http_client()?;
    let token = microsoft_store_access_token(&cfg, &client)?;
    let remote = fetch_microsoft_listings(&client, &token, &seller_id, product_id, locales)?;
    write_imported_microsoft_listings(project_dir, &root, &remote)?;
    let summary = json!({
        "provider": "microsoft-store",
        "product_id": product_id,
        "imported_locales": remote.iter().map(|item| item.language.as_str()).collect::<Vec<_>>(),
        "status": "imported"
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!("Imported {} Microsoft Store listing(s)", remote.len());
    }
    Ok(())
}

pub(super) fn release_config_diff(project_dir: &Path, json_output: bool) -> Result<()> {
    let root = read_release_provider_toml(project_dir)?;
    let cfg = microsoft_store_config(project_dir)?;
    let product_id = product_id(&cfg)?;
    let seller_id = seller_id(&cfg)?;
    let locales = resolve_release_locales(&root, None)?;
    let local = resolve_microsoft_listings(project_dir, &root, &locales)?;
    let client = http_client()?;
    let token = microsoft_store_access_token(&cfg, &client)?;
    let remote = fetch_microsoft_listings(&client, &token, &seller_id, product_id, None)?;
    let diff = microsoft_listing_diff(&local, &remote);
    if json_output {
        println!("{}", serde_json::to_string_pretty(&diff)?);
    } else if diff.as_array().is_some_and(Vec::is_empty) {
        println!(
            "Microsoft Store metadata is in sync for {} locale(s)",
            locales.len()
        );
    } else {
        println!("Microsoft Store metadata differences:");
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

pub(super) fn release_config_push(
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
    let cfg = microsoft_store_config(project_dir)?;
    let product_id = product_id(&cfg)?;
    let seller_id = seller_id(&cfg)?;
    let locales = resolve_release_locales(&root, locales_arg)?;
    let local = resolve_microsoft_listings(project_dir, &root, &locales)?;
    if dry_run {
        let value = json!({
            "provider": "microsoft-store",
            "product_id": product_id,
            "locales": locales,
            "listings": local,
            "status": "dry-run"
        });
        if json_output {
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!("Would push {} Microsoft Store listing(s)", local.len());
        }
        return Ok(());
    }

    let client = http_client()?;
    let token = microsoft_store_access_token(&cfg, &client)?;
    let remote = fetch_microsoft_raw_listings(&client, &token, &seller_id, product_id, None)?;
    let mut responses = Vec::new();
    for listing in &local {
        let existing = find_microsoft_listing_value(&remote, &listing.language);
        let payload = microsoft_listing_payload(existing, listing);
        let response = client
            .put(format!(
                "{MICROSOFT_STORE_API}/submission/v1/product/{product_id}/metadata"
            ))
            .bearer_auth(&token)
            .header("X-Seller-Account-Id", &seller_id)
            .json(&payload)
            .send()
            .with_context(|| {
                format!(
                    "failed to push Microsoft Store metadata for {}",
                    listing.language
                )
            })?;
        let value = json_response(response, "Microsoft Store metadata push")?;
        microsoft_store_success(&value, "Microsoft Store metadata push")?;
        responses.push(value);
    }
    let summary = json!({
        "provider": "microsoft-store",
        "product_id": product_id,
        "pushed_locales": local.iter().map(|item| item.language.as_str()).collect::<Vec<_>>(),
        "responses": responses,
        "status": "pushed"
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!("Pushed {} Microsoft Store listing(s)", local.len());
    }
    Ok(())
}

fn fetch_microsoft_listings(
    client: &Client,
    token: &str,
    seller_id: &str,
    product_id: &str,
    locales: Option<&str>,
) -> Result<Vec<MicrosoftListing>> {
    let value = fetch_microsoft_raw_listings(client, token, seller_id, product_id, locales)?;
    microsoft_listings_from_value(&value)
}

fn fetch_microsoft_raw_listings(
    client: &Client,
    token: &str,
    seller_id: &str,
    product_id: &str,
    locales: Option<&str>,
) -> Result<Value> {
    let mut url =
        format!("{MICROSOFT_STORE_API}/submission/v1/product/{product_id}/metadata/listings");
    if let Some(locales) = locales.filter(|value| !value.trim().is_empty()) {
        url.push_str("?languages=");
        url.push_str(&encode_query(locales));
    }
    let response = client
        .get(url)
        .bearer_auth(token)
        .header("X-Seller-Account-Id", seller_id)
        .send()
        .context("failed to fetch Microsoft Store listing metadata")?;
    json_response(response, "Microsoft Store metadata listing")
}

fn microsoft_listings_from_value(value: &Value) -> Result<Vec<MicrosoftListing>> {
    let listings = value
        .pointer("/responseData/listings")
        .or_else(|| value.get("listings"))
        .and_then(Value::as_array)
        .context("Microsoft Store metadata response did not contain responseData.listings")?;
    listings
        .iter()
        .map(microsoft_listing_from_value)
        .collect::<Result<Vec<_>>>()
}

fn microsoft_listing_from_value(value: &Value) -> Result<MicrosoftListing> {
    let language = field_string(value, &["language", "locale"])
        .context("Microsoft Store listing did not contain language")?;
    let keywords = field_array(value, &["keywords", "keywordTerms"]);
    let search_terms = field_array(value, &["searchTerms", "search_terms"]);
    let description = field_string(value, &["description"]).unwrap_or_default();
    Ok(MicrosoftListing {
        language,
        title: field_string(value, &["title", "name", "sortTitle"]),
        short_description: field_string(
            value,
            &["shortDescription", "short_description", "subtitle"],
        ),
        description,
        keywords,
        privacy_url: field_string(value, &["privacyUrl", "privacyPolicyUrl", "privacy_url"]),
        support_url: field_string(value, &["supportUrl", "support_url"]),
        release_notes: field_string(value, &["whatsNew", "releaseNotes", "release_notes"]),
        features: field_array(value, &["features", "productFeatures"]),
        search_terms,
    })
}

fn find_microsoft_listing_value<'a>(value: &'a Value, language: &str) -> Option<&'a Value> {
    value
        .pointer("/responseData/listings")
        .or_else(|| value.get("listings"))
        .and_then(Value::as_array)?
        .iter()
        .find(|item| {
            field_string(item, &["language", "locale"])
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(language))
        })
}

fn resolve_microsoft_listings(
    project_dir: &Path,
    root: &ReleaseProviderToml,
    locales: &[String],
) -> Result<Vec<MicrosoftListing>> {
    locales
        .iter()
        .map(|locale| resolve_microsoft_listing(project_dir, root, locale))
        .collect()
}

fn resolve_microsoft_listing(
    project_dir: &Path,
    root: &ReleaseProviderToml,
    locale: &str,
) -> Result<MicrosoftListing> {
    let release = active_release(root).context("release.active_release must point to a release")?;
    let metadata_path = release
        .metadata
        .as_deref()
        .context("active release metadata path is required for Microsoft Store metadata sync")?;
    let metadata = read_release_metadata(project_dir, metadata_path)?;
    let release_metadata = metadata
        .microsoft_store
        .get(locale)
        .or_else(|| metadata.microsoft_store.get(&locale.to_ascii_lowercase()))
        .with_context(|| {
            format!("active release metadata [microsoft_store.{locale}].description is required")
        })?;
    let listing = root
        .release
        .as_ref()
        .and_then(|release| release.store_listing.get("microsoft_store"))
        .and_then(|store| {
            store
                .get(locale)
                .or_else(|| store.get(&locale.to_ascii_lowercase()))
        })
        .cloned()
        .unwrap_or_default();
    let description = release_metadata
        .description
        .clone()
        .filter(|value| !value.trim().is_empty())
        .with_context(|| {
            format!("active release metadata [microsoft_store.{locale}].description is required")
        })?;
    let release_notes = release
        .release_notes
        .as_deref()
        .map(|notes_dir| project_dir.join(notes_dir).join(format!("{locale}.md")))
        .filter(|path| path.exists())
        .map(fs::read_to_string)
        .transpose()?;
    Ok(MicrosoftListing {
        language: locale.to_string(),
        title: listing.title.or(listing.name),
        short_description: listing.short_description.or(listing.subtitle),
        description,
        keywords: listing.keywords,
        privacy_url: listing.privacy_url,
        support_url: listing.support_url,
        release_notes,
        features: release_metadata.features.clone(),
        search_terms: release_metadata.search_terms.clone(),
    })
}

fn microsoft_listing_payload(existing: Option<&Value>, listing: &MicrosoftListing) -> Value {
    let mut listing_value = existing.cloned().unwrap_or_else(|| json!({}));
    set_json_field(
        &mut listing_value,
        "language",
        Some(listing.language.clone()),
    );
    set_json_field(
        &mut listing_value,
        "description",
        Some(listing.description.clone()),
    );
    set_json_field(
        &mut listing_value,
        "shortDescription",
        listing.short_description.clone(),
    );
    set_json_field(&mut listing_value, "sortTitle", listing.title.clone());
    set_json_field(
        &mut listing_value,
        "privacyPolicyUrl",
        listing.privacy_url.clone(),
    );
    set_json_field(
        &mut listing_value,
        "supportUrl",
        listing.support_url.clone(),
    );
    set_json_field(
        &mut listing_value,
        "whatsNew",
        listing.release_notes.clone(),
    );
    set_json_array(&mut listing_value, "keywords", &listing.keywords);
    set_json_array(&mut listing_value, "searchTerms", &listing.search_terms);
    set_json_array(&mut listing_value, "features", &listing.features);
    json!({ "listings": listing_value })
}

fn write_imported_microsoft_listings(
    project_dir: &Path,
    root: &ReleaseProviderToml,
    remote: &[MicrosoftListing],
) -> Result<()> {
    let release = active_release(root).context("release.active_release must point to a release")?;
    let metadata_path = release
        .metadata
        .as_deref()
        .context("active release metadata path is required for Microsoft Store metadata import")?;
    let toml_path = project_dir.join("fission.toml");
    let mut fission_doc = parse_toml_edit_document(&fs::read_to_string(&toml_path)?, &toml_path)?;
    for listing in remote {
        if let Some(title) = listing.title.clone() {
            set_toml_edit_path(
                &mut fission_doc,
                &format!(
                    "release.store_listing.microsoft_store.{}.title",
                    listing.language
                ),
                toml_edit::value(title),
            )?;
        }
        if let Some(short_description) = listing.short_description.clone() {
            set_toml_edit_path(
                &mut fission_doc,
                &format!(
                    "release.store_listing.microsoft_store.{}.short_description",
                    listing.language
                ),
                toml_edit::value(short_description),
            )?;
        }
        if let Some(privacy_url) = listing.privacy_url.clone() {
            set_toml_edit_path(
                &mut fission_doc,
                &format!(
                    "release.store_listing.microsoft_store.{}.privacy_url",
                    listing.language
                ),
                toml_edit::value(privacy_url),
            )?;
        }
    }
    write_toml_edit_document(&toml_path, &fission_doc)?;

    let metadata_abs = project_dir.join(metadata_path);
    let mut metadata_doc: toml::Value = if metadata_abs.exists() {
        toml::from_str(&fs::read_to_string(&metadata_abs)?)?
    } else {
        toml::Value::Table(Default::default())
    };
    for listing in remote {
        set_toml_path(
            &mut metadata_doc,
            &format!("microsoft_store.{}.description", listing.language),
            toml::Value::String(listing.description.clone()),
        )?;
        if !listing.features.is_empty() {
            set_toml_path(
                &mut metadata_doc,
                &format!("microsoft_store.{}.features", listing.language),
                toml::Value::Array(
                    listing
                        .features
                        .iter()
                        .cloned()
                        .map(toml::Value::String)
                        .collect(),
                ),
            )?;
        }
        if !listing.search_terms.is_empty() {
            set_toml_path(
                &mut metadata_doc,
                &format!("microsoft_store.{}.search_terms", listing.language),
                toml::Value::Array(
                    listing
                        .search_terms
                        .iter()
                        .cloned()
                        .map(toml::Value::String)
                        .collect(),
                ),
            )?;
        }
    }
    if let Some(parent) = metadata_abs.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(metadata_abs, toml::to_string_pretty(&metadata_doc)? + "\n")?;
    Ok(())
}

fn microsoft_listing_diff(local: &[MicrosoftListing], remote: &[MicrosoftListing]) -> Value {
    let mut diffs = Vec::new();
    for local_listing in local {
        let remote_listing = remote
            .iter()
            .find(|item| item.language.eq_ignore_ascii_case(&local_listing.language));
        let Some(remote_listing) = remote_listing else {
            diffs.push(json!({
                "locale": local_listing.language,
                "field": "listing",
                "local": "present",
                "remote": null
            }));
            continue;
        };
        push_option_diff(
            &mut diffs,
            &local_listing.language,
            "title",
            local_listing.title.as_deref(),
            remote_listing.title.as_deref(),
        );
        push_option_diff(
            &mut diffs,
            &local_listing.language,
            "short_description",
            local_listing.short_description.as_deref(),
            remote_listing.short_description.as_deref(),
        );
        push_option_diff(
            &mut diffs,
            &local_listing.language,
            "description",
            Some(local_listing.description.as_str()),
            Some(remote_listing.description.as_str()),
        );
        push_option_diff(
            &mut diffs,
            &local_listing.language,
            "privacy_url",
            local_listing.privacy_url.as_deref(),
            remote_listing.privacy_url.as_deref(),
        );
        push_option_diff(
            &mut diffs,
            &local_listing.language,
            "release_notes",
            local_listing.release_notes.as_deref(),
            remote_listing.release_notes.as_deref(),
        );
        if local_listing.keywords != remote_listing.keywords {
            diffs.push(json!({
                "locale": local_listing.language,
                "field": "keywords",
                "local": local_listing.keywords,
                "remote": remote_listing.keywords
            }));
        }
        if local_listing.features != remote_listing.features {
            diffs.push(json!({
                "locale": local_listing.language,
                "field": "features",
                "local": local_listing.features,
                "remote": remote_listing.features
            }));
        }
        if local_listing.search_terms != remote_listing.search_terms {
            diffs.push(json!({
                "locale": local_listing.language,
                "field": "search_terms",
                "local": local_listing.search_terms,
                "remote": remote_listing.search_terms
            }));
        }
    }
    Value::Array(diffs)
}

fn resolve_release_locales(
    root: &ReleaseProviderToml,
    explicit: Option<&str>,
) -> Result<Vec<String>> {
    if let Some(explicit) = explicit {
        return parse_locale_list(explicit);
    }
    let release = active_release(root).context("release.active_release must point to a release")?;
    let locales = if release.locales.is_empty() {
        root.release
            .as_ref()
            .map(|release| release.default_locales.clone())
            .unwrap_or_default()
    } else {
        release.locales.clone()
    };
    if locales.is_empty() {
        bail!("active release must declare locales or release.default_locales")
    }
    Ok(locales)
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

fn read_release_metadata(project_dir: &Path, relative: &str) -> Result<ReleaseMetadataToml> {
    let path = project_dir.join(relative);
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))
}

fn read_release_provider_toml(project_dir: &Path) -> Result<ReleaseProviderToml> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))
}

fn microsoft_store_config(project_dir: &Path) -> Result<MicrosoftStoreConfig> {
    Ok(read_release_provider_toml(project_dir)?
        .distribution
        .and_then(|distribution| distribution.microsoft_store)
        .unwrap_or_default())
}

fn product_id(cfg: &MicrosoftStoreConfig) -> Result<&str> {
    cfg.product_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .context("distribution.microsoft_store.product_id is required")
}

fn seller_id(cfg: &MicrosoftStoreConfig) -> Result<String> {
    env_value("MICROSOFT_STORE_SELLER_ID")
        .or(cfg.seller_id.clone())
        .context("distribution.microsoft_store.seller_id or MICROSOFT_STORE_SELLER_ID is required")
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
            provider_secret(publish::DistributionProvider::MicrosoftStore, &[])
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

fn field_string(value: &Value, fields: &[&str]) -> Option<String> {
    fields.iter().find_map(|field| {
        value
            .get(*field)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn field_array(value: &Value, fields: &[&str]) -> Vec<String> {
    fields
        .iter()
        .find_map(|field| value.get(*field).and_then(Value::as_array))
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn set_json_field(value: &mut Value, field: &str, data: Option<String>) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    if let Some(data) = data.filter(|value| !value.trim().is_empty()) {
        object.insert(field.to_string(), Value::String(data));
    } else {
        object.remove(field);
    }
}

fn set_json_array(value: &mut Value, field: &str, data: &[String]) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    if data.is_empty() {
        object.remove(field);
    } else {
        object.insert(
            field.to_string(),
            Value::Array(data.iter().cloned().map(Value::String).collect()),
        );
    }
}

fn push_option_diff(
    diffs: &mut Vec<Value>,
    locale: &str,
    field: &str,
    local: Option<&str>,
    remote: Option<&str>,
) {
    if local != remote {
        diffs.push(json!({
            "locale": locale,
            "field": field,
            "local": local,
            "remote": remote
        }));
    }
}

fn encode_query(value: &str) -> String {
    let mut out = String::new();
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b',' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microsoft_listing_payload_updates_existing_listing() {
        let existing = json!({"language":"en-us","description":"old","keep":"yes"});
        let payload = microsoft_listing_payload(
            Some(&existing),
            &MicrosoftListing {
                language: "en-us".to_string(),
                title: Some("Todo".to_string()),
                short_description: Some("Plan work".to_string()),
                description: "A production task app.".to_string(),
                keywords: vec!["todo".to_string(), "tasks".to_string()],
                privacy_url: Some("https://example.com/privacy".to_string()),
                support_url: None,
                release_notes: Some("First release".to_string()),
                features: vec!["Fast lists".to_string()],
                search_terms: vec!["productivity".to_string()],
            },
        );
        assert_eq!(payload["listings"]["language"], "en-us");
        assert_eq!(payload["listings"]["description"], "A production task app.");
        assert_eq!(payload["listings"]["sortTitle"], "Todo");
        assert_eq!(payload["listings"]["keep"], "yes");
        assert_eq!(payload["listings"]["keywords"][0], "todo");
    }

    #[test]
    fn microsoft_listing_parser_accepts_response_data_shape() {
        let value = json!({
            "responseData": {
                "listings": [{
                    "language": "en-us",
                    "description": "A production app.",
                    "shortDescription": "Short",
                    "sortTitle": "Todo",
                    "privacyPolicyUrl": "https://example.com/privacy",
                    "whatsNew": "Changed",
                    "features": ["Fast"],
                    "searchTerms": ["tasks"]
                }]
            }
        });
        let listings = microsoft_listings_from_value(&value).unwrap();
        assert_eq!(listings[0].language, "en-us");
        assert_eq!(listings[0].title.as_deref(), Some("Todo"));
        assert_eq!(listings[0].features, vec!["Fast"]);
        assert_eq!(listings[0].search_terms, vec!["tasks"]);
    }
}
