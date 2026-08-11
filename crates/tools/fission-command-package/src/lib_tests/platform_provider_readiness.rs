use super::support::{unique_dir, write_minimal_site};
use super::*;

#[test]
fn github_base_path_accounts_for_custom_domain() {
    let cfg = GithubPagesConfig {
        custom_domain: Some("docs.example.com".to_string()),
        repo: Some("repo".to_string()),
        ..Default::default()
    };
    assert_eq!(expected_github_base_path(&cfg, Some("repo")), "/");
    let cfg = GithubPagesConfig {
        repo: Some("repo".to_string()),
        ..Default::default()
    };
    assert_eq!(expected_github_base_path(&cfg, Some("repo")), "/repo/");
}

#[test]
fn android_aab_readiness_checks_official_toolchain() {
    let dir = unique_dir("android-aab-readiness");
    write_minimal_site(&dir);
    let checks = readiness_package(&dir, Some(Target::Android), Some(PackageFormat::Aab), true)
        .expect("readiness should produce checks even when blocked");
    for id in [
        "release.package.android_aab_script_exists",
        "release.package.android_sdk_configured",
        "release.package.android_ndk_configured",
        "release.package.aapt2_available",
        "release.package.zipalign_available",
        "release.package.apksigner_available",
        "release.package.bundletool_available",
    ] {
        assert!(checks.iter().any(|check| check.id == id), "missing {id}");
    }
}

#[test]
fn windows_msix_readiness_checks_manifest_packager_and_signing_source() {
    let dir = unique_dir("windows-msix-readiness");
    write_minimal_site(&dir);
    let checks = readiness_package(&dir, Some(Target::Windows), Some(PackageFormat::Msix), true)
        .expect("readiness should produce checks even when blocked");
    for id in [
        "release.package.windows_msix_manifest_exists",
        "release.package.windows_msix_script_exists",
        "release.package.makeappx_available",
        "release.package.signtool_available",
        "release.package.windows_signing_source",
    ] {
        assert!(checks.iter().any(|check| check.id == id), "missing {id}");
    }
}

#[test]
fn linux_run_readiness_checks_configured_installer_script() {
    let dir = unique_dir("linux-run-installer-readiness");
    write_minimal_site(&dir);
    fs::write(
        dir.join("fission.toml"),
        r#"
targets = ["linux"]

[app]
name = "demo"
app_id = "com.example.demo"

[package.linux.run]
installer_script = "platforms/linux/package-run.sh"
"#,
    )
    .unwrap();

    let checks = readiness_package(&dir, Some(Target::Linux), Some(PackageFormat::Run), false)
        .expect("readiness should report a missing custom installer");
    let installer = checks
        .iter()
        .find(|check| check.id == "release.package.linux_run_installer_script_exists")
        .expect("custom Linux installer readiness check");

    assert_eq!(installer.status, CheckStatus::Missing);
}

#[test]
fn terminal_run_is_a_supported_package_pair() {
    let dir = unique_dir("terminal-run-readiness");
    write_minimal_site(&dir);
    let checks = readiness_package(
        &dir,
        Some(Target::Terminal),
        Some(PackageFormat::Run),
        false,
    )
    .expect("terminal run readiness should be modeled");
    let format = checks
        .iter()
        .find(|check| check.id == "release.package.format_supported")
        .unwrap();
    assert_eq!(format.status, CheckStatus::Passed);
    assert!(checks
        .iter()
        .any(|check| check.id == "release.package.cargo_available"));
}

#[test]
fn cloudflare_readiness_requires_wrangler_backend() {
    let dir = unique_dir("cloudflare-readiness");
    write_minimal_site(&dir);
    let mut toml = fs::read_to_string(dir.join("fission.toml")).unwrap();
    toml.push_str(
        r#"
[distribution.cloudflare_pages.production]
account_id = "account"
project_name = "site-demo"
"#,
    );
    fs::write(dir.join("fission.toml"), toml).unwrap();
    let config = load_publish_manifest(&dir).unwrap();
    let checks = readiness_distribute(
        &dir,
        DistributionProvider::CloudflarePages,
        "production",
        None,
        None,
        None,
        &config,
    )
    .unwrap();
    assert!(checks
        .iter()
        .any(|check| check.id == "release.cloudflare_pages.wrangler_available"));
}

