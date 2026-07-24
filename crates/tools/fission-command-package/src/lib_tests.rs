use super::*;

fn unique_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fission-publish-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn write_minimal_site(dir: &Path) {
    fs::create_dir_all(dir.join("content")).unwrap();
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::write(
        dir.join("assets/app-icon.png"),
        [
            0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H',
            b'D', b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, b'I', b'D', b'A', b'T', 0x78,
            0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00,
            0x00, 0x00, 0x00, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82,
        ],
    )
    .unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"targets = ["static-site"]

[app]
name = "site-demo"
app_id = "com.example.site_demo"

[site]
title = "Site Demo"
out_dir = "dist/site"
generate_sitemap = false
generate_robots = false

[distribution.github_pages.production]
owner = "example"
repo = "site-demo"
mode = "actions"
site_kind = "project"
base_path = "/site-demo/"

[distribution.github_releases.production]
owner = "example"
repo = "site-demo"
tag = "v1.2.3"
name = "Site Demo 1.2.3"
draft = true
prerelease = false
replace_assets = true
upload_artifact_manifest = true
"#,
    )
    .unwrap();
    fs::write(
        dir.join("content/index.md"),
        "---\ntitle: Home\n---\n# Home\n",
    )
    .unwrap();
}

#[test]
fn static_package_builds_artifact_manifest() {
    let dir = unique_dir("package");
    write_minimal_site(&dir);
    let manifest = package::package_static(&PackageOptions {
        project_dir: dir.clone(),
        target: Target::Site,
        format: PackageFormat::Static,
        release: true,
        variant: None,
        json: false,
    })
    .unwrap();
    assert_eq!(manifest.target, "static-site");
    assert!(dir
        .join("target/fission/release/static-site/static/artifact-manifest.json")
        .exists());
    assert!(manifest
        .artifacts
        .iter()
        .any(|file| file.relative_path == "index.html"));
    assert!(dir
        .join("target/fission/release/static-site/static/fission-route-manifest.json")
        .exists());
    assert!(dir
        .join("target/fission/release/static-site/static/fission-mime-map.json")
        .exists());
    assert!(manifest
        .validation
        .checks
        .iter()
        .any(|check| check.id == "release.package.artifact.primary_present"));
    assert!(manifest.validation.checks.iter().any(|check| {
        check.id == "release.package.static_load_smoke.receipt"
            && check.status == CheckStatus::Passed
    }));
    assert!(dir
        .join("target/fission/release/static-site/static/package-validation/load-smoke.json")
        .exists());
}

#[test]
fn artifact_manifest_uses_target_specific_release_version() {
    let dir = unique_dir("target-release-version");
    fs::create_dir_all(dir.join("out")).unwrap();
    fs::write(dir.join("out/app.ipa"), b"ipa").unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
name = "Demo"
app_id = "com.example.demo"
version = "1.0.0"
build = 7

[package.android]
version_name = "9.9.9"
version_code = 99

[package.ios]
marketing_version = "2.0.0"
build_number = "42"
"#,
    )
    .unwrap();
    let project = FissionProject {
        app: fission_command_core::AppConfig {
            name: "Demo".to_string(),
            app_id: "com.example.demo".to_string(),
            splash: None,
        },
        targets: std::collections::BTreeSet::from([Target::Ios]),
        capabilities: std::collections::BTreeSet::new(),
        native: fission_command_core::NativeConfig::default(),
    };

    let manifest = build_artifact_manifest(
        &project,
        &PackageOptions {
            project_dir: dir.clone(),
            target: Target::Ios,
            format: PackageFormat::Ipa,
            release: true,
            variant: None,
            json: false,
        },
        &dir.join("out"),
        "release",
    )
    .unwrap();

    assert_eq!(manifest.project.version.as_deref(), Some("2.0.0"));
    assert_eq!(manifest.project.build, Some(42));
}

