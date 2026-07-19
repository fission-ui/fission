use super::package_validation::{
    package_artifact_checks, package_install_smoke_check, prepare_package_validation_inputs,
};
use super::*;
use fission_command_core::AppConfig;
use std::collections::BTreeSet;
use std::process::Command;

#[test]
fn macos_info_plist_includes_capability_usage_descriptions() {
    let project = FissionProject {
        app: AppConfig {
            name: "demo".to_string(),
            app_id: "com.example.demo".to_string(),
            splash: None,
        },
        targets: BTreeSet::from([Target::Macos]),
        capabilities: BTreeSet::from([
            PlatformCapability::BarcodeScanner,
            PlatformCapability::Bluetooth,
            PlatformCapability::Geolocation,
            PlatformCapability::Microphone,
        ]),
        native: Default::default(),
    };

    let plist = render_info_plist(
        &project,
        "Demo",
        "demo",
        &MacosPackageConfig::default(),
        "1.2.3",
        "42",
    );

    assert!(plist.contains("NSBluetoothAlwaysUsageDescription"));
    assert!(plist.contains("NSCameraUsageDescription"));
    assert!(plist.contains("NSLocationWhenInUseUsageDescription"));
    assert!(plist.contains("NSMicrophoneUsageDescription"));
    assert!(plist.contains("<key>CFBundleShortVersionString</key>\n  <string>1.2.3</string>"));
    assert!(plist.contains("<key>CFBundleVersion</key>\n  <string>42</string>"));
}

#[test]
fn server_dockerfile_builds_workspace_package_and_artifacts() {
    let dockerfile = render_server_dockerfile(
        "debian:bookworm-slim",
        8080,
        "examples/pokemon-card-store",
        "pokemon-card-store",
        "pokemon-card-store",
        " --release --package-no-default-features --package-feature browser",
    );

    assert!(dockerfile.contains("COPY workspace/ ."));
    assert!(dockerfile.contains("WORKDIR /workspace/examples/pokemon-card-store"));
    assert!(dockerfile.contains("rustup target add wasm32-unknown-unknown"));
    assert!(dockerfile
        .contains("cargo build --release --package pokemon-card-store --bin pokemon-card-store"));
    assert!(dockerfile.contains("artifacts --package-name pokemon-card-store --release --package-no-default-features --package-feature browser"));
    assert!(dockerfile.contains(
        "COPY --from=builder /workspace/examples/pokemon-card-store/fission.toml /app/fission.toml"
    ));
    assert!(dockerfile.contains("ENV FISSION_SERVER_ARTIFACTS=/app/server-artifacts"));
    assert!(
        dockerfile.contains("CMD [\"sh\", \"-c\", \"exec /usr/local/bin/pokemon-card-store serve")
    );
}

