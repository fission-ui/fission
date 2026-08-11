use super::support::{unique_dir, write_minimal_site};
use super::*;

#[test]
fn distribution_receipt_value_includes_release_context() {
    let dir = unique_dir("receipt-context");
    write_minimal_site(&dir);
    let artifact_manifest_path =
        dir.join("target/fission/release/static-site/static/artifact-manifest.json");
    fs::create_dir_all(artifact_manifest_path.parent().unwrap()).unwrap();
    let manifest = ArtifactManifest {
        schema_version: 1,
        created_at_unix_seconds: 123,
        project: ArtifactProject {
            app_id: "com.example.site_demo".to_string(),
            name: "site-demo".to_string(),
            build: Some(42),
            version: Some("1.2.3".to_string()),
        },
        target: "static-site".to_string(),
        format: "static".to_string(),
        profile: "release".to_string(),
        variant: None,
        root_dir: "target/fission/release/static-site/static".to_string(),
        source_config: Vec::new(),
        artifacts: vec![ArtifactFile {
            kind: "asset".to_string(),
            purpose: None,
            platform: None,
            upload_provider: None,
            path: dir
                .join("target/fission/release/static-site/static/app.zip")
                .display()
                .to_string(),
            relative_path: "app.zip".to_string(),
            sha256: "abc".to_string(),
            size_bytes: 12,
            mime_type: "application/zip".to_string(),
        }],
        icon_manifest: None,
        signing: None,
        notarization: None,
        validation: ArtifactValidation {
            state: "passed".to_string(),
            checks: Vec::new(),
        },
    };
    fs::write(
        &artifact_manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let content_manifest_path = dir
        .join("release-content/screenshots/rendered/github-releases/release-content-manifest.json");
    fs::create_dir_all(content_manifest_path.parent().unwrap()).unwrap();
    fs::write(&content_manifest_path, br#"{"assets":[]}"#).unwrap();
    let receipt = DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: 456,
        provider: "github-releases".to_string(),
        site: "production".to_string(),
        action: "publish".to_string(),
        artifact_manifest: Some(artifact_manifest_path.display().to_string()),
        deployment_id: Some("v1.2.3".to_string()),
        canonical_url: Some("https://example.com/release".to_string()),
        preview_url: None,
        custom_domain: None,
        status: "published".to_string(),
        stdout: Some(
            serde_json::to_string(&json!({
                "uploaded": [{
                    "relative_path": "app.zip",
                    "provider_id": "provider/app.zip",
                    "url": "https://cdn.example.com/app.zip"
                }]
            }))
            .unwrap(),
        ),
        stderr: None,
        manual_follow_up: Vec::new(),
    };
    let value = distribution_receipt_value(
        &DistributeOptions {
            project_dir: dir,
            provider: DistributionProvider::GithubReleases,
            action: DistributeAction::Publish,
            target: Some(Target::Site),
            format: Some(PackageFormat::Static),
            artifact: Some(artifact_manifest_path.clone()),
            site: "production".to_string(),
            deploy: Some("v1.2.3".to_string()),
            track: Some("stable".to_string()),
            locales: vec!["en-US".to_string()],
            dry_run: false,
            yes: true,
            json: true,
        },
        &receipt,
        Some(&artifact_manifest_path),
        Some(&manifest),
    )
    .unwrap();
    assert_eq!(value["release_id"], "1.2.3+42");
    assert_eq!(value["target"], "static-site");
    assert_eq!(value["format"], "static");
    assert_eq!(value["track_channel"], "stable");
    assert_eq!(value["locales"][0], "en-US");
    assert_eq!(value["artifact_hash"], "abc");
    assert_eq!(
        value["release_content_manifest"],
        content_manifest_path.display().to_string()
    );
    assert_eq!(
        value["release_content_manifest_sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert_eq!(value["uploaded_assets"][0]["relative_path"], "app.zip");
    assert_eq!(
        value["uploaded_assets"][0]["provider_id"],
        "provider/app.zip"
    );
    assert_eq!(
        value["uploaded_assets"][0]["url"],
        "https://cdn.example.com/app.zip"
    );
    assert_eq!(value["uploaded_bytes"], 12);
}
