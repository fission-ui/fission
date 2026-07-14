use super::*;
use md5::{Digest, Md5};
use reqwest::blocking::{Body, Client};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const MICROSOFT_STORE_API: &str = "https://api.store.microsoft.com";
const MICROSOFT_STORE_SCOPE: &str = "https://api.store.microsoft.com/.default";

#[derive(Debug, Deserialize, Default)]
struct ContentToml {
    release: Option<ContentReleaseRoot>,
}

#[derive(Debug, Deserialize, Default)]
struct ContentReleaseRoot {
    assets: Option<ProviderAssets>,
}

#[derive(Debug, Deserialize, Default)]
struct ProviderAssets {
    app_store: Option<AppStoreAssets>,
    microsoft_store: Option<MicrosoftStoreAssets>,
}

#[derive(Debug, Deserialize, Default)]
struct ProviderDistribution {
    microsoft_store: Option<MicrosoftStoreContentConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct MicrosoftStoreContentConfig {
    product_id: Option<String>,
    tenant_id: Option<String>,
    client_id: Option<String>,
    seller_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct MicrosoftContentToml {
    distribution: Option<ProviderDistribution>,
}

#[derive(Debug, Deserialize, Default)]
struct AppStoreAssets {
    screenshot_sets_dir: Option<String>,
    app_previews_dir: Option<String>,
    #[serde(default)]
    review_attachments: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct MicrosoftStoreAssets {
    screenshot_sets_dir: Option<String>,
    trailers_dir: Option<String>,
    logo_dir: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RenderManifestFile {
    #[serde(default)]
    assets: Vec<RenderedAssetFile>,
}

#[derive(Debug, Deserialize, Default)]
struct RenderedAssetFile {
    kind: String,
    output: String,
    sha256: Option<String>,
    size_bytes: Option<u64>,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ProviderReleaseContentAsset {
    provider: String,
    role: String,
    locale: Option<String>,
    path: String,
    mime_type: String,
    sha256: Option<String>,
    size_bytes: Option<u64>,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AppStoreAssetReservation {
    data: AppStoreAssetReservationData,
}

#[derive(Debug, Deserialize)]
struct AppStoreAssetReservationData {
    id: String,
    attributes: Option<AppStoreAssetReservationAttributes>,
}

#[derive(Debug, Deserialize)]
struct AppStoreAssetReservationAttributes {
    #[serde(default, rename = "uploadOperations")]
    upload_operations: Vec<AppStoreUploadOperation>,
}

#[derive(Debug, Deserialize)]
struct AppStoreUploadOperation {
    method: String,
    url: String,
    offset: u64,
    length: u64,
    #[serde(default, rename = "requestHeaders")]
    request_headers: Vec<AppStoreUploadHeader>,
}

#[derive(Debug, Deserialize)]
struct AppStoreUploadHeader {
    name: String,
    value: String,
}

#[derive(Clone, Debug, Deserialize)]
struct AppStoreScreenshotSet {
    id: String,
    #[serde(rename = "screenshotDisplayType")]
    screenshot_display_type: String,
}

#[derive(Clone, Debug, Deserialize)]
struct AppStorePreviewSet {
    id: String,
    #[serde(rename = "previewType")]
    preview_type: String,
}

#[derive(Debug, Deserialize)]
struct MicrosoftOAuthTokenResponse {
    access_token: String,
}

pub(super) fn app_store_release_content_push(
    project_dir: &Path,
    locales_arg: Option<&str>,
    dry_run: bool,
    yes: bool,
) -> Result<Value> {
    provider_release_content_push(
        project_dir,
        DistributionProvider::AppStore,
        locales_arg,
        dry_run,
        yes,
    )
}

pub(super) fn microsoft_store_release_content_push(
    project_dir: &Path,
    locales_arg: Option<&str>,
    dry_run: bool,
    yes: bool,
) -> Result<Value> {
    provider_release_content_push(
        project_dir,
        DistributionProvider::MicrosoftStore,
        locales_arg,
        dry_run,
        yes,
    )
}

fn provider_release_content_push(
    project_dir: &Path,
    provider: DistributionProvider,
    locales_arg: Option<&str>,
    dry_run: bool,
    yes: bool,
) -> Result<Value> {
    if !dry_run && !yes {
        bail!("release-content push prepares provider assets; pass --yes after reviewing `release-content validate`");
    }
    let root = read_release_provider_toml(project_dir)?;
    let locales = resolve_release_locales(&root, locales_arg)?;
    let assets = collect_provider_release_content_assets(project_dir, provider, &locales)?;
    if assets.is_empty() {
        return Ok(json!({
            "provider": provider.as_str(),
            "locales": locales,
            "assets": [],
            "status": "skipped",
            "reason": "no rendered or configured release-content assets were found"
        }));
    }
    if provider == DistributionProvider::AppStore {
        return app_store_provider_release_content_push(
            project_dir,
            &root,
            &locales,
            assets,
            dry_run,
        );
    }
    if provider == DistributionProvider::MicrosoftStore {
        return microsoft_store_provider_release_content_push(
            project_dir,
            &locales,
            assets,
            dry_run,
        );
    }
    if dry_run {
        return Ok(json!({
            "provider": provider.as_str(),
            "locales": locales,
            "assets": assets,
            "status": "dry-run",
            "mode": "handoff-staging"
        }));
    }
    let handoff = stage_provider_asset_handoff(project_dir, provider, &assets)?;
    Ok(json!({
        "provider": provider.as_str(),
        "locales": locales,
        "assets": assets,
        "handoff_manifest": handoff.display().to_string(),
        "status": "staged",
        "mode": "handoff-staging",
        "manual_follow_up": provider_handoff_follow_up(provider)
    }))
}

fn app_store_provider_release_content_push(
    project_dir: &Path,
    root: &ReleaseProviderToml,
    locales: &[String],
    assets: Vec<ProviderReleaseContentAsset>,
    dry_run: bool,
) -> Result<Value> {
    let mut screenshots = Vec::new();
    let mut previews = Vec::new();
    let mut review_attachments = Vec::new();
    let mut handoff = Vec::new();
    for asset in assets {
        match asset.role.as_str() {
            "screenshot" if asset.mime_type.starts_with("image/") => screenshots.push(asset),
            "app-preview" if asset.mime_type.starts_with("video/") => previews.push(asset),
            "review-attachment" => review_attachments.push(asset),
            _ => handoff.push(asset),
        }
    }
    let mut upload_plan = Vec::new();
    for asset in &screenshots {
        upload_plan.push(app_store_screenshot_plan(asset, locales)?);
    }
    for asset in &previews {
        upload_plan.push(app_store_preview_plan(asset, locales)?);
    }
    for asset in &review_attachments {
        upload_plan.push(json!({
            "kind": "review-attachment",
            "path": asset.path,
            "mime_type": asset.mime_type,
            "sha256": asset.sha256,
            "size_bytes": asset.size_bytes,
        }));
    }
    if dry_run {
        return Ok(json!({
            "provider": "app-store",
            "locales": locales,
            "assets": upload_plan,
            "handoff_assets": handoff,
            "status": "dry-run",
            "mode": if handoff.is_empty() { "direct-upload" } else { "direct-upload-with-handoff" },
        }));
    }

    let mut uploaded = upload_app_store_screenshots(project_dir, root, locales, &screenshots)?;
    uploaded.extend(upload_app_store_previews(
        project_dir,
        root,
        locales,
        &previews,
    )?);
    uploaded.extend(upload_app_store_review_attachments(
        project_dir,
        root,
        &review_attachments,
    )?);
    let handoff_manifest = if handoff.is_empty() {
        None
    } else {
        Some(stage_provider_asset_handoff(
            project_dir,
            DistributionProvider::AppStore,
            &handoff,
        )?)
    };
    let has_handoff = handoff_manifest.is_some();
    Ok(json!({
        "provider": "app-store",
        "locales": locales,
        "assets": uploaded,
        "handoff_assets": handoff,
        "handoff_manifest": handoff_manifest.map(|path| path.display().to_string()),
        "status": if has_handoff { "partial" } else { "pushed" },
        "mode": if has_handoff { "direct-upload-with-handoff" } else { "direct-upload" },
        "manual_follow_up": if has_handoff {
            vec![
                "App Store screenshots, app previews, and review attachments were uploaded directly where configured; upload any staged unsupported assets from the handoff manifest.".to_string(),
            ]
        } else {
            Vec::<String>::new()
        },
    }))
}

fn app_store_screenshot_plan(
    asset: &ProviderReleaseContentAsset,
    locales: &[String],
) -> Result<Value> {
    let display_type = app_store_screenshot_display_type(Path::new(&asset.path))?;
    Ok(json!({
        "kind": "screenshot",
        "locale": asset.locale.clone().or_else(|| locales.first().cloned()),
        "display_type": display_type,
        "path": asset.path,
        "mime_type": asset.mime_type,
        "sha256": asset.sha256,
        "size_bytes": asset.size_bytes,
    }))
}

fn app_store_preview_plan(
    asset: &ProviderReleaseContentAsset,
    locales: &[String],
) -> Result<Value> {
    let preview_type = app_store_preview_type(Path::new(&asset.path))?;
    Ok(json!({
        "kind": "app-preview",
        "locale": asset.locale.clone().or_else(|| locales.first().cloned()),
        "preview_type": preview_type,
        "path": asset.path,
        "mime_type": asset.mime_type,
        "sha256": asset.sha256,
        "size_bytes": asset.size_bytes,
    }))
}

fn upload_app_store_screenshots(
    project_dir: &Path,
    root: &ReleaseProviderToml,
    release_locales: &[String],
    screenshots: &[ProviderReleaseContentAsset],
) -> Result<Vec<Value>> {
    if screenshots.is_empty() {
        return Ok(Vec::new());
    }
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let version_id = app_store_version_id(root, &client, &token, &app_id)?;
    let localizations = fetch_app_store_version_localizations(&client, &token, &version_id)?;
    let mut localization_ids = BTreeMap::new();
    for localization in localizations {
        if let Some(id) = localization.id {
            localization_ids.insert(localization.locale, id);
        }
    }
    let mut screenshot_sets: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut uploaded = Vec::new();
    for asset in screenshots {
        let locale = asset
            .locale
            .clone()
            .or_else(|| release_locales.first().cloned())
            .context("App Store screenshot asset did not resolve a locale")?;
        let localization_id = localization_ids
            .get(&locale)
            .or_else(|| localization_ids.get(&locale.to_ascii_lowercase()))
            .with_context(|| {
                format!(
                    "App Store localization {locale} is missing; run `fission release-config push --provider app-store --yes` first"
                )
            })?;
        let display_type = app_store_screenshot_display_type(Path::new(&asset.path))?;
        let set_key = (localization_id.clone(), display_type.clone());
        let screenshot_set_id = if let Some(id) = screenshot_sets.get(&set_key) {
            id.clone()
        } else {
            let id =
                replace_app_store_screenshot_set(&client, &token, localization_id, &display_type)?;
            screenshot_sets.insert(set_key, id.clone());
            id
        };
        let screenshot_id =
            upload_app_store_screenshot(&client, &token, &screenshot_set_id, asset)?;
        uploaded.push(json!({
            "locale": locale,
            "display_type": display_type,
            "screenshot_set_id": screenshot_set_id,
            "screenshot_id": screenshot_id,
            "path": asset.path,
            "mime_type": asset.mime_type,
            "sha256": asset.sha256,
            "size_bytes": asset.size_bytes,
        }));
    }
    Ok(uploaded)
}

fn replace_app_store_screenshot_set(
    client: &Client,
    token: &str,
    localization_id: &str,
    display_type: &str,
) -> Result<String> {
    let existing = fetch_app_store_screenshot_sets(client, token, localization_id)?;
    for set in existing
        .iter()
        .filter(|set| set.screenshot_display_type == display_type)
    {
        delete_app_store_screenshot_set(client, token, &set.id)?;
    }
    create_app_store_screenshot_set(client, token, localization_id, display_type)
}

fn fetch_app_store_screenshot_sets(
    client: &Client,
    token: &str,
    localization_id: &str,
) -> Result<Vec<AppStoreScreenshotSet>> {
    let url = format!(
        "{APP_STORE_API}/v1/appStoreVersionLocalizations/{localization_id}/appScreenshotSets?limit=200"
    );
    let value = json_response(
        client
            .get(url)
            .bearer_auth(token)
            .send()
            .context("failed to fetch App Store screenshot sets")?,
        "App Store screenshot sets list",
    )?;
    Ok(value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?.to_string();
            let display_type = item
                .pointer("/attributes/screenshotDisplayType")?
                .as_str()?
                .to_string();
            Some(AppStoreScreenshotSet {
                id,
                screenshot_display_type: display_type,
            })
        })
        .collect())
}

fn delete_app_store_screenshot_set(client: &Client, token: &str, set_id: &str) -> Result<()> {
    let response = client
        .delete(format!("{APP_STORE_API}/v1/appScreenshotSets/{set_id}"))
        .bearer_auth(token)
        .send()
        .context("failed to delete existing App Store screenshot set")?;
    let status = response.status();
    let text = response
        .text()
        .context("failed to read App Store screenshot set delete response")?;
    if !status.is_success() {
        bail!("App Store screenshot set delete failed with {status}: {text}");
    }
    Ok(())
}

fn create_app_store_screenshot_set(
    client: &Client,
    token: &str,
    localization_id: &str,
    display_type: &str,
) -> Result<String> {
    let payload = json!({
        "data": {
            "type": "appScreenshotSets",
            "attributes": {
                "screenshotDisplayType": display_type
            },
            "relationships": {
                "appStoreVersionLocalization": {
                    "data": {
                        "type": "appStoreVersionLocalizations",
                        "id": localization_id
                    }
                }
            }
        }
    });
    let value = json_response(
        client
            .post(format!("{APP_STORE_API}/v1/appScreenshotSets"))
            .bearer_auth(token)
            .json(&payload)
            .send()
            .context("failed to create App Store screenshot set")?,
        "App Store screenshot set create",
    )?;
    value
        .pointer("/data/id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .context("App Store screenshot set create response did not contain data.id")
}

fn upload_app_store_screenshot(
    client: &Client,
    token: &str,
    screenshot_set_id: &str,
    asset: &ProviderReleaseContentAsset,
) -> Result<String> {
    let path = Path::new(&asset.path);
    let file_size = file_len(path)?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .context("App Store screenshot path does not have a file name")?;
    let reservation =
        reserve_app_store_screenshot(client, token, screenshot_set_id, file_name, file_size)?;
    upload_reserved_app_store_asset(
        client,
        path,
        file_size,
        &reservation,
        "App Store screenshot",
    )?;
    commit_app_store_screenshot(client, token, &reservation.data.id, &md5_path(path)?)?;
    Ok(reservation.data.id)
}

fn reserve_app_store_screenshot(
    client: &Client,
    token: &str,
    screenshot_set_id: &str,
    file_name: &str,
    file_size: u64,
) -> Result<AppStoreAssetReservation> {
    let payload = json!({
        "data": {
            "type": "appScreenshots",
            "attributes": {
                "fileName": file_name,
                "fileSize": file_size
            },
            "relationships": {
                "appScreenshotSet": {
                    "data": {
                        "type": "appScreenshotSets",
                        "id": screenshot_set_id
                    }
                }
            }
        }
    });
    let response = client
        .post(format!("{APP_STORE_API}/v1/appScreenshots"))
        .bearer_auth(token)
        .json(&payload)
        .send()
        .context("failed to reserve App Store screenshot upload")?;
    let value = json_response(response, "App Store screenshot reserve")?;
    serde_json::from_value(value).context("failed to parse App Store screenshot reservation")
}

fn upload_app_store_asset_part(
    client: &Client,
    path: &Path,
    file_size: u64,
    operation: &AppStoreUploadOperation,
    label: &str,
) -> Result<()> {
    let method = Method::from_bytes(operation.method.as_bytes())
        .with_context(|| format!("unsupported App Store upload method {}", operation.method))?;
    let end = operation
        .offset
        .checked_add(operation.length)
        .context("upload operation range overflow")?;
    if end > file_size {
        bail!(
            "App Store upload operation range {}..{} exceeds asset size {}",
            operation.offset,
            end,
            file_size
        );
    }
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    file.seek(SeekFrom::Start(operation.offset))
        .with_context(|| format!("failed to seek {}", path.display()))?;
    let mut headers = HeaderMap::new();
    for header in &operation.request_headers {
        let name = HeaderName::from_bytes(header.name.as_bytes())
            .with_context(|| format!("invalid App Store upload header {}", header.name))?;
        let value = HeaderValue::from_str(&header.value).with_context(|| {
            format!("invalid App Store upload header value for {}", header.name)
        })?;
        headers.insert(name, value);
    }
    let response = client
        .request(method, &operation.url)
        .headers(headers)
        .body(Body::sized(file.take(operation.length), operation.length))
        .send()
        .with_context(|| format!("failed to upload {label} part"))?;
    let status = response.status();
    let text = response
        .text()
        .context("failed to read App Store screenshot upload response")?;
    if !status.is_success() {
        bail!("{label} part upload failed with {status}: {text}");
    }
    Ok(())
}

fn commit_app_store_screenshot(
    client: &Client,
    token: &str,
    screenshot_id: &str,
    md5: &str,
) -> Result<()> {
    let payload = json!({
        "data": {
            "type": "appScreenshots",
            "id": screenshot_id,
            "attributes": {
                "uploaded": true,
                "sourceFileChecksum": md5
            }
        }
    });
    json_response(
        client
            .patch(format!("{APP_STORE_API}/v1/appScreenshots/{screenshot_id}"))
            .bearer_auth(token)
            .header(CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .context("failed to commit App Store screenshot upload")?,
        "App Store screenshot commit",
    )?;
    Ok(())
}

fn upload_app_store_previews(
    project_dir: &Path,
    root: &ReleaseProviderToml,
    release_locales: &[String],
    previews: &[ProviderReleaseContentAsset],
) -> Result<Vec<Value>> {
    if previews.is_empty() {
        return Ok(Vec::new());
    }
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let version_id = app_store_version_id(root, &client, &token, &app_id)?;
    let localization_ids = app_store_localization_ids(&client, &token, &version_id)?;
    let mut preview_sets: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut uploaded = Vec::new();
    for asset in previews {
        let locale = asset
            .locale
            .clone()
            .or_else(|| release_locales.first().cloned())
            .context("App Store preview asset did not resolve a locale")?;
        let localization_id = app_store_localization_id(&localization_ids, &locale)?;
        let preview_type = app_store_preview_type(Path::new(&asset.path))?;
        let set_key = (localization_id.clone(), preview_type.clone());
        let preview_set_id = if let Some(id) = preview_sets.get(&set_key) {
            id.clone()
        } else {
            let id =
                replace_app_store_preview_set(&client, &token, &localization_id, &preview_type)?;
            preview_sets.insert(set_key, id.clone());
            id
        };
        let preview_id = upload_app_store_preview(&client, &token, &preview_set_id, asset)?;
        uploaded.push(json!({
            "kind": "app-preview",
            "locale": locale,
            "preview_type": preview_type,
            "preview_set_id": preview_set_id,
            "preview_id": preview_id,
            "path": asset.path,
            "mime_type": asset.mime_type,
            "sha256": asset.sha256,
            "size_bytes": asset.size_bytes,
        }));
    }
    Ok(uploaded)
}

fn upload_app_store_review_attachments(
    project_dir: &Path,
    root: &ReleaseProviderToml,
    attachments: &[ProviderReleaseContentAsset],
) -> Result<Vec<Value>> {
    if attachments.is_empty() {
        return Ok(Vec::new());
    }
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let version_id = app_store_version_id(root, &client, &token, &app_id)?;
    let review_detail_id = app_store_review_detail_id(&client, &token, &version_id)?;
    let mut uploaded = Vec::new();
    for asset in attachments {
        let attachment_id =
            upload_app_store_review_attachment(&client, &token, &review_detail_id, asset)?;
        uploaded.push(json!({
            "kind": "review-attachment",
            "review_detail_id": review_detail_id,
            "attachment_id": attachment_id,
            "path": asset.path,
            "mime_type": asset.mime_type,
            "sha256": asset.sha256,
            "size_bytes": asset.size_bytes,
        }));
    }
    Ok(uploaded)
}

fn app_store_localization_ids(
    client: &Client,
    token: &str,
    version_id: &str,
) -> Result<BTreeMap<String, String>> {
    let localizations = fetch_app_store_version_localizations(client, token, version_id)?;
    let mut ids = BTreeMap::new();
    for localization in localizations {
        if let Some(id) = localization.id {
            ids.insert(localization.locale, id);
        }
    }
    Ok(ids)
}

fn app_store_localization_id(ids: &BTreeMap<String, String>, locale: &str) -> Result<String> {
    ids.get(locale)
        .or_else(|| ids.get(&locale.to_ascii_lowercase()))
        .cloned()
        .with_context(|| {
            format!(
                "App Store localization {locale} is missing; run `fission release-config push --provider app-store --yes` first"
            )
        })
}

fn replace_app_store_preview_set(
    client: &Client,
    token: &str,
    localization_id: &str,
    preview_type: &str,
) -> Result<String> {
    let existing = fetch_app_store_preview_sets(client, token, localization_id)?;
    for set in existing
        .iter()
        .filter(|set| set.preview_type == preview_type)
    {
        delete_app_store_preview_set(client, token, &set.id)?;
    }
    create_app_store_preview_set(client, token, localization_id, preview_type)
}

fn fetch_app_store_preview_sets(
    client: &Client,
    token: &str,
    localization_id: &str,
) -> Result<Vec<AppStorePreviewSet>> {
    let url = format!(
        "{APP_STORE_API}/v1/appStoreVersionLocalizations/{localization_id}/appPreviewSets?limit=200"
    );
    let value = json_response(
        client
            .get(url)
            .bearer_auth(token)
            .send()
            .context("failed to fetch App Store preview sets")?,
        "App Store preview sets list",
    )?;
    Ok(value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?.to_string();
            let preview_type = item
                .pointer("/attributes/previewType")?
                .as_str()?
                .to_string();
            Some(AppStorePreviewSet { id, preview_type })
        })
        .collect())
}

fn delete_app_store_preview_set(client: &Client, token: &str, set_id: &str) -> Result<()> {
    let response = client
        .delete(format!("{APP_STORE_API}/v1/appPreviewSets/{set_id}"))
        .bearer_auth(token)
        .send()
        .context("failed to delete existing App Store preview set")?;
    let status = response.status();
    let text = response
        .text()
        .context("failed to read App Store preview set delete response")?;
    if !status.is_success() {
        bail!("App Store preview set delete failed with {status}: {text}");
    }
    Ok(())
}

fn create_app_store_preview_set(
    client: &Client,
    token: &str,
    localization_id: &str,
    preview_type: &str,
) -> Result<String> {
    let payload = json!({
        "data": {
            "type": "appPreviewSets",
            "attributes": {
                "previewType": preview_type
            },
            "relationships": {
                "appStoreVersionLocalization": {
                    "data": {
                        "type": "appStoreVersionLocalizations",
                        "id": localization_id
                    }
                }
            }
        }
    });
    let value = json_response(
        client
            .post(format!("{APP_STORE_API}/v1/appPreviewSets"))
            .bearer_auth(token)
            .json(&payload)
            .send()
            .context("failed to create App Store preview set")?,
        "App Store preview set create",
    )?;
    value
        .pointer("/data/id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .context("App Store preview set create response did not contain data.id")
}

fn upload_app_store_preview(
    client: &Client,
    token: &str,
    preview_set_id: &str,
    asset: &ProviderReleaseContentAsset,
) -> Result<String> {
    let path = Path::new(&asset.path);
    let file_size = file_len(path)?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .context("App Store preview path does not have a file name")?;
    let reservation =
        reserve_app_store_preview(client, token, preview_set_id, file_name, file_size)?;
    upload_reserved_app_store_asset(client, path, file_size, &reservation, "App Store preview")?;
    commit_app_store_asset(
        client,
        token,
        "appPreviews",
        &reservation.data.id,
        &md5_path(path)?,
    )?;
    Ok(reservation.data.id)
}

fn reserve_app_store_preview(
    client: &Client,
    token: &str,
    preview_set_id: &str,
    file_name: &str,
    file_size: u64,
) -> Result<AppStoreAssetReservation> {
    let payload = json!({
        "data": {
            "type": "appPreviews",
            "attributes": {
                "fileName": file_name,
                "fileSize": file_size
            },
            "relationships": {
                "appPreviewSet": {
                    "data": {
                        "type": "appPreviewSets",
                        "id": preview_set_id
                    }
                }
            }
        }
    });
    let response = client
        .post(format!("{APP_STORE_API}/v1/appPreviews"))
        .bearer_auth(token)
        .json(&payload)
        .send()
        .context("failed to reserve App Store preview upload")?;
    let value = json_response(response, "App Store preview reserve")?;
    serde_json::from_value(value).context("failed to parse App Store preview reservation")
}

fn app_store_review_detail_id(client: &Client, token: &str, version_id: &str) -> Result<String> {
    let value = json_response(
        client
            .get(format!(
                "{APP_STORE_API}/v1/appStoreVersions/{version_id}/relationships/appStoreReviewDetail"
            ))
            .bearer_auth(token)
            .send()
            .context("failed to fetch App Store review detail relationship")?,
        "App Store review detail relationship",
    )?;
    value
        .pointer("/data/id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .context("App Store review detail is missing; configure review details before uploading review attachments")
}

fn upload_app_store_review_attachment(
    client: &Client,
    token: &str,
    review_detail_id: &str,
    asset: &ProviderReleaseContentAsset,
) -> Result<String> {
    let path = Path::new(&asset.path);
    let file_size = file_len(path)?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .context("App Store review attachment path does not have a file name")?;
    let reservation =
        reserve_app_store_review_attachment(client, token, review_detail_id, file_name, file_size)?;
    upload_reserved_app_store_asset(
        client,
        path,
        file_size,
        &reservation,
        "App Store review attachment",
    )?;
    commit_app_store_asset(
        client,
        token,
        "appStoreReviewAttachments",
        &reservation.data.id,
        &md5_path(path)?,
    )?;
    Ok(reservation.data.id)
}

fn reserve_app_store_review_attachment(
    client: &Client,
    token: &str,
    review_detail_id: &str,
    file_name: &str,
    file_size: u64,
) -> Result<AppStoreAssetReservation> {
    let payload = json!({
        "data": {
            "type": "appStoreReviewAttachments",
            "attributes": {
                "fileName": file_name,
                "fileSize": file_size
            },
            "relationships": {
                "appStoreReviewDetail": {
                    "data": {
                        "type": "appStoreReviewDetails",
                        "id": review_detail_id
                    }
                }
            }
        }
    });
    let response = client
        .post(format!("{APP_STORE_API}/v1/appStoreReviewAttachments"))
        .bearer_auth(token)
        .json(&payload)
        .send()
        .context("failed to reserve App Store review attachment upload")?;
    let value = json_response(response, "App Store review attachment reserve")?;
    serde_json::from_value(value).context("failed to parse App Store review attachment reservation")
}

fn upload_reserved_app_store_asset(
    client: &Client,
    path: &Path,
    file_size: u64,
    reservation: &AppStoreAssetReservation,
    label: &str,
) -> Result<()> {
    let operations = reservation
        .data
        .attributes
        .as_ref()
        .context("App Store asset reservation did not include attributes")?
        .upload_operations
        .as_slice();
    if operations.is_empty() {
        bail!("{label} reservation did not include upload operations");
    }
    for operation in operations {
        upload_app_store_asset_part(client, path, file_size, operation, label)?;
    }
    Ok(())
}

fn commit_app_store_asset(
    client: &Client,
    token: &str,
    resource_type: &str,
    resource_id: &str,
    md5: &str,
) -> Result<()> {
    let payload = json!({
        "data": {
            "type": resource_type,
            "id": resource_id,
            "attributes": {
                "uploaded": true,
                "sourceFileChecksum": md5
            }
        }
    });
    json_response(
        client
            .patch(format!("{APP_STORE_API}/v1/{resource_type}/{resource_id}"))
            .bearer_auth(token)
            .header(CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .with_context(|| format!("failed to commit App Store {resource_type} upload"))?,
        &format!("App Store {resource_type} commit"),
    )?;
    Ok(())
}

fn app_store_screenshot_display_type(path: &Path) -> Result<String> {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter_map(app_store_screenshot_display_type_segment)
        .next()
        .map(str::to_string)
        .with_context(|| {
            format!(
                "App Store screenshot path {} must include a display type directory such as APP_IPHONE_67, APP_IPHONE_65, APP_IPAD_PRO_3GEN_129, APP_IPAD_PRO_3GEN_11, or APP_DESKTOP",
                path.display()
            )
        })
}

fn app_store_screenshot_display_type_segment(segment: &str) -> Option<&'static str> {
    match segment
        .to_ascii_uppercase()
        .replace(['-', '.', ' '], "_")
        .as_str()
    {
        "APP_IPHONE_67" | "IPHONE_67" | "IPHONE_6_7" => Some("APP_IPHONE_67"),
        "APP_IPHONE_65" | "IPHONE_65" | "IPHONE_6_5" => Some("APP_IPHONE_65"),
        "APP_IPHONE_61" | "IPHONE_61" | "IPHONE_6_1" => Some("APP_IPHONE_61"),
        "APP_IPHONE_58" | "IPHONE_58" | "IPHONE_5_8" => Some("APP_IPHONE_58"),
        "APP_IPAD_PRO_3GEN_129" | "IPAD_129" | "IPAD_12_9" | "TABLET_129" | "TABLET_12_9" => {
            Some("APP_IPAD_PRO_3GEN_129")
        }
        "APP_IPAD_PRO_3GEN_11" | "IPAD_11" | "TABLET_11" => Some("APP_IPAD_PRO_3GEN_11"),
        "APP_DESKTOP" | "MAC" | "DESKTOP" => Some("APP_DESKTOP"),
        _ => None,
    }
}

fn app_store_preview_type(path: &Path) -> Result<String> {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter_map(app_store_preview_type_segment)
        .next()
        .map(str::to_string)
        .with_context(|| {
            format!(
                "App Store preview path {} must include a preview type directory such as IPHONE_67, IPHONE_65, IPAD_PRO_3GEN_129, IPAD_PRO_3GEN_11, or DESKTOP",
                path.display()
            )
        })
}

fn app_store_preview_type_segment(segment: &str) -> Option<&'static str> {
    match segment
        .to_ascii_uppercase()
        .replace(['-', '.', ' '], "_")
        .as_str()
    {
        "APP_IPHONE_67" | "IPHONE_67" | "IPHONE_6_7" => Some("IPHONE_67"),
        "APP_IPHONE_65" | "IPHONE_65" | "IPHONE_6_5" => Some("IPHONE_65"),
        "APP_IPHONE_61" | "IPHONE_61" | "IPHONE_6_1" => Some("IPHONE_61"),
        "APP_IPHONE_58" | "IPHONE_58" | "IPHONE_5_8" => Some("IPHONE_58"),
        "APP_IPAD_PRO_3GEN_129"
        | "IPAD_PRO_3GEN_129"
        | "IPAD_129"
        | "IPAD_12_9"
        | "TABLET_129"
        | "TABLET_12_9" => Some("IPAD_PRO_3GEN_129"),
        "APP_IPAD_PRO_3GEN_11" | "IPAD_PRO_3GEN_11" | "IPAD_11" | "TABLET_11" => {
            Some("IPAD_PRO_3GEN_11")
        }
        "APP_DESKTOP" | "DESKTOP" | "MAC" => Some("DESKTOP"),
        _ => None,
    }
}

fn microsoft_store_provider_release_content_push(
    project_dir: &Path,
    locales: &[String],
    assets: Vec<ProviderReleaseContentAsset>,
    dry_run: bool,
) -> Result<Value> {
    let (uploadable, handoff): (Vec<_>, Vec<_>) = assets.into_iter().partition(|asset| {
        matches!(asset.role.as_str(), "screenshot" | "logo")
            && asset.mime_type.starts_with("image/")
    });
    let upload_plan = uploadable
        .iter()
        .map(|asset| {
            json!({
                "locale": asset.locale.clone().or_else(|| locales.first().cloned()),
                "kind": microsoft_listing_asset_kind(asset),
                "path": asset.path,
                "mime_type": asset.mime_type,
                "sha256": asset.sha256,
                "size_bytes": asset.size_bytes,
            })
        })
        .collect::<Vec<_>>();
    if dry_run {
        return Ok(json!({
            "provider": "microsoft-store",
            "locales": locales,
            "assets": upload_plan,
            "handoff_assets": handoff,
            "status": "dry-run",
            "mode": if handoff.is_empty() { "direct-upload" } else { "direct-upload-with-handoff" },
        }));
    }
    let uploaded = upload_microsoft_listing_assets(project_dir, locales, &uploadable)?;
    let handoff_manifest = if handoff.is_empty() {
        None
    } else {
        Some(stage_provider_asset_handoff(
            project_dir,
            DistributionProvider::MicrosoftStore,
            &handoff,
        )?)
    };
    let has_handoff = handoff_manifest.is_some();
    let staged_only = has_handoff && uploaded.is_empty();
    Ok(json!({
        "provider": "microsoft-store",
        "locales": locales,
        "assets": uploaded,
        "handoff_assets": handoff,
        "handoff_manifest": handoff_manifest.map(|path| path.display().to_string()),
        "status": if staged_only { "staged" } else if has_handoff { "partial" } else { "pushed" },
        "mode": if staged_only { "handoff-staging" } else if has_handoff { "direct-upload-with-handoff" } else { "direct-upload" },
        "manual_follow_up": if staged_only {
            provider_handoff_follow_up(DistributionProvider::MicrosoftStore)
        } else if has_handoff {
            vec![
                "Microsoft Store screenshots/logos were uploaded directly; upload staged trailers from the handoff manifest.".to_string(),
            ]
        } else {
            Vec::<String>::new()
        },
    }))
}

fn upload_microsoft_listing_assets(
    project_dir: &Path,
    release_locales: &[String],
    assets: &[ProviderReleaseContentAsset],
) -> Result<Vec<Value>> {
    if assets.is_empty() {
        return Ok(Vec::new());
    }
    let cfg = microsoft_store_content_config(project_dir)?;
    let product_id = cfg
        .product_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .context("distribution.microsoft_store.product_id is required")?;
    let seller_id = env_value("MICROSOFT_STORE_SELLER_ID")
        .or(cfg.seller_id.clone())
        .context(
            "distribution.microsoft_store.seller_id or MICROSOFT_STORE_SELLER_ID is required",
        )?;
    let client = http_client()?;
    let token = microsoft_store_content_access_token(&cfg, &client)?;
    let mut by_locale: BTreeMap<String, Vec<&ProviderReleaseContentAsset>> = BTreeMap::new();
    for asset in assets {
        let locale = asset
            .locale
            .clone()
            .or_else(|| release_locales.first().cloned())
            .context("Microsoft Store asset did not resolve a locale")?;
        by_locale.entry(locale).or_default().push(asset);
    }
    let mut uploaded = Vec::new();
    for (locale, locale_assets) in by_locale {
        let screenshot_assets = locale_assets
            .iter()
            .copied()
            .filter(|asset| asset.role == "screenshot")
            .collect::<Vec<_>>();
        let logo_assets = locale_assets
            .iter()
            .copied()
            .filter(|asset| asset.role == "logo")
            .collect::<Vec<_>>();
        let create_response = create_microsoft_listing_assets(
            &client,
            &token,
            &seller_id,
            product_id,
            &locale,
            screenshot_assets.len(),
            logo_assets.len(),
        )?;
        let created_screenshots = microsoft_created_listing_assets(
            &create_response,
            "/responseData/listingAssets/screenshots",
        );
        let created_logos = microsoft_created_listing_assets(
            &create_response,
            "/responseData/listingAssets/storeLogos",
        );
        if created_screenshots.len() != screenshot_assets.len() {
            bail!(
                "Microsoft Store returned {} screenshot upload slot(s) for {} requested asset(s)",
                created_screenshots.len(),
                screenshot_assets.len()
            );
        }
        if created_logos.len() != logo_assets.len() {
            bail!(
                "Microsoft Store returned {} logo upload slot(s) for {} requested asset(s)",
                created_logos.len(),
                logo_assets.len()
            );
        }
        let mut screenshot_commit = Vec::new();
        for (asset, upload) in screenshot_assets.iter().zip(created_screenshots.iter()) {
            upload_microsoft_listing_asset(&client, asset, upload)?;
            screenshot_commit.push(microsoft_listing_asset_commit_entry(upload)?);
            uploaded.push(microsoft_uploaded_asset_value(
                &locale,
                "screenshot",
                asset,
                upload,
            ));
        }
        let mut logo_commit = Vec::new();
        for (asset, upload) in logo_assets.iter().zip(created_logos.iter()) {
            upload_microsoft_listing_asset(&client, asset, upload)?;
            logo_commit.push(microsoft_listing_asset_commit_entry(upload)?);
            uploaded.push(microsoft_uploaded_asset_value(
                &locale, "logo", asset, upload,
            ));
        }
        let commit_response = commit_microsoft_listing_assets(
            &client,
            &token,
            &seller_id,
            product_id,
            &locale,
            screenshot_commit,
            logo_commit,
        )?;
        microsoft_store_success(&commit_response, "Microsoft Store listing assets commit")?;
    }
    Ok(uploaded)
}

fn create_microsoft_listing_assets(
    client: &Client,
    token: &str,
    seller_id: &str,
    product_id: &str,
    locale: &str,
    screenshots: usize,
    logos: usize,
) -> Result<Value> {
    let payload = microsoft_listing_asset_create_payload(locale, screenshots, logos);
    let response = client
        .post(format!(
            "{MICROSOFT_STORE_API}/submission/v1/product/{product_id}/listings/assets/create"
        ))
        .bearer_auth(token)
        .header("X-Seller-Account-Id", seller_id)
        .json(&payload)
        .send()
        .context("failed to create Microsoft Store listing asset upload slots")?;
    let value = json_response(response, "Microsoft Store listing assets create")?;
    microsoft_store_success(&value, "Microsoft Store listing assets create")?;
    Ok(value)
}

fn upload_microsoft_listing_asset(
    client: &Client,
    asset: &ProviderReleaseContentAsset,
    upload: &Value,
) -> Result<()> {
    let url = upload
        .get("primaryAssetUploadUrl")
        .and_then(Value::as_str)
        .context(
            "Microsoft Store listing asset create response did not include primaryAssetUploadUrl",
        )?;
    let method = upload
        .get("httpMethod")
        .and_then(Value::as_str)
        .unwrap_or("PUT");
    let method = Method::from_bytes(method.as_bytes())
        .with_context(|| format!("unsupported Microsoft Store upload method {method}"))?;
    let mut headers = HeaderMap::new();
    if let Some(map) = upload.get("httpHeaders").and_then(Value::as_object) {
        for (name, value) in map {
            let Some(value) = value.as_str() else {
                continue;
            };
            let name = HeaderName::from_bytes(name.as_bytes())
                .with_context(|| format!("invalid Microsoft Store upload header {name}"))?;
            let value = HeaderValue::from_str(value).with_context(|| {
                format!("invalid Microsoft Store upload header value for {name}")
            })?;
            headers.insert(name, value);
        }
    }
    if !headers.contains_key("x-ms-blob-type") {
        headers.insert("x-ms-blob-type", HeaderValue::from_static("BlockBlob"));
    }
    let path = Path::new(&asset.path);
    let file_size = file_len(path)?;
    let file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let response = client
        .request(method, url)
        .headers(headers)
        .body(Body::sized(file, file_size))
        .send()
        .context("failed to upload Microsoft Store listing asset")?;
    let status = response.status();
    let text = response
        .text()
        .context("failed to read Microsoft Store listing asset upload response")?;
    if !status.is_success() {
        bail!("Microsoft Store listing asset upload failed with {status}: {text}");
    }
    Ok(())
}

fn commit_microsoft_listing_assets(
    client: &Client,
    token: &str,
    seller_id: &str,
    product_id: &str,
    locale: &str,
    screenshots: Vec<Value>,
    logos: Vec<Value>,
) -> Result<Value> {
    let mut listing_assets = serde_json::Map::new();
    listing_assets.insert("language".to_string(), Value::String(locale.to_string()));
    if !screenshots.is_empty() {
        listing_assets.insert("screenshots".to_string(), Value::Array(screenshots));
    }
    if !logos.is_empty() {
        listing_assets.insert("storeLogos".to_string(), Value::Array(logos));
    }
    let payload = json!({ "listingAssets": Value::Object(listing_assets) });
    let response = client
        .put(format!(
            "{MICROSOFT_STORE_API}/submission/v1/product/{product_id}/listings/assets/commit"
        ))
        .bearer_auth(token)
        .header("X-Seller-Account-Id", seller_id)
        .json(&payload)
        .send()
        .context("failed to commit Microsoft Store listing assets")?;
    json_response(response, "Microsoft Store listing assets commit")
}

fn microsoft_created_listing_assets(value: &Value, pointer: &str) -> Vec<Value> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn microsoft_listing_asset_commit_entry(upload: &Value) -> Result<Value> {
    let id = upload
        .get("id")
        .and_then(Value::as_str)
        .context("Microsoft Store listing asset upload slot did not include id")?;
    let asset_url = upload
        .get("primaryAssetUploadUrl")
        .and_then(Value::as_str)
        .context(
            "Microsoft Store listing asset upload slot did not include primaryAssetUploadUrl",
        )?;
    Ok(json!({
        "id": id,
        "assetUrl": asset_url,
    }))
}

fn microsoft_uploaded_asset_value(
    locale: &str,
    kind: &str,
    asset: &ProviderReleaseContentAsset,
    upload: &Value,
) -> Value {
    json!({
        "locale": locale,
        "kind": kind,
        "id": upload.get("id").and_then(Value::as_str),
        "path": asset.path,
        "mime_type": asset.mime_type,
        "sha256": asset.sha256,
        "size_bytes": asset.size_bytes,
    })
}

fn microsoft_listing_asset_create_payload(locale: &str, screenshots: usize, logos: usize) -> Value {
    json!({
        "language": locale,
        "createAssetRequest": {
            "Screenshot": screenshots,
            "Logo": logos,
        }
    })
}

fn microsoft_listing_asset_kind(asset: &ProviderReleaseContentAsset) -> &'static str {
    if asset.role == "logo" {
        "logo"
    } else {
        "screenshot"
    }
}

fn microsoft_store_content_config(project_dir: &Path) -> Result<MicrosoftStoreContentConfig> {
    let path = project_dir.join("fission.toml");
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(toml::from_str::<MicrosoftContentToml>(&text)
        .with_context(|| format!("failed to parse {}", path.display()))?
        .distribution
        .and_then(|distribution| distribution.microsoft_store)
        .unwrap_or_default())
}

fn microsoft_store_content_access_token(
    cfg: &MicrosoftStoreContentConfig,
    client: &Client,
) -> Result<String> {
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
        .or_else(|| env_value("PARTNER_CENTER_CLIENT_SECRET"))
        .context("MICROSOFT_STORE_CLIENT_SECRET or PARTNER_CENTER_CLIENT_SECRET is required")?;
    let response = client
        .post(format!(
            "https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token"
        ))
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("scope", MICROSOFT_STORE_SCOPE),
        ])
        .send()
        .context("failed to request Microsoft Store access token")?;
    let token: MicrosoftOAuthTokenResponse = response
        .error_for_status()
        .context("Microsoft Store access token request failed")?
        .json()
        .context("failed to parse Microsoft Store access token response")?;
    Ok(token.access_token)
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

fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn collect_provider_release_content_assets(
    project_dir: &Path,
    provider: DistributionProvider,
    release_locales: &[String],
) -> Result<Vec<ProviderReleaseContentAsset>> {
    let content = read_content_toml(project_dir)?;
    let mut assets = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(manifest_path) = rendered_release_content_manifest(project_dir, provider) {
        let manifest: RenderManifestFile = serde_json::from_slice(&fs::read(&manifest_path)?)?;
        for asset in manifest.assets {
            let path = resolve_asset_path(project_dir, &asset.output);
            push_asset(
                &mut assets,
                &mut seen,
                provider,
                rendered_role(provider, &path, &asset),
                inferred_locale(&path, release_locales),
                path,
                Some(asset),
            )?;
        }
    }
    match provider {
        DistributionProvider::AppStore => {
            if let Some(app) = content
                .release
                .as_ref()
                .and_then(|release| release.assets.as_ref())
                .and_then(|assets| assets.app_store.as_ref())
            {
                collect_dir_assets(
                    &mut assets,
                    &mut seen,
                    provider,
                    project_dir,
                    app.screenshot_sets_dir.as_deref(),
                    "screenshot",
                    release_locales,
                )?;
                collect_dir_assets(
                    &mut assets,
                    &mut seen,
                    provider,
                    project_dir,
                    app.app_previews_dir.as_deref(),
                    "app-preview",
                    release_locales,
                )?;
                for attachment in &app.review_attachments {
                    push_asset(
                        &mut assets,
                        &mut seen,
                        provider,
                        "review-attachment".to_string(),
                        None,
                        resolve_asset_path(project_dir, attachment),
                        None,
                    )?;
                }
            }
        }
        DistributionProvider::MicrosoftStore => {
            if let Some(ms) = content
                .release
                .as_ref()
                .and_then(|release| release.assets.as_ref())
                .and_then(|assets| assets.microsoft_store.as_ref())
            {
                collect_dir_assets(
                    &mut assets,
                    &mut seen,
                    provider,
                    project_dir,
                    ms.screenshot_sets_dir.as_deref(),
                    "screenshot",
                    release_locales,
                )?;
                collect_dir_assets(
                    &mut assets,
                    &mut seen,
                    provider,
                    project_dir,
                    ms.trailers_dir.as_deref(),
                    "trailer",
                    release_locales,
                )?;
                collect_dir_assets(
                    &mut assets,
                    &mut seen,
                    provider,
                    project_dir,
                    ms.logo_dir.as_deref(),
                    "logo",
                    release_locales,
                )?;
            }
        }
        _ => {}
    }
    Ok(assets)
}

fn collect_dir_assets(
    assets: &mut Vec<ProviderReleaseContentAsset>,
    seen: &mut BTreeSet<PathBuf>,
    provider: DistributionProvider,
    project_dir: &Path,
    dir: Option<&str>,
    role: &str,
    locales: &[String],
) -> Result<()> {
    let Some(dir) = dir.filter(|value| !value.trim().is_empty()) else {
        return Ok(());
    };
    let root = resolve_asset_path(project_dir, dir);
    for path in release_asset_files(&root)? {
        push_asset(
            assets,
            seen,
            provider,
            role.to_string(),
            inferred_locale(&path, locales),
            path,
            None,
        )?;
    }
    Ok(())
}

fn push_asset(
    assets: &mut Vec<ProviderReleaseContentAsset>,
    seen: &mut BTreeSet<PathBuf>,
    provider: DistributionProvider,
    role: String,
    locale: Option<String>,
    path: PathBuf,
    rendered: Option<RenderedAssetFile>,
) -> Result<()> {
    if !path.exists() {
        bail!("release-content asset does not exist: {}", path.display());
    }
    if !seen.insert(path.clone()) {
        return Ok(());
    }
    let mime_type = mime_type(&path)
        .with_context(|| format!("unsupported release-content asset: {}", path.display()))?;
    let sha256 = match rendered.as_ref().and_then(|asset| asset.sha256.clone()) {
        Some(value) => Some(value),
        None => Some(sha256_path(&path)?),
    };
    let size_bytes = rendered
        .as_ref()
        .and_then(|asset| asset.size_bytes)
        .or_else(|| fs::metadata(&path).ok().map(|metadata| metadata.len()));
    assets.push(ProviderReleaseContentAsset {
        provider: provider.as_str().to_string(),
        role,
        locale,
        path: path.display().to_string(),
        mime_type,
        sha256,
        size_bytes,
        width: rendered.as_ref().and_then(|asset| asset.width),
        height: rendered.as_ref().and_then(|asset| asset.height),
    });
    Ok(())
}

fn stage_provider_asset_handoff(
    project_dir: &Path,
    provider: DistributionProvider,
    assets: &[ProviderReleaseContentAsset],
) -> Result<PathBuf> {
    let dir = project_dir
        .join("target/fission/release-content")
        .join(provider.as_str())
        .join(format!(
            "asset-handoff-{}",
            super::super::now_unix_seconds()
        ));
    fs::create_dir_all(&dir)?;
    let manifest_path = dir.join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "created_at_unix_seconds": super::super::now_unix_seconds(),
            "provider": provider.as_str(),
            "assets": assets,
            "manual_follow_up": provider_handoff_follow_up(provider),
        }))?,
    )?;
    Ok(manifest_path)
}

