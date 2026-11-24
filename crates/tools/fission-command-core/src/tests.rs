use super::*;

#[test]
fn generated_starter_teaches_compose_as_the_default_authoring_api() {
    assert!(APP_RS.contains("fission_component!"));
    assert!(APP_RS.contains("on_press: |_, state, _cx|"));
    assert!(APP_RS.contains("key: \"increment\""));
    assert!(APP_RS.contains("semantics_identifier(\"counter.increment\")"));
    assert!(!APP_RS.contains("impl From<CounterApp> for Widget"));
    assert!(!APP_RS.contains("#[fission_reducer(Increment)]"));
}

fn unique_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "fission-command-core-{name}-{}",
        std::process::id()
    ));
    fs::remove_dir_all(&dir).ok();
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn web_bootstrap_imports_the_cargo_library_not_the_display_name() {
    let dir = unique_dir("web-bootstrap-library-name");
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"qualification-package\"\nversion = \"0.1.0\"\n\n[lib]\nname = \"qualification_runtime\"\n",
    )
    .unwrap();
    let project = FissionProject {
        app: AppConfig {
            name: "Friendly Display Name".to_string(),
            app_id: "com.example.qualification".to_string(),
            splash: None,
        },
        targets: BTreeSet::from([Target::Web]),
        capabilities: BTreeSet::new(),
        native: NativeConfig::default(),
    };

    let bootstrap = render_web_bootstrap(&dir, &project);

    assert!(bootstrap.contains("./pkg/qualification_runtime.js"));
    assert!(!bootstrap.contains("Friendly Display Name"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn target_sync_preserves_features_not_owned_by_the_cli() {
    let dir = unique_dir("target-sync-preserves-features");
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"qualification\"\nversion = \"0.1.0\"\n\n[dependencies]\nfission = { version = \"0.16.0\", default-features = false, features = [\"desktop\", \"scene2d\", \"game\", \"physics-rapier2d\"] }\n",
    )
    .unwrap();
    let project = FissionProject {
        app: AppConfig {
            name: "Qualification".to_string(),
            app_id: "com.example.qualification".to_string(),
            splash: None,
        },
        targets: BTreeSet::from([Target::Linux, Target::Web]),
        capabilities: BTreeSet::new(),
        native: NativeConfig::default(),
    };

    sync_cargo_fission_dependency(&dir, &project, None).unwrap();

    let manifest = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
    for feature in ["desktop", "web", "scene2d", "game", "physics-rapier2d"] {
        assert!(
            manifest.contains(&format!("\"{feature}\"")),
            "missing preserved feature {feature}: {manifest}"
        );
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn project_assets_stage_nested_resources_and_replace_stale_output() {
    let dir = unique_dir("stage-project-assets");
    let project = dir.join("project");
    let destination = dir.join("destination");
    fs::create_dir_all(project.join("assets/intelligence")).unwrap();
    fs::create_dir_all(destination.join("assets/stale")).unwrap();
    fs::write(
        project.join("assets/intelligence/base.pdb.zst"),
        b"signed base",
    )
    .unwrap();
    fs::write(destination.join("assets/stale/old"), b"stale").unwrap();

    let staged = stage_project_assets(&project, &destination)
        .unwrap()
        .expect("assets directory should be staged");

    assert_eq!(staged, destination.join("assets"));
    assert_eq!(
        fs::read(staged.join("intelligence/base.pdb.zst")).unwrap(),
        b"signed base"
    );
    assert!(!staged.join("stale/old").exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn project_assets_are_optional_but_must_be_a_directory_when_present() {
    let dir = unique_dir("stage-project-assets-validation");
    let project = dir.join("project");
    let destination = dir.join("destination");
    fs::create_dir_all(&project).unwrap();

    assert_eq!(stage_project_assets(&project, &destination).unwrap(), None);

    fs::write(project.join("assets"), b"not a directory").unwrap();
    let error = stage_project_assets(&project, &destination).unwrap_err();
    assert!(error.to_string().contains("project assets path"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn windows_release_sync_updates_appx_identity() {
    let dir = unique_dir("windows-release-sync");
    let windows_dir = dir.join("platforms/windows");
    fs::create_dir_all(&windows_dir).unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"[package.windows]
identity_name = "Example.App"
publisher = "CN=Example & Co"
"#,
    )
    .unwrap();
    let manifest = windows_dir.join("Package.appxmanifest");
    fs::write(
        &manifest,
        r#"<Package>
  <Identity Name="Old.App" Publisher="CN=Old" Version="0.0.0.0" />
</Package>
"#,
    )
    .unwrap();

    sync_release_platform_config(
        &dir,
        Target::Windows,
        &ReleaseVersionConfig {
            version: Some("1.2.3".to_string()),
            build: Some(42),
        },
    )
    .unwrap();

    let updated = fs::read_to_string(&manifest).unwrap();
    assert!(updated.contains(r#"Name="Example.App""#));
    assert!(updated.contains(r#"Publisher="CN=Example &amp; Co""#));
    assert!(updated.contains(r#"Version="1.2.3.42""#));
}

#[test]
fn windows_release_sync_rejects_invalid_version() {
    let dir = unique_dir("windows-release-invalid-version");
    let windows_dir = dir.join("platforms/windows");
    fs::create_dir_all(&windows_dir).unwrap();
    fs::write(
        windows_dir.join("Package.appxmanifest"),
        r#"<Package><Identity Version="0.0.0.0" /></Package>"#,
    )
    .unwrap();

    let error = sync_release_platform_config(
        &dir,
        Target::Windows,
        &ReleaseVersionConfig {
            version: Some("1.2.beta".to_string()),
            build: Some(1),
        },
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("Windows package version `1.2.beta` must be numeric"));
}

#[test]
fn windows_scaffold_includes_opt_in_nsis_shortcut_identity_support() {
    let dir = unique_dir("windows-shortcut-aumid-scaffold");
    let project = FissionProject {
        app: AppConfig {
            name: "Example App".to_string(),
            app_id: "com.example.app".to_string(),
            splash: None,
        },
        targets: BTreeSet::from([Target::Windows]),
        capabilities: BTreeSet::new(),
        native: NativeConfig::default(),
    };

    scaffold_windows_bundle(&dir, &project, WritePolicy::Overwrite).unwrap();

    let source =
        fs::read_to_string(dir.join("platforms/windows/shortcut-aumid-helper.cpp")).unwrap();
    assert!(source.contains("PKEY_AppUserModel_ID"));
    assert!(source.contains("length > 128"));
    assert!(source.contains("std::iswspace"));

    let build =
        fs::read_to_string(dir.join("platforms/windows/build-shortcut-aumid-helper.ps1")).unwrap();
    assert!(build.contains(r#"[ValidateSet("x64", "arm64")]"#));
    assert!(build.contains("/MT"));
    assert!(build.contains("Microsoft.VisualStudio.Component.VC.Tools.ARM64"));
    assert!(build.contains("propsys.lib"));

    let nsis =
        fs::read_to_string(dir.join("platforms/windows/fission-shortcut-aumid.nsh")).unwrap();
    assert!(nsis.contains("nsExec::ExecToStack"));
    assert!(nsis.contains("FissionEmbedShortcutAppUserModelIdHelper"));
    assert!(nsis.contains("FissionSetShortcutAppUserModelId"));
    assert!(nsis.contains("Abort"));
    assert!(!nsis.contains("WinShell"));

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn windows_shortcut_identity_support_is_opt_in() {
    let source = render_windows_shortcut_aumid_helper_source();
    let build = render_windows_shortcut_aumid_helper_build_script();
    let nsis = render_windows_shortcut_aumid_nsis_include();

    assert!(source.contains("argv[2]"));
    assert!(build.contains("$Architecture"));
    assert!(nsis.contains("APP_USER_MODEL_ID"));
    assert!(!nsis.contains("APP_USER_MODEL_ID ="));
    assert!(!nsis.contains("!define FISSION_APP_USER_MODEL_ID"));
}

#[test]
fn android_scaffold_uses_game_activity_text_host() {
    let dir = unique_dir("android-game-activity-scaffold");
    let project = FissionProject {
        app: AppConfig {
            name: "Text Session Demo".to_string(),
            app_id: "com.example.text_session".to_string(),
            splash: None,
        },
        targets: BTreeSet::from([Target::Android]),
        capabilities: BTreeSet::new(),
        native: NativeConfig::default(),
    };

    scaffold_android_bundle(&dir, &project, WritePolicy::Overwrite).unwrap();

    assert!(dir
        .join("platforms/android/java/rs/fission/runtime/FissionAndroidCapabilities.java")
        .exists());
    let capabilities = fs::read_to_string(
        dir.join("platforms/android/java/rs/fission/runtime/FissionAndroidCapabilities.java"),
    )
    .unwrap();
    assert!(capabilities.contains("Intent.ACTION_OPEN_DOCUMENT"));
    assert!(capabilities.contains("Intent.ACTION_CREATE_DOCUMENT"));
    assert!(capabilities.contains("nativeFilePickerResult"));
    assert!(!dir
        .join("platforms/android/java/rs/fission/runtime/FissionFileSystem.java")
        .exists());

    let activity = fs::read_to_string(
        dir.join("platforms/android/java/rs/fission/runtime/FissionActivity.java"),
    )
    .unwrap();
    assert!(activity.contains("extends GameActivity"));
    assert!(activity.contains("com.google.androidgamesdk.GameActivity"));

    let gradle = fs::read_to_string(dir.join("platforms/android/app/build.gradle.kts")).unwrap();
    assert!(gradle.contains("org.jetbrains.kotlin:kotlin-bom:1.8.22"));
    assert!(gradle.contains("androidx.appcompat:appcompat:1.7.0"));
    assert!(gradle.contains("androidx.games:games-activity:4.4.0"));

    let manifest = fs::read_to_string(dir.join("platforms/android/AndroidManifest.xml")).unwrap();
    assert!(manifest.contains("rs.fission.runtime.FissionActivity"));
    assert!(manifest.contains("android:hasCode=\"true\""));

    for script in ["package-apk.sh", "package-aab.sh"] {
        let package = fs::read_to_string(dir.join("platforms/android").join(script)).unwrap();
        assert!(package.contains("CXX_aarch64_linux_android"));
        assert!(package.contains("aarch64-linux-android${ANDROID_MIN_API_LEVEL}-clang++"));
    }

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn android_filesystem_capability_installs_only_its_generated_bridge() {
    let dir = unique_dir("android-filesystem-scaffold");
    let project = FissionProject {
        app: AppConfig {
            name: "Files Demo".to_string(),
            app_id: "com.example.files".to_string(),
            splash: None,
        },
        targets: BTreeSet::from([Target::Android]),
        capabilities: BTreeSet::from([PlatformCapability::FileSystem]),
        native: NativeConfig::default(),
    };

    scaffold_android_bundle(&dir, &project, WritePolicy::Overwrite).unwrap();

    let bridge = fs::read_to_string(
        dir.join("platforms/android/java/rs/fission/runtime/FissionFileSystem.java"),
    )
    .unwrap();
    assert!(bridge.contains("Intent.ACTION_OPEN_DOCUMENT_TREE"));
    assert!(bridge.contains("takePersistableUriPermission"));
    assert!(bridge.contains("ParcelFileDescriptor"));

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn macos_release_sync_updates_info_plist_version() {
    let dir = unique_dir("macos-release-sync");
    let macos_dir = dir.join("platforms/macos");
    fs::create_dir_all(&macos_dir).unwrap();
    let plist = macos_dir.join("Info.plist");
    fs::write(
        &plist,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
  <key>CFBundleShortVersionString</key>
  <string>0.0.1</string>
  <key>CFBundleVersion</key>
  <string>1</string>
</dict>
</plist>
"#,
    )
    .unwrap();

    sync_release_platform_config(
        &dir,
        Target::Macos,
        &ReleaseVersionConfig {
            version: Some("1.2.3".to_string()),
            build: Some(42),
        },
    )
    .unwrap();

    let updated = fs::read_to_string(&plist).unwrap();
    assert!(updated.contains("<string>1.2.3</string>"));
    assert!(updated.contains("<string>42</string>"));
}

#[test]
fn project_config_includes_release_package_defaults() {
    let dir = unique_dir("release-package-defaults");
    let project = FissionProject {
        app: AppConfig {
            name: "release-demo".to_string(),
            app_id: "com.example.release_demo".to_string(),
            splash: None,
        },
        targets: BTreeSet::from([Target::Android, Target::Ios, Target::Macos, Target::Windows]),
        capabilities: BTreeSet::new(),
        native: NativeConfig::default(),
    };

    write_project_config(&dir, &project).unwrap();

    let text = fs::read_to_string(dir.join("fission.toml")).unwrap();
    assert!(text.contains("version = \"0.1.0\""));
    assert!(text.contains("build = 1"));
    assert!(text.contains("[package.android]"));
    assert!(text.contains("package_name = \"com.example.release_demo\""));
    assert!(text.contains("keystore_env = \"ANDROID_KEYSTORE\""));
    assert!(text.contains("[package.ios]"));
    assert!(text.contains("bundle_id = \"com.example.release_demo\""));
    assert!(text.contains("[package.macos]"));
    assert!(text.contains("marketing_version = \"0.1.0\""));
    assert!(text.contains("build_number = \"1\""));
    assert!(text.contains("[package.windows]"));
    assert!(text.contains("identity_name = \"com.example.release.demo\""));
    assert!(text.contains("certificate_base64_env = \"WINDOWS_CERTIFICATE_BASE64\""));
    assert!(text.contains("[distribution.play_store]"));
    assert!(text
        .contains("service_account_json_base64_env = \"PLAY_STORE_SERVICE_ACCOUNT_JSON_BASE64\""));
    assert!(text.contains("[distribution.app_store]"));
    assert!(text.contains("api_key_base64_env = \"APP_STORE_CONNECT_API_KEY_BASE64\""));
    assert!(text.contains("[distribution.microsoft_store]"));
    assert!(text.contains("client_secret_env = \"MICROSOFT_STORE_CLIENT_SECRET\""));
}

#[test]
fn target_aliases_parse_legacy_names_and_write_canonical_names() {
    assert_eq!(
        <Target as clap::ValueEnum>::from_str("site", true).unwrap(),
        Target::Site
    );
    assert_eq!(
        <Target as clap::ValueEnum>::from_str("server", true).unwrap(),
        Target::Server
    );

    let dir = unique_dir("target-aliases");
    fs::write(
        dir.join("fission.toml"),
        r#"targets = ["site", "server"]

[app]
name = "Alias Demo"
app_id = "com.example.alias"
"#,
    )
    .unwrap();

    let project = read_project_config(&dir).unwrap();
    assert!(project.targets.contains(&Target::Site));
    assert!(project.targets.contains(&Target::Server));

    write_project_config(&dir, &project).unwrap();
    let updated = fs::read_to_string(dir.join("fission.toml")).unwrap();
    assert!(updated.contains("\"static-site\""));
    assert!(updated.contains("\"ssr\""));
    assert!(!updated.contains("\"site\""));
    assert!(!updated.contains("\"server\""));
}

#[test]
fn filesystem_capability_accepts_alias_and_serializes_canonical_name() {
    assert_eq!(
        <PlatformCapability as clap::ValueEnum>::from_str("file-system", true).unwrap(),
        PlatformCapability::FileSystem
    );
    assert_eq!(
        serde_json::to_string(&PlatformCapability::FileSystem).unwrap(),
        "\"filesystem\""
    );
}

#[test]
fn static_site_uses_the_scaffold_path_created_by_add_target() {
    assert_eq!(
        Target::Site.scaffold_relative_path(),
        "platforms/site/README.md"
    );
}

#[test]
fn app_id_accepts_short_id_alias() {
    let dir = unique_dir("app-id-alias");
    fs::write(
        dir.join("fission.toml"),
        r#"targets = ["android"]

[app]
name = "Alias Demo"
id = "com.example.alias"
"#,
    )
    .unwrap();

    let project = read_project_config(&dir).unwrap();
    assert_eq!(project.app.app_id, "com.example.alias");
}
