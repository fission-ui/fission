use super::*;
use std::collections::BTreeSet;

fn metadata_package(name: &str, manifest_path: &str) -> CargoMetadataPackage {
    CargoMetadataPackage {
        name: name.into(),
        version: "0.1.0".into(),
        manifest_path: PathBuf::from(manifest_path),
        targets: Vec::new(),
    }
}

fn project() -> FissionProject {
    FissionProject {
        app: fission_command_core::AppConfig {
            name: "field-inspector".into(),
            app_id: "com.fission.examples.fieldinspector".into(),
            splash: None,
        },
        targets: BTreeSet::new(),
        capabilities: BTreeSet::new(),
        native: Default::default(),
    }
}

#[test]
fn linux_development_entry_uses_app_identity() {
    let entry = render_linux_desktop_entry(&project(), Path::new("/tmp/field-inspector"));
    assert!(entry.contains("Name=Field Inspector"));
    assert!(entry.contains("Icon=com.fission.examples.fieldinspector"));
    assert!(entry.contains("StartupWMClass=com.fission.examples.fieldinspector"));
}

#[test]
fn windows_development_manifest_uses_app_identity() {
    let manifest = render_windows_development_manifest(&project());
    assert!(manifest.contains("com.fission.examples.fieldinspector"));
    assert!(manifest.contains("PerMonitorV2"));
    assert!(manifest.contains("asInvoker"));
}

#[test]
fn macos_development_plist_uses_package_identity() {
    let binary = DesktopBinary {
        version: "1.2.3".into(),
        executable_name: "field-inspector".into(),
        path: PathBuf::from("/tmp/field-inspector"),
    };
    let config = MacosPackageConfig {
        bundle_id: Some("com.example.packaged".into()),
        minimum_os: Some("14.0".into()),
        ..Default::default()
    };

    let plist = render_macos_run_info_plist(&project(), &binary, "Field Inspector", &config);

    assert!(plist.contains("<string>com.example.packaged</string>"));
    assert!(plist.contains("<key>LSMinimumSystemVersion</key>\n  <string>14.0</string>"));
}

#[test]
fn macos_run_bundle_is_staged_outside_the_project_volume() {
    let project_dir = Path::new("/Volumes/My Shared Files/projects/example");

    let staging = macos_run_staging_root(project_dir, &project(), "debug");

    assert!(staging.starts_with(env::temp_dir()));
    assert!(!staging.starts_with(project_dir));
    assert!(staging.ends_with("debug"));
    assert!(staging
        .to_string_lossy()
        .contains("com.fission.examples.fieldinspector"));
}

#[test]
fn desktop_package_selection_uses_nested_project_manifest() {
    let metadata = CargoMetadata {
        packages: vec![
            metadata_package("workspace-first", "/workspace/crates/first/Cargo.toml"),
            metadata_package("desktop", "/workspace/crates/desktop/Cargo.toml"),
        ],
        target_directory: PathBuf::from("/workspace/target"),
    };

    let package =
        desktop_package_for_manifest(&metadata, Path::new("/workspace/crates/desktop/Cargo.toml"))
            .expect("desktop package should be selected");

    assert_eq!(package.name, "desktop");
}

#[test]
fn desktop_package_selection_does_not_fall_back_to_first_workspace_member() {
    let metadata = CargoMetadata {
        packages: vec![metadata_package(
            "workspace-first",
            "/workspace/crates/first/Cargo.toml",
        )],
        target_directory: PathBuf::from("/workspace/target"),
    };

    let error =
        desktop_package_for_manifest(&metadata, Path::new("/workspace/crates/desktop/Cargo.toml"))
            .expect_err("an unmatched project manifest must fail");

    assert!(error.to_string().contains("crates/desktop/Cargo.toml"));
}
