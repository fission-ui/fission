use super::*;
use std::path::PathBuf;

fn unique_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "fission-release-content-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_content_project(dir: &Path) {
    fs::create_dir_all(dir.join("release-content/screenshots/raw/en-US")).unwrap();
    fs::write(
        dir.join("release-content/screenshots/raw/en-US/home.png"),
        b"png",
    )
    .unwrap();
    fs::create_dir_all(dir.join("tests/release_screenshots")).unwrap();
    fs::write(
        dir.join("tests/release_screenshots/home.toml"),
        "wait = true\n",
    )
    .unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
name = "content-demo"
app_id = "com.example.content_demo"

[release.screenshots]
raw_dir = "release-content/screenshots/raw"
rendered_dir = "release-content/screenshots/rendered"

[[release.screenshots.scenarios]]
id = "home"
name = "Home"
targets = ["web"]
script = "tests/release_screenshots/home.toml"
wait_for = "semantic:home"

[release.assets.play_store]
screenshot_sets_dir = "release-content/screenshots/rendered/play-store"
feature_graphic = "release-content/screenshots/raw/en-US/home.png"
"#,
    )
    .unwrap();
}

fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend_from_slice(&13u32.to_be_bytes());
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes.extend_from_slice(&0u32.to_be_bytes());
    bytes
}

#[test]
fn render_release_content_copies_raw_assets_and_writes_manifest() {
    let dir = unique_dir("render");
    write_content_project(&dir);
    let report = render_release_content(&dir, DistributionProvider::PlayStore).unwrap();
    assert_ne!(report.status, "blocked");
    assert!(dir
        .join("release-content/screenshots/rendered/play-store/en-US/home.png")
        .exists());
    let manifest =
        dir.join("release-content/screenshots/rendered/play-store/release-content-manifest.json");
    assert!(manifest.exists());
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(manifest).unwrap()).unwrap();
    let sha = manifest["assets"][0]["sha256"].as_str().unwrap();
    assert_eq!(sha.len(), 64);
}

#[test]
fn image_dimensions_reads_png_header() {
    let dir = unique_dir("png-dimensions");
    let path = dir.join("screen.png");
    fs::write(&path, png_header(1440, 2560)).unwrap();
    assert_eq!(image_dimensions(&path).unwrap(), Some((1440, 2560)));
}

#[test]
fn provider_asset_validation_reports_dimensions() {
    let dir = unique_dir("asset-rules");
    let provider_dir = dir.join("release-content/screenshots/rendered/play-store/en-US");
    fs::create_dir_all(&provider_dir).unwrap();
    fs::write(provider_dir.join("one.png"), png_header(1440, 2560)).unwrap();
    fs::write(provider_dir.join("two.png"), png_header(1440, 2560)).unwrap();
    let mut checks = Vec::new();
    validate_rendered_asset_rules(
        DistributionProvider::PlayStore,
        &dir.join("release-content/screenshots/rendered/play-store"),
        &mut checks,
    );
    assert!(checks.iter().any(|check| {
        check.id == "release_content.play-store.screenshot_count" && check.status == "passed"
    }));
    assert!(checks
        .iter()
        .any(|check| { check.id.ends_with(".dimensions") && check.status == "passed" }));
}

#[test]
fn required_assets_accept_configured_provider_screenshot_dir() {
    let dir = unique_dir("required-assets");
    fs::create_dir_all(dir.join("release-content/manual/app-store/en-US/APP_IPHONE_67")).unwrap();
    fs::write(
        dir.join("release-content/manual/app-store/en-US/APP_IPHONE_67/home.png"),
        png_header(1440, 2560),
    )
    .unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
name = "content-demo"
app_id = "com.example.content_demo"

[release.screenshots]
raw_dir = "release-content/screenshots/raw"
rendered_dir = "release-content/screenshots/rendered"

[release.assets.app_store]
screenshot_sets_dir = "release-content/manual/app-store"
"#,
    )
    .unwrap();

    let report = validate_release_content_model(&dir, Some(DistributionProvider::AppStore));
    let required = report
        .checks
        .iter()
        .find(|check| check.id == "release_content.app-store.required_assets")
        .unwrap();
    assert_eq!(required.status, "passed");
}