fn provider_handoff_follow_up(provider: DistributionProvider) -> Vec<String> {
    match provider {
        DistributionProvider::AppStore => vec![
            "Upload the staged App Store assets in App Store Connect for the active app version.".to_string(),
            "Fission records the exact files and hashes in the handoff manifest so this manual provider step remains auditable.".to_string(),
        ],
        DistributionProvider::MicrosoftStore => vec![
            "Upload the staged Microsoft Store trailers through Partner Center or provider tooling for the current submission.".to_string(),
            "Fission records the exact files and hashes in the handoff manifest so this manual provider step remains auditable.".to_string(),
        ],
        _ => Vec::new(),
    }
}

fn read_content_toml(project_dir: &Path) -> Result<ContentToml> {
    let path = project_dir.join("fission.toml");
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn rendered_release_content_manifest(
    project_dir: &Path,
    provider: DistributionProvider,
) -> Option<PathBuf> {
    let path = project_dir
        .join("release-content/screenshots/rendered")
        .join(provider.as_str())
        .join("release-content-manifest.json");
    path.exists().then_some(path)
}

fn rendered_role(
    provider: DistributionProvider,
    path: &Path,
    rendered: &RenderedAssetFile,
) -> String {
    if rendered.kind == "video" {
        return match provider {
            DistributionProvider::AppStore => "app-preview",
            DistributionProvider::MicrosoftStore => "trailer",
            _ => "video",
        }
        .to_string();
    }
    let lowered = path.to_string_lossy().to_ascii_lowercase();
    if lowered.contains("logo") {
        "logo".to_string()
    } else {
        "screenshot".to_string()
    }
}

fn inferred_locale(path: &Path, locales: &[String]) -> Option<String> {
    let path = path.to_string_lossy();
    locales
        .iter()
        .find(|locale| path.split(['/', '\\']).any(|part| part == locale.as_str()))
        .cloned()
        .or_else(|| locales.first().cloned())
}

fn resolve_asset_path(project_dir: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        project_dir.join(path)
    }
}

