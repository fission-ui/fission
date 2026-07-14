use super::*;
use reqwest::blocking::Body;
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Digest;
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Default)]
struct RenderManifestFile {
    #[serde(default)]
    assets: Vec<RenderedAssetFile>,
}

#[derive(Debug, Deserialize, Default)]
struct RenderedAssetFile {
    kind: String,
    output: String,
    source: Option<String>,
    sha256: Option<String>,
    size_bytes: Option<u64>,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct PlayReleaseContentAsset {
    locale: String,
    image_type: String,
    path: String,
    mime_type: String,
    sha256: Option<String>,
    size_bytes: Option<u64>,
    width: Option<u32>,
    height: Option<u32>,
}

pub(super) fn play_release_content_push(
    project_dir: &Path,
    locales_arg: Option<&str>,
    dry_run: bool,
    yes: bool,
) -> Result<Value> {
    if !dry_run && !yes {
        bail!("release-content push mutates provider assets; pass --yes after reviewing `release-content validate`");
    }
    let root = read_release_provider_toml(project_dir)?;
    let cfg = root
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.play_store.clone())
        .unwrap_or_default();
    let package_name = cfg.package_name.as_deref().context(
        "distribution.play_store.package_name is required for Play release-content push",
    )?;
    let locales = resolve_release_locales(&root, locales_arg)?;
    let assets = collect_play_release_content_assets(project_dir, &root, &locales)?;
    if assets.is_empty() {
        return Ok(json!({
            "provider": "play-store",
            "package_name": package_name,
            "locales": locales,
            "assets": [],
            "status": "skipped",
            "reason": "no rendered screenshots or Play feature graphic were found"
        }));
    }
    if dry_run {
        return Ok(json!({
            "provider": "play-store",
            "package_name": package_name,
            "locales": locales,
            "assets": assets,
            "replace_existing_image_sets": true,
            "status": "dry-run"
        }));
    }
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let mut responses = Vec::new();
    for (locale, image_type) in play_release_content_groups(&assets) {
        let response = delete_play_images(
            &client,
            &token,
            package_name,
            &edit_id,
            &locale,
            &image_type,
        )?;
        responses.push(json!({
            "operation": "delete-existing-images",
            "locale": locale,
            "image_type": image_type,
            "response": response
        }));
    }
    for asset in &assets {
        let response = upload_play_image(&client, &token, package_name, &edit_id, asset)?;
        responses.push(json!({
            "operation": "upload-image",
            "locale": asset.locale,
            "image_type": asset.image_type,
            "path": asset.path,
            "response": response
        }));
    }
    validate_play_edit(&client, &token, package_name, &edit_id)?;
    commit_play_edit(&client, &token, package_name, &edit_id)?;
    Ok(json!({
        "provider": "play-store",
        "package_name": package_name,
        "edit_id": edit_id,
        "locales": locales,
        "assets": assets,
        "responses": responses,
        "status": "pushed"
    }))
}

fn collect_play_release_content_assets(
    project_dir: &Path,
    root: &ReleaseProviderToml,
    release_locales: &[String],
) -> Result<Vec<PlayReleaseContentAsset>> {
    let mut assets = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(manifest_path) =
        rendered_release_content_manifest(project_dir, DistributionProvider::PlayStore)
    {
        let manifest_text = fs::read_to_string(&manifest_path)
            .with_context(|| format!("failed to read {}", manifest_path.display()))?;
        let manifest: RenderManifestFile = serde_json::from_str(&manifest_text)
            .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
        for asset in manifest.assets {
            if asset.kind != "image" {
                continue;
            }
            let path = resolve_asset_path(project_dir, &asset.output);
            let image_type = infer_play_image_type(&path, asset.source.as_deref());
            push_play_content_asset(
                &mut assets,
                &mut seen,
                &path,
                image_type,
                inferred_locales(&path, release_locales),
                Some(asset),
            )?;
        }
    }
    if let Some(play_assets) = root
        .release
        .as_ref()
        .and_then(|release| release.assets.as_ref())
        .and_then(|assets| assets.play_store.as_ref())
    {
        if let Some(feature_graphic) = play_assets.feature_graphic.as_deref() {
            let path = resolve_asset_path(project_dir, feature_graphic);
            push_play_content_asset(
                &mut assets,
                &mut seen,
                &path,
                "featureGraphic".to_string(),
                release_locales.to_vec(),
                None,
            )?;
        }
        if let Some(screenshot_sets_dir) = play_assets.screenshot_sets_dir.as_deref() {
            let root_path = resolve_asset_path(project_dir, screenshot_sets_dir);
            for path in release_asset_files(&root_path)? {
                let image_type = infer_play_image_type(&path, None);
                push_play_content_asset(
                    &mut assets,
                    &mut seen,
                    &path,
                    image_type,
                    inferred_locales(&path, release_locales),
                    None,
                )?;
            }
        }
    }
    Ok(assets)
}

