use super::*;

#[test]
fn app_store_dry_run_plans_direct_screenshot_preview_and_review_assets() {
    let dir =
        std::env::temp_dir().join(format!("fission-app-store-content-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(
        dir.join("release-content/screenshots/rendered/app-store/en-US/APP_IPHONE_67"),
    )
    .unwrap();
    fs::create_dir_all(dir.join("release-content/previews/app-store/en-US/IPHONE_67")).unwrap();
    fs::create_dir_all(dir.join("release-content/review")).unwrap();
    let screenshot =
        dir.join("release-content/screenshots/rendered/app-store/en-US/APP_IPHONE_67/home.png");
    let preview = dir.join("release-content/previews/app-store/en-US/IPHONE_67/demo.mp4");
    let attachment = dir.join("release-content/review/demo.pdf");
    fs::write(&screenshot, b"png").unwrap();
    fs::write(&preview, b"mp4").unwrap();
    fs::write(&attachment, b"pdf").unwrap();
    fs::write(
        dir.join("release-content/screenshots/rendered/app-store/release-content-manifest.json"),
        serde_json::to_vec(&json!({
            "assets": [{
                "kind": "image",
                "output": screenshot.display().to_string(),
                "sha256": "abc",
                "size_bytes": 3,
                "width": 1290,
                "height": 2796
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[release]
default_locales = ["en-US"]

[release.assets.app_store]
app_previews_dir = "release-content/previews/app-store"
review_attachments = ["release-content/review/demo.pdf"]
"#,
    )
    .unwrap();

    let value = app_store_release_content_push(&dir, None, true, true).unwrap();
    assert_eq!(value.get("status").and_then(Value::as_str), Some("dry-run"));
    assert_eq!(
        value.get("assets").and_then(Value::as_array).unwrap().len(),
        3
    );
    assert_eq!(
        value
            .get("handoff_assets")
            .and_then(Value::as_array)
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        value.get("mode").and_then(Value::as_str),
        Some("direct-upload")
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn microsoft_store_dry_run_plans_direct_screenshot_upload() {
    let dir =
        std::env::temp_dir().join(format!("fission-microsoft-content-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("release-content/screenshots/rendered/microsoft-store/en-US"))
        .unwrap();
    let screenshot =
        dir.join("release-content/screenshots/rendered/microsoft-store/en-US/home.png");
    fs::write(&screenshot, b"png").unwrap();
    fs::write(
        dir.join(
            "release-content/screenshots/rendered/microsoft-store/release-content-manifest.json",
        ),
        serde_json::to_vec(&json!({
            "assets": [{
                "kind": "image",
                "output": screenshot.display().to_string(),
                "sha256": "abc",
                "size_bytes": 3
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        dir.join("fission.toml"),
        "[release]\ndefault_locales = [\"en-US\"]\n",
    )
    .unwrap();

    let value = microsoft_store_release_content_push(&dir, None, true, true).unwrap();
    assert_eq!(value.get("status").and_then(Value::as_str), Some("dry-run"));
    assert_eq!(
        value.get("mode").and_then(Value::as_str),
        Some("direct-upload")
    );
    assert_eq!(
        value.get("assets").and_then(Value::as_array).unwrap().len(),
        1
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn microsoft_store_handoff_writes_manifest_for_trailers() {
    let dir =
        std::env::temp_dir().join(format!("fission-microsoft-trailer-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("release-content/screenshots/rendered/microsoft-store/en-US"))
        .unwrap();
    let trailer =
        dir.join("release-content/screenshots/rendered/microsoft-store/en-US/trailer.mp4");
    fs::write(&trailer, b"mp4").unwrap();
    fs::write(
        dir.join(
            "release-content/screenshots/rendered/microsoft-store/release-content-manifest.json",
        ),
        serde_json::to_vec(&json!({
            "assets": [{
                "kind": "video",
                "output": trailer.display().to_string(),
                "sha256": "abc",
                "size_bytes": 3
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        dir.join("fission.toml"),
        "[release]\ndefault_locales = [\"en-US\"]\n",
    )
    .unwrap();

    let value = microsoft_store_release_content_push(&dir, None, false, true).unwrap();
    let manifest = value
        .get("handoff_manifest")
        .and_then(Value::as_str)
        .unwrap();
    assert!(Path::new(manifest).exists());
    assert_eq!(value.get("status").and_then(Value::as_str), Some("staged"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn app_store_display_type_resolves_explicit_device_folders() {
    assert_eq!(
        app_store_screenshot_display_type(Path::new("en-US/APP_IPHONE_67/home.png")).unwrap(),
        "APP_IPHONE_67"
    );
    assert_eq!(
        app_store_screenshot_display_type(Path::new("en-US/ipad-12_9/home.png")).unwrap(),
        "APP_IPAD_PRO_3GEN_129"
    );
    assert_eq!(
        app_store_screenshot_display_type(Path::new("en-US/mac/desktop.png")).unwrap(),
        "APP_DESKTOP"
    );
    assert!(app_store_screenshot_display_type(Path::new("en-US/iphone/home.png")).is_err());
}

#[test]
fn app_store_checksum_uses_md5_hex_for_asset_commit() {
    assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
}

#[test]
fn microsoft_listing_asset_create_payload_counts_screenshots_and_logos() {
    let payload = microsoft_listing_asset_create_payload("en-us", 3, 1);
    assert_eq!(payload["language"], "en-us");
    assert_eq!(payload["createAssetRequest"]["Screenshot"], 3);
    assert_eq!(payload["createAssetRequest"]["Logo"], 1);
}
