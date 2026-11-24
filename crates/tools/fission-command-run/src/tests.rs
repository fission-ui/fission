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
fn browser_test_server_serves_multiple_resources() {
    let root = env::temp_dir().join(format!(
        "fission-web-test-server-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create test root");
    fs::write(root.join("index.html"), "ready").expect("write index");
    fs::write(root.join("bootstrap.mjs"), "export const ready = true;").expect("write module");

    let server = StaticTestServer::start(root.clone()).expect("start test server");
    let index = ureq::get(&format!("{}/", server.base_url()))
        .call()
        .expect("fetch index");
    assert_eq!(
        index.header("cross-origin-opener-policy"),
        Some("same-origin")
    );
    assert_eq!(
        index.header("cross-origin-embedder-policy"),
        Some("require-corp")
    );
    assert_eq!(index.into_string().expect("read index"), "ready");
    let module = ureq::get(&format!("{}/bootstrap.mjs", server.base_url()))
        .call()
        .expect("fetch module");
    assert_eq!(
        module.header("content-type"),
        Some("text/javascript; charset=utf-8")
    );
    assert_eq!(
        module.header("cross-origin-opener-policy"),
        Some("same-origin")
    );
    assert_eq!(
        module.header("cross-origin-embedder-policy"),
        Some("require-corp")
    );
    assert_eq!(
        module.into_string().expect("read module"),
        "export const ready = true;"
    );

    drop(server);
    fs::remove_dir_all(root).expect("remove test root");
}

#[test]
fn browser_test_server_accepts_renderer_diagnostics() {
    let root = env::temp_dir().join(format!(
        "fission-web-test-server-diagnostics-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create test root");

    let server = StaticTestServer::start(root.clone()).expect("start test server");
    let response = ureq::post(&format!("{}/__fission/renderer", server.base_url()))
        .send_string(r#"{"active":"canvas2d-software"}"#)
        .expect("post renderer diagnostics");
    assert_eq!(response.status(), 204);
    assert_eq!(
        response.header("cross-origin-opener-policy"),
        Some("same-origin")
    );
    assert_eq!(
        response.header("cross-origin-embedder-policy"),
        Some("require-corp")
    );

    drop(server);
    fs::remove_dir_all(root).expect("remove test root");
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

#[test]
fn web_build_forwards_cargo_feature_overrides_to_wasm_pack() {
    let command = web_build_command(
        Path::new("/workspace/app"),
        Path::new("/workspace/app/platforms/web/pkg"),
        false,
        &["fixtures".into(), "diagnostics".into()],
        true,
        true,
        Some(1),
    )
    .unwrap();
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        args,
        [
            "build",
            "/workspace/app",
            "--target",
            "web",
            "--out-dir",
            "/workspace/app/platforms/web/pkg",
            "--dev",
            "--no-default-features",
            "--features",
            "fixtures,diagnostics",
            "--config",
            "profile.dev.package.\"*\".opt-level=1",
        ]
    );
    let wasm_flags = command
        .get_envs()
        .find_map(|(name, value)| {
            (name == WEB_WASM_TARGET_RUSTFLAGS)
                .then(|| value.and_then(|value| value.to_str()))
                .flatten()
        })
        .expect("Web builds must set target-specific compiler flags");
    assert!(wasm_flags.contains("target-feature=+simd128"));
}

#[test]
fn web_test_build_sets_the_compile_time_switch() {
    let mut command = web_build_command(
        Path::new("/workspace/app"),
        Path::new("/workspace/app/platforms/web/pkg"),
        false,
        &[],
        false,
        true,
        Some(1),
    )
    .unwrap();
    enable_web_test_control(&mut command);
    let configured = command
        .get_envs()
        .filter_map(|(name, value)| value.map(|value| (name, value)))
        .any(|(name, value)| name == "FISSION_WEB_TEST_CONTROL" && value == "1");

    assert!(configured, "test builds must identify their shell artifact");
}

#[test]
fn web_simd_defaults_on_and_accepts_scalar_builds() {
    assert!(parse_web_wasm_simd(None).unwrap());
    assert!(parse_web_wasm_simd(Some("simd")).unwrap());
    assert!(!parse_web_wasm_simd(Some("scalar")).unwrap());
    assert!(!parse_web_wasm_simd(Some("0")).unwrap());
    assert!(parse_web_wasm_simd(Some("mystery")).is_err());
}

#[test]
fn web_dev_dependency_optimization_defaults_to_one_and_accepts_overrides() {
    assert_eq!(parse_web_dev_dependency_opt_level(None).unwrap(), 1);
    assert_eq!(parse_web_dev_dependency_opt_level(Some("0")).unwrap(), 0);
    assert_eq!(parse_web_dev_dependency_opt_level(Some(" 2 ")).unwrap(), 2);
    assert_eq!(parse_web_dev_dependency_opt_level(Some("3")).unwrap(), 3);
    assert!(parse_web_dev_dependency_opt_level(Some("4")).is_err());
    assert!(parse_web_dev_dependency_opt_level(Some("fast")).is_err());
}

#[test]
fn web_release_build_does_not_add_dev_dependency_optimization() {
    let command = web_build_command(
        Path::new("/workspace/app"),
        Path::new("/workspace/app/platforms/web/pkg"),
        true,
        &[],
        false,
        true,
        None,
    )
    .unwrap();
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(args.last().map(String::as_str), Some("--release"));
    assert!(!args.iter().any(|arg| arg == "--config"));
}

#[test]
fn web_simd_preserves_other_target_features_and_supports_scalar_override() {
    let configured = update_simd_target_feature_flags(
        "-C opt-level=2 -C target-feature=+atomics,-simd128",
        true,
    )
    .unwrap();
    assert_eq!(
        configured,
        "-C opt-level=2 -C target-feature=+atomics,+simd128"
    );
    let configured_args = shell_words::split(&configured).unwrap();
    assert!(configured_args
        .windows(2)
        .any(|pair| pair == ["-C", "opt-level=2"]));
    assert!(configured_args
        .windows(2)
        .any(|pair| pair == ["-C", "target-feature=+atomics,+simd128"]));

    let scalar =
        update_simd_target_feature_flags("-C target-feature=+atomics,+simd128", false).unwrap();
    assert_eq!(scalar, "-C target-feature=+atomics");
    assert_eq!(
        shell_words::split(&scalar).unwrap(),
        ["-C", "target-feature=+atomics"]
    );
    assert_eq!(update_simd_target_feature_flags("", false).unwrap(), "");
}

#[test]
fn web_build_keeps_an_absolute_project_path_lexical() {
    let project_dir = env::current_dir()
        .expect("current directory")
        .join("path-that-need-not-exist")
        .join("..");

    assert_eq!(absolute_path(&project_dir).unwrap(), project_dir);
}

#[test]
fn cargo_feature_overrides_reject_non_web_targets() {
    let error = ensure_web_cargo_feature_target(Target::Linux, &["fixtures".into()], false)
        .expect_err("non-Web Cargo feature overrides must be rejected");

    assert!(error.to_string().contains("require `--target web`"));
    ensure_web_cargo_feature_target(Target::Linux, &[], false)
        .expect("default lifecycle commands remain supported");
}