fn push_play_content_asset(
    assets: &mut Vec<PlayReleaseContentAsset>,
    seen: &mut BTreeSet<(String, String, String)>,
    path: &Path,
    image_type: String,
    locales: Vec<String>,
    rendered: Option<RenderedAssetFile>,
) -> Result<()> {
    if !path.exists() {
        bail!("release-content asset does not exist: {}", path.display());
    }
    let mime_type = image_mime_type(path)
        .with_context(|| format!("unsupported Play image asset format: {}", path.display()))?;
    let metadata =
        fs::metadata(path).with_context(|| format!("failed to stat {}", path.display()))?;
    let sha256 = match rendered.as_ref().and_then(|asset| asset.sha256.clone()) {
        Some(value) => Some(value),
        None => Some(sha256_path(path)?),
    };
    for locale in locales {
        let display_path = path.display().to_string();
        if seen.insert((locale.clone(), image_type.clone(), display_path.clone())) {
            assets.push(PlayReleaseContentAsset {
                locale,
                image_type: image_type.clone(),
                path: display_path,
                mime_type: mime_type.to_string(),
                sha256: sha256.clone(),
                size_bytes: rendered
                    .as_ref()
                    .and_then(|asset| asset.size_bytes)
                    .or(Some(metadata.len())),
                width: rendered.as_ref().and_then(|asset| asset.width),
                height: rendered.as_ref().and_then(|asset| asset.height),
            });
        }
    }
    Ok(())
}