#[test]
fn file_distribution_readiness_requires_explicit_conflict_policy() {
    let dir = unique_dir("file-conflict-policy-readiness");
    write_minimal_site(&dir);
    let mut toml = fs::read_to_string(dir.join("fission.toml")).unwrap();
    toml.push_str(
        r#"
[distribution.s3.production]
bucket = "downloads"

[distribution.onedrive.production]
path_prefix = "Fission releases"

[distribution.dropbox.production]
path_prefix = "/Fission releases"
"#,
    );
    fs::write(dir.join("fission.toml"), toml).unwrap();
    let config = load_publish_manifest(&dir).unwrap();

    let s3 = readiness_distribute(
        &dir,
        DistributionProvider::S3,
        "production",
        None,
        None,
        None,
        &config,
    )
    .unwrap();
    assert!(s3.iter().any(|check| {
        check.id == "release.s3.overwrite_policy_explicit"
            && check.severity == CheckSeverity::Warning
            && check.status == CheckStatus::Missing
    }));

    let onedrive = readiness_distribute(
        &dir,
        DistributionProvider::OneDrive,
        "production",
        None,
        None,
        None,
        &config,
    )
    .unwrap();
    assert!(onedrive.iter().any(|check| {
        check.id == "release.onedrive.conflict_behavior_explicit"
            && check.severity == CheckSeverity::Warning
            && check.status == CheckStatus::Missing
    }));

    let dropbox = readiness_distribute(
        &dir,
        DistributionProvider::Dropbox,
        "production",
        None,
        None,
        None,
        &config,
    )
    .unwrap();
    assert!(dropbox.iter().any(|check| {
        check.id == "release.dropbox.write_mode_explicit"
            && check.severity == CheckSeverity::Warning
            && check.status == CheckStatus::Missing
    }));
}

#[test]
fn static_host_readiness_rejects_non_static_artifact_format() {
    let dir = unique_dir("static-host-format-readiness");
    write_minimal_site(&dir);
    let artifact_root = dir.join("target/fission/release/linux/run");
    fs::create_dir_all(&artifact_root).unwrap();
    let binary = artifact_root.join("site-demo.run");
    fs::write(&binary, b"run").unwrap();
    fs::write(artifact_root.join("index.html"), b"<html></html>").unwrap();
    let manifest_path = artifact_root.join(ARTIFACT_MANIFEST);
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&ArtifactManifest {
            schema_version: 1,
            created_at_unix_seconds: 0,
            project: ArtifactProject {
                app_id: "com.example.site_demo".to_string(),
                name: "site-demo".to_string(),
                build: Some(42),
                version: Some("1.2.3".to_string()),
            },
            target: "linux".to_string(),
            format: "run".to_string(),
            profile: "release".to_string(),
            variant: None,
            root_dir: artifact_root.display().to_string(),
            source_config: Vec::new(),
            artifacts: vec![ArtifactFile {
                kind: "asset".to_string(),
                purpose: None,
                platform: None,
                upload_provider: None,
                path: binary.display().to_string(),
                relative_path: "site-demo.run".to_string(),
                sha256: "abc".to_string(),
                size_bytes: 3,
                mime_type: "application/octet-stream".to_string(),
            }],
            icon_manifest: None,
            signing: None,
            notarization: None,
            validation: ArtifactValidation {
                state: "passed".to_string(),
                checks: Vec::new(),
            },
        })
        .unwrap(),
    )
    .unwrap();
    let config = load_publish_manifest(&dir).unwrap();

    let checks = readiness_distribute(
        &dir,
        DistributionProvider::GithubPages,
        "production",
        None,
        None,
        Some(&manifest_path),
        &config,
    )
    .unwrap();

    assert!(checks.iter().any(|check| {
        check.id == "release.distribution.static_artifact_format"
            && check.status == CheckStatus::Failed
    }));
}