#[test]
fn artifact_manifest_uses_macos_release_version() {
    let dir = unique_dir("macos-target-release-version");
    fs::create_dir_all(dir.join("out/Demo.app/Contents/MacOS")).unwrap();
    fs::write(dir.join("out/Demo.app/Contents/MacOS/demo"), b"bin").unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
name = "Demo"
app_id = "com.example.demo"
version = "1.0.0"
build = 7

[package.macos]
bundle_id = "com.example.demo"
marketing_version = "3.0.0"
build_number = "108"
"#,
    )
    .unwrap();
    let project = FissionProject {
        app: fission_command_core::AppConfig {
            name: "Demo".to_string(),
            app_id: "com.example.demo".to_string(),
            splash: None,
        },
        targets: std::collections::BTreeSet::from([Target::Macos]),
        capabilities: std::collections::BTreeSet::new(),
        native: fission_command_core::NativeConfig::default(),
    };

    let manifest = build_artifact_manifest(
        &project,
        &PackageOptions {
            project_dir: dir.clone(),
            target: Target::Macos,
            format: PackageFormat::App,
            release: true,
            variant: None,
            json: false,
        },
        &dir.join("out"),
        "release",
    )
    .unwrap();

    assert_eq!(manifest.project.version.as_deref(), Some("3.0.0"));
    assert_eq!(manifest.project.build, Some(108));
}

#[test]
fn artifact_manifest_records_icon_manifest() {
    let dir = unique_dir("icon-manifest");
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("out/Demo.app/Contents/Resources")).unwrap();
    fs::write(dir.join("assets/app-icon.png"), b"icon-source").unwrap();
    fs::write(
        dir.join("out/Demo.app/Contents/Resources/AppIcon.png"),
        b"icon-output",
    )
    .unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
name = "Demo"
app_id = "com.example.demo"
"#,
    )
    .unwrap();
    let project = FissionProject {
        app: fission_command_core::AppConfig {
            name: "Demo".to_string(),
            app_id: "com.example.demo".to_string(),
            splash: None,
        },
        targets: std::collections::BTreeSet::from([Target::Macos]),
        capabilities: std::collections::BTreeSet::new(),
        native: fission_command_core::NativeConfig::default(),
    };

    let manifest = build_artifact_manifest(
        &project,
        &PackageOptions {
            project_dir: dir.clone(),
            target: Target::Macos,
            format: PackageFormat::App,
            release: true,
            variant: None,
            json: false,
        },
        &dir.join("out"),
        "release",
    )
    .unwrap();

    let icon_manifest = manifest.icon_manifest.expect("icon manifest");
    assert_eq!(icon_manifest.outputs, 1);
    let path = PathBuf::from(&icon_manifest.path);
    assert!(path.exists());
    let value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(value["target"], "macos");
    assert_eq!(value["sources"][0]["path"], "assets/app-icon.png");
    assert_eq!(
        value["outputs"][0]["package_path"],
        "Demo.app/Contents/Resources/AppIcon.png"
    );
    assert!(manifest
        .source_config
        .iter()
        .any(|entry| entry.kind == "fission_manifest" && entry.path == "fission.toml"));
}

#[test]
fn signing_context_records_non_secret_identity() {
    let dir = unique_dir("signing-context");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[package.android]
keystore_alias = "upload"
"#,
    )
    .unwrap();
    let signing = package_signing_context(
        &dir,
        Target::Android,
        PackageFormat::Aab,
        false,
        None,
        &[ReadinessCheck {
            id: "release.package.signature.android_aab".to_string(),
            severity: CheckSeverity::Warning,
            status: CheckStatus::Passed,
            summary: "signed".to_string(),
            details: None,
            remediation: Vec::new(),
        }],
    )
    .unwrap()
    .expect("signing context");

    assert_eq!(signing.state, "signed");
    assert_eq!(signing.identity.as_deref(), Some("upload"));
    assert_eq!(signing.certificate_sha256, None);
}