fn play_release_content_groups(assets: &[PlayReleaseContentAsset]) -> Vec<(String, String)> {
    assets
        .iter()
        .map(|asset| (asset.locale.clone(), asset.image_type.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn delete_play_images(
    client: &Client,
    token: &str,
    package_name: &str,
    edit_id: &str,
    locale: &str,
    image_type: &str,
) -> Result<Value> {
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/listings/{locale}/{image_type}"
    );
    let response = client
        .delete(url)
        .bearer_auth(token)
        .send()
        .with_context(|| {
            format!("failed to delete existing Google Play {locale} {image_type} images")
        })?;
    json_or_empty_response(response, "Google Play images deleteall")
}

fn upload_play_image(
    client: &Client,
    token: &str,
    package_name: &str,
    edit_id: &str,
    asset: &PlayReleaseContentAsset,
) -> Result<Value> {
    let url = format!(
        "{PLAY_UPLOAD_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/listings/{}/{}?uploadType=media",
        asset.locale, asset.image_type
    );
    let len = fs::metadata(&asset.path)
        .with_context(|| format!("failed to stat {}", asset.path))?
        .len();
    let file =
        fs::File::open(&asset.path).with_context(|| format!("failed to open {}", asset.path))?;
    let response = client
        .post(url)
        .bearer_auth(token)
        .header(CONTENT_TYPE, asset.mime_type.as_str())
        .body(Body::sized(file, len))
        .send()
        .with_context(|| format!("failed to upload {} to Google Play", asset.path))?;
    json_response(response, "Google Play image upload")
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

fn resolve_asset_path(project_dir: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() || path.exists() {
        path
    } else {
        project_dir.join(path)
    }
}

fn inferred_locales(path: &Path, configured: &[String]) -> Vec<String> {
    let mut locales = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter(|component| configured.iter().any(|locale| locale == component))
        .map(str::to_string)
        .collect::<Vec<_>>();
    locales.sort();
    locales.dedup();
    if locales.is_empty() {
        configured.to_vec()
    } else {
        locales
    }
}

fn infer_play_image_type(path: &Path, source: Option<&str>) -> String {
    let haystack =
        format!("{} {}", path.display(), source.unwrap_or_default()).to_ascii_lowercase();
    if haystack.contains("feature") && haystack.contains("graphic") {
        "featureGraphic"
    } else if haystack.contains("7-inch")
        || haystack.contains("seven-inch")
        || haystack.contains("seven_inch")
        || haystack.contains("tablet-7")
    {
        "sevenInchScreenshots"
    } else if haystack.contains("10-inch")
        || haystack.contains("ten-inch")
        || haystack.contains("ten_inch")
        || haystack.contains("tablet-10")
        || haystack.contains("tablet")
    {
        "tenInchScreenshots"
    } else if haystack.contains("wear") || haystack.contains("watch") {
        "wearScreenshots"
    } else if haystack.contains("tv") || haystack.contains("television") {
        "tvScreenshots"
    } else {
        "phoneScreenshots"
    }
    .to_string()
}

fn image_mime_type(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        _ => None,
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
        } else if image_mime_type(&path).is_some() {
            files.push(path);
        }
    }
    Ok(())
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

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn json_or_empty_response(response: Response, operation: &str) -> Result<Value> {
    let status = response.status();
    let text = response.text()?;
    if !status.is_success() {
        bail!("{operation} failed with {status}: {text}");
    }
    if text.trim().is_empty() {
        Ok(json!({}))
    } else {
        serde_json::from_str(&text)
            .with_context(|| format!("failed to parse {operation} response: {text}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_release_content_dry_run_collects_rendered_manifest_assets() {
        let dir = std::env::temp_dir().join(format!(
            "fission-play-release-content-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("release-content/screenshots/rendered/play-store/en-US/phone"))
            .unwrap();
        let screenshot =
            dir.join("release-content/screenshots/rendered/play-store/en-US/phone/home.png");
        fs::write(&screenshot, b"png").unwrap();
        fs::write(
            dir.join(
                "release-content/screenshots/rendered/play-store/release-content-manifest.json",
            ),
            serde_json::to_vec_pretty(&json!({
                "assets": [{
                    "kind": "image",
                    "output": screenshot.display().to_string(),
                    "sha256": "abc123",
                    "size_bytes": 3,
                    "width": 1080,
                    "height": 1920
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            dir.join("fission.toml"),
            r#"
[distribution.play_store]
package_name = "com.example.demo"

[release]
default_locales = ["en-US"]
"#,
        )
        .unwrap();

        let value = play_release_content_push(&dir, None, true, false).unwrap();
        assert_eq!(
            value.pointer("/status").and_then(Value::as_str),
            Some("dry-run")
        );
        assert_eq!(
            value.pointer("/assets/0/locale").and_then(Value::as_str),
            Some("en-US")
        );
        assert_eq!(
            value
                .pointer("/assets/0/image_type")
                .and_then(Value::as_str),
            Some("phoneScreenshots")
        );
        assert_eq!(
            value.pointer("/assets/0/sha256").and_then(Value::as_str),
            Some("abc123")
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn play_image_type_inference_matches_play_listing_types() {
        assert_eq!(
            infer_play_image_type(Path::new("release/feature-graphic.png"), None),
            "featureGraphic"
        );
        assert_eq!(
            infer_play_image_type(Path::new("screenshots/en-US/tablet-7/home.png"), None),
            "sevenInchScreenshots"
        );
        assert_eq!(
            infer_play_image_type(Path::new("screenshots/en-US/tablet-10/home.png"), None),
            "tenInchScreenshots"
        );
        assert_eq!(
            infer_play_image_type(Path::new("screenshots/en-US/wear/home.png"), None),
            "wearScreenshots"
        );
    }
}