#[test]
fn static_site_docker_context_can_generate_axum_server_crate() {
    let root = std::env::temp_dir().join(format!(
        "fission-static-docker-context-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    write_static_server_crate(&root, DockerStaticAdapter::Axum).unwrap();

    let manifest = fs::read_to_string(root.join("server/Cargo.toml")).unwrap();
    let main = fs::read_to_string(root.join("server/src/main.rs")).unwrap();
    assert!(manifest.contains("tower-http"));
    assert!(main.contains("ServeDir::new"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn docker_source_copy_skips_tmp_and_target_directories() {
    let root =
        std::env::temp_dir().join(format!("fission-docker-source-copy-{}", std::process::id()));
    let source = root.join("source");
    let dest = root.join("dest");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(source.join(".tmp/cache")).unwrap();
    fs::create_dir_all(source.join("target/debug")).unwrap();
    fs::create_dir_all(source.join("platforms/android/build")).unwrap();
    fs::write(source.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
    fs::write(source.join(".tmp/cache/secret"), "do not copy").unwrap();
    fs::write(source.join("target/debug/app"), "do not copy").unwrap();
    fs::write(
        source.join("platforms/android/build/app.apk"),
        "do not copy",
    )
    .unwrap();

    copy_docker_source_tree(&source, &dest).unwrap();

    assert!(dest.join("Cargo.toml").exists());
    assert!(!dest.join(".tmp").exists());
    assert!(!dest.join("target").exists());
    assert!(!dest.join("platforms").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn docker_image_name_sanitizes_human_app_names() {
    assert_eq!(
        sanitize_docker_image_name("Pokemon Card Store!"),
        "pokemon-card-store"
    );
    assert_eq!(sanitize_docker_image_name("___"), "fission-app");
}

#[test]
fn install_smoke_receipt_must_match_target_format_and_status() {
    let root = std::env::temp_dir().join(format!("fission-install-smoke-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    fs::write(
        root.join("install-smoke.json"),
        r#"{"target":"android","format":"apk","status":"passed"}"#,
    )
    .unwrap();
    let check = package_install_smoke_check(Target::Android, PackageFormat::Apk, &root);
    assert_eq!(check.status, CheckStatus::Passed);

    fs::write(
        root.join("install-smoke.json"),
        r#"{"target":"android","format":"aab","status":"passed"}"#,
    )
    .unwrap();
    let check = package_install_smoke_check(Target::Android, PackageFormat::Apk, &root);
    assert_eq!(check.status, CheckStatus::Failed);

    fs::write(
        root.join("install-smoke.json"),
        r#"{"target":"android","format":"apk","status":"failed"}"#,
    )
    .unwrap();
    let check = package_install_smoke_check(Target::Android, PackageFormat::Apk, &root);
    assert_eq!(check.status, CheckStatus::Failed);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn package_structure_checks_validate_common_store_artifacts() {
    let root =
        std::env::temp_dir().join(format!("fission-package-structure-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    let apk = root.join("app.apk");
    write_zip(&apk, &["AndroidManifest.xml"]);
    assert_structure_passes(&root, Target::Android, PackageFormat::Apk, &apk);

    let aab = root.join("app.aab");
    write_zip(
        &aab,
        &["BundleConfig.pb", "base/manifest/AndroidManifest.xml"],
    );
    assert_structure_passes(&root, Target::Android, PackageFormat::Aab, &aab);

    let ipa = root.join("app.ipa");
    write_zip(&ipa, &["Payload/Demo.app/Info.plist"]);
    assert_structure_passes(&root, Target::Ios, PackageFormat::Ipa, &ipa);

    let msix = root.join("app.msix");
    write_zip(&msix, &["AppxManifest.xml"]);
    assert_structure_passes(&root, Target::Windows, PackageFormat::Msix, &msix);

    let app = root.join("Demo.app");
    fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
    fs::write(app.join("Contents/Info.plist"), "<plist/>").unwrap();
    let checks = package_artifact_checks(
        &PackageOptions {
            project_dir: root.clone(),
            target: Target::Macos,
            format: PackageFormat::App,
            release: true,
            variant: None,
            json: false,
        },
        &root,
        &manifest_for(PackageFormat::App, &app),
    );
    assert!(checks.iter().any(|check| {
        check.id == "release.package.structure.macos_app" && check.status == CheckStatus::Passed
    }));

    let broken = root.join("broken.apk");
    write_zip(&broken, &["classes.dex"]);
    let checks = structure_checks(&root, Target::Android, PackageFormat::Apk, &broken);
    assert!(checks.iter().any(|check| {
        check.id == "release.package.structure.android_apk" && check.status == CheckStatus::Failed
    }));

    let _ = fs::remove_dir_all(&root);
}

fn assert_structure_passes(root: &Path, target: Target, format: PackageFormat, artifact: &Path) {
    let checks = structure_checks(root, target, format, artifact);
    assert!(checks.iter().any(|check| {
        check.id.starts_with("release.package.structure.") && check.status == CheckStatus::Passed
    }));
}

fn structure_checks(
    root: &Path,
    target: Target,
    format: PackageFormat,
    artifact: &Path,
) -> Vec<ReadinessCheck> {
    package_artifact_checks(
        &PackageOptions {
            project_dir: root.to_path_buf(),
            target,
            format,
            release: true,
            variant: None,
            json: false,
        },
        root,
        &manifest_for(format, artifact),
    )
}

fn manifest_for(format: PackageFormat, artifact: &Path) -> ArtifactManifest {
    ArtifactManifest {
        schema_version: 1,
        created_at_unix_seconds: 0,
        project: ArtifactProject {
            app_id: "com.example.demo".to_string(),
            name: "Demo".to_string(),
            build: Some(1),
            version: Some("1.0.0".to_string()),
        },
        target: "test".to_string(),
        format: format.as_str().to_string(),
        profile: "release".to_string(),
        variant: None,
        root_dir: artifact
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .display()
            .to_string(),
        source_config: Vec::new(),
        artifacts: vec![ArtifactFile {
            kind: "primary".to_string(),
            purpose: None,
            platform: None,
            upload_provider: None,
            path: artifact.display().to_string(),
            relative_path: artifact
                .file_name()
                .and_then(OsStr::to_str)
                .unwrap_or("artifact")
                .to_string(),
            sha256: "0".repeat(64),
            size_bytes: fs::metadata(artifact).map(|m| m.len()).unwrap_or(1),
            mime_type: "application/octet-stream".to_string(),
        }],
        icon_manifest: None,
        signing: None,
        notarization: None,
        validation: ArtifactValidation {
            state: "pending".to_string(),
            checks: Vec::new(),
        },
    }
}

fn write_zip(path: &Path, entries: &[&str]) {
    let file = fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    for entry in entries {
        zip.start_file(*entry, options).unwrap();
        zip.write_all(b"fixture").unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn manifest_validation_state_warns_on_warning_severity_failures() {
    let checks = vec![check(
        "release.package.install_smoke.receipt",
        CheckSeverity::Warning,
        CheckStatus::Failed,
        "package install smoke receipt proves target and format passed",
        Some("status=failed".to_string()),
        vec!["Run the platform install/smoke workflow."],
    )];

    assert_eq!(manifest_validation_state(&checks), "warning");
}

#[test]
fn linux_run_package_supports_verify_install_and_uninstall() {
    let root =
        std::env::temp_dir().join(format!("fission-run-script-smoke-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let payload = root.join("payload");
    let install = root.join("install");
    fs::create_dir_all(&payload).unwrap();
    fs::write(payload.join("demo"), "#!/bin/sh\nexit 0\n").unwrap();
    let run_path = root.join("demo.run");

    write_linux_run(&payload, &run_path, "demo", "demo").unwrap();

    let verify = Command::new("sh")
        .arg(&run_path)
        .arg("--verify")
        .output()
        .unwrap();
    assert!(
        verify.status.success(),
        "verify failed: {}",
        String::from_utf8_lossy(&verify.stderr)
    );

    let install_output = Command::new("sh")
        .arg(&run_path)
        .arg("--install")
        .env("FISSION_INSTALL_DIR", &install)
        .output()
        .unwrap();
    assert!(
        install_output.status.success(),
        "install failed: {}",
        String::from_utf8_lossy(&install_output.stderr)
    );
    assert!(install.join("demo").exists());
    assert!(install.join(".fission-install-receipt").exists());

    let uninstall = Command::new("sh")
        .arg(&run_path)
        .arg("--uninstall")
        .env("FISSION_INSTALL_DIR", &install)
        .output()
        .unwrap();
    assert!(
        uninstall.status.success(),
        "uninstall failed: {}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    assert!(!install.exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn terminal_run_package_validation_writes_passing_install_smoke_receipt() {
    let root = std::env::temp_dir().join(format!(
        "fission-run-validation-smoke-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let project_dir = root.join("project");
    let staging = root.join("staging");
    let payload = root.join("payload");
    fs::create_dir_all(&project_dir).unwrap();
    fs::create_dir_all(&staging).unwrap();
    fs::create_dir_all(&payload).unwrap();
    fs::write(payload.join("demo"), "#!/bin/sh\nexit 0\n").unwrap();
    write_linux_run(&payload, &staging.join("demo.run"), "demo", "demo").unwrap();

    let options = PackageOptions {
        project_dir,
        target: Target::Terminal,
        format: PackageFormat::Run,
        release: true,
        variant: None,
        json: false,
    };
    prepare_package_validation_inputs(&options, &staging).unwrap();

    let check = package_install_smoke_check(Target::Terminal, PackageFormat::Run, &staging);
    assert_eq!(check.status, CheckStatus::Passed);
    assert!(staging
        .join("package-validation/install-smoke.json")
        .exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn msix_native_manifest_excludes_driver_packages() {
    let root = std::env::temp_dir().join(format!(
        "fission-windows-native-manifest-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let products = vec![
        BuiltWindowsNativeProduct {
            module: "security".into(),
            name: "provider".into(),
            kind: NativeWindowsProductKind::Runtime,
            source: root.join("provider.dll"),
            destination: PathBuf::from("native/provider.dll"),
        },
        BuiltWindowsNativeProduct {
            module: "security".into(),
            name: "minifilter".into(),
            kind: NativeWindowsProductKind::DriverPackage,
            source: root.join("driver"),
            destination: PathBuf::from("driver"),
        },
    ];

    let manifest =
        write_windows_native_products_manifest(&root, &products, false, "release", "msix", None)
            .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&fs::read(manifest).unwrap()).unwrap();
    let products = value["products"].as_array().unwrap();

    assert_eq!(products.len(), 1);
    assert_eq!(products[0]["kind"], "runtime");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn custom_packager_receives_selected_variant() {
    let root = std::env::temp_dir().join(format!(
        "fission-package-variant-env-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let script = root.join("package.sh");
    fs::write(
        &script,
        "#!/bin/sh\nprintf '%s' \"$FISSION_VARIANT\" > selected-variant.txt\ntouch output.pkg\nprintf '%s\\n' output.pkg\n",
    )
    .unwrap();
    let variant: fission_command_core::NativeVariant = "scanner".parse().unwrap();

    let output = run_packaging_script_with_env(&root, &script, true, Some(&variant), &[])
        .unwrap()
        .unwrap();

    assert_eq!(output, root.join("output.pkg"));
    assert_eq!(
        fs::read_to_string(root.join("selected-variant.txt")).unwrap(),
        "scanner"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn linux_native_manifest_records_staged_product_digest() {
    let root = std::env::temp_dir().join(format!(
        "fission-linux-native-manifest-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let staged = root.join("libexec/mount-helper");
    fs::create_dir_all(staged.parent().unwrap()).unwrap();
    fs::write(&staged, b"helper").unwrap();
    let products = vec![BuiltLinuxNativeProduct {
        module: "security".into(),
        name: "mount-helper".into(),
        kind: NativeLinuxProductKind::PrivilegedHelper,
        source: root.join("build/mount-helper"),
        destination: PathBuf::from("libexec/mount-helper"),
    }];

    let manifest =
        write_linux_native_products_manifest(&root, &products, "release", "run").unwrap();
    let value: serde_json::Value = serde_json::from_slice(&fs::read(manifest).unwrap()).unwrap();

    assert_eq!(value["products"][0]["kind"], "privileged-helper");
    assert_eq!(value["products"][0]["size_bytes"], 6);
    assert_eq!(
        value["products"][0]["sha256"],
        "e81d3b0e9d82feaaf5f6e55bdff24731d7eee08632ffa63801e6397290c5d20a"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn windows_package_config_reads_exe_installer_script() {
    let root = std::env::temp_dir().join(format!(
        "fission-windows-package-config-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("fission.toml"),
        r#"
[package.windows]
exe_installer_script = "platforms/windows/package-exe.ps1"
"#,
    )
    .unwrap();

    let config = windows_package_config(&root).unwrap();

    assert_eq!(
        config.exe_installer_script.as_deref(),
        Some("platforms/windows/package-exe.ps1")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn windows_packaging_environment_exposes_validated_project_assets() {
    let root = std::env::temp_dir().join(format!(
        "fission-windows-package-assets-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("assets/intelligence")).unwrap();

    let environment = windows_packaging_environment(
        &root,
        &root.join("demo.exe"),
        &root.join(".fission/native/windows-products.json"),
    )
    .unwrap()
    .into_iter()
    .collect::<std::collections::BTreeMap<_, _>>();

    assert_eq!(
        environment.get(&OsString::from("FISSION_WINDOWS_ASSETS_DIR")),
        Some(&root.join("assets"))
    );

    fs::remove_dir_all(root.join("assets")).unwrap();
    fs::write(root.join("assets"), b"not a directory").unwrap();
    let error = windows_packaging_environment(
        &root,
        &root.join("demo.exe"),
        &root.join(".fission/native/windows-products.json"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("project assets path"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn linux_package_config_reads_run_installer_script() {
    let root = std::env::temp_dir().join(format!(
        "fission-linux-package-config-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("fission.toml"),
        r#"
[package.linux.run]
installer_script = "platforms/linux/package-run.sh"
"#,
    )
    .unwrap();

    let config = linux_package_config(&root).unwrap();

    assert_eq!(
        config.run.and_then(|run| run.installer_script).as_deref(),
        Some("platforms/linux/package-run.sh")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn linux_packaging_environment_exposes_payload_binary_and_manifest() {
    let root = PathBuf::from("/tmp/fission-linux-package");
    let environment = linux_packaging_environment(
        &root,
        &root.join("demo"),
        &root.join(".fission/native/linux-products.json"),
    );
    let environment = environment
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();

    assert_eq!(
        environment.get(&OsString::from("FISSION_LINUX_PAYLOAD_DIR")),
        Some(&root)
    );
    assert_eq!(
        environment.get(&OsString::from("LINUX_BINARY")),
        Some(&root.join("demo"))
    );
    assert_eq!(
        environment.get(&OsString::from("FISSION_LINUX_NATIVE_PRODUCTS_MANIFEST")),
        Some(&root.join(".fission/native/linux-products.json"))
    );
}

#[test]
fn packaging_script_resolves_relative_project_script_and_payload_paths() {
    let current_dir = std::env::current_dir().unwrap();
    let root = current_dir.join(format!("fission-relative-packager-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let project_dir = root.strip_prefix(&current_dir).unwrap();
    let script = project_dir.join("package.sh");
    let payload = project_dir.join("payload.bin");
    fs::write(
        root.join("package.sh"),
        "#!/bin/sh\nset -eu\ntest -f \"$TEST_PAYLOAD\"\nprintf artifact.run > artifact.run\nprintf '%s\\n' artifact.run\n",
    )
    .unwrap();
    fs::write(root.join("payload.bin"), b"payload").unwrap();

    let output = run_packaging_script_with_env(
        project_dir,
        &script,
        false,
        None,
        &[(OsString::from("TEST_PAYLOAD"), payload)],
    )
    .unwrap();

    assert_eq!(output.as_deref(), Some(root.join("artifact.run").as_path()));
    let _ = fs::remove_dir_all(root);
}
