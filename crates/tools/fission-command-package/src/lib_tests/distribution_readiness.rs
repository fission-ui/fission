use super::support::{unique_dir, write_minimal_site};
use super::*;

#[test]
fn distribution_readiness_blocks_failed_artifact_validation() {
    let dir = unique_dir("artifact-validation-readiness");
    write_minimal_site(&dir);
    let artifact = package_silent(PackageOptions {
        project_dir: dir.clone(),
        target: Target::Site,
        format: PackageFormat::Static,
        release: true,
        variant: None,
        json: false,
    })
    .unwrap();
    let mut manifest: Value =
        serde_json::from_str(&fs::read_to_string(&artifact).unwrap()).unwrap();
    manifest["validation"]["state"] = Value::String("failed".to_string());
    manifest["validation"]["checks"] = json!([{
        "id": "release.package.artifact.primary_present",
        "severity": "error",
        "status": "failed",
        "summary": "primary package artifact exists",
        "details": "forced failure",
        "remediation": ["repackage"]
    }]);
    fs::write(&artifact, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();

    let checks = distribution_readiness_checks(
        &dir,
        DistributionProvider::GithubPages,
        "production",
        None,
        None,
        Some(&artifact),
    )
    .unwrap();
    let validation = checks
        .iter()
        .find(|check| check.id == "release.distribution.artifact_validation")
        .unwrap();
    assert_eq!(validation.severity, CheckSeverity::Error);
    assert_eq!(validation.status, CheckStatus::Failed);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn distribution_readiness_blocks_artifact_hash_mismatch() {
    let dir = unique_dir("artifact-hash-readiness");
    write_minimal_site(&dir);
    let artifact = package_silent(PackageOptions {
        project_dir: dir.clone(),
        target: Target::Site,
        format: PackageFormat::Static,
        release: true,
        variant: None,
        json: false,
    })
    .unwrap();
    let manifest: Value = serde_json::from_str(&fs::read_to_string(&artifact).unwrap()).unwrap();
    let packaged_file = manifest
        .get("artifacts")
        .and_then(Value::as_array)
        .and_then(|artifacts| artifacts.first())
        .and_then(|artifact| artifact.get("path"))
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .expect("package should record at least one artifact");
    fs::write(&packaged_file, b"tampered").unwrap();

    let checks = distribution_readiness_checks(
        &dir,
        DistributionProvider::GithubPages,
        "production",
        None,
        None,
        Some(&artifact),
    )
    .unwrap();
    let hashes = checks
        .iter()
        .find(|check| check.id == "release.distribution.artifact_hashes_match")
        .expect("distribution readiness should verify artifact hashes");
    assert_eq!(hashes.severity, CheckSeverity::Error);
    assert_eq!(hashes.status, CheckStatus::Failed);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn distribution_readiness_blocks_stale_source_config() {
    let dir = unique_dir("artifact-source-config-readiness");
    write_minimal_site(&dir);
    let artifact = package_silent(PackageOptions {
        project_dir: dir.clone(),
        target: Target::Site,
        format: PackageFormat::Static,
        release: true,
        variant: None,
        json: false,
    })
    .unwrap();
    fs::write(
        dir.join("fission.toml"),
        format!(
            "{}\n# changed after packaging\n",
            fs::read_to_string(dir.join("fission.toml")).unwrap()
        ),
    )
    .unwrap();

    let checks = distribution_readiness_checks(
        &dir,
        DistributionProvider::GithubPages,
        "production",
        None,
        None,
        Some(&artifact),
    )
    .unwrap();
    let source_config = checks
        .iter()
        .find(|check| check.id == "release.distribution.artifact_source_config_current")
        .expect("distribution readiness should verify source config hashes");
    assert_eq!(source_config.severity, CheckSeverity::Error);
    assert_eq!(source_config.status, CheckStatus::Failed);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn distribution_readiness_checks_receipt_path_writable() {
    let dir = unique_dir("distribution-receipt-writable");
    write_minimal_site(&dir);
    let artifact = package_silent(PackageOptions {
        project_dir: dir.clone(),
        target: Target::Site,
        format: PackageFormat::Static,
        release: true,
        variant: None,
        json: false,
    })
    .unwrap();

    let checks = distribution_readiness_checks(
        &dir,
        DistributionProvider::GithubPages,
        "production",
        None,
        None,
        Some(&artifact),
    )
    .unwrap();
    let receipt = checks
        .iter()
        .find(|check| check.id == "release.distribution.receipt_path_writable")
        .expect("distribution readiness should check receipt path writability");
    assert_eq!(receipt.severity, CheckSeverity::Error);
    assert_eq!(receipt.status, CheckStatus::Passed);
    assert!(checks.iter().any(|check| {
        check.id == "release.distribution.dry_run_supported" && check.status == CheckStatus::Passed
    }));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn static_package_includes_configured_secondary_artifacts() {
    let dir = unique_dir("secondary-artifacts");
    write_minimal_site(&dir);
    fs::create_dir_all(dir.join("release-content/symbols")).unwrap();
    fs::write(dir.join("release-content/symbols/app.dSYM.zip"), b"symbols").unwrap();
    let mut toml = fs::read_to_string(dir.join("fission.toml")).unwrap();
    toml.push_str(
        r#"
[[package.symbols]]
path = "release-content/symbols/app.dSYM.zip"
platform = "ios"
upload_provider = "crash-service"
"#,
    );
    fs::write(dir.join("fission.toml"), toml).unwrap();

    let manifest = package::package_static(&PackageOptions {
        project_dir: dir.clone(),
        target: Target::Site,
        format: PackageFormat::Static,
        release: true,
        variant: None,
        json: false,
    })
    .unwrap();

    let symbols = manifest
        .artifacts
        .iter()
        .find(|file| file.kind == "debug_symbols")
        .expect("debug symbols should be present");
    assert_eq!(symbols.platform.as_deref(), Some("ios"));
    assert_eq!(symbols.upload_provider.as_deref(), Some("crash-service"));
}

#[test]
fn distribution_readiness_warns_when_symbols_target_other_provider() {
    let dir = unique_dir("secondary-artifacts-other-provider");
    write_minimal_site(&dir);
    fs::create_dir_all(dir.join("release-content/symbols")).unwrap();
    fs::write(dir.join("release-content/symbols/app.dSYM.zip"), b"symbols").unwrap();
    let mut toml = fs::read_to_string(dir.join("fission.toml")).unwrap();
    toml.push_str(
        r#"
[[package.symbols]]
path = "release-content/symbols/app.dSYM.zip"
platform = "ios"
upload_provider = "crash-service"
"#,
    );
    fs::write(dir.join("fission.toml"), toml).unwrap();
    let artifact = package_silent(PackageOptions {
        project_dir: dir.clone(),
        target: Target::Site,
        format: PackageFormat::Static,
        release: true,
        variant: None,
        json: false,
    })
    .unwrap();

    let checks = distribution_readiness_checks(
        &dir,
        DistributionProvider::GithubReleases,
        "production",
        None,
        None,
        Some(&artifact),
    )
    .unwrap();
    let symbols = checks
        .iter()
        .find(|check| check.id == "release.distribution.debug_symbols_upload_state")
        .expect("distribution readiness should report symbol upload coverage");
    assert_eq!(symbols.severity, CheckSeverity::Warning);
    assert_eq!(symbols.status, CheckStatus::Warning);
    assert!(symbols
        .details
        .as_deref()
        .is_some_and(|details| details.contains("crash-service")));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn distribution_readiness_passes_when_selected_provider_uploads_symbols() {
    let dir = unique_dir("secondary-artifacts-selected-provider");
    write_minimal_site(&dir);
    fs::create_dir_all(dir.join("release-content/symbols")).unwrap();
    fs::write(dir.join("release-content/symbols/app.dSYM.zip"), b"symbols").unwrap();
    let mut toml = fs::read_to_string(dir.join("fission.toml")).unwrap();
    toml.push_str(
        r#"
[[package.symbols]]
path = "release-content/symbols/app.dSYM.zip"
platform = "ios"
upload_provider = "github-releases"
"#,
    );
    fs::write(dir.join("fission.toml"), toml).unwrap();
    let artifact = package_silent(PackageOptions {
        project_dir: dir.clone(),
        target: Target::Site,
        format: PackageFormat::Static,
        release: true,
        variant: None,
        json: false,
    })
    .unwrap();

    let checks = distribution_readiness_checks(
        &dir,
        DistributionProvider::GithubReleases,
        "production",
        None,
        None,
        Some(&artifact),
    )
    .unwrap();
    let symbols = checks
        .iter()
        .find(|check| check.id == "release.distribution.debug_symbols_upload_state")
        .expect("distribution readiness should report symbol upload coverage");
    assert_eq!(symbols.severity, CheckSeverity::Info);
    assert_eq!(symbols.status, CheckStatus::Passed);
    assert!(symbols
        .details
        .as_deref()
        .is_some_and(|details| details.contains("selected_provider_uploads=true")));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn github_pages_setup_writes_workflow() {
    let dir = unique_dir("github-setup");
    write_minimal_site(&dir);
    let config = load_publish_manifest(&dir).unwrap();
    setup_github_pages(
        &DistributeOptions {
            project_dir: dir.clone(),
            provider: DistributionProvider::GithubPages,
            action: DistributeAction::Setup,
            target: None,
            format: None,
            artifact: None,
            site: "production".to_string(),
            deploy: None,
            track: None,
            locales: Vec::new(),
            dry_run: false,
            yes: true,
            json: false,
        },
        &config,
    )
    .unwrap();
    let workflow = fs::read_to_string(dir.join(".github/workflows/fission-pages.yml")).unwrap();
    assert!(workflow.contains("actions/upload-pages-artifact"));
    assert!(workflow.contains("actions/deploy-pages"));
    assert!(workflow.contains("fission package"));
}