#[test]
fn macos_artifact_context_uses_release_signing_overlay_only_for_release() {
    let dir = unique_dir("macos-release-signing-context");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"
[package.macos]
bundle_id = "com.example.demo"

[package.macos.release]
signing_identity = "Developer ID Application: Example Ltd"
installer_identity = "Developer ID Installer: Example Ltd"
notarize = true
"#,
    )
    .unwrap();

    assert_eq!(
        package_signing_identity(&dir, Target::Macos, PackageFormat::App, false, None).unwrap(),
        None
    );
    assert_eq!(
        package_signing_identity(&dir, Target::Macos, PackageFormat::App, true, None).unwrap(),
        Some("Developer ID Application: Example Ltd".to_string())
    );
    assert_eq!(
        package_signing_identity(&dir, Target::Macos, PackageFormat::Pkg, true, None).unwrap(),
        Some("Developer ID Installer: Example Ltd".to_string())
    );
    assert!(
        package_notarization_context(&dir, Target::Macos, false, None)
            .unwrap()
            .is_none()
    );
    assert!(
        package_notarization_context(&dir, Target::Macos, true, None)
            .unwrap()
            .is_some()
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn macos_app_store_package_uses_productbuild_component_archive() {
    let app = Path::new("/tmp/Developer Defence.app");
    let pkg = Path::new("/tmp/Developer-Defence.pkg");
    let config = fission_command_core::MacosPackageConfig {
        installer_identity: Some("3rd Party Mac Developer Installer: Example Ltd".to_string()),
        pkg_builder: Some("productbuild".to_string()),
        ..Default::default()
    };

    let (builder, arguments) = package::macos_pkg_builder_command(app, pkg, None, &config).unwrap();

    assert_eq!(builder, "productbuild");
    assert_eq!(
        arguments,
        [
            "--sign",
            "3rd Party Mac Developer Installer: Example Ltd",
            "--component",
            "/tmp/Developer Defence.app",
            "/Applications",
            "/tmp/Developer-Defence.pkg",
        ]
        .map(std::ffi::OsString::from)
    );
}

#[test]
fn macos_developer_id_package_uses_a_non_relocatable_component_root() {
    let app = Path::new("/tmp/app-staging/Developer Defence.app");
    let pkg = Path::new("/tmp/Developer-Defence.pkg");
    let component_plist = Path::new("/tmp/components.plist");
    let config = fission_command_core::MacosPackageConfig {
        installer_identity: Some("Developer ID Installer: Example Ltd".to_string()),
        ..Default::default()
    };

    let (builder, arguments) =
        package::macos_pkg_builder_command(app, pkg, Some(component_plist), &config).unwrap();

    assert_eq!(builder, "pkgbuild");
    assert_eq!(
        arguments,
        [
            "--root",
            "/tmp/app-staging",
            "--install-location",
            "/Applications",
            "--component-plist",
            "/tmp/components.plist",
            "--sign",
            "Developer ID Installer: Example Ltd",
            "/tmp/Developer-Defence.pkg",
        ]
        .map(std::ffi::OsString::from)
    );
}

#[test]
fn macos_component_plist_disables_bundle_relocation() {
    let dir = unique_dir("macos-component-plist");
    let app = dir.join("app-staging/Developer & Defence.app");
    fs::create_dir_all(&app).unwrap();

    let component_plist = package::write_macos_component_plist(&dir, &app).unwrap();
    let contents = fs::read_to_string(component_plist).unwrap();

    assert!(contents.contains("<string>Developer &amp; Defence.app</string>"));
    assert!(contents.contains("<key>BundleIsRelocatable</key>\n    <false/>"));
    assert!(contents.contains("<key>BundleIsVersionChecked</key>\n    <false/>"));
    assert!(contents.contains("<key>BundleHasStrictIdentifier</key>\n    <true/>"));
    assert!(contents.contains("<key>BundleOverwriteAction</key>\n    <string>upgrade</string>"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn macos_info_plist_includes_configured_application_category() {
    let config = fission_command_core::MacosPackageConfig {
        application_category: Some("public.app-category.developer-tools".to_string()),
        ..Default::default()
    };

    assert_eq!(
        package::render_macos_application_category_entry(&config),
        "  <key>LSApplicationCategoryType</key>\n  <string>public.app-category.developer-tools</string>"
    );
}

#[test]
fn app_store_status_prefers_review_submission_state_over_build_processing() {
    let status = stores::app_store_observed_status(
        &json!({
            "data": [{
                "attributes": { "processingState": "VALID" }
            }]
        }),
        &json!({
            "data": [{
                "attributes": { "state": "WAITING_FOR_REVIEW" }
            }]
        }),
        &json!({ "data": [] }),
    );
    assert_eq!(status, "waiting_for_review");
}

#[test]
fn app_store_status_queries_the_sortable_global_builds_endpoint() {
    let url = stores::app_store_builds_status_url("6794005791");

    assert!(url.contains("/v1/builds?filter[app]=6794005791"));
    assert!(url.contains("sort=-uploadedDate"));
    assert!(!url.contains("/v1/apps/6794005791/builds"));
}

#[test]
fn app_store_status_reports_build_upload_failures_before_a_build_exists() {
    let status = stores::app_store_observed_status(
        &json!({ "data": [] }),
        &json!({ "data": [] }),
        &json!({
            "data": [{
                "attributes": {
                    "state": {
                        "state": "FAILED",
                        "errors": [{ "code": "91109" }]
                    }
                }
            }]
        }),
    );

    assert_eq!(status, "failed");
}

#[test]
fn app_store_build_upload_status_uses_the_app_relationship_endpoint() {
    let url = stores::app_store_build_uploads_status_url("6794005791");

    assert!(url.contains("/v1/apps/6794005791/buildUploads"));
    assert!(url.contains("sort=-uploadedDate"));
    assert!(url.contains("fields[buildUploads]"));
}

#[test]
fn app_store_upload_follow_up_points_to_review_submission_command() {
    let msg = stores::app_store_upload_follow_up(
        "app-store-review",
        Path::new("target/fission/release/ios/ipa/artifact-manifest.json"),
    );
    assert!(
        msg.contains("fission distribute promote --provider app-store --track app-store-review")
    );
}

#[test]
fn provider_stdio_line_events_are_bounded_and_structured() {
    let mut events = Vec::new();
    push_provider_stdio_line_events(&mut events, "provider.stdout.line", "one\n\ntwo\n");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].id, "provider.stdout.line");
    assert_eq!(events[0].details.as_deref(), Some("one"));
    assert_eq!(events[1].details.as_deref(), Some("two"));
}

#[test]
fn distribute_publish_outcome_records_structured_events() {
    let dir = unique_dir("publish-events");
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
            json: true,
        },
        &config,
    )
    .unwrap();

    let outcome = distribute_publish_outcome(DistributeOptions {
        project_dir: dir,
        provider: DistributionProvider::GithubPages,
        action: DistributeAction::Publish,
        target: Some(Target::Site),
        format: Some(PackageFormat::Static),
        artifact: Some(artifact),
        site: "production".to_string(),
        deploy: None,
        track: None,
        locales: Vec::new(),
        dry_run: true,
        yes: true,
        json: true,
    })
    .unwrap();

    let event_ids = outcome
        .events
        .iter()
        .map(|event| event.id.as_str())
        .collect::<Vec<_>>();
    assert!(event_ids.contains(&"distribution.config"));
    assert!(event_ids.contains(&"distribution.artifact_manifest"));
    assert!(event_ids.contains(&"distribution.readiness"));
    assert!(event_ids.contains(&"provider.publish"));
    assert!(event_ids.contains(&"provider.request"));
    assert!(event_ids.contains(&"provider.response"));
    assert!(event_ids.contains(&"provider.upload_plan"));
    assert!(event_ids.contains(&"provider.uploaded_asset"));
    assert!(event_ids.contains(&"distribution.receipt"));
    let receipt_event_ids = outcome.receipt["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|event| event.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(receipt_event_ids.contains(&"provider.request"));
    assert!(receipt_event_ids.contains(&"provider.response"));
    assert!(receipt_event_ids.contains(&"provider.uploaded_asset"));
    assert!(receipt_event_ids.contains(&"distribution.receipt"));
    assert_eq!(outcome.receipt["status"], "workflow-required");
}

#[test]
fn distribute_setup_writes_action_receipt() {
    let dir = unique_dir("setup-receipt");
    write_minimal_site(&dir);

    distribute(DistributeOptions {
        project_dir: dir.clone(),
        provider: DistributionProvider::GithubPages,
        action: DistributeAction::Setup,
        target: Some(Target::Site),
        format: Some(PackageFormat::Static),
        artifact: None,
        site: "production".to_string(),
        deploy: None,
        track: None,
        locales: vec!["en-US".to_string()],
        dry_run: false,
        yes: true,
        json: true,
    })
    .unwrap();

    let receipt_dir = dir.join("target/fission/distribution/github-pages/production");
    let receipt_path = fs::read_dir(&receipt_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("setup-") && name.ends_with(".json"))
        })
        .unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(receipt_path).unwrap()).unwrap();

    assert_eq!(
        value.get("status").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(value.get("action").and_then(Value::as_str), Some("setup"));
    let event_ids = value
        .get("events")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|event| event.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(event_ids.contains(&"distribution.config"));
    assert!(event_ids.contains(&"distribution.setup"));
    assert!(event_ids.contains(&"distribution.receipt"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn package_readiness_reports_release_profile_when_requested() {
    let dir = unique_dir("package-readiness-release-profile");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
name = "Demo"
app_id = "com.example.demo"
targets = ["terminal"]
"#,
    )
    .unwrap();

    let checks =
        readiness::readiness_package(&dir, Some(Target::Terminal), Some(PackageFormat::Run), true)
            .unwrap();

    let profile = checks
        .iter()
        .find(|check| check.id == "release.package.profile_selected")
        .unwrap();
    assert_eq!(profile.details.as_deref(), Some("release"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_package_readiness_requires_resolved_version_and_build() {
    let dir = unique_dir("package-readiness-version-build");
    write_minimal_site(&dir);

    let checks =
        readiness::readiness_package(&dir, Some(Target::Site), Some(PackageFormat::Static), true)
            .unwrap();

    let version = checks
        .iter()
        .find(|check| check.id == "release.package.version_resolved")
        .expect("package readiness should check release version");
    let build = checks
        .iter()
        .find(|check| check.id == "release.package.build_resolved")
        .expect("package readiness should check release build");

    assert_eq!(version.severity, CheckSeverity::Error);
    assert_eq!(build.severity, CheckSeverity::Error);
    assert_eq!(build.status, CheckStatus::Missing);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn package_readiness_checks_output_location_writable() {
    let dir = unique_dir("package-readiness-output-writable");
    write_minimal_site(&dir);

    let checks =
        readiness::readiness_package(&dir, Some(Target::Site), Some(PackageFormat::Static), true)
            .unwrap();
    let output = checks
        .iter()
        .find(|check| check.id == "release.package.output_writable")
        .expect("package readiness should check package output writability");

    assert_eq!(output.severity, CheckSeverity::Error);
    assert_eq!(output.status, CheckStatus::Passed);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn package_readiness_passes_when_existing_manifest_is_current() {
    let dir = unique_dir("package-existing-manifest-current");
    write_minimal_site(&dir);
    package_silent(PackageOptions {
        project_dir: dir.clone(),
        target: Target::Site,
        format: PackageFormat::Static,
        release: true,
        variant: None,
        json: false,
    })
    .unwrap();

    let checks = package_readiness_checks_for_profile(
        &dir,
        Some(Target::Site),
        Some(PackageFormat::Static),
        true,
    )
    .unwrap();
    let manifest = checks
        .iter()
        .find(|check| check.id == "release.package.existing_artifact_manifest_current")
        .expect("package readiness should report existing artifact manifest state");
    assert_eq!(manifest.severity, CheckSeverity::Info);
    assert_eq!(manifest.status, CheckStatus::Passed);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn package_readiness_warns_when_existing_manifest_is_stale() {
    let dir = unique_dir("package-existing-manifest-stale");
    write_minimal_site(&dir);
    package_silent(PackageOptions {
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

    let checks = package_readiness_checks_for_profile(
        &dir,
        Some(Target::Site),
        Some(PackageFormat::Static),
        true,
    )
    .unwrap();
    let manifest = checks
        .iter()
        .find(|check| check.id == "release.package.existing_artifact_manifest_current")
        .expect("package readiness should report existing artifact manifest state");
    assert_eq!(manifest.severity, CheckSeverity::Warning);
    assert_eq!(manifest.status, CheckStatus::Warning);
    assert!(manifest
        .details
        .as_deref()
        .is_some_and(|details| details.contains("fission.toml changed")));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn package_readiness_checks_target_identity_matches_app_id() {
    let dir = unique_dir("package-readiness-target-identity");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"targets = ["android"]

[app]
name = "Demo"
app_id = "com.example.demo"
version = "1.2.3"
build = 42

[package.android]
package_name = "com.other.demo"
version_name = "1.2.3"
version_code = 42
"#,
    )
    .unwrap();

    let checks =
        readiness::readiness_package(&dir, Some(Target::Android), Some(PackageFormat::Aab), true)
            .unwrap();
    let identity = checks
        .iter()
        .find(|check| check.id == "release.package.android.package_name_matches_app_id")
        .expect("package readiness should check Android package identity");

    assert_eq!(identity.severity, CheckSeverity::Error);
    assert_eq!(identity.status, CheckStatus::Failed);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn package_readiness_checks_macos_bundle_id_matches_app_id() {
    let dir = unique_dir("package-readiness-macos-identity");
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::write(
        dir.join("assets/app-icon.png"),
        [
            0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H',
            b'D', b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, b'I', b'D', b'A', b'T', 0x78,
            0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00,
            0x00, 0x00, 0x00, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82,
        ],
    )
    .unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"targets = ["macos"]

[app]
name = "Demo"
app_id = "com.example.demo"
version = "1.2.3"
build = 42

[package.macos]
bundle_id = "com.other.demo"
marketing_version = "1.2.3"
build_number = "42"
"#,
    )
    .unwrap();

    let checks =
        readiness::readiness_package(&dir, Some(Target::Macos), Some(PackageFormat::App), true)
            .unwrap();
    let identity = checks
        .iter()
        .find(|check| check.id == "release.package.macos.bundle_id_matches_app_id")
        .expect("package readiness should check macOS package identity");

    assert_eq!(identity.severity, CheckSeverity::Error);
    assert_eq!(identity.status, CheckStatus::Failed);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn json_package_readiness_fails_when_blocked() {
    let dir = unique_dir("json-readiness-blocked");
    fs::create_dir_all(&dir).unwrap();

    let error = readiness(ReadinessOptions {
        project_dir: dir.clone(),
        kind: ReadinessKind::Package,
        target: Some(Target::Site),
        format: Some(PackageFormat::Apk),
        provider: None,
        artifact: None,
        site: "production".to_string(),
        track: None,
        release: false,
        json: true,
    })
    .unwrap_err()
    .to_string();

    assert!(error.contains("readiness checks failed"));

    let _ = fs::remove_dir_all(&dir);
}

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

#[test]
fn report_status_keeps_recommended_missing_checks_visible() {
    let checks = vec![ReadinessCheck {
        id: "release_content.play_store.feature_graphic".to_string(),
        severity: CheckSeverity::Warning,
        status: CheckStatus::Missing,
        summary: "feature graphic exists".to_string(),
        details: None,
        remediation: Vec::new(),
    }];

    assert_eq!(report_status(&checks), "warning");
}

#[test]
fn failed_distribution_receipt_records_stage_error() {
    let dir = unique_dir("failed-distribution-receipt");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
name = "site-demo"
app_id = "com.example.site_demo"
version = "1.2.3"
build = 4
"#,
    )
    .unwrap();
    let options = DistributeOptions {
        project_dir: dir.clone(),
        provider: DistributionProvider::S3,
        action: DistributeAction::Publish,
        target: Some(Target::Site),
        format: Some(PackageFormat::Static),
        artifact: None,
        site: "production".to_string(),
        deploy: None,
        track: None,
        locales: vec!["en-US".to_string()],
        dry_run: false,
        yes: true,
        json: true,
    };
    let mut events = Vec::new();

    let path = write_failed_distribution_receipt(
        &options,
        &mut events,
        "distribution.readiness",
        "S3 bucket is missing",
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();

    assert_eq!(value.get("status").and_then(Value::as_str), Some("failed"));
    assert_eq!(value.get("provider").and_then(Value::as_str), Some("s3"));
    assert_eq!(
        value.get("stderr").and_then(Value::as_str),
        Some("S3 bucket is missing")
    );
    let event_ids = value
        .get("events")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|event| event.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert_eq!(
        event_ids,
        vec![
            "distribution.readiness",
            "distribution.failed",
            "distribution.receipt"
        ]
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn distribution_receipt_paths_do_not_overwrite_same_second_receipts() {
    let dir = unique_dir("receipt-path-unique");
    let receipt = DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: 123,
        provider: "s3".to_string(),
        site: "production".to_string(),
        action: "publish".to_string(),
        artifact_manifest: None,
        deployment_id: None,
        canonical_url: None,
        preview_url: None,
        custom_domain: None,
        status: "published".to_string(),
        stdout: None,
        stderr: None,
        manual_follow_up: Vec::new(),
    };
    let first = receipt_output_path(&dir, &receipt);
    fs::create_dir_all(first.parent().unwrap()).unwrap();
    fs::write(&first, "{}").unwrap();

    let second = receipt_output_path(&dir, &receipt);

    assert_ne!(first, second);
    assert!(second
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "publish-123-2.json"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn distribute_status_failure_writes_failed_receipt() {
    let dir = unique_dir("failed-distribution-status");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
name = "site-demo"
app_id = "com.example.site_demo"
version = "1.2.3"
build = 4
"#,
    )
    .unwrap();
    let options = DistributeOptions {
        project_dir: dir.clone(),
        provider: DistributionProvider::S3,
        action: DistributeAction::Status,
        target: Some(Target::Site),
        format: Some(PackageFormat::Static),
        artifact: None,
        site: "production".to_string(),
        deploy: None,
        track: None,
        locales: vec!["en-US".to_string()],
        dry_run: false,
        yes: true,
        json: true,
    };

    let error = distribute_status_outcome(options).unwrap_err().to_string();

    assert!(error.contains("distribution status failed"));
    assert!(error.contains("distribution receipt:"));
    let receipt_dir = dir.join("target/fission/distribution/s3/production");
    let receipt_path = fs::read_dir(&receipt_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("status-") && name.ends_with(".json"))
        })
        .unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(receipt_path).unwrap()).unwrap();

    assert_eq!(value.get("status").and_then(Value::as_str), Some("failed"));
    assert_eq!(value.get("action").and_then(Value::as_str), Some("status"));
    let event_ids = value
        .get("events")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|event| event.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(event_ids.contains(&"distribution.config"));
    assert!(event_ids.contains(&"provider.status"));
    assert!(event_ids.contains(&"distribution.failed"));
    assert!(event_ids.contains(&"distribution.receipt"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn distribute_lifecycle_failure_writes_failed_receipt() {
    let dir = unique_dir("failed-distribution-lifecycle");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[app]
name = "site-demo"
app_id = "com.example.site_demo"
version = "1.2.3"
build = 4
"#,
    )
    .unwrap();
    let options = DistributeOptions {
        project_dir: dir.clone(),
        provider: DistributionProvider::S3,
        action: DistributeAction::Promote,
        target: Some(Target::Site),
        format: Some(PackageFormat::Static),
        artifact: None,
        site: "production".to_string(),
        deploy: None,
        track: None,
        locales: vec!["en-US".to_string()],
        dry_run: false,
        yes: true,
        json: true,
    };

    let error = distribute(options).unwrap_err().to_string();

    assert!(error.contains("promote failed"));
    assert!(error.contains("distribution receipt:"));
    let receipt_dir = dir.join("target/fission/distribution/s3/production");
    let receipt_path = fs::read_dir(&receipt_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("promote-") && name.ends_with(".json"))
        })
        .unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(receipt_path).unwrap()).unwrap();

    assert_eq!(value.get("status").and_then(Value::as_str), Some("failed"));
    assert_eq!(value.get("action").and_then(Value::as_str), Some("promote"));
    let event_ids = value
        .get("events")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|event| event.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(event_ids.contains(&"distribution.config"));
    assert!(event_ids.contains(&"provider.lifecycle"));
    assert!(event_ids.contains(&"distribution.failed"));

    let _ = fs::remove_dir_all(&dir);
}