fn release_asset_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_release_asset_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_release_asset_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_release_asset_files(&path, files)?;
        } else if mime_type(&path).is_some() {
            files.push(path);
        }
    }
    Ok(())
}

fn mime_type(path: &Path) -> Option<String> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => Some("image/png".to_string()),
        "jpg" | "jpeg" => Some("image/jpeg".to_string()),
        "webp" => Some("image/webp".to_string()),
        "mp4" => Some("video/mp4".to_string()),
        "mov" => Some("video/quicktime".to_string()),
        "m4v" => Some("video/x-m4v".to_string()),
        "pdf" => Some("application/pdf".to_string()),
        _ => None,
    }
}

fn sha256_path(path: &Path) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let len = file
            .read(&mut buffer)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if len == 0 {
            break;
        }
        hasher.update(&buffer[..len]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

fn md5_path(path: &Path) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut hasher = Md5::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let len = file
            .read(&mut buffer)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if len == 0 {
            break;
        }
        hasher.update(&buffer[..len]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

#[cfg(test)]
fn md5_hex(bytes: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(bytes);
    hex_lower(&hasher.finalize())
}

fn file_len(path: &Path) -> Result<u64> {
    Ok(fs::metadata(path)
        .with_context(|| format!("failed to stat {}", path.display()))?
        .len())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
#[path = "provider_content_tests.rs"]
mod provider_content_tests;
