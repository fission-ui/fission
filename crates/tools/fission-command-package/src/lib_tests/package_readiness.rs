use super::support::{unique_dir, write_minimal_site};
use super::*;

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