#[test]
fn github_releases_readiness_is_not_static_site_specific() {
    let dir = unique_dir("github-releases-readiness");
    write_minimal_site(&dir);
    let artifact_root = dir.join("target/fission/release/linux/run");
    fs::create_dir_all(&artifact_root).unwrap();
    let binary = artifact_root.join("site-demo.run");
    fs::write(&binary, b"run").unwrap();
    let manifest_path = artifact_root.join(ARTIFACT_MANIFEST);
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&ArtifactManifest {
            schema_version: 1,
            created_at_unix_seconds: 0,
            project: ArtifactProject {
                app_id: "com.example.site_demo".to_string(),
                name: "site-demo".to_string(),
                build: Some(42),
                version: Some("1.2.3".to_string()),
            },
            target: "linux".to_string(),
            format: "run".to_string(),
            profile: "release".to_string(),
            variant: None,
            root_dir: artifact_root.display().to_string(),
            source_config: Vec::new(),
            artifacts: vec![ArtifactFile {
                kind: "asset".to_string(),
                purpose: None,
                platform: None,
                upload_provider: None,
                path: binary.display().to_string(),
                relative_path: "site-demo.run".to_string(),
                sha256: "abc".to_string(),
                size_bytes: 3,
                mime_type: "application/octet-stream".to_string(),
            }],
            icon_manifest: None,
            signing: None,
            notarization: None,
            validation: ArtifactValidation {
                state: "passed".to_string(),
                checks: Vec::new(),
            },
        })
        .unwrap(),
    )
    .unwrap();
    let config = load_publish_manifest(&dir).unwrap();
    let checks = readiness_distribute(
        &dir,
        DistributionProvider::GithubReleases,
        "production",
        None,
        None,
        Some(&manifest_path),
        &config,
    )
    .unwrap();
    assert!(checks.iter().any(|check| {
        check.id == "release.github_releases.assets_available"
            && check.status == CheckStatus::Passed
    }));
    assert!(!checks
        .iter()
        .any(|check| check.id == "release.distribution.static_root_exists"));
}

#[test]
fn github_releases_readiness_requires_explicit_asset_conflict_policy() {
    let dir = unique_dir("github-releases-conflict-policy");
    write_minimal_site(&dir);
    let toml = fs::read_to_string(dir.join("fission.toml"))
        .unwrap()
        .replace("replace_assets = true\n", "");
    fs::write(dir.join("fission.toml"), toml).unwrap();
    let config = load_publish_manifest(&dir).unwrap();

    let checks = readiness_distribute(
        &dir,
        DistributionProvider::GithubReleases,
        "production",
        None,
        None,
        None,
        &config,
    )
    .unwrap();

    assert!(checks.iter().any(|check| {
        check.id == "release.github_releases.replace_assets_explicit"
            && check.severity == CheckSeverity::Warning
            && check.status == CheckStatus::Missing
    }));
}

#[test]
fn microsoft_store_msix_readiness_uses_msstore_not_package_url() {
    let dir = unique_dir("microsoft-msix-readiness");
    write_minimal_site(&dir);
    let mut toml = fs::read_to_string(dir.join("fission.toml")).unwrap();
    toml.push_str(
        r#"
[distribution.microsoft_store]
product_id = "9N1234567890"
package_identity_name = "Example.SiteDemo"
package_type = "msix"
"#,
    );
    fs::write(dir.join("fission.toml"), toml).unwrap();

    let artifact_root = dir.join("target/fission/release/windows/msix");
    fs::create_dir_all(&artifact_root).unwrap();
    let package = artifact_root.join("site-demo.msixupload");
    fs::write(&package, b"msix").unwrap();
    let manifest_path = artifact_root.join(ARTIFACT_MANIFEST);
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&ArtifactManifest {
            schema_version: 1,
            created_at_unix_seconds: 0,
            project: ArtifactProject {
                app_id: "com.example.site_demo".to_string(),
                name: "site-demo".to_string(),
                build: Some(42),
                version: Some("1.2.3".to_string()),
            },
            target: "windows".to_string(),
            format: "msix".to_string(),
            profile: "release".to_string(),
            variant: None,
            root_dir: artifact_root.display().to_string(),
            source_config: Vec::new(),
            artifacts: vec![ArtifactFile {
                kind: "installer".to_string(),
                purpose: Some("store-upload".to_string()),
                platform: Some("windows".to_string()),
                upload_provider: Some("microsoft-store".to_string()),
                path: package.display().to_string(),
                relative_path: "site-demo.msixupload".to_string(),
                sha256: "abc".to_string(),
                size_bytes: 4,
                mime_type: "application/vnd.ms-appx".to_string(),
            }],
            icon_manifest: None,
            signing: None,
            notarization: None,
            validation: ArtifactValidation {
                state: "passed".to_string(),
                checks: Vec::new(),
            },
        })
        .unwrap(),
    )
    .unwrap();

    let config = load_publish_manifest(&dir).unwrap();
    let checks = readiness_distribute(
        &dir,
        DistributionProvider::MicrosoftStore,
        "production",
        None,
        None,
        Some(&manifest_path),
        &config,
    )
    .unwrap();

    assert!(checks
        .iter()
        .any(|check| check.id == "release.microsoft_store.msstore_available"));
    assert!(checks.iter().any(|check| {
        check.id == "release.microsoft_store.msix_upload_artifact_present"
            && check.status == CheckStatus::Passed
    }));
    assert!(!checks
        .iter()
        .any(|check| check.id == "release.microsoft_store.package_url_configured"));
}