#[test]
fn app_store_validation_requires_explicit_display_type_directory() {
    let dir = unique_dir("app-store-display-type");
    let provider_dir = dir.join("release-content/screenshots/rendered/app-store/en-US");
    fs::create_dir_all(&provider_dir).unwrap();
    fs::write(provider_dir.join("home.png"), png_header(1440, 2560)).unwrap();
    let mut checks = Vec::new();
    validate_rendered_asset_rules(
        DistributionProvider::AppStore,
        &dir.join("release-content/screenshots/rendered/app-store"),
        &mut checks,
    );
    assert!(checks.iter().any(|check| {
        check.id == "release_content.app-store.image.home.display_type" && check.status == "failed"
    }));
}

#[test]
fn app_store_validation_requires_explicit_preview_type_directory() {
    let dir = unique_dir("app-store-preview-type");
    let preview_dir = dir.join("release-content/previews/app-store/en-US");
    fs::create_dir_all(&preview_dir).unwrap();
    fs::write(preview_dir.join("demo.mp4"), b"mp4").unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
name = "content-demo"
app_id = "com.example.content_demo"

[release.assets.app_store]
app_previews_dir = "release-content/previews/app-store"
"#,
    )
    .unwrap();

    let report = validate_release_content_model(&dir, Some(DistributionProvider::AppStore));
    assert!(report.checks.iter().any(|check| {
        check.id == "release_content.app-store.video.demo.preview_type" && check.status == "failed"
    }));
}

#[test]
fn content_manifest_materializes_referenced_release_files() {
    let dir = unique_dir("manifest");
    fs::create_dir_all(dir.join("release-content/metadata/1.0.0+1/notes")).unwrap();
    fs::write(
        dir.join("release-content/metadata/1.0.0+1/release.toml"),
        "[play_store.en-US]\nfull_description = \"Demo\"\n",
    )
    .unwrap();
    fs::write(
        dir.join("release-content/metadata/1.0.0+1/notes/en-US.md"),
        "Initial release",
    )
    .unwrap();
    fs::create_dir_all(dir.join("release-content/screenshots/rendered/play-store")).unwrap();
    fs::write(
        dir.join("release-content/screenshots/rendered/play-store/release-content-manifest.json"),
        r#"{"assets":[{"kind":"image","output":"home.png"}]}"#,
    )
    .unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"
[release]
active_release = "1.0.0+1"
default_locales = ["en-US"]

[release.screenshots]
rendered_dir = "release-content/screenshots/rendered"

[[releases]]
id = "1.0.0+1"
version = "1.0.0"
metadata = "release-content/metadata/1.0.0+1/release.toml"
release_notes = "release-content/metadata/1.0.0+1/notes"
"#,
    )
    .unwrap();

    let path = materialize_release_content_manifest(&dir, DistributionProvider::PlayStore).unwrap();
    let path = path.expect("manifest should be written");
    let value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(
        value.pointer("/active_release").and_then(Value::as_str),
        Some("1.0.0+1")
    );
    assert_eq!(
        value
            .pointer("/rendered_screenshots/asset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value
            .get("referenced_files")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn screenshot_step_payload_uses_test_control_protocol() {
    let step = ScreenshotStep {
        cmd: "tap_text".to_string(),
        text: Some("Save".to_string()),
        ..Default::default()
    };
    let payload = step_payload(&step, Path::new("/tmp"), "store", "save").unwrap();
    assert_eq!(payload["cmd"], "TapText");
    assert_eq!(payload["text"], "Save");
}

#[test]
fn screenshot_wait_for_uses_selector_protocol() {
    let payload = wait_for_payload(
        "semantic:checkout.submit",
        std::time::Duration::from_secs(7),
    )
    .unwrap();

    assert_eq!(payload["cmd"], "WaitForVisible");
    assert_eq!(payload["timeout_ms"], 7000);
    assert_eq!(
        payload["query"]["selector"]["SemanticIdentifier"]["identifier"],
        "checkout.submit"
    );
}

#[test]
fn screenshot_step_payload_supports_selector_actions() {
    let step = ScreenshotStep {
        cmd: "fill_text".to_string(),
        selector: Some("test_id:login.email#1".to_string()),
        text: Some("person@example.com".to_string()),
        ..Default::default()
    };
    let payload = step_payload(&step, Path::new("/tmp"), "store", "login").unwrap();

    assert_eq!(payload["cmd"], "FillText");
    assert_eq!(payload["text"], "person@example.com");
    assert_eq!(payload["query"]["index"], 1);
    assert_eq!(
        payload["query"]["selector"]["TestId"]["test_id"],
        "login.email"
    );
}
