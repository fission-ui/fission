use super::support::{unique_dir, write_minimal_site};
use super::*;

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
