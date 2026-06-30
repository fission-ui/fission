use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{value, Array, DocumentMut, InlineTable, Item, Table, Value};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const ANDROID_GRADLE_PLUGIN_VERSION: &str = "8.13.2";
const DEFAULT_APP_ICON_PNG: &[u8] = include_bytes!("../assets/fission_logo.png");

mod icons;
mod splash;
pub use icons::{copy_icon_for_bundle, normalized_extension, resolve_app_icon, ResolvedIcon};
pub use splash::{SplashConfig, SplashResizeMode};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    Android,
    Ios,
    Linux,
    Macos,
    Server,
    Site,
    Web,
    Windows,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlatformCapability {
    BarcodeScanner,
    Biometric,
    Bluetooth,
    Camera,
    Geolocation,
    Haptics,
    Microphone,
    Nfc,
    Notifications,
    Passkeys,
    VolumeControl,
    Wifi,
}

impl PlatformCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BarcodeScanner => "barcode-scanner",
            Self::Biometric => "biometric",
            Self::Bluetooth => "bluetooth",
            Self::Camera => "camera",
            Self::Geolocation => "geolocation",
            Self::Haptics => "haptics",
            Self::Microphone => "microphone",
            Self::Nfc => "nfc",
            Self::Notifications => "notifications",
            Self::Passkeys => "passkeys",
            Self::VolumeControl => "volume-control",
            Self::Wifi => "wifi",
        }
    }
}

impl Target {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Android => "android",
            Self::Ios => "ios",
            Self::Linux => "linux",
            Self::Macos => "macos",
            Self::Server => "server",
            Self::Site => "site",
            Self::Web => "web",
            Self::Windows => "windows",
        }
    }

    pub fn scaffold_relative_path(self) -> &'static str {
        match self {
            Self::Android => "platforms/android/README.md",
            Self::Ios => "platforms/ios/README.md",
            Self::Linux => "platforms/linux/README.md",
            Self::Macos => "platforms/macos/README.md",
            Self::Server => "platforms/server/README.md",
            Self::Site => "platforms/site/README.md",
            Self::Web => "platforms/web/README.md",
            Self::Windows => "platforms/windows/README.md",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum DistributionProvider {
    #[value(name = "app-store")]
    AppStore,
    #[value(name = "github-pages")]
    GithubPages,
    #[value(name = "github-releases")]
    GithubReleases,
    #[value(name = "cloudflare-pages")]
    CloudflarePages,
    #[value(name = "docker-registry")]
    DockerRegistry,
    Dropbox,
    #[value(name = "google-drive")]
    GoogleDrive,
    #[value(name = "microsoft-store")]
    MicrosoftStore,
    Netlify,
    #[value(name = "onedrive")]
    OneDrive,
    #[value(name = "play-store")]
    PlayStore,
    S3,
}

impl DistributionProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AppStore => "app-store",
            Self::GithubPages => "github-pages",
            Self::GithubReleases => "github-releases",
            Self::CloudflarePages => "cloudflare-pages",
            Self::DockerRegistry => "docker-registry",
            Self::Dropbox => "dropbox",
            Self::GoogleDrive => "google-drive",
            Self::MicrosoftStore => "microsoft-store",
            Self::Netlify => "netlify",
            Self::OneDrive => "onedrive",
            Self::PlayStore => "play-store",
            Self::S3 => "s3",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FissionProject {
    pub app: AppConfig,
    pub targets: BTreeSet<Target>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub capabilities: BTreeSet<PlatformCapability>,
    #[serde(default, skip_serializing_if = "NativeConfig::is_empty")]
    pub native: NativeConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub name: String,
    #[serde(alias = "identifier")]
    pub app_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub splash: Option<SplashConfig>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<NativeModuleConfig>,
}

impl NativeConfig {
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeModuleConfig {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "NativeAndroidModuleConfig::is_empty")]
    pub android: NativeAndroidModuleConfig,
    #[serde(default, skip_serializing_if = "NativeIosModuleConfig::is_empty")]
    pub ios: NativeIosModuleConfig,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeAndroidModuleConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repositories: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gradle_dependencies: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_dirs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub manifest_application_entries: Vec<String>,
}

impl NativeAndroidModuleConfig {
    pub fn is_empty(&self) -> bool {
        self.repositories.is_empty()
            && self.gradle_dependencies.is_empty()
            && self.source_dirs.is_empty()
            && self.permissions.is_empty()
            && self.manifest_application_entries.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeIosModuleConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub swift_packages: Vec<NativeIosSwiftPackageConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_dirs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked_frameworks: Vec<String>,
}

impl NativeIosModuleConfig {
    pub fn is_empty(&self) -> bool {
        self.swift_packages.is_empty()
            && self.source_dirs.is_empty()
            && self.linked_frameworks.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeIosSwiftPackageConfig {
    pub url: String,
    pub product: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CargoManifest {
    package: Option<CargoPackage>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    pub name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WritePolicy {
    Overwrite,
    PreserveExisting,
}

pub fn init_project(
    root: &Path,
    name: Option<String>,
    app_id: Option<String>,
    local_path: Option<PathBuf>,
) -> Result<()> {
    let existing_project = root.exists() && root.read_dir()?.next().is_some();
    fs::create_dir_all(root.join("src"))?;

    let write_policy = if existing_project {
        WritePolicy::PreserveExisting
    } else {
        WritePolicy::Overwrite
    };
    let project = initial_project_config(root, name, app_id)?;

    write_file_with_policy(
        &root.join("Cargo.toml"),
        &render_cargo_toml(&project, local_path.as_deref()),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("src/main.rs"),
        &render_app_main(project.app.name.as_str()),
        write_policy,
    )?;
    write_file_with_policy(&root.join("src/lib.rs"), APP_LIB, write_policy)?;
    write_file_with_policy(&root.join("src/app.rs"), APP_RS, write_policy)?;
    write_binary_file_with_policy(
        &root.join("assets/app-icon.png"),
        DEFAULT_APP_ICON_PNG,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("README.md"),
        &render_project_readme(&project),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join(".gitignore"),
        "target/\nplatforms/*/build/\n",
        write_policy,
    )?;
    write_project_config(root, &project)?;

    let targets = project.targets.iter().copied().collect::<Vec<_>>();
    for target in targets {
        scaffold_target_with_policy(root, &project, target, write_policy)?;
    }
    sync_platform_config(root, &project)?;
    sync_cargo_fission_dependency(root, &project, local_path.as_deref())?;

    Ok(())
}

fn initial_project_config(
    root: &Path,
    name: Option<String>,
    app_id: Option<String>,
) -> Result<FissionProject> {
    let existing = if root.join("fission.toml").exists() {
        Some(read_project_config(root)?)
    } else {
        None
    };
    let cargo_name = cargo_package_name(root);
    if let (Some(requested), Some(cargo_name)) = (&name, &cargo_name) {
        let requested = normalize_crate_name(requested);
        let cargo_name = normalize_crate_name(cargo_name);
        if requested != cargo_name {
            bail!(
                "refusing to set app name `{requested}` for existing Cargo package `{cargo_name}`; rename the package in Cargo.toml first or omit --name"
            );
        }
    }
    let project_name = cargo_name
        .or(name)
        .or_else(|| existing.as_ref().map(|project| project.app.name.clone()))
        .unwrap_or_else(|| {
            root.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("fission-app")
                .to_string()
        });
    let normalized_name = normalize_crate_name(&project_name);

    let mut targets = existing
        .as_ref()
        .map(|project| project.targets.clone())
        .unwrap_or_default();
    targets.extend(detect_project_targets(root));
    if targets.is_empty() {
        targets.extend([Target::Windows, Target::Macos, Target::Linux]);
    }

    Ok(FissionProject {
        app: AppConfig {
            name: normalized_name.clone(),
            app_id: app_id
                .or_else(|| existing.as_ref().map(|project| project.app.app_id.clone()))
                .unwrap_or_else(|| format!("com.example.{}", normalized_name.replace('-', "_"))),
            splash: existing
                .as_ref()
                .and_then(|project| project.app.splash.clone()),
        },
        targets,
        capabilities: existing
            .as_ref()
            .map(|project| project.capabilities.clone())
            .unwrap_or_default(),
        native: existing
            .as_ref()
            .map(|project| project.native.clone())
            .unwrap_or_default(),
    })
}

pub fn cargo_package_name(root: &Path) -> Option<String> {
    let manifest = fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let manifest: CargoManifest = toml::from_str(&manifest).ok()?;
    manifest.package.map(|package| package.name)
}

fn detect_project_targets(root: &Path) -> BTreeSet<Target> {
    let mut targets = BTreeSet::new();
    if root.join("src/main.rs").exists() || root.join("src/lib.rs").exists() {
        targets.extend([Target::Windows, Target::Macos, Target::Linux]);
    }
    for (target, relative) in [
        (Target::Android, "platforms/android"),
        (Target::Ios, "platforms/ios"),
        (Target::Linux, "platforms/linux"),
        (Target::Macos, "platforms/macos"),
        (Target::Server, "platforms/server"),
        (Target::Site, "content"),
        (Target::Web, "platforms/web"),
        (Target::Windows, "platforms/windows"),
    ] {
        if root.join(relative).exists() {
            targets.insert(target);
        }
    }
    targets
}

pub fn add_targets(project_dir: &Path, targets: &[Target]) -> Result<()> {
    if targets.is_empty() {
        bail!("no targets provided");
    }
    let mut project = read_project_config(project_dir)?;
    for target in targets {
        let target_exists =
            project.targets.contains(target) || target_scaffold_dir_exists(project_dir, *target);
        project.targets.insert(*target);
        let write_policy = if target_exists {
            WritePolicy::PreserveExisting
        } else {
            WritePolicy::Overwrite
        };
        scaffold_target_with_policy(project_dir, &project, *target, write_policy)?;
    }
    sync_platform_config(project_dir, &project)?;
    write_project_config(project_dir, &project)?;
    update_cargo_fission_features(project_dir, &project)?;
    write_file_with_policy(
        &project_dir.join("README.md"),
        &render_project_readme(&project),
        WritePolicy::PreserveExisting,
    )?;
    Ok(())
}

pub fn add_capabilities(project_dir: &Path, capabilities: &[PlatformCapability]) -> Result<()> {
    if capabilities.is_empty() {
        bail!("no capabilities provided");
    }
    let mut project = read_project_config(project_dir)?;
    for capability in capabilities {
        project.capabilities.insert(*capability);
    }
    write_project_config(project_dir, &project)?;
    sync_platform_config(project_dir, &project)?;
    Ok(())
}

pub fn sync_platform_config(root: &Path, project: &FissionProject) -> Result<()> {
    apply_platform_capability_config(root, project)?;
    apply_native_module_config(root, project)?;
    splash::apply_platform_splash_config(root, project)?;
    icons::apply_platform_icon_config(root, project)?;
    apply_mobile_run_script_hardening(root, project)?;
    Ok(())
}

fn apply_native_module_config(root: &Path, project: &FissionProject) -> Result<()> {
    if project.targets.contains(&Target::Android) {
        write_file(
            &root.join("platforms/android/native-modules.gradle"),
            &render_android_native_modules_gradle(project),
        )?;
        apply_android_settings_gradle_hardening(root, project)?;
        apply_android_native_manifest_entries(root, project)?;
    }
    if project.targets.contains(&Target::Ios) {
        write_file(
            &root.join("platforms/ios/NativeModules/Package.swift"),
            &render_ios_native_modules_package(project),
        )?;
        write_file(
            &root.join(
                "platforms/ios/NativeModules/Sources/FissionNativeModules/FissionNativeCapabilities.swift",
            ),
            render_ios_native_capabilities_swift(),
        )?;
        sync_ios_native_module_sources(root, project)?;
    }
    Ok(())
}

fn apply_android_native_manifest_entries(root: &Path, project: &FissionProject) -> Result<()> {
    let entries = render_android_native_application_entries(project);
    if entries.trim().is_empty() {
        return Ok(());
    }
    let path = root.join("platforms/android/AndroidManifest.xml");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let missing = entries
        .lines()
        .filter(|entry| !entry.trim().is_empty() && !existing.contains(entry.trim()))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }

    let insertion = format!("{}\n", missing.join("\n"));
    let marker =
        "        <activity\n            android:name=\"rs.fission.runtime.FissionActivity\"";
    let updated = if let Some(index) = existing.find(marker) {
        let mut updated = existing.clone();
        updated.insert_str(index, &insertion);
        updated
    } else if let Some(index) = existing.find("</application>") {
        let mut updated = existing.clone();
        updated.insert_str(index, &insertion);
        updated
    } else {
        existing
    };

    if updated != fs::read_to_string(&path)? {
        fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn sync_ios_native_module_sources(root: &Path, project: &FissionProject) -> Result<()> {
    let generated_root = root.join("platforms/ios/NativeModules/Sources/FissionNativeModules");
    fs::create_dir_all(&generated_root)
        .with_context(|| format!("failed to create {}", generated_root.display()))?;

    for module in &project.native.modules {
        let module_dir = generated_root.join(swift_module_source_dir_name(&module.name));
        if module_dir.exists() {
            fs::remove_dir_all(&module_dir)
                .with_context(|| format!("failed to remove {}", module_dir.display()))?;
        }
        if module.ios.source_dirs.is_empty() {
            continue;
        }
        fs::create_dir_all(&module_dir)
            .with_context(|| format!("failed to create {}", module_dir.display()))?;
        for source_dir in &module.ios.source_dirs {
            let source_dir = source_dir.trim();
            if source_dir.is_empty() {
                continue;
            }
            let source = resolve_project_path(root, source_dir);
            copy_dir_contents(&source, &module_dir).with_context(|| {
                format!(
                    "failed to copy iOS native module source {} into {}",
                    source.display(),
                    module_dir.display()
                )
            })?;
        }
    }
    Ok(())
}

fn resolve_project_path(root: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn swift_module_source_dir_name(name: &str) -> String {
    let mut output = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch);
        } else if !output.ends_with('_') {
            output.push('_');
        }
    }
    let output = output.trim_matches('_');
    if output.is_empty() {
        "module".to_string()
    } else {
        output.to_string()
    }
}

fn copy_dir_contents(source: &Path, dest: &Path) -> Result<()> {
    if source.is_file() {
        let file_name = source
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("source file has no file name"))?;
        fs::create_dir_all(dest)?;
        fs::copy(source, dest.join(file_name))?;
        return Ok(());
    }
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read native source dir {}", source.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let target = dest.join(entry.file_name());
        if path.is_dir() {
            copy_dir_contents(&path, &target)?;
        } else if path.is_file() {
            fs::copy(&path, &target)
                .with_context(|| format!("failed to copy {}", path.display()))?;
        }
    }
    Ok(())
}

fn apply_mobile_run_script_hardening(root: &Path, project: &FissionProject) -> Result<()> {
    if project.targets.contains(&Target::Ios) {
        apply_ios_run_script_hardening(root)?;
        apply_ios_package_script_hardening(root)?;
    }
    if project.targets.contains(&Target::Android) {
        apply_android_run_script_hardening(root)?;
        apply_android_package_script_hardening(root)?;
        apply_android_manifest_hardening(root)?;
        apply_android_root_build_gradle_hardening(root)?;
        apply_android_app_build_gradle_hardening(root)?;
        apply_android_gradle_properties_hardening(root)?;
    }
    Ok(())
}

fn apply_ios_run_script_hardening(root: &Path) -> Result<()> {
    let path = root.join("platforms/ios/run-sim.sh");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if existing.contains("IOS_SIM_UNINSTALL_BEFORE_INSTALL") {
        return Ok(());
    }
    let marker = "xcrun simctl bootstatus \"$DEVICE_ID\" -b\n";
    let insertion = "xcrun simctl bootstatus \"$DEVICE_ID\" -b\nif [[ \"${IOS_SIM_UNINSTALL_BEFORE_INSTALL:-1}\" == \"1\" ]]; then\n  xcrun simctl uninstall \"$DEVICE_ID\" \"$BUNDLE_ID\" >/dev/null 2>&1 || true\nfi\n";
    let updated = existing.replacen(marker, insertion, 1);
    fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))
}

fn apply_ios_package_script_hardening(root: &Path) -> Result<()> {
    let path = root.join("platforms/ios/package-sim.sh");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if !existing.contains("import plistlib") {
        return Ok(());
    }
    let Some(start) = existing.find("python3 - <<'PY' \"$SCRIPT_DIR/Info.plist\"") else {
        return Ok(());
    };
    let Some(relative_end) = existing[start..].find("\nPY") else {
        return Ok(());
    };
    let end = start + relative_end + "\nPY\n".len();
    let mut updated = existing;
    updated.replace_range(start..end, IOS_INFO_PLIST_PLUTIL_PATCH);
    fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))
}

fn apply_android_run_script_hardening(root: &Path) -> Result<()> {
    let path = root.join("platforms/android/run-emulator.sh");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if existing.contains(":app:assemble") {
        return Ok(());
    }
    let mut updated = existing.clone();
    let wait_function = android_wait_for_boot_function();
    if let Some(start) = updated.find("wait_for_android_boot() {") {
        let marker = "\n}\n\nANDROID_EMULATOR_API_LEVEL=";
        if let Some(relative_end) = updated[start..].find(marker) {
            let end = start + relative_end + "\n}\n\n".len();
            updated.replace_range(start..end, &format!("{wait_function}\n\n"));
        }
    } else {
        updated = updated.replacen(
            "\nANDROID_EMULATOR_API_LEVEL=",
            &format!("\n{wait_function}\n\nANDROID_EMULATOR_API_LEVEL="),
            1,
        );
    }
    updated =
        replace_android_boot_wait_after(updated, "  disown || true\n", "  wait_for_android_boot\n");
    updated = replace_android_boot_wait_after(
        updated,
        "  \"$EMULATOR_BIN\" \"${EMULATOR_ARGS[@]}\" >/tmp/fission-android-emulator.log 2>&1 &\n",
        "  wait_for_android_boot\n",
    );
    if !updated.contains(
        "printf 'Using existing emulator %s\\n' \"$RUNNING_EMULATOR\"\n  wait_for_android_boot\n",
    ) {
        updated = updated.replacen(
            "printf 'Using existing emulator %s\\n' \"$RUNNING_EMULATOR\"\n",
            "printf 'Using existing emulator %s\\n' \"$RUNNING_EMULATOR\"\n  wait_for_android_boot\n",
            1,
        );
    }
    while updated.contains("  wait_for_android_boot\n  wait_for_android_boot\n") {
        updated = updated.replace(
            "  wait_for_android_boot\n  wait_for_android_boot\n",
            "  wait_for_android_boot\n",
        );
    }
    updated = updated.replace(
        "\"$ADB\" install -r \"$APK\"",
        "read -r -a ADB_INSTALL_FLAGS <<< \"${ADB_INSTALL_FLAGS:---no-streaming -r}\"\n\"$ADB\" install \"${ADB_INSTALL_FLAGS[@]}\" \"$APK\"",
    );
    if updated != existing {
        fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn apply_android_package_script_hardening(root: &Path) -> Result<()> {
    let path = root.join("platforms/android/package-apk.sh");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut updated = existing.clone();
    if updated.contains("import re\nimport sys\n") && !updated.contains("import pathlib\n") {
        updated = updated.replace(
            "import re\nimport sys\n",
            "import pathlib\nimport re\nimport sys\n",
        );
    }
    let has_code_line = r#"has_code = "true" if pathlib.Path(dest).with_name("apk-root").joinpath("classes.dex").exists() else "false"
manifest = re.sub(r'android:hasCode="(?:true|false)"', f'android:hasCode="{has_code}"', manifest)
"#;
    if !updated.contains("android:hasCode=") || !updated.contains("with_name(\"apk-root\")") {
        updated = updated.replace(
            "manifest = re.sub(r'android:targetSdkVersion=\"\\d+\"', f'android:targetSdkVersion=\"{target_api}\"', manifest)\n",
            &format!(
                "manifest = re.sub(r'android:targetSdkVersion=\"\\d+\"', f'android:targetSdkVersion=\"{{target_api}}\"', manifest)\n{has_code_line}"
            ),
        );
    }
    if updated != existing {
        fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn apply_android_manifest_hardening(root: &Path) -> Result<()> {
    let path = root.join("platforms/android/AndroidManifest.xml");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if existing.contains("rs.fission.runtime.FissionActivity") {
        return Ok(());
    }
    let updated = existing.replace(r#"android:hasCode="true""#, r#"android:hasCode="false""#);
    if updated != existing {
        fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn apply_android_root_build_gradle_hardening(root: &Path) -> Result<()> {
    let path = root.join("platforms/android/build.gradle.kts");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut updated = String::new();
    for line in existing.lines() {
        if line
            .trim_start()
            .starts_with("id(\"com.android.application\") version ")
        {
            let indent = line
                .chars()
                .take_while(|ch| ch.is_whitespace())
                .collect::<String>();
            updated.push_str(&format!(
                "{indent}id(\"com.android.application\") version \"{ANDROID_GRADLE_PLUGIN_VERSION}\" apply false\n"
            ));
        } else {
            updated.push_str(line);
            updated.push('\n');
        }
    }
    if updated != existing {
        fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn apply_android_app_build_gradle_hardening(root: &Path) -> Result<()> {
    let path = root.join("platforms/android/app/build.gradle.kts");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut updated = existing.replace("../native-modules.gradle.kts", "../native-modules.gradle");
    if !updated.contains("../native-modules.gradle") {
        updated.push_str("\napply(from = \"../native-modules.gradle\")\n");
    }
    if updated != existing {
        fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn apply_android_gradle_properties_hardening(root: &Path) -> Result<()> {
    let path = root.join("platforms/android/gradle.properties");
    if !path.exists() {
        return fs::write(&path, render_android_gradle_properties())
            .with_context(|| format!("failed to write {}", path.display()));
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut saw_androidx = false;
    let mut saw_jvmargs = false;
    let mut saw_compile_warning = false;
    let mut updated = String::new();
    for line in existing.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("android.useAndroidX=") {
            updated.push_str("android.useAndroidX=true\n");
            saw_androidx = true;
        } else if trimmed.starts_with("org.gradle.jvmargs=") {
            updated.push_str(line);
            updated.push('\n');
            saw_jvmargs = true;
        } else if trimmed.starts_with("android.javaCompile.suppressSourceTargetDeprecationWarning=")
        {
            updated.push_str(line);
            updated.push('\n');
            saw_compile_warning = true;
        } else {
            updated.push_str(line);
            updated.push('\n');
        }
    }
    if !saw_androidx {
        if !updated.ends_with('\n') {
            updated.push('\n');
        }
        updated.push_str("android.useAndroidX=true\n");
    }
    if !saw_jvmargs {
        updated.push_str("org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8\n");
    }
    if !saw_compile_warning {
        updated.push_str("android.javaCompile.suppressSourceTargetDeprecationWarning=true\n");
    }
    if updated != existing {
        fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn apply_android_settings_gradle_hardening(root: &Path, project: &FissionProject) -> Result<()> {
    let path = root.join("platforms/android/settings.gradle.kts");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let missing = android_dependency_repositories(project)
        .into_iter()
        .filter(|repository| !existing.contains(repository))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }
    let marker = "    repositories {\n";
    let Some(index) = existing.find(marker) else {
        return Ok(());
    };
    let mut insertion = String::new();
    for repository in missing {
        insertion.push_str("        ");
        insertion.push_str(&repository);
        insertion.push('\n');
    }
    let mut updated = existing;
    updated.insert_str(index + marker.len(), &insertion);
    fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))
}

fn android_wait_for_boot_function() -> &'static str {
    r#"wait_for_android_boot() {
  "$ADB" wait-for-device
  until "$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r' | grep -q '^1$'; do
    sleep 1
  done
  local deadline=$((SECONDS + 180))
  until "$ADB" shell cmd package list packages >/dev/null 2>&1; do
    if (( SECONDS > deadline )); then
      printf 'Android package manager did not become available. Restart the emulator with ANDROID_EMULATOR_RESTART=1 and try again.\n' >&2
      exit 1
    fi
    sleep 1
  done
}"#
}

fn replace_android_boot_wait_after(mut text: String, marker: &str, replacement: &str) -> String {
    let Some(start) = text.find(marker) else {
        return text;
    };
    let wait_start = start + marker.len();
    let old_wait = "  \"$ADB\" wait-for-device\n  until \"$ADB\" shell getprop sys.boot_completed 2>/dev/null | tr -d '\\r' | grep -q '^1$'; do\n    sleep 1\n  done\n";
    if text[wait_start..].starts_with(old_wait) {
        text.replace_range(wait_start..wait_start + old_wait.len(), replacement);
    }
    text
}

const IOS_INFO_PLIST_PLUTIL_PATCH: &str = r#"cp "$SCRIPT_DIR/Info.plist" "$BUNDLE_DIR/Info.plist"
PLUTIL=$(xcrun --find plutil 2>/dev/null || command -v plutil || true)
if [[ -z "$PLUTIL" ]]; then
  printf 'plutil not found. Install Xcode command line tools to package the iOS simulator app.\n' >&2
  exit 1
fi
"$PLUTIL" -replace CFBundleIdentifier -string "$BUNDLE_ID" "$BUNDLE_DIR/Info.plist"
"$PLUTIL" -replace CFBundleDisplayName -string "$DISPLAY_NAME" "$BUNDLE_DIR/Info.plist"
"$PLUTIL" -replace CFBundleName -string "$DISPLAY_NAME" "$BUNDLE_DIR/Info.plist"
"$PLUTIL" -replace CFBundleExecutable -string "$EXECUTABLE_NAME" "$BUNDLE_DIR/Info.plist"
"#;

fn apply_platform_capability_config(root: &Path, project: &FissionProject) -> Result<()> {
    if project.capabilities.is_empty() {
        return Ok(());
    }
    if project.targets.contains(&Target::Android) {
        ensure_android_capability_helper(root)?;
        apply_android_capability_config(root, project)?;
    }
    if project.targets.contains(&Target::Ios) {
        apply_ios_capability_config(root, project)?;
    }
    Ok(())
}

fn ensure_android_capability_helper(root: &Path) -> Result<()> {
    write_file_with_policy(
        &root.join("platforms/android/java/rs/fission/runtime/FissionAndroidCapabilities.java"),
        render_android_capabilities_java(),
        WritePolicy::PreserveExisting,
    )
}

fn apply_android_capability_config(root: &Path, project: &FissionProject) -> Result<()> {
    let path = root.join("platforms/android/AndroidManifest.xml");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut capabilities = String::new();
    if project.capabilities.contains(&PlatformCapability::Nfc)
        && !existing.contains("android.permission.NFC")
    {
        capabilities.push_str(&render_android_nfc_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Notifications)
        && !existing.contains("android.permission.POST_NOTIFICATIONS")
    {
        capabilities.push_str(&render_android_notifications_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Biometric)
        && !existing.contains("android.permission.USE_BIOMETRIC")
    {
        capabilities.push_str(&render_android_biometric_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Bluetooth)
    {
        capabilities.push_str(&render_missing_android_bluetooth_manifest_entries(
            &existing,
        ));
    }
    if project
        .capabilities
        .contains(&PlatformCapability::BarcodeScanner)
        && !project.capabilities.contains(&PlatformCapability::Camera)
        && !existing.contains("android.permission.CAMERA")
    {
        capabilities.push_str(&render_android_barcode_camera_manifest_entries());
    }
    if project.capabilities.contains(&PlatformCapability::Camera) {
        capabilities.push_str(&render_missing_android_camera_manifest_entries(&existing));
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Geolocation)
        && !existing.contains("android.permission.ACCESS_FINE_LOCATION")
    {
        capabilities.push_str(&render_android_geolocation_manifest_entries());
    }
    if project.capabilities.contains(&PlatformCapability::Haptics)
        && !existing.contains("android.permission.VIBRATE")
    {
        capabilities.push_str(&render_android_haptics_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Microphone)
        && !existing.contains("android.permission.RECORD_AUDIO")
    {
        capabilities.push_str(&render_android_microphone_manifest_entries());
    }
    if project.capabilities.contains(&PlatformCapability::Wifi) {
        capabilities.push_str(&render_missing_android_wifi_manifest_entries(&existing));
    }
    if project
        .capabilities
        .contains(&PlatformCapability::VolumeControl)
        && !existing.contains("android.permission.MODIFY_AUDIO_SETTINGS")
    {
        capabilities.push_str(&render_android_volume_manifest_entries());
    }
    if capabilities.is_empty() {
        return Ok(());
    }
    let marker = r#"    <uses-permission android:name="android.permission.INTERNET" />"#;
    let updated = if existing.contains(marker) {
        existing.replacen(marker, &format!("{marker}\n{capabilities}"), 1)
    } else {
        existing.replacen("<uses-sdk", &format!("{capabilities}\n    <uses-sdk"), 1)
    };
    fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))
}

fn apply_ios_capability_config(root: &Path, project: &FissionProject) -> Result<()> {
    let info_path = root.join("platforms/ios/Info.plist");
    if info_path.exists() {
        let existing = fs::read_to_string(&info_path)
            .with_context(|| format!("failed to read {}", info_path.display()))?;
        if project.capabilities.contains(&PlatformCapability::Nfc)
            && !existing.contains("NFCReaderUsageDescription")
        {
            let entry = "  <key>NFCReaderUsageDescription</key>\n  <string>This app uses NFC to scan nearby tags when you request it.</string>\n";
            let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
            fs::write(&info_path, updated)
                .with_context(|| format!("failed to write {}", info_path.display()))?;
        }
    }

    if project.capabilities.contains(&PlatformCapability::Nfc) {
        let entitlements_path = root.join("platforms/ios/Entitlements.plist");
        if entitlements_path.exists() {
            let existing = fs::read_to_string(&entitlements_path)
                .with_context(|| format!("failed to read {}", entitlements_path.display()))?;
            if !existing.contains("com.apple.developer.nfc.readersession.formats") {
                let entry = "  <key>com.apple.developer.nfc.readersession.formats</key>\n  <array>\n    <string>NDEF</string>\n  </array>\n";
                let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
                fs::write(&entitlements_path, updated)
                    .with_context(|| format!("failed to write {}", entitlements_path.display()))?;
            }
        } else {
            write_file_with_policy(
                &entitlements_path,
                IOS_NFC_ENTITLEMENTS_PLIST,
                WritePolicy::PreserveExisting,
            )?;
        }
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Biometric)
        && info_path.exists()
    {
        let existing = fs::read_to_string(&info_path)
            .with_context(|| format!("failed to read {}", info_path.display()))?;
        if !existing.contains("NSFaceIDUsageDescription") {
            let entry = "  <key>NSFaceIDUsageDescription</key>\n  <string>This app uses biometrics to authenticate you when you request it.</string>\n";
            let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
            fs::write(&info_path, updated)
                .with_context(|| format!("failed to write {}", info_path.display()))?;
        }
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Bluetooth)
        && info_path.exists()
    {
        let existing = fs::read_to_string(&info_path)
            .with_context(|| format!("failed to read {}", info_path.display()))?;
        if !existing.contains("NSBluetoothAlwaysUsageDescription") {
            let entry = "  <key>NSBluetoothAlwaysUsageDescription</key>\n  <string>This app uses Bluetooth when you request nearby-device features.</string>\n";
            let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
            fs::write(&info_path, updated)
                .with_context(|| format!("failed to write {}", info_path.display()))?;
        }
    }
    if project
        .capabilities
        .contains(&PlatformCapability::BarcodeScanner)
        && info_path.exists()
    {
        let existing = fs::read_to_string(&info_path)
            .with_context(|| format!("failed to read {}", info_path.display()))?;
        if !existing.contains("NSCameraUsageDescription") {
            let entry = "  <key>NSCameraUsageDescription</key>\n  <string>This app uses the camera to scan barcodes when you request it.</string>\n";
            let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
            fs::write(&info_path, updated)
                .with_context(|| format!("failed to write {}", info_path.display()))?;
        }
    }
    if project.capabilities.contains(&PlatformCapability::Camera) && info_path.exists() {
        let existing = fs::read_to_string(&info_path)
            .with_context(|| format!("failed to read {}", info_path.display()))?;
        if !existing.contains("NSCameraUsageDescription") {
            let entry = "  <key>NSCameraUsageDescription</key>\n  <string>This app uses the camera when you request camera features.</string>\n";
            let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
            fs::write(&info_path, updated)
                .with_context(|| format!("failed to write {}", info_path.display()))?;
        }
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Geolocation)
        && info_path.exists()
    {
        let existing = fs::read_to_string(&info_path)
            .with_context(|| format!("failed to read {}", info_path.display()))?;
        if !existing.contains("NSLocationWhenInUseUsageDescription") {
            let entry = "  <key>NSLocationWhenInUseUsageDescription</key>\n  <string>This app uses your location when you request location-aware features.</string>\n";
            let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
            fs::write(&info_path, updated)
                .with_context(|| format!("failed to write {}", info_path.display()))?;
        }
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Microphone)
        && info_path.exists()
    {
        let existing = fs::read_to_string(&info_path)
            .with_context(|| format!("failed to read {}", info_path.display()))?;
        if !existing.contains("NSMicrophoneUsageDescription") {
            let entry = "  <key>NSMicrophoneUsageDescription</key>\n  <string>This app uses the microphone when you request audio capture.</string>\n";
            let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
            fs::write(&info_path, updated)
                .with_context(|| format!("failed to write {}", info_path.display()))?;
        }
    }
    if project.capabilities.contains(&PlatformCapability::Wifi) && info_path.exists() {
        let existing = fs::read_to_string(&info_path)
            .with_context(|| format!("failed to read {}", info_path.display()))?;
        if !existing.contains("NSLocationWhenInUseUsageDescription") {
            let entry = "  <key>NSLocationWhenInUseUsageDescription</key>\n  <string>This app uses location permission where the platform requires it for Wi-Fi information.</string>\n";
            let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
            fs::write(&info_path, updated)
                .with_context(|| format!("failed to write {}", info_path.display()))?;
        }
    }
    if project.capabilities.contains(&PlatformCapability::Wifi) {
        let entitlements_path = root.join("platforms/ios/Entitlements.plist");
        apply_ios_wifi_entitlements(&entitlements_path)?;
    }
    Ok(())
}

fn apply_ios_wifi_entitlements(path: &Path) -> Result<()> {
    if path.exists() {
        let existing = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let mut entry = String::new();
        if !existing.contains("com.apple.developer.networking.wifi-info") {
            entry.push_str("  <key>com.apple.developer.networking.wifi-info</key>\n  <true/>\n");
        }
        if !existing.contains("com.apple.developer.networking.HotspotConfiguration") {
            entry.push_str(
                "  <key>com.apple.developer.networking.HotspotConfiguration</key>\n  <true/>\n",
            );
        }
        if entry.is_empty() {
            return Ok(());
        }
        let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
        fs::write(path, updated).with_context(|| format!("failed to write {}", path.display()))?;
        return Ok(());
    }
    write_file_with_policy(
        path,
        IOS_WIFI_ENTITLEMENTS_PLIST,
        WritePolicy::PreserveExisting,
    )
}

fn target_scaffold_dir_exists(project_dir: &Path, target: Target) -> bool {
    Path::new(target.scaffold_relative_path())
        .parent()
        .is_some_and(|relative| project_dir.join(relative).exists())
}

fn write_project_config(root: &Path, project: &FissionProject) -> Result<()> {
    let path = root.join("fission.toml");
    if path.exists() {
        let existing = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let mut doc = existing
            .parse::<DocumentMut>()
            .with_context(|| format!("failed to parse {}", path.display()))?;
        update_project_config_document(&mut doc, project);
        write_file(&path, &doc.to_string())?;
        return Ok(());
    }
    let data = toml::to_string_pretty(project)?;
    write_file(&path, &(data + "\n"))
}

fn update_project_config_document(doc: &mut DocumentMut, project: &FissionProject) {
    doc["targets"] = value(string_array(
        project.targets.iter().map(|target| target.as_str()),
    ));
    if project.capabilities.is_empty() {
        doc.as_table_mut().remove("capabilities");
    } else {
        doc["capabilities"] = value(string_array(
            project
                .capabilities
                .iter()
                .map(|capability| capability.as_str()),
        ));
    }

    if !doc["app"].is_table() {
        doc["app"] = Item::Table(Table::new());
    }
    doc["app"]["name"] = value(project.app.name.clone());
    doc["app"]["app_id"] = value(project.app.app_id.clone());
    if let Some(splash) = &project.app.splash {
        if !doc["app"]["splash"].is_table() {
            doc["app"]["splash"] = Item::Table(Table::new());
        }
        let splash_item = &mut doc["app"]["splash"];
        if let Some(background_color) = &splash.background_color {
            splash_item["background_color"] = value(background_color.clone());
        }
        if let Some(image) = &splash.image {
            splash_item["image"] = value(image.clone());
        }
        if let Some(resize_mode) = splash.resize_mode {
            splash_item["resize_mode"] = value(match resize_mode {
                SplashResizeMode::Center => "center",
                SplashResizeMode::Contain => "contain",
                SplashResizeMode::Cover => "cover",
            });
        }
        if let Some(animated_icon) = &splash.android_animated_icon {
            splash_item["android_animated_icon"] = value(animated_icon.clone());
        }
        if let Some(duration) = splash.android_animation_duration_ms {
            splash_item["android_animation_duration_ms"] = value(i64::from(duration));
        }
    } else if let Some(app) = doc["app"].as_table_like_mut() {
        app.remove("splash");
    }
}

fn string_array<'a>(values: impl Iterator<Item = &'a str>) -> Array {
    let mut array = Array::new();
    for value in values {
        let mut value = Value::from(value);
        value.decor_mut().set_prefix("\n    ");
        array.push_formatted(value);
    }
    array.set_trailing("\n");
    array.set_trailing_comma(true);
    array
}

pub fn read_project_config(root: &Path) -> Result<FissionProject> {
    let path = root.join("fission.toml");
    let data = fs::read_to_string(&path).with_context(|| {
        format!(
            "failed to read {}; run `fission init {}` to register this project without overwriting existing files",
            path.display(),
            root.display()
        )
    })?;
    toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))
}

fn update_cargo_fission_features(root: &Path, project: &FissionProject) -> Result<()> {
    sync_cargo_fission_dependency(root, project, None)
}

fn sync_cargo_fission_dependency(
    root: &Path,
    project: &FissionProject,
    local_path: Option<&Path>,
) -> Result<()> {
    let path = root.join("Cargo.toml");
    let Ok(text) = fs::read_to_string(&path) else {
        return Ok(());
    };

    let mut doc = text
        .parse::<DocumentMut>()
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let features = fission_features_for_targets(&project.targets);
    let mut changed = false;

    if !doc.get("dependencies").is_some_and(Item::is_table_like) {
        doc["dependencies"] = Item::Table(Table::new());
        changed = true;
    }

    let use_workspace_fission = local_path.is_none()
        && workspace_has_fission_dependency(&doc)
        && doc
            .get("dependencies")
            .and_then(Item::as_table_like)
            .is_none_or(|dependencies| !dependencies.contains_key("fission"));
    let deps = doc["dependencies"]
        .as_table_like_mut()
        .expect("dependencies table was just created");
    let dep = deps.entry("fission").or_insert(Item::None);
    changed |= sync_fission_dependency_item(dep, &features, local_path, use_workspace_fission)?;

    if changed {
        fs::write(&path, doc.to_string())
            .with_context(|| format!("failed to update {}", path.display()))?;
    }
    Ok(())
}

fn workspace_has_fission_dependency(doc: &DocumentMut) -> bool {
    doc.get("workspace")
        .and_then(Item::as_table_like)
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(Item::as_table_like)
        .is_some_and(|dependencies| dependencies.contains_key("fission"))
}

fn sync_fission_dependency_item(
    item: &mut Item,
    features: &[&'static str],
    local_path: Option<&Path>,
    use_workspace_fission: bool,
) -> Result<bool> {
    match item {
        Item::None => {
            *item = Item::Value(Value::InlineTable(new_fission_dependency_table(
                features,
                local_path,
                use_workspace_fission,
            )));
            Ok(true)
        }
        Item::Value(Value::String(version)) => {
            let mut table = InlineTable::new();
            table.insert("version", Value::String(version.clone()));
            sync_fission_inline_table(&mut table, features, local_path, use_workspace_fission);
            *item = Item::Value(Value::InlineTable(table));
            Ok(true)
        }
        Item::Value(Value::InlineTable(table)) => Ok(sync_fission_inline_table(
            table,
            features,
            local_path,
            use_workspace_fission,
        )),
        Item::Table(table) => Ok(sync_fission_table(
            table,
            features,
            local_path,
            use_workspace_fission,
        )),
        _ => bail!("unsupported fission dependency format in Cargo.toml"),
    }
}

fn new_fission_dependency_table(
    features: &[&'static str],
    local_path: Option<&Path>,
    use_workspace_fission: bool,
) -> InlineTable {
    let mut table = InlineTable::new();
    if let Some(root) = local_path {
        table.insert(
            "path",
            Value::from(
                root.join("crates/authoring/fission")
                    .to_string_lossy()
                    .to_string(),
            ),
        );
    } else if use_workspace_fission {
        table.insert("workspace", Value::from(true));
    } else {
        table.insert("version", Value::from(CURRENT_VERSION));
    }
    table.insert("default-features", Value::from(false));
    table.insert("features", cargo_feature_array_value(features));
    table
}

fn sync_fission_inline_table(
    table: &mut InlineTable,
    features: &[&'static str],
    local_path: Option<&Path>,
    use_workspace_fission: bool,
) -> bool {
    let before = table.to_string();
    if let Some(root) = local_path {
        table.insert(
            "path",
            Value::from(
                root.join("crates/authoring/fission")
                    .to_string_lossy()
                    .to_string(),
            ),
        );
        table.remove("version");
        table.remove("workspace");
    } else if use_workspace_fission
        && !table.contains_key("path")
        && !table.contains_key("version")
        && !table.contains_key("git")
    {
        table.insert("workspace", Value::from(true));
    } else if !table.contains_key("path")
        && !table.contains_key("version")
        && !table.contains_key("workspace")
        && !table.contains_key("git")
    {
        table.insert("version", Value::from(CURRENT_VERSION));
    }
    table.insert("default-features", Value::from(false));
    table.insert("features", cargo_feature_array_value(features));
    table.to_string() != before
}

fn sync_fission_table(
    table: &mut Table,
    features: &[&'static str],
    local_path: Option<&Path>,
    use_workspace_fission: bool,
) -> bool {
    let before = table.to_string();
    if let Some(root) = local_path {
        table["path"] = value(
            root.join("crates/authoring/fission")
                .to_string_lossy()
                .to_string(),
        );
        table.remove("version");
        table.remove("workspace");
    } else if use_workspace_fission
        && !table.contains_key("path")
        && !table.contains_key("version")
        && !table.contains_key("git")
    {
        table["workspace"] = value(true);
    } else if !table.contains_key("path")
        && !table.contains_key("version")
        && !table.contains_key("workspace")
        && !table.contains_key("git")
    {
        table["version"] = value(CURRENT_VERSION);
    }
    table["default-features"] = value(false);
    table["features"] = Item::Value(cargo_feature_array_value(features));
    table.to_string() != before
}

fn cargo_feature_array_value(features: &[&'static str]) -> Value {
    let mut array = Array::new();
    for feature in features {
        array.push(*feature);
    }
    Value::Array(array)
}

fn scaffold_target_with_policy(
    root: &Path,
    project: &FissionProject,
    target: Target,
    write_policy: WritePolicy,
) -> Result<()> {
    let relative = Path::new(target.scaffold_relative_path());
    let text = match target {
        Target::Android => {
            scaffold_android_bundle(root, project, write_policy)?;
            platform_readme(
                "Android",
                "Runnable emulator target. The CLI generates a Gradle Android project shell plus scripts that build, install, and launch the Fission app on an Android emulator.",
                &[
                    "Install the Rust target: `rustup target add aarch64-linux-android`.",
                    "Run `fission doctor android --project-dir .` to check SDK, NDK, emulator, and Rust target setup.",
                    "Run `fission devices --project-dir .` to list connected Android devices and configured emulators.",
                    "Run `fission run --target android --project-dir .` to build, install, launch, and attach to logs.",
                    "Run `fission run --target android --device <adb-serial> --project-dir .` to launch on a specific device.",
                    "Run `fission test --target android --project-dir .` for an emulator launch plus test-control health check.",
                    "Run `./platforms/android/run-emulator.sh` from the project root to build, package, install, and launch the app on the configured emulator.",
                    "Override `ANDROID_HOME`, `ANDROID_NDK`, `ANDROID_MIN_API_LEVEL`, `ANDROID_TARGET_API_LEVEL`, `ANDROID_AVD_NAME`, or `ANDROID_SYSTEM_IMAGE` if your local SDK setup differs.",
                    "Set `ANDROID_EMULATOR_HEADLESS=1` for background/CI runs, or `ANDROID_EMULATOR_RESTART=1` to relaunch a hidden emulator visibly.",
                    "The generated package uses `assets/app-icon.png` as its default launcher icon.",
                    "Configure `[app.splash]` in `fission.toml` to generate the native Android launch theme, splash background, static image, and optional Android animated drawable.",
                    "Run `fission add-capability nfc --project-dir .` to add NFC manifest permission and feature declarations.",
                    "Run `fission add-capability notifications --project-dir .` to add Android notification permission for API 33 and newer.",
                    "Run `fission add-capability biometric --project-dir .` to add biometric manifest permissions.",
                    "Run `fission add-capability passkeys --project-dir .` to record passkey/WebAuthn use. Android passkeys also require Digital Asset Links and host Credential Manager integration for production sign-in.",
                    "Run `fission add-capability bluetooth --project-dir .` to add Bluetooth permissions and optional hardware feature declarations.",
                    "Run `fission add-capability barcode-scanner --project-dir .` to add camera permission for barcode scanning.",
                    "Run `fission add-capability camera --project-dir .` to add camera permission and optional camera/flash hardware feature declarations.",
                    "Run `fission add-capability geolocation --project-dir .` to add location permissions.",
                    "Run `fission add-capability haptics --project-dir .` to add the vibration permission.",
                    "Run `fission add-capability microphone --project-dir .` to add audio recording permission.",
                    "Run `fission add-capability volume-control --project-dir .` to add Android audio settings permission.",
                    "Run `fission add-capability wifi --project-dir .` to add Wi-Fi permissions and optional hardware feature declarations.",
                    "Set `FISSION_TEST_CONTROL_PORT=<host-port>` before `run-emulator.sh`; the script forwards it to the fixed in-app device port.",
                ],
            )
        }
        Target::Ios => {
            scaffold_ios_bundle(root, project, write_policy)?;
            platform_readme(
                "iOS",
                "Simulator target. The CLI generates a simulator app bundle template plus shell scripts that build, install, launch, and smoke-test the Fission app with `simctl`.",
                &[
                    "Install the Rust targets: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`.",
                    "Run `fission doctor ios --project-dir .` to check Xcode, simulator, and Rust target setup.",
                    "Confirm the simulator SDK path with `xcrun --sdk iphonesimulator --show-sdk-path`.",
                    "Run `fission devices --project-dir .` to list available iOS simulators.",
                    "Run `fission run --target ios --project-dir .` to build, install, launch, and attach to simulator logs.",
                    "Run `fission run --target ios --device <simulator-udid> --project-dir .` to launch on a specific simulator.",
                    "Run `fission test --target ios --project-dir .` for a simulator launch plus test-control health check.",
                    "Run `./platforms/ios/run-sim.sh` from the project root to build, install, and launch the app on the first available iPhone simulator.",
                    "The generated bundle uses `assets/app-icon.png` as its default app icon.",
                    "Configure `[app.splash]` in `fission.toml` to generate the native iOS launch storyboard and splash image copied into the simulator bundle.",
                    "Run `fission add-capability nfc --project-dir .` to add the NFC usage description and entitlements file.",
                    "Run `fission add-capability notifications --project-dir .` to record local-notification use. iOS prompts at runtime and does not require an Info.plist usage key for local notifications.",
                    "Run `fission add-capability biometric --project-dir .` to add the Face ID usage description.",
                    "Run `fission add-capability passkeys --project-dir .` to record passkey/WebAuthn use. iOS production passkeys require associated domains such as `webcredentials:example.com` in the app entitlements.",
                    "Run `fission add-capability bluetooth --project-dir .` to add the Bluetooth usage description.",
                    "Run `fission add-capability barcode-scanner --project-dir .` to add the camera usage description for barcode scanning.",
                    "Run `fission add-capability camera --project-dir .` to add the camera usage description.",
                    "Run `fission add-capability geolocation --project-dir .` to add the location usage description.",
                    "Run `fission add-capability microphone --project-dir .` to add the microphone usage description.",
                    "Run `fission add-capability wifi --project-dir .` to add Wi-Fi entitlements and the location usage description required by current-network information APIs.",
                    "Volume control does not require an iOS Info.plist key in the generated scaffold.",
                    "Haptics do not require an iOS Info.plist key in the generated scaffold.",
                    "Set `FISSION_TEST_CONTROL_PORT=<port>` before `run-sim.sh` to expose the in-app test control server on the host.",
                    "Set `IOS_SIM_DEVICE_ID=<udid>` if you want a specific simulator device.",
                    "Set `IOS_SIM_HEADLESS=1` for CI or background-only simulator runs; otherwise the script opens Simulator visibly.",
                ],
            )
        }
        Target::Web => {
            scaffold_web_bundle(root, project, write_policy)?;
            platform_readme(
                "Web",
                "Runnable browser target. The CLI generates a WASM host page plus helper scripts that build the app with `wasm-pack` and serve it locally.",
                &[
                    "Install the Rust target: `rustup target add wasm32-unknown-unknown`.",
                    "Install `wasm-pack` once: `cargo install wasm-pack`.",
                    "Install Node.js 22+ so the smoke test can inspect Chrome/Chromium CDP runtime and console output.",
                    "Run `fission doctor web --project-dir .` to check wasm-pack, generated JavaScript glue, Chrome/Chromium, and Rust target setup.",
                    "Run `fission devices --project-dir .` to confirm Chrome/Chromium detection.",
                    "Run `fission run --target web --project-dir .` to build, serve, open, and attach to the local server.",
                    "Run `fission run --target web --detach --project-dir .` to keep the local server running in the background.",
                    "Run `fission test --target web --project-dir .` for a headless Chrome/Chromium CDP smoke test.",
                    "Run `./platforms/web/run-browser.sh` from the project root to build the wasm package and serve the app locally.",
                    "Set `FISSION_WEB_PORT=<port>` or `FISSION_WEB_HOST=<host>` if the default `127.0.0.1:8123` does not suit your machine.",
                    "Set `FISSION_WEB_OPEN=1` if you want the helper script to open a browser tab automatically.",
                    "The generated page uses `assets/app-icon.png` as its default favicon/app icon seed.",
                ],
            )
        }
        Target::Server => platform_readme(
            "Server",
            "Server-rendered Fission target. The CLI runs the app through the server shell for dynamic HTML, revalidated pages, server jobs, signed actions, worker artifacts, and focused browser islands.",
            &[
                "Configure `[server].entry` in `fission.toml` so the CLI can invoke the server app.",
                "Run `fission server check --project-dir .` to render all declared server routes.",
                "Run `fission server serve --project-dir .` to serve the app locally.",
                "Run `fission server artifacts --project-dir .` to generate browser worker and island WASM shims.",
                "Run `fission package --target server --format docker-image --release --project-dir .` to package the server app as an OCI/Docker image.",
            ],
        ),
        Target::Site => {
            write_file_with_policy(
                &root.join("content/getting-started.md"),
                "---\ntitle: Site content\ndescription: Static site content rendered by the Fission static site shell.\n---\n\n# Site content\n\nAdd Markdown files under `content/`. `fission site build` renders them through real Fission widgets, lowers the nodes to Core IR, and emits static HTML.\n",
                write_policy,
            )?;
            platform_readme(
                "Static site",
                "Static multi-page website target. The site shell renders Markdown content through real Fission widgets, lowers nodes to Core IR, and emits semantic static HTML.",
                &[
                    "Add Markdown or MDX content under `content/`.",
                    "Run `fission site routes --project-dir .` to list generated routes.",
                    "Run `fission site build --project-dir .` to render HTML into `target/fission/site`.",
                    "Run `fission site serve --project-dir .` to build and serve the generated site locally.",
                    "Unsupported interactive widgets fail during the static render instead of silently falling back to JavaScript.",
                ],
            )
        }
        Target::Linux | Target::Macos | Target::Windows => platform_readme(
            match target {
                Target::Linux => "Linux",
                Target::Macos => "macOS",
                Target::Windows => "Windows",
                _ => unreachable!(),
            },
            "Runnable target. Desktop platforms share the default `src/main.rs` entrypoint through `DesktopApp`.",
            &[
                "Run `fission run --project-dir .` from the project root to launch the desktop app and attach output.",
                "Run `fission build --project-dir . --release` for a release desktop build.",
                "Run `fission test --project-dir .` for the app crate's Rust tests.",
                "This target uses the default Vello desktop shell path.",
            ],
        ),
    };
    write_file_with_policy(&root.join(relative), &text, write_policy)
}

fn scaffold_ios_bundle(
    root: &Path,
    project: &FissionProject,
    write_policy: WritePolicy,
) -> Result<()> {
    let executable = ios_executable_name(project);
    let bundle_name = ios_bundle_name(project);
    let plist = render_ios_plist(project, &executable);
    let package_script = render_ios_package_script(project, &bundle_name, &executable);
    let run_script = render_ios_run_script(project);
    let test_script = render_ios_test_script();

    write_file_with_policy(&root.join("platforms/ios/Info.plist"), &plist, write_policy)?;
    write_file_with_policy(
        &root.join("platforms/ios/Package.swift"),
        &render_ios_host_package(project),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/ios/Sources/FissionHost/FissionNativeCapabilities.swift"),
        render_ios_host_native_capabilities_swift(),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/ios/NativeModules/README.md"),
        IOS_NATIVE_MODULES_README,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/ios/NativeModules/Package.swift"),
        &render_ios_native_modules_package(project),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join(
            "platforms/ios/NativeModules/Sources/FissionNativeModules/FissionNativeCapabilities.swift",
        ),
        render_ios_native_capabilities_swift(),
        write_policy,
    )?;
    sync_ios_native_module_sources(root, project)?;
    if project.capabilities.contains(&PlatformCapability::Nfc)
        || project.capabilities.contains(&PlatformCapability::Wifi)
    {
        write_file_with_policy(
            &root.join("platforms/ios/Entitlements.plist"),
            &render_ios_entitlements_plist(project),
            write_policy,
        )?;
    }
    write_file_with_policy(
        &root.join("platforms/ios/package-sim.sh"),
        &package_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/ios/run-sim.sh"),
        &run_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/ios/test-sim.sh"),
        &test_script,
        write_policy,
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for relative in [
            "platforms/ios/package-sim.sh",
            "platforms/ios/run-sim.sh",
            "platforms/ios/test-sim.sh",
        ] {
            let path = root.join(relative);
            if path.exists() {
                fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
            }
        }
    }
    Ok(())
}

fn scaffold_android_bundle(
    root: &Path,
    project: &FissionProject,
    write_policy: WritePolicy,
) -> Result<()> {
    let manifest = render_android_manifest(project);
    let package_script = render_android_package_script(project);
    let run_script = render_android_run_script(project);
    let test_script = render_android_test_script();

    write_file_with_policy(
        &root.join("platforms/android/settings.gradle.kts"),
        &render_android_settings_gradle(project),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/build.gradle.kts"),
        &render_android_root_build_gradle(),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/gradle.properties"),
        render_android_gradle_properties(),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/app/build.gradle.kts"),
        &render_android_app_build_gradle(project),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/native-modules.gradle"),
        &render_android_native_modules_gradle(project),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/AndroidManifest.xml"),
        &manifest,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/package-apk.sh"),
        &package_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/run-emulator.sh"),
        &run_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/test-emulator.sh"),
        &test_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/java/rs/fission/runtime/FissionActivity.java"),
        render_android_activity_java(),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/native-modules/README.md"),
        ANDROID_NATIVE_MODULES_README,
        write_policy,
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for relative in [
            "platforms/android/package-apk.sh",
            "platforms/android/run-emulator.sh",
            "platforms/android/test-emulator.sh",
        ] {
            let path = root.join(relative);
            if path.exists() {
                fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
            }
        }
    }
    Ok(())
}

fn scaffold_web_bundle(
    root: &Path,
    project: &FissionProject,
    write_policy: WritePolicy,
) -> Result<()> {
    let index_html = render_web_index(project);
    let bootstrap = render_web_bootstrap(project);
    let build_script = render_web_build_script();
    let run_script = render_web_run_script(project);
    let test_script = render_web_test_script(project);

    write_file_with_policy(
        &root.join("platforms/web/index.html"),
        &index_html,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/web/bootstrap.mjs"),
        &bootstrap,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/web/build-wasm.sh"),
        &build_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/web/run-browser.sh"),
        &run_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/web/test-browser.sh"),
        &test_script,
        write_policy,
    )?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for relative in [
            "platforms/web/build-wasm.sh",
            "platforms/web/run-browser.sh",
            "platforms/web/test-browser.sh",
        ] {
            let path = root.join(relative);
            if path.exists() {
                let mut perms = fs::metadata(&path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(path, perms)?;
            }
        }
    }

    Ok(())
}

pub(crate) fn write_file(path: &Path, contents: &str) -> Result<()> {
    write_file_with_policy(path, contents, WritePolicy::Overwrite)
}

fn write_file_with_policy(path: &Path, contents: &str, write_policy: WritePolicy) -> Result<()> {
    if write_policy == WritePolicy::PreserveExisting && path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

fn write_binary_file_with_policy(
    path: &Path,
    contents: &[u8],
    write_policy: WritePolicy,
) -> Result<()> {
    if write_policy == WritePolicy::PreserveExisting && path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

fn render_cargo_toml(project: &FissionProject, local_path: Option<&Path>) -> String {
    let feature_list = render_fission_feature_list(&project.targets);
    let deps = if let Some(root) = local_path {
        let fission_path = root.join("crates/authoring/fission");
        format!(
            "fission = {{ path = {:?}, default-features = false, features = [{}] }}\n",
            fission_path.to_string_lossy().to_string(),
            feature_list
        )
    } else {
        format!(
            "fission = {{ version = \"{}\", default-features = false, features = [{}] }}\n",
            CURRENT_VERSION, feature_list
        )
    };
    let lib_name = project.app.name.replace('-', "_");

    format!(
        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\nname = \"{}\"\ncrate-type = [\"cdylib\", \"rlib\"]\n\n[dependencies]\nanyhow = \"1\"\nserde = {{ version = \"1\", features = [\"derive\"] }}\n{}\n[target.'cfg(target_arch = \"wasm32\")'.dependencies]\nconsole_error_panic_hook = \"0.1\"\nwasm-bindgen = \"0.2\"\n",
        project.app.name, lib_name, deps
    )
}

fn render_fission_feature_list(targets: &BTreeSet<Target>) -> String {
    fission_features_for_targets(targets)
        .into_iter()
        .map(|feature| format!("\"{feature}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

fn fission_features_for_targets(targets: &BTreeSet<Target>) -> Vec<&'static str> {
    let mut features = Vec::new();
    if targets
        .iter()
        .any(|target| matches!(target, Target::Linux | Target::Macos | Target::Windows))
    {
        features.push("desktop");
    }
    if targets.contains(&Target::Web) {
        features.push("web");
    }
    if targets.contains(&Target::Android) {
        features.push("android");
    }
    if targets.contains(&Target::Ios) {
        features.push("ios");
    }
    if targets.contains(&Target::Site) {
        features.push("site");
    }
    if targets.contains(&Target::Server) {
        features.push("server");
    }
    features
}

fn render_project_readme(project: &FissionProject) -> String {
    let mut targets = String::new();
    for target in &project.targets {
        targets.push_str(&format!("- `{}`\n", target.as_str()));
    }
    format!(
        "# {}\n\nGenerated by `fission init`.\n\n## Targets\n\n{}\n## Commands\n\n- `fission doctor --project-dir .` -- check local SDKs, browsers, emulators, and Rust targets\n- `fission devices --project-dir .` -- list runnable desktop, browser, simulator, emulator, and device targets\n- `fission run --project-dir .` -- launch the desktop app and attach to output\n- `fission run --target web --project-dir .` -- launch the web app and attach to the local server\n- `fission run --target ios --project-dir .` -- build, install, launch, and attach to simulator logs\n- `fission run --target android --project-dir .` -- build, install, launch, and attach to Android logs\n- `fission run --target <target> --device <id> --detach --project-dir .` -- launch without attaching\n- `fission logs --target <target> --device <id> --project-dir . --follow` -- attach later where supported\n- `fission build --target <target> --project-dir . --release` -- build a target without launching it\n- `fission test --target <target> --project-dir .` -- run the generated platform smoke test\n- `fission add-target web ios android --project-dir .` -- scaffold more targets\n- `fission add-capability nfc notifications biometric passkeys bluetooth barcode-scanner camera geolocation haptics microphone volume-control wifi --project-dir .` -- declare host capabilities and update platform config where possible\n- `cat platforms/<target>/README.md` -- inspect target-specific prerequisites and environment variables\n\n## Assets\n\n- `assets/app-icon.png` is the default app icon seed copied from Fission's `docs/fission_logo.png`\n\n## Status\n\nDesktop, web, iOS simulator, and Android emulator workflows are runnable through `fission run`. The platform scripts remain checked in so CI and advanced users can call the lower-level build, run, and smoke-test steps directly when needed.\n",
        project.app.name, targets
    )
}

fn platform_readme(title: &str, summary: &str, bullets: &[&str]) -> String {
    let mut out = format!("# {} target\n\n{}\n", title, summary);
    for bullet in bullets {
        out.push_str(&format!("\n- {}", bullet));
    }
    out.push('\n');
    out
}

fn normalize_crate_name(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            'A'..='Z' => ch.to_ascii_lowercase(),
            'a'..='z' | '0'..='9' => ch,
            _ => '-',
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub fn ios_executable_name(project: &FissionProject) -> String {
    project.app.name.replace('-', "_")
}

fn ios_bundle_name(project: &FissionProject) -> String {
    let mut out = String::new();
    let mut uppercase_next = true;
    for ch in project.app.name.chars() {
        match ch {
            '-' | '_' | ' ' => uppercase_next = true,
            _ if uppercase_next => {
                out.extend(ch.to_uppercase());
                uppercase_next = false;
            }
            _ => out.push(ch),
        }
    }
    if out.is_empty() {
        "FissionApp".to_string()
    } else {
        out
    }
}

fn android_library_name(project: &FissionProject) -> String {
    project.app.name.replace('-', "_")
}

fn android_root_project_name(project: &FissionProject) -> String {
    project.app.name.replace('-', "_")
}

fn render_android_settings_gradle(project: &FissionProject) -> String {
    let repositories = android_dependency_repositories(project)
        .into_iter()
        .map(|repository| format!("        {repository}\n"))
        .collect::<String>();
    format!(
        r#"pluginManagement {{
    repositories {{
        google()
        mavenCentral()
        gradlePluginPortal()
    }}
}}

dependencyResolutionManagement {{
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {{
{repositories}
    }}
}}

rootProject.name = "{name}-android"
include(":app")
"#,
        name = android_root_project_name(project),
    )
}

fn render_android_root_build_gradle() -> String {
    format!(
        r#"plugins {{
    id("com.android.application") version "{ANDROID_GRADLE_PLUGIN_VERSION}" apply false
}}
"#
    )
}

fn render_android_gradle_properties() -> &'static str {
    "android.useAndroidX=true\norg.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8\nandroid.javaCompile.suppressSourceTargetDeprecationWarning=true\n"
}

fn render_android_app_build_gradle(project: &FissionProject) -> String {
    format!(
        r#"plugins {{
    id("com.android.application")
}}

android {{
    namespace = "{app_id}"
    compileSdk = (System.getenv("ANDROID_TARGET_API_LEVEL") ?: "35").toInt()

    defaultConfig {{
        applicationId = "{app_id}"
        minSdk = (System.getenv("ANDROID_MIN_API_LEVEL") ?: "24").toInt()
        targetSdk = (System.getenv("ANDROID_TARGET_API_LEVEL") ?: "35").toInt()
        versionCode = 1
        versionName = "0.1.0"
    }}

    sourceSets {{
        getByName("main") {{
            manifest.srcFile("../AndroidManifest.xml")
            java.srcDirs("../java")
            res.srcDirs("../res", "src/main/res")
            jniLibs.srcDirs("src/main/jniLibs")
        }}
    }}
}}

apply(from = "../native-modules.gradle")
"#,
        app_id = project.app.app_id,
    )
}

fn render_android_native_modules_gradle(project: &FissionProject) -> String {
    let mut dependencies = Vec::new();
    let mut source_dirs = Vec::new();
    for module in &project.native.modules {
        for dependency in &module.android.gradle_dependencies {
            if let Some(dependency) = normalize_gradle_dependency(dependency) {
                dependencies.push((module.name.as_str(), dependency));
            }
        }
        for source_dir in &module.android.source_dirs {
            let source_dir = source_dir.trim();
            if !source_dir.is_empty() {
                source_dirs.push((module.name.as_str(), source_dir.to_string()));
            }
        }
    }

    let mut out = String::from(
        "// Generated by Fission. Native capability modules append Android SDK wiring here.\n",
    );
    if dependencies.is_empty() && source_dirs.is_empty() {
        out.push_str("// No Android native modules are configured in fission.toml.\n");
        return out;
    }
    if !source_dirs.is_empty() {
        out.push_str("\ndef fissionProjectDir = rootProject.projectDir.toPath().resolve('../..').normalize().toFile()\n");
        out.push_str("android {\n");
        out.push_str("    sourceSets {\n");
        out.push_str("        main {\n");
        for (module, source_dir) in &source_dirs {
            out.push_str("            // ");
            out.push_str(module);
            out.push('\n');
            out.push_str("            java.srcDir(new File(fissionProjectDir, ");
            out.push_str(&groovy_string_literal(source_dir));
            out.push_str("))\n");
        }
        out.push_str("        }\n");
        out.push_str("    }\n");
        out.push_str("}\n");
    }
    if !dependencies.is_empty() {
        out.push_str("\ndependencies {\n");
        for (module, dependency) in dependencies {
            out.push_str("    // ");
            out.push_str(module);
            out.push('\n');
            out.push_str("    ");
            out.push_str(&dependency);
            out.push('\n');
        }
        out.push_str("}\n");
    }
    out
}

fn android_dependency_repositories(project: &FissionProject) -> BTreeSet<String> {
    let mut repositories = BTreeSet::new();
    repositories.insert("google()".to_string());
    repositories.insert("mavenCentral()".to_string());
    for module in &project.native.modules {
        for repository in &module.android.repositories {
            if let Some(repository) = normalize_gradle_repository(repository) {
                repositories.insert(repository);
            }
        }
    }
    repositories
}

fn normalize_gradle_repository(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    match value {
        "google" | "google()" => Some("google()".to_string()),
        "mavenCentral" | "mavenCentral()" => Some("mavenCentral()".to_string()),
        "gradlePluginPortal" | "gradlePluginPortal()" => Some("gradlePluginPortal()".to_string()),
        _ if value.contains('(') => Some(value.to_string()),
        _ => Some(format!("maven(\"{value}\")")),
    }
}

fn normalize_gradle_dependency(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Some((configuration, dependency)) = split_gradle_dependency_invocation(value) {
        Some(format!("{configuration} {}", dependency.trim()))
    } else if value.contains('(') {
        Some(format!("implementation {value}"))
    } else {
        Some(format!("implementation {}", groovy_string_literal(value)))
    }
}

fn split_gradle_dependency_invocation(value: &str) -> Option<(&str, &str)> {
    let open = value.find('(')?;
    if !value.ends_with(')') {
        return None;
    }
    let configuration = value[..open].trim();
    if !is_gradle_dependency_configuration(configuration) {
        return None;
    }
    let dependency = value[open + 1..value.len() - 1].trim();
    if dependency.is_empty() {
        return None;
    }
    Some((configuration, dependency))
}

fn is_gradle_dependency_configuration(value: &str) -> bool {
    matches!(
        value,
        "implementation"
            | "api"
            | "compileOnly"
            | "runtimeOnly"
            | "testImplementation"
            | "testCompileOnly"
            | "testRuntimeOnly"
            | "androidTestImplementation"
            | "androidTestCompileOnly"
            | "androidTestRuntimeOnly"
            | "debugImplementation"
            | "debugCompileOnly"
            | "debugRuntimeOnly"
            | "releaseImplementation"
            | "releaseCompileOnly"
            | "releaseRuntimeOnly"
            | "kapt"
            | "ksp"
    )
}

fn groovy_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn render_android_activity_java() -> &'static str {
    r#"package rs.fission.runtime;

public final class FissionActivity extends android.app.NativeActivity {
}
"#
}

const ANDROID_NATIVE_MODULES_README: &str = r#"# Android native modules

This directory is reserved for native capability module sources copied or owned by the app shell.

Generic dependency and repository wiring is generated into `../native-modules.gradle` from
`fission.toml` `[native]` module declarations. Fission does not ship payment, camera-addon,
scanner-addon, or other app-specific modules in core; those crates provide their native adapters.
"#;

fn render_ios_host_package(project: &FissionProject) -> String {
    format!(
        r#"// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "{name}FissionHost",
    platforms: [
        .iOS(.v16),
    ],
    products: [
        .library(name: "FissionHost", targets: ["FissionHost"]),
    ],
    dependencies: [
        .package(path: "NativeModules"),
    ],
    targets: [
        .target(
            name: "FissionHost",
            dependencies: [
                .product(name: "FissionNativeModules", package: "NativeModules"),
            ],
            path: "Sources/FissionHost"
        ),
    ]
)
"#,
        name = ios_bundle_name(project),
    )
}

fn render_ios_native_modules_package(project: &FissionProject) -> String {
    let package_dependencies = project
        .native
        .modules
        .iter()
        .flat_map(|module| module.ios.swift_packages.iter())
        .map(render_ios_swift_package_dependency)
        .collect::<Vec<_>>();
    let target_dependencies = project
        .native
        .modules
        .iter()
        .flat_map(|module| module.ios.swift_packages.iter())
        .map(render_ios_swift_product_dependency)
        .collect::<Vec<_>>();

    let dependencies = if package_dependencies.is_empty() {
        String::new()
    } else {
        format!(
            "\n        {}\n    ",
            package_dependencies.join(",\n        ")
        )
    };
    let target_dependencies = if target_dependencies.is_empty() {
        String::new()
    } else {
        format!(
            "\n                {}\n            ",
            target_dependencies.join(",\n                ")
        )
    };

    format!(
        r#"// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "NativeModules",
    platforms: [
        .iOS(.v16),
    ],
    products: [
        .library(name: "FissionNativeModules", targets: ["FissionNativeModules"]),
    ],
    dependencies: [{dependencies}],
    targets: [
        .target(
            name: "FissionNativeModules",
            dependencies: [{target_dependencies}],
            path: "Sources/FissionNativeModules"
        ),
    ]
)
"#
    )
}

fn render_ios_swift_package_dependency(package: &NativeIosSwiftPackageConfig) -> String {
    let version = package
        .from
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("0.0.0");
    format!(".package(url: {:?}, from: {:?})", package.url, version)
}

fn render_ios_swift_product_dependency(package: &NativeIosSwiftPackageConfig) -> String {
    let package_name = package
        .url
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(package.product.as_str())
        .trim_end_matches(".git");
    format!(
        ".product(name: {:?}, package: {:?})",
        package.product, package_name
    )
}

fn render_ios_host_native_capabilities_swift() -> &'static str {
    r#"import Foundation
import FissionNativeModules

public enum FissionHostNativeCapabilities {
    public static func present(name: String, requestID: UInt64, payload: Data, completion: @escaping (Result<Data, Error>) -> Void) -> Bool {
        FissionNativeCapabilityRegistry.shared.present(name: name, requestID: requestID, payload: payload, completion: completion)
    }
}
"#
}

fn render_ios_native_capabilities_swift() -> &'static str {
    r#"import Foundation

public protocol FissionNativeCapability {
    var name: String { get }
    func present(requestID: UInt64, payload: Data, completion: @escaping (Result<Data, Error>) -> Void)
}

public final class FissionNativeCapabilityRegistry {
    public static let shared = FissionNativeCapabilityRegistry()
    private var capabilities: [String: FissionNativeCapability] = [:]

    private init() {}

    public func register(_ capability: FissionNativeCapability) {
        capabilities[capability.name] = capability
    }

    public func present(name: String, requestID: UInt64, payload: Data, completion: @escaping (Result<Data, Error>) -> Void) -> Bool {
        guard let capability = capabilities[name] else {
            return false
        }
        capability.present(requestID: requestID, payload: payload, completion: completion)
        return true
    }
}
"#
}

const IOS_NATIVE_MODULES_README: &str = r#"# iOS native modules

This Swift package is the app-owned integration point for native capability modules.

Fission generates `Package.swift` from `fission.toml` `[native]` module declarations. Capability
crates can provide Swift sources or package dependencies here without adding product-specific
logic to Fission itself.
"#;

fn render_ios_plist(project: &FissionProject, executable: &str) -> String {
    let capability_entries = render_ios_info_plist_capability_entries(project);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>{display_name}</string>
  <key>CFBundleExecutable</key>
  <string>{executable}</string>
  <key>CFBundleIdentifier</key>
  <string>{bundle_id}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>{display_name}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>UILaunchStoryboardName</key>
  <string>LaunchScreen</string>
  <key>LSRequiresIPhoneOS</key>
  <true/>
  <key>MinimumOSVersion</key>
  <string>18.0</string>
{capability_entries}
  <key>UIDeviceFamily</key>
  <array>
    <integer>1</integer>
    <integer>2</integer>
  </array>
</dict>
</plist>
"#,
        display_name = ios_bundle_name(project),
        executable = executable,
        bundle_id = project.app.app_id,
        capability_entries = capability_entries,
    )
}

fn render_ios_info_plist_capability_entries(project: &FissionProject) -> String {
    let mut out = String::new();
    if project.capabilities.contains(&PlatformCapability::Nfc) {
        out.push_str("  <key>NFCReaderUsageDescription</key>\n  <string>This app uses NFC to scan nearby tags when you request it.</string>\n");
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Biometric)
    {
        out.push_str("  <key>NSFaceIDUsageDescription</key>\n  <string>This app uses biometrics to authenticate you when you request it.</string>\n");
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Bluetooth)
    {
        out.push_str("  <key>NSBluetoothAlwaysUsageDescription</key>\n  <string>This app uses Bluetooth when you request nearby-device features.</string>\n");
    }
    if project
        .capabilities
        .contains(&PlatformCapability::BarcodeScanner)
    {
        out.push_str("  <key>NSCameraUsageDescription</key>\n  <string>This app uses the camera to scan barcodes when you request it.</string>\n");
    }
    if project.capabilities.contains(&PlatformCapability::Camera)
        && !project
            .capabilities
            .contains(&PlatformCapability::BarcodeScanner)
    {
        out.push_str("  <key>NSCameraUsageDescription</key>\n  <string>This app uses the camera when you request camera features.</string>\n");
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Geolocation)
    {
        out.push_str("  <key>NSLocationWhenInUseUsageDescription</key>\n  <string>This app uses your location when you request location-aware features.</string>\n");
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Microphone)
    {
        out.push_str("  <key>NSMicrophoneUsageDescription</key>\n  <string>This app uses the microphone when you request audio capture.</string>\n");
    }
    if project.capabilities.contains(&PlatformCapability::Wifi)
        && !project
            .capabilities
            .contains(&PlatformCapability::Geolocation)
    {
        out.push_str("  <key>NSLocationWhenInUseUsageDescription</key>\n  <string>This app uses location permission where the platform requires it for Wi-Fi information.</string>\n");
    }
    out
}

fn render_ios_package_script(
    project: &FissionProject,
    bundle_name: &str,
    executable: &str,
) -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
TARGET="${{IOS_SIM_TARGET:-aarch64-apple-ios-sim}}"
PROFILE="${{IOS_SIM_PROFILE:-debug}}"
PACKAGE_NAME="{package_name}"
BUNDLE_ID="${{IOS_BUNDLE_ID:-{bundle_id}}}"
DISPLAY_NAME="${{IOS_DISPLAY_NAME:-{bundle_name}}}"
EXECUTABLE_NAME="${{IOS_EXECUTABLE_NAME:-{executable}}}"
BUNDLE_NAME="${{IOS_BUNDLE_NAME:-$DISPLAY_NAME.app}}"
BUILD_DIR="$SCRIPT_DIR/build/$PROFILE"
BUNDLE_DIR="$BUILD_DIR/$BUNDLE_NAME"

BUILD_ARGS=(build --manifest-path "$PROJECT_DIR/Cargo.toml" --target "$TARGET" --package "$PACKAGE_NAME")
ARTIFACT_DIR=debug
if [[ "$PROFILE" == "release" ]]; then
  BUILD_ARGS+=(--release)
  ARTIFACT_DIR=release
fi

cargo "${{BUILD_ARGS[@]}}"
TARGET_DIR=$(python3 - <<'PY' "$PROJECT_DIR/Cargo.toml"
import json
import subprocess
import sys

manifest = sys.argv[1]
metadata = json.loads(
    subprocess.check_output(
        ["cargo", "metadata", "--manifest-path", manifest, "--format-version", "1", "--no-deps"]
    )
)
print(metadata["target_directory"])
PY
)

rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR"
cp "$TARGET_DIR/$TARGET/$ARTIFACT_DIR/$PACKAGE_NAME" "$BUNDLE_DIR/$EXECUTABLE_NAME"
chmod +x "$BUNDLE_DIR/$EXECUTABLE_NAME"
{plist_patch}
shopt -s nullglob
PLATFORM_APP_ICONS=("$SCRIPT_DIR"/AppIcon.*)
if (( ${{#PLATFORM_APP_ICONS[@]}} == 0 )); then
  cp "$PROJECT_DIR/assets/app-icon.png" "$BUNDLE_DIR/AppIcon.png"
else
  app_icon="${{PLATFORM_APP_ICONS[0]}}"
  cp "$app_icon" "$BUNDLE_DIR/$(basename "$app_icon")"
fi
shopt -u nullglob
shopt -s nullglob
SPLASH_IMAGES=("$SCRIPT_DIR"/SplashImage.*)
if (( ${{#SPLASH_IMAGES[@]}} == 0 )); then
  cp "$PROJECT_DIR/assets/app-icon.png" "$BUNDLE_DIR/SplashImage.png"
else
  for splash_image in "${{SPLASH_IMAGES[@]}}"; do
    cp "$splash_image" "$BUNDLE_DIR/"
  done
fi
shopt -u nullglob
if [[ -f "$SCRIPT_DIR/LaunchScreen.storyboard" ]]; then
  IBTOOL=$(xcrun --find ibtool 2>/dev/null || true)
  if [[ -z "$IBTOOL" ]]; then
    printf 'ibtool not found. Install Xcode command line tools to compile the iOS launch screen storyboard.\n' >&2
    exit 1
  fi
  "$IBTOOL" \
    --errors \
    --warnings \
    --notices \
    --target-device iphone \
    --target-device ipad \
    --minimum-deployment-target 18.0 \
    --output-format human-readable-text \
    --compile "$BUNDLE_DIR/LaunchScreen.storyboardc" \
    "$SCRIPT_DIR/LaunchScreen.storyboard"
fi
printf 'APPL????' > "$BUNDLE_DIR/PkgInfo"
printf '%s\n' "$BUNDLE_DIR"
"#,
        package_name = project.app.name,
        bundle_id = project.app.app_id,
        bundle_name = bundle_name,
        executable = executable,
        plist_patch = IOS_INFO_PLIST_PLUTIL_PATCH,
    )
}

fn render_ios_run_script(project: &FissionProject) -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)
BUNDLE_DIR=$("$SCRIPT_DIR/package-sim.sh")
BUNDLE_ID="${{IOS_BUNDLE_ID:-{bundle_id}}}"
DEVICE_ID="${{IOS_SIM_DEVICE_ID:-}}"

if [[ -z "$DEVICE_ID" ]]; then
  DEVICE_ID=$(python3 - <<'PY'
import json
import subprocess
payload = json.loads(subprocess.check_output(["xcrun", "simctl", "list", "devices", "available", "-j"]))
for runtime, devices in payload["devices"].items():
    if not runtime.startswith("com.apple.CoreSimulator.SimRuntime.iOS-"):
        continue
    for device in devices:
        if device.get("isAvailable") and "iPhone" in device["name"]:
            print(device["udid"])
            raise SystemExit(0)
raise SystemExit("no available iPhone simulator found")
PY
)
fi

if [[ "${{IOS_SIM_HEADLESS:-0}}" != "1" ]] && command -v open >/dev/null 2>&1; then
  open -a Simulator --args -CurrentDeviceUDID "$DEVICE_ID" >/dev/null 2>&1 \
    || open -a Simulator >/dev/null 2>&1 \
    || true
fi

xcrun simctl boot "$DEVICE_ID" >/dev/null 2>&1 || true
xcrun simctl bootstatus "$DEVICE_ID" -b
if [[ "${{IOS_SIM_UNINSTALL_BEFORE_INSTALL:-1}}" == "1" ]]; then
  xcrun simctl uninstall "$DEVICE_ID" "$BUNDLE_ID" >/dev/null 2>&1 || true
fi
xcrun simctl install "$DEVICE_ID" "$BUNDLE_DIR"

if [[ -n "${{FISSION_TEST_CONTROL_PORT:-}}" ]]; then
  SIMCTL_CHILD_FISSION_TEST_CONTROL_PORT="${{FISSION_TEST_CONTROL_PORT}}" \
    xcrun simctl launch --terminate-running-process "$DEVICE_ID" "$BUNDLE_ID"
else
  xcrun simctl launch --terminate-running-process "$DEVICE_ID" "$BUNDLE_ID"
fi
"#,
        bundle_id = project.app.app_id,
    )
}

fn render_ios_test_script() -> String {
    r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
export FISSION_TEST_CONTROL_PORT="${FISSION_TEST_CONTROL_PORT:-48711}"

"$SCRIPT_DIR/run-sim.sh"

python3 - <<'PY' "$FISSION_TEST_CONTROL_PORT"
import sys
import time
import urllib.request

port = sys.argv[1]
url = f"http://127.0.0.1:{port}/health"
deadline = time.time() + 90
last_error = None
while time.time() < deadline:
    try:
        with urllib.request.urlopen(url, timeout=1) as response:
            body = response.read().decode("utf-8", "replace")
        if response.status == 200 and '"status":"ok"' in body:
            print(f"iOS simulator test control is healthy on {url}")
            raise SystemExit(0)
    except Exception as error:
        last_error = error
    time.sleep(1)
raise SystemExit(f"iOS simulator test control did not become healthy on {url}: {last_error}")
PY
"#
    .to_string()
}

fn render_android_manifest(project: &FissionProject) -> String {
    let capability_entries = render_android_capability_manifest_entries(project);
    let native_application_entries = render_android_native_application_entries(project);
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="{app_id}">

    <uses-permission android:name="android.permission.INTERNET" />
{capability_entries}

    <uses-sdk
        android:minSdkVersion="24"
        android:targetSdkVersion="35" />

    <application
        android:debuggable="true"
        android:extractNativeLibs="true"
        android:hasCode="true"
        android:icon="@drawable/app_icon"
        android:label="{label}">
{native_application_entries}
        <activity
            android:name="rs.fission.runtime.FissionActivity"
            android:configChanges="orientation|keyboardHidden|screenSize|screenLayout|smallestScreenSize|uiMode|density"
            android:exported="true"
            android:launchMode="singleTask"
            android:theme="@style/FissionLaunchTheme">
            <meta-data
                android:name="android.app.lib_name"
                android:value="{lib_name}" />
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>

</manifest>
"#,
        app_id = project.app.app_id,
        label = ios_bundle_name(project),
        lib_name = android_library_name(project),
        capability_entries = capability_entries,
        native_application_entries = native_application_entries,
    )
}

fn render_android_native_application_entries(project: &FissionProject) -> String {
    let mut out = String::new();
    for module in &project.native.modules {
        for entry in &module.android.manifest_application_entries {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            out.push_str("        ");
            out.push_str(entry);
            if !entry.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    out
}

fn render_android_capability_manifest_entries(project: &FissionProject) -> String {
    let mut out = String::new();
    if project.capabilities.contains(&PlatformCapability::Nfc) {
        out.push_str(&render_android_nfc_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Notifications)
    {
        out.push_str(&render_android_notifications_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Biometric)
    {
        out.push_str(&render_android_biometric_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Bluetooth)
    {
        out.push_str(&render_android_bluetooth_manifest_entries());
    }
    if project.capabilities.contains(&PlatformCapability::Camera) {
        out.push_str(&render_android_camera_manifest_entries());
    } else if project
        .capabilities
        .contains(&PlatformCapability::BarcodeScanner)
    {
        out.push_str(&render_android_barcode_camera_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Geolocation)
    {
        out.push_str(&render_android_geolocation_manifest_entries());
    }
    if project.capabilities.contains(&PlatformCapability::Haptics) {
        out.push_str(&render_android_haptics_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Microphone)
    {
        out.push_str(&render_android_microphone_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::VolumeControl)
    {
        out.push_str(&render_android_volume_manifest_entries());
    }
    if project.capabilities.contains(&PlatformCapability::Wifi) {
        out.push_str(&render_android_wifi_manifest_entries());
    }
    for permission in android_native_module_permissions(project) {
        out.push_str(&format!(
            "    <uses-permission android:name=\"{}\" />\n",
            permission
        ));
    }
    out
}

fn android_native_module_permissions(project: &FissionProject) -> BTreeSet<String> {
    project
        .native
        .modules
        .iter()
        .flat_map(|module| module.android.permissions.iter())
        .map(|permission| permission.trim().to_string())
        .filter(|permission| !permission.is_empty())
        .collect()
}

fn render_android_nfc_manifest_entries() -> String {
    let mut out = String::new();
    out.push_str("    <uses-permission android:name=\"android.permission.NFC\" />\n");
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.nfc\" android:required=\"false\" />\n",
    );
    out
}

fn render_android_notifications_manifest_entries() -> String {
    "    <uses-permission android:name=\"android.permission.POST_NOTIFICATIONS\" />\n".to_string()
}

fn render_android_biometric_manifest_entries() -> String {
    let mut out = String::new();
    out.push_str("    <uses-permission android:name=\"android.permission.USE_BIOMETRIC\" />\n");
    out.push_str("    <uses-permission android:name=\"android.permission.USE_FINGERPRINT\" android:maxSdkVersion=\"28\" />\n");
    out
}

fn render_android_bluetooth_manifest_entries() -> String {
    let mut out = String::new();
    out.push_str("    <uses-permission android:name=\"android.permission.BLUETOOTH\" android:maxSdkVersion=\"30\" />\n");
    out.push_str("    <uses-permission android:name=\"android.permission.BLUETOOTH_ADMIN\" android:maxSdkVersion=\"30\" />\n");
    out.push_str("    <uses-permission android:name=\"android.permission.BLUETOOTH_SCAN\" android:usesPermissionFlags=\"neverForLocation\" />\n");
    out.push_str("    <uses-permission android:name=\"android.permission.BLUETOOTH_CONNECT\" />\n");
    out.push_str(
        "    <uses-permission android:name=\"android.permission.BLUETOOTH_ADVERTISE\" />\n",
    );
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.bluetooth\" android:required=\"false\" />\n",
    );
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.bluetooth_le\" android:required=\"false\" />\n",
    );
    out
}

fn render_missing_android_bluetooth_manifest_entries(existing: &str) -> String {
    let mut out = String::new();
    if !existing.contains("android.permission.BLUETOOTH\"") {
        out.push_str("    <uses-permission android:name=\"android.permission.BLUETOOTH\" android:maxSdkVersion=\"30\" />\n");
    }
    if !existing.contains("android.permission.BLUETOOTH_ADMIN") {
        out.push_str("    <uses-permission android:name=\"android.permission.BLUETOOTH_ADMIN\" android:maxSdkVersion=\"30\" />\n");
    }
    if !existing.contains("android.permission.BLUETOOTH_SCAN") {
        out.push_str("    <uses-permission android:name=\"android.permission.BLUETOOTH_SCAN\" android:usesPermissionFlags=\"neverForLocation\" />\n");
    }
    if !existing.contains("android.permission.BLUETOOTH_CONNECT") {
        out.push_str(
            "    <uses-permission android:name=\"android.permission.BLUETOOTH_CONNECT\" />\n",
        );
    }
    if !existing.contains("android.permission.BLUETOOTH_ADVERTISE") {
        out.push_str(
            "    <uses-permission android:name=\"android.permission.BLUETOOTH_ADVERTISE\" />\n",
        );
    }
    if !existing.contains("android.hardware.bluetooth\"") {
        out.push_str(
            "    <uses-feature android:name=\"android.hardware.bluetooth\" android:required=\"false\" />\n",
        );
    }
    if !existing.contains("android.hardware.bluetooth_le") {
        out.push_str(
            "    <uses-feature android:name=\"android.hardware.bluetooth_le\" android:required=\"false\" />\n",
        );
    }
    out
}

fn render_android_barcode_camera_manifest_entries() -> String {
    let mut out = String::new();
    out.push_str("    <uses-permission android:name=\"android.permission.CAMERA\" />\n");
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.camera.any\" android:required=\"false\" />\n",
    );
    out
}

fn render_android_camera_manifest_entries() -> String {
    let mut out = String::new();
    out.push_str("    <uses-permission android:name=\"android.permission.CAMERA\" />\n");
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.camera.any\" android:required=\"false\" />\n",
    );
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.camera\" android:required=\"false\" />\n",
    );
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.camera.front\" android:required=\"false\" />\n",
    );
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.camera.flash\" android:required=\"false\" />\n",
    );
    out
}

fn render_missing_android_camera_manifest_entries(existing: &str) -> String {
    let mut out = String::new();
    if !existing.contains("android.permission.CAMERA") {
        out.push_str("    <uses-permission android:name=\"android.permission.CAMERA\" />\n");
    }
    if !existing.contains("android.hardware.camera.any") {
        out.push_str(
            "    <uses-feature android:name=\"android.hardware.camera.any\" android:required=\"false\" />\n",
        );
    }
    if !existing.contains("android.hardware.camera\"") {
        out.push_str(
            "    <uses-feature android:name=\"android.hardware.camera\" android:required=\"false\" />\n",
        );
    }
    if !existing.contains("android.hardware.camera.front") {
        out.push_str(
            "    <uses-feature android:name=\"android.hardware.camera.front\" android:required=\"false\" />\n",
        );
    }
    if !existing.contains("android.hardware.camera.flash") {
        out.push_str(
            "    <uses-feature android:name=\"android.hardware.camera.flash\" android:required=\"false\" />\n",
        );
    }
    out
}

fn render_android_geolocation_manifest_entries() -> String {
    let mut out = String::new();
    out.push_str(
        "    <uses-permission android:name=\"android.permission.ACCESS_COARSE_LOCATION\" />\n",
    );
    out.push_str(
        "    <uses-permission android:name=\"android.permission.ACCESS_FINE_LOCATION\" />\n",
    );
    out
}

fn render_android_haptics_manifest_entries() -> String {
    "    <uses-permission android:name=\"android.permission.VIBRATE\" />\n".to_string()
}

fn render_android_microphone_manifest_entries() -> String {
    "    <uses-permission android:name=\"android.permission.RECORD_AUDIO\" />\n".to_string()
}

fn render_android_volume_manifest_entries() -> String {
    "    <uses-permission android:name=\"android.permission.MODIFY_AUDIO_SETTINGS\" />\n"
        .to_string()
}

fn render_android_wifi_manifest_entries() -> String {
    let mut out = String::new();
    out.push_str("    <uses-permission android:name=\"android.permission.ACCESS_WIFI_STATE\" />\n");
    out.push_str("    <uses-permission android:name=\"android.permission.CHANGE_WIFI_STATE\" />\n");
    out.push_str(
        "    <uses-permission android:name=\"android.permission.ACCESS_NETWORK_STATE\" />\n",
    );
    out.push_str(
        "    <uses-permission android:name=\"android.permission.CHANGE_NETWORK_STATE\" />\n",
    );
    out.push_str("    <uses-permission android:name=\"android.permission.NEARBY_WIFI_DEVICES\" android:usesPermissionFlags=\"neverForLocation\" />\n");
    out.push_str("    <uses-permission android:name=\"android.permission.ACCESS_FINE_LOCATION\" android:maxSdkVersion=\"32\" />\n");
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.wifi\" android:required=\"false\" />\n",
    );
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.wifi.direct\" android:required=\"false\" />\n",
    );
    out
}

fn render_missing_android_wifi_manifest_entries(existing: &str) -> String {
    let mut out = String::new();
    if !existing.contains("android.permission.ACCESS_WIFI_STATE") {
        out.push_str(
            "    <uses-permission android:name=\"android.permission.ACCESS_WIFI_STATE\" />\n",
        );
    }
    if !existing.contains("android.permission.CHANGE_WIFI_STATE") {
        out.push_str(
            "    <uses-permission android:name=\"android.permission.CHANGE_WIFI_STATE\" />\n",
        );
    }
    if !existing.contains("android.permission.ACCESS_NETWORK_STATE") {
        out.push_str(
            "    <uses-permission android:name=\"android.permission.ACCESS_NETWORK_STATE\" />\n",
        );
    }
    if !existing.contains("android.permission.CHANGE_NETWORK_STATE") {
        out.push_str(
            "    <uses-permission android:name=\"android.permission.CHANGE_NETWORK_STATE\" />\n",
        );
    }
    if !existing.contains("android.permission.NEARBY_WIFI_DEVICES") {
        out.push_str("    <uses-permission android:name=\"android.permission.NEARBY_WIFI_DEVICES\" android:usesPermissionFlags=\"neverForLocation\" />\n");
    }
    if !existing.contains("android.permission.ACCESS_FINE_LOCATION") {
        out.push_str("    <uses-permission android:name=\"android.permission.ACCESS_FINE_LOCATION\" android:maxSdkVersion=\"32\" />\n");
    }
    if !existing.contains("android.hardware.wifi\"") {
        out.push_str(
            "    <uses-feature android:name=\"android.hardware.wifi\" android:required=\"false\" />\n",
        );
    }
    if !existing.contains("android.hardware.wifi.direct") {
        out.push_str(
            "    <uses-feature android:name=\"android.hardware.wifi.direct\" android:required=\"false\" />\n",
        );
    }
    out
}

fn render_ios_entitlements_plist(project: &FissionProject) -> String {
    let mut entries = String::new();
    if project.capabilities.contains(&PlatformCapability::Nfc) {
        entries.push_str("  <key>com.apple.developer.nfc.readersession.formats</key>\n  <array>\n    <string>NDEF</string>\n  </array>\n");
    }
    if project.capabilities.contains(&PlatformCapability::Wifi) {
        entries.push_str("  <key>com.apple.developer.networking.wifi-info</key>\n  <true/>\n");
        entries.push_str(
            "  <key>com.apple.developer.networking.HotspotConfiguration</key>\n  <true/>\n",
        );
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n{entries}</dict>\n</plist>\n"
    )
}

const IOS_NFC_ENTITLEMENTS_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>com.apple.developer.nfc.readersession.formats</key>
  <array>
    <string>NDEF</string>
  </array>
</dict>
</plist>
"#;

const IOS_WIFI_ENTITLEMENTS_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>com.apple.developer.networking.wifi-info</key>
  <true/>
  <key>com.apple.developer.networking.HotspotConfiguration</key>
  <true/>
</dict>
</plist>
"#;

fn render_android_capabilities_java() -> &'static str {
    include_str!("../assets/android/rs/fission/runtime/FissionAndroidCapabilities.java")
}

fn render_android_package_script(project: &FissionProject) -> String {
    let lib_name = android_library_name(project);
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
TARGET="${{ANDROID_TARGET_TRIPLE:-aarch64-linux-android}}"
PACKAGE_NAME="{package_name}"
LIB_NAME="{lib_name}"
PROFILE="${{ANDROID_PROFILE:-debug}}"
ANDROID_HOME="${{ANDROID_HOME:-${{ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}}}"
ANDROID_MIN_API_LEVEL="${{ANDROID_MIN_API_LEVEL:-${{ANDROID_API_LEVEL:-24}}}}"

find_android_ndk() {{
  if [[ -n "${{ANDROID_NDK:-}}" ]]; then
    printf '%s\n' "$ANDROID_NDK"
    return
  fi
  local ndk_root="$ANDROID_HOME/ndk"
  if [[ ! -d "$ndk_root" ]]; then
    printf 'Android NDK not found. Set ANDROID_NDK or install one under %s.\n' "$ndk_root" >&2
    return 1
  fi
  local ndk
  ndk=$(find "$ndk_root" -maxdepth 1 -mindepth 1 -type d | sort -V | tail -1)
  if [[ -z "$ndk" ]]; then
    printf 'Android NDK not found. Set ANDROID_NDK or install one under %s.\n' "$ndk_root" >&2
    return 1
  fi
  printf '%s\n' "$ndk"
}}

detect_android_toolchain() {{
  local prebuilt_root="$ANDROID_NDK/toolchains/llvm/prebuilt"
  local host
  for host in darwin-aarch64 darwin-x86_64 linux-x86_64 windows-x86_64; do
    if [[ -d "$prebuilt_root/$host/bin" ]]; then
      printf '%s\n' "$prebuilt_root/$host/bin"
      return
    fi
  done
  local fallback
  fallback=$(find "$prebuilt_root" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | sort | head -1 || true)
  if [[ -n "$fallback" && -d "$fallback/bin" ]]; then
    printf '%s\n' "$fallback/bin"
    return
  fi
  printf 'No Android NDK LLVM prebuilt toolchain found under %s. Expected a prebuilt host directory such as darwin-x86_64 or linux-x86_64.\n' "$prebuilt_root" >&2
  return 1
}}

detect_latest_android_api() {{
  find "$ANDROID_HOME/platforms" -maxdepth 1 -type d -name 'android-*' 2>/dev/null \
    | sed 's#.*android-##' \
    | sort -n \
    | tail -1
}}

ANDROID_TARGET_API_LEVEL="${{ANDROID_TARGET_API_LEVEL:-$(detect_latest_android_api)}}"
if [[ -z "$ANDROID_TARGET_API_LEVEL" ]]; then
  printf 'No Android platform found under %s/platforms. Install one with sdkmanager "platforms;android-35" or newer.\n' "$ANDROID_HOME" >&2
  exit 1
fi

ANDROID_NDK=$(find_android_ndk)
ANDROID_TOOLCHAIN="${{ANDROID_TOOLCHAIN:-$(detect_android_toolchain)}}"
CC_aarch64_linux_android="${{CC_aarch64_linux_android:-$ANDROID_TOOLCHAIN/aarch64-linux-android${{ANDROID_MIN_API_LEVEL}}-clang}}"
AR_aarch64_linux_android="${{AR_aarch64_linux_android:-$ANDROID_TOOLCHAIN/llvm-ar}}"
CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="${{CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER:-$CC_aarch64_linux_android}}"
CARGO_TARGET_AARCH64_LINUX_ANDROID_AR="${{CARGO_TARGET_AARCH64_LINUX_ANDROID_AR:-$AR_aarch64_linux_android}}"
export ANDROID_HOME ANDROID_NDK ANDROID_MIN_API_LEVEL ANDROID_TARGET_API_LEVEL ANDROID_TOOLCHAIN CC_aarch64_linux_android AR_aarch64_linux_android
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER CARGO_TARGET_AARCH64_LINUX_ANDROID_AR

if [[ -n "${{FISSION_GRADLE:-}}" ]]; then
  read -r -a GRADLE_CMD <<< "$FISSION_GRADLE"
elif [[ -x "$SCRIPT_DIR/gradlew" ]]; then
  GRADLE_CMD=("$SCRIPT_DIR/gradlew")
else
  if ! command -v gradle >/dev/null 2>&1; then
    printf 'Gradle is required for the generated Android project shell. Install Gradle or add a wrapper under %s.\n' "$SCRIPT_DIR" >&2
    exit 1
  fi
  GRADLE_CMD=(gradle)
fi

BUILD_ARGS=(build --manifest-path "$PROJECT_DIR/Cargo.toml" --lib --target "$TARGET" --package "$PACKAGE_NAME")
ARTIFACT_DIR=debug
GRADLE_VARIANT=Debug
GRADLE_OUTPUT_DIR=debug
if [[ "$PROFILE" == "release" ]]; then
  BUILD_ARGS+=(--release)
  ARTIFACT_DIR=release
  GRADLE_VARIANT=Release
  GRADLE_OUTPUT_DIR=release
fi

cargo "${{BUILD_ARGS[@]}}"
TARGET_DIR=$(python3 - <<'PY' "$PROJECT_DIR/Cargo.toml"
import json
import subprocess
import sys

manifest = sys.argv[1]
metadata = json.loads(
    subprocess.check_output(
        ["cargo", "metadata", "--manifest-path", manifest, "--format-version", "1", "--no-deps"]
    )
)
print(metadata["target_directory"])
PY
)

SO_PATH="$TARGET_DIR/$TARGET/$ARTIFACT_DIR/lib$LIB_NAME.so"
JNI_DIR="$SCRIPT_DIR/app/src/main/jniLibs/arm64-v8a"
GENERATED_RES_DIR="$SCRIPT_DIR/app/src/main/res/drawable-nodpi"
mkdir -p "$JNI_DIR" "$GENERATED_RES_DIR"
cp "$SO_PATH" "$JNI_DIR/lib$LIB_NAME.so"
shopt -s nullglob
APP_ICONS=("$SCRIPT_DIR"/res/drawable-nodpi/app_icon.* "$SCRIPT_DIR"/res/drawable/app_icon.*)
if (( ${{#APP_ICONS[@]}} == 0 )); then
  cp "$PROJECT_DIR/assets/app-icon.png" "$GENERATED_RES_DIR/app_icon.png"
fi
shopt -u nullglob
shopt -s nullglob
SPLASH_IMAGES=("$SCRIPT_DIR"/res/drawable-nodpi/fission_splash_image.*)
if (( ${{#SPLASH_IMAGES[@]}} == 0 )); then
  cp "$PROJECT_DIR/assets/app-icon.png" "$GENERATED_RES_DIR/fission_splash_image.png"
fi
shopt -u nullglob

"${{GRADLE_CMD[@]}}" -p "$SCRIPT_DIR" ":app:assemble$GRADLE_VARIANT"

APK="$SCRIPT_DIR/app/build/outputs/apk/$GRADLE_OUTPUT_DIR/app-$GRADLE_OUTPUT_DIR.apk"
if [[ ! -f "$APK" ]]; then
  printf 'Gradle did not produce the expected APK: %s\n' "$APK" >&2
  exit 1
fi
printf '%s\n' "$APK"
"#,
        package_name = project.app.name,
        lib_name = lib_name,
    )
}

fn render_android_run_script(project: &FissionProject) -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)
ANDROID_HOME="${{ANDROID_HOME:-${{ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}}}"
ADB="$ANDROID_HOME/platform-tools/adb"
EMULATOR_BIN="$ANDROID_HOME/emulator/emulator"
AVDMANAGER="${{ANDROID_AVDMANAGER:-$ANDROID_HOME/cmdline-tools/latest/bin/avdmanager}}"

detect_latest_emulator_api() {{
  find "$ANDROID_HOME/system-images" -path '*/google_apis/arm64-v8a' -type d 2>/dev/null \
    | sed -n 's#.*system-images/android-\([0-9][0-9]*\)/google_apis/arm64-v8a#\1#p' \
    | sort -n \
    | tail -1
}}

android_system_image_path() {{
  local image="$1"
  image="${{image#system-images;}}"
  printf '%s/system-images/%s\n' "$ANDROID_HOME" "${{image//;/\/}}"
}}

wait_for_android_boot() {{
  "$ADB" wait-for-device
  until "$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r' | grep -q '^1$'; do
    sleep 1
  done
  local deadline=$((SECONDS + 180))
  until "$ADB" shell cmd package list packages >/dev/null 2>&1; do
    if (( SECONDS > deadline )); then
      printf 'Android package manager did not become available. Restart the emulator with ANDROID_EMULATOR_RESTART=1 and try again.\n' >&2
      exit 1
    fi
    sleep 1
  done
}}

ANDROID_EMULATOR_API_LEVEL="${{ANDROID_EMULATOR_API_LEVEL:-$(detect_latest_emulator_api)}}"
if [[ -z "$ANDROID_EMULATOR_API_LEVEL" ]]; then
  printf 'No Android arm64 google_apis emulator image found under %s/system-images.\nInstall one with sdkmanager "system-images;android-35;google_apis;arm64-v8a" or set ANDROID_SYSTEM_IMAGE.\n' "$ANDROID_HOME" >&2
  exit 1
fi
AVD_NAME="${{ANDROID_AVD_NAME:-FissionApi${{ANDROID_EMULATOR_API_LEVEL}}Arm64}}"
SYSTEM_IMAGE="${{ANDROID_SYSTEM_IMAGE:-system-images;android-${{ANDROID_EMULATOR_API_LEVEL}};google_apis;arm64-v8a}}"
DEVICE_PORT="${{ANDROID_TEST_CONTROL_DEVICE_PORT:-48761}}"
HOST_PORT="${{FISSION_TEST_CONTROL_PORT:-48761}}"
HEADLESS="${{ANDROID_EMULATOR_HEADLESS:-0}}"
RESTART_EMULATOR="${{ANDROID_EMULATOR_RESTART:-0}}"

for tool in "$ADB" "$EMULATOR_BIN" "$AVDMANAGER"; do
  if [[ ! -x "$tool" ]]; then
    printf 'Required Android tool is missing or not executable: %s\nRun `fission doctor android --project-dir .` for setup help.\n' "$tool" >&2
    exit 1
  fi
done

if ! "$AVDMANAGER" list avd | grep -q "Name: $AVD_NAME"; then
  if [[ ! -d "$(android_system_image_path "$SYSTEM_IMAGE")" ]]; then
    printf 'Android system image is not installed: %s\nInstall it with sdkmanager "%s" or set ANDROID_SYSTEM_IMAGE.\n' "$SYSTEM_IMAGE" "$SYSTEM_IMAGE" >&2
    exit 1
  fi
  echo "no" | "$AVDMANAGER" create avd -n "$AVD_NAME" -k "$SYSTEM_IMAGE" --abi "google_apis/arm64-v8a" --device "pixel_5"
fi

RUNNING_EMULATOR=$("$ADB" devices | awk '/^emulator-.*device$/ {{ print $1; exit }}')
if [[ -n "$RUNNING_EMULATOR" && "$RESTART_EMULATOR" == "1" ]]; then
  "$ADB" -s "$RUNNING_EMULATOR" emu kill >/dev/null || true
  until ! "$ADB" devices | grep -q '^emulator-'; do
    sleep 1
  done
  RUNNING_EMULATOR=""
fi

if [[ -z "$RUNNING_EMULATOR" ]]; then
  EMULATOR_ARGS=(-avd "$AVD_NAME" -gpu "${{ANDROID_EMULATOR_GPU:-swiftshader_indirect}}" -no-audio)
  if [[ "$HEADLESS" == "1" ]]; then
    EMULATOR_ARGS+=(-no-window)
  fi
  printf 'Launching emulator %s (%s)\n' "$AVD_NAME" "$([[ "$HEADLESS" == "1" ]] && echo headless || echo visible)"
  nohup "$EMULATOR_BIN" "${{EMULATOR_ARGS[@]}}" >/tmp/fission-android-emulator.log 2>&1 &
  disown || true
  wait_for_android_boot
else
  printf 'Using existing emulator %s\n' "$RUNNING_EMULATOR"
  wait_for_android_boot
  if [[ "$HEADLESS" != "1" ]]; then
    printf 'If the window is not visible, restart with ANDROID_EMULATOR_RESTART=1 to relaunch a visible emulator.\n'
  fi
fi

APK=$("$SCRIPT_DIR/package-apk.sh")
read -r -a ADB_INSTALL_FLAGS <<< "${{ADB_INSTALL_FLAGS:---no-streaming -r}}"
"$ADB" install "${{ADB_INSTALL_FLAGS[@]}}" "$APK"
"$ADB" forward "tcp:$HOST_PORT" "tcp:$DEVICE_PORT"
"$ADB" shell am start -n {app_id}/rs.fission.runtime.FissionActivity >/dev/null
printf 'APK=%s\n' "$APK"
"#,
        app_id = project.app.app_id,
    )
}

fn render_android_test_script() -> String {
    r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
export FISSION_TEST_CONTROL_PORT="${FISSION_TEST_CONTROL_PORT:-48761}"

"$SCRIPT_DIR/run-emulator.sh"

python3 - <<'PY' "$FISSION_TEST_CONTROL_PORT"
import sys
import time
import urllib.request

port = sys.argv[1]
url = f"http://127.0.0.1:{port}/health"
deadline = time.time() + 90
last_error = None
while time.time() < deadline:
    try:
        with urllib.request.urlopen(url, timeout=1) as response:
            body = response.read().decode("utf-8", "replace")
        if response.status == 200 and '"status":"ok"' in body:
            print(f"Android emulator test control is healthy on {url}")
            raise SystemExit(0)
    except Exception as error:
        last_error = error
    time.sleep(1)
raise SystemExit(f"Android emulator test control did not become healthy on {url}: {last_error}")
PY
"#
    .to_string()
}

fn render_web_index(project: &FissionProject) -> String {
    let title = ios_bundle_name(project);
    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{title}</title>
    <link rel="icon" type="image/png" href="../../assets/app-icon.png" />
    <style>
      :root {{
        color-scheme: dark;
        background: #14171f;
      }}
      html, body {{
        margin: 0;
        width: 100%;
        height: 100%;
        overflow: hidden;
        overscroll-behavior: none;
        background: #14171f;
      }}
      body, #fission-web-mount {{
        width: 100vw;
        height: 100vh;
      }}
      canvas {{
        display: block;
        width: 100vw;
        height: 100vh;
        border: 0;
        outline: none;
        user-select: none;
        -webkit-user-drag: none;
        touch-action: none;
        -webkit-tap-highlight-color: transparent;
      }}
      canvas:focus, canvas:focus-visible {{
        outline: none;
      }}
    </style>
  </head>
  <body>
    <main id="fission-web-mount" aria-label="{title}"></main>
    <script type="module" src="./bootstrap.mjs"></script>
  </body>
</html>
"#,
        title = title,
    )
}

fn render_web_bootstrap(project: &FissionProject) -> String {
    let module_name = project.app.name.replace('-', "_");
    format!(
        "import init from \"./pkg/{}.js\";\n\nawait init();\n",
        module_name
    )
}

fn render_web_build_script() -> String {
    r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
PROFILE="${FISSION_WEB_PROFILE:-dev}"
BUILD_ARGS=(build "$PROJECT_DIR" --target web --out-dir "$SCRIPT_DIR/pkg")

if [[ "$PROFILE" == "release" ]]; then
  BUILD_ARGS+=(--release)
else
  BUILD_ARGS+=(--dev)
fi

wasm-pack "${BUILD_ARGS[@]}"
"#
    .to_string()
}

fn render_web_run_script(_project: &FissionProject) -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
HOST="${{FISSION_WEB_HOST:-127.0.0.1}}"
REQUESTED_PORT="${{FISSION_WEB_PORT:-8123}}"
PORT="$REQUESTED_PORT"
if [[ -z "${{FISSION_WEB_PORT:-}}" ]]; then
  PORT=$(python3 - "$HOST" "$REQUESTED_PORT" <<'PY'
import socket
import sys

host = sys.argv[1]
start = int(sys.argv[2])
for port in range(start, start + 51):
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            probe.bind((host, port))
        except OSError:
            continue
        print(port)
        raise SystemExit(0)
raise SystemExit(f"no free web port found from {{host}}:{{start}}")
PY
)
  if [[ "$PORT" != "$REQUESTED_PORT" ]]; then
    printf 'Port %s:%s is already in use; using %s:%s.\n' "$HOST" "$REQUESTED_PORT" "$HOST" "$PORT"
  fi
fi
URL="http://${{HOST}}:${{PORT}}/platforms/web/"

"$SCRIPT_DIR/build-wasm.sh"

printf 'Serving %s\n' "$URL"
printf 'Press Ctrl+C to stop the local server.\n'
if [[ "${{FISSION_WEB_OPEN:-0}}" == "1" ]]; then
  if command -v open >/dev/null 2>&1; then
    open "$URL"
  elif command -v xdg-open >/dev/null 2>&1; then
    xdg-open "$URL"
  elif command -v cmd.exe >/dev/null 2>&1; then
    cmd.exe /C start "$URL"
  else
    printf 'No browser opener found. Open %s manually.\n' "$URL"
  fi
fi

cd "$PROJECT_DIR"
python3 -m http.server "$PORT" --bind "$HOST"
"#
    )
}

fn render_web_test_script(_project: &FissionProject) -> String {
    r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
HOST="${FISSION_WEB_HOST:-127.0.0.1}"
REQUESTED_PORT="${FISSION_WEB_PORT:-8123}"
PORT="$REQUESTED_PORT"
if [[ -z "${FISSION_WEB_PORT:-}" ]]; then
  PORT=$(python3 - "$HOST" "$REQUESTED_PORT" <<'PY'
import socket
import sys

host = sys.argv[1]
start = int(sys.argv[2])
for port in range(start, start + 51):
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            probe.bind((host, port))
        except OSError:
            continue
        print(port)
        raise SystemExit(0)
raise SystemExit(f"no free web port found from {host}:{start}")
PY
)
  if [[ "$PORT" != "$REQUESTED_PORT" ]]; then
    printf 'Port %s:%s is already in use; using %s:%s.\n' "$HOST" "$REQUESTED_PORT" "$HOST" "$PORT"
  fi
fi
REQUESTED_CDP_PORT="${FISSION_WEB_CDP_PORT:-9222}"
CDP_PORT="$REQUESTED_CDP_PORT"
if [[ -z "${FISSION_WEB_CDP_PORT:-}" ]]; then
  CDP_PORT=$(python3 - "127.0.0.1" "$REQUESTED_CDP_PORT" <<'PY'
import socket
import sys

host = sys.argv[1]
start = int(sys.argv[2])
for port in range(start, start + 51):
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            probe.bind((host, port))
        except OSError:
            continue
        print(port)
        raise SystemExit(0)
raise SystemExit(f"no free CDP port found from {host}:{start}")
PY
)
  if [[ "$CDP_PORT" != "$REQUESTED_CDP_PORT" ]]; then
    printf 'CDP port 127.0.0.1:%s is already in use; using 127.0.0.1:%s.\n' "$REQUESTED_CDP_PORT" "$CDP_PORT"
  fi
fi
URL="http://${HOST}:${PORT}/platforms/web/"
PROFILE_DIR="$SCRIPT_DIR/build/chrome-profile"

require_node_websocket() {
  if ! command -v node >/dev/null 2>&1; then
    printf 'Node.js was not found. Install Node 22+ so the generated browser smoke test can inspect Chrome CDP console/runtime errors.\n' >&2
    exit 1
  fi
  if ! node -e 'process.exit(typeof WebSocket === "function" ? 0 : 1)' >/dev/null 2>&1; then
    printf 'Node.js is available but does not expose the built-in WebSocket client. Install Node 22+ for Chrome CDP smoke tests.\n' >&2
    exit 1
  fi
}

detect_chrome() {
  if [[ -n "${FISSION_CHROME:-}" && -x "$FISSION_CHROME" ]]; then
    printf '%s\n' "$FISSION_CHROME"
    return
  fi
  local candidate
  for candidate in \
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
    "/Applications/Chromium.app/Contents/MacOS/Chromium" \
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"; do
    if [[ -x "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return
    fi
  done
  for candidate in google-chrome chromium chromium-browser chrome; do
    if command -v "$candidate" >/dev/null 2>&1; then
      command -v "$candidate"
      return
    fi
  done
  return 1
}

require_node_websocket
"$SCRIPT_DIR/build-wasm.sh"

mkdir -p "$SCRIPT_DIR/build"
cd "$PROJECT_DIR"
python3 -m http.server "$PORT" --bind "$HOST" >"$SCRIPT_DIR/build/web-server.log" 2>&1 &
SERVER_PID=$!

cleanup() {
  if [[ -n "${CHROME_PID:-}" ]]; then
    kill "$CHROME_PID" >/dev/null 2>&1 || true
  fi
  kill "$SERVER_PID" >/dev/null 2>&1 || true
}
trap cleanup EXIT

printf 'Running transient web smoke test at %s\n' "$URL"
printf 'The local server is stopped automatically when this script exits.\n'

python3 - <<'PY' "$URL"
import sys
import time
import urllib.request

url = sys.argv[1]
deadline = time.time() + 30
last_error = None
while time.time() < deadline:
    try:
        with urllib.request.urlopen(url, timeout=1) as response:
            if response.status == 200:
                raise SystemExit(0)
    except Exception as error:
        last_error = error
    time.sleep(0.5)
raise SystemExit(f"web server did not serve {url}: {last_error}")
PY

CHROME=$(detect_chrome) || {
  printf 'Chrome/Chromium was not found. Set FISSION_CHROME=/path/to/chrome or run `fission doctor web --project-dir .`.\n' >&2
  exit 1
}

rm -rf "$PROFILE_DIR"
"$CHROME" \
  --headless=new \
  --enable-unsafe-webgpu \
  --no-first-run \
  --no-default-browser-check \
  --remote-debugging-port="$CDP_PORT" \
  --user-data-dir="$PROFILE_DIR" \
  "$URL" >"$SCRIPT_DIR/build/chrome.log" 2>&1 &
CHROME_PID=$!

CDP_PORT="$CDP_PORT" FISSION_WEB_URL="$URL" node <<'NODE'
const cdpPort = process.env.CDP_PORT;
const expectedUrl = process.env.FISSION_WEB_URL;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function waitForTarget() {
  const deadline = Date.now() + 60_000;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${cdpPort}/json/list`);
      const targets = await response.json();
      const target = targets.find((entry) => entry.type === 'page' && entry.url.startsWith(expectedUrl));
      if (target?.webSocketDebuggerUrl) {
        return target.webSocketDebuggerUrl;
      }
    } catch (error) {
      lastError = error;
    }
    await sleep(250);
  }
  throw new Error(`Chrome CDP target did not become ready for ${expectedUrl}: ${lastError?.message ?? lastError}`);
}

class CdpClient {
  constructor(url) {
    this.url = url;
    this.ws = null;
    this.nextId = 1;
    this.pending = new Map();
    this.errors = [];
  }

  async open() {
    await new Promise((resolve, reject) => {
      const ws = new WebSocket(this.url);
      this.ws = ws;
      ws.addEventListener('open', resolve, { once: true });
      ws.addEventListener('error', (event) => reject(new Error(`CDP websocket error: ${event.message ?? 'unknown error'}`)), { once: true });
      ws.addEventListener('message', (event) => this.onMessage(event.data));
      ws.addEventListener('close', () => {
        for (const { reject: rejectPending } of this.pending.values()) {
          rejectPending(new Error('CDP websocket closed'));
        }
        this.pending.clear();
      });
    });
  }

  send(method, params = {}) {
    const id = this.nextId++;
    const message = { id, method, params };
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`CDP command timed out: ${method}`));
      }, 10_000);
      this.pending.set(id, { resolve, reject, timeout, method });
      this.ws.send(JSON.stringify(message));
    });
  }

  onMessage(raw) {
    const message = JSON.parse(raw);
    if (message.id) {
      const pending = this.pending.get(message.id);
      if (!pending) return;
      clearTimeout(pending.timeout);
      this.pending.delete(message.id);
      if (message.error) {
        pending.reject(new Error(`${pending.method}: ${message.error.message}`));
      } else {
        pending.resolve(message.result ?? {});
      }
      return;
    }

    if (message.method === 'Runtime.exceptionThrown') {
      this.errors.push(formatException(message.params?.exceptionDetails));
    } else if (message.method === 'Runtime.consoleAPICalled') {
      const type = message.params?.type;
      if (type === 'error' || type === 'assert') {
        this.errors.push(`console.${type}: ${(message.params?.args ?? []).map(formatRemoteObject).join(' ')}`);
      }
    } else if (message.method === 'Log.entryAdded') {
      const entry = message.params?.entry;
      if (entry?.level === 'error') {
        if ((entry.url ?? '').endsWith('/__fission/renderer')) {
          return;
        }
        this.errors.push(`browser log error: ${entry.text}${entry.url ? ` (${entry.url}:${entry.lineNumber ?? 0})` : ''}`);
      }
    }
  }

  close() {
    this.ws?.close();
  }
}

function formatRemoteObject(value) {
  if (!value) return '<missing>';
  if (Object.prototype.hasOwnProperty.call(value, 'value')) return JSON.stringify(value.value);
  return value.description ?? value.unserializableValue ?? value.type ?? '<unknown>';
}

function formatException(details) {
  if (!details) return 'runtime exception: <missing details>';
  const exception = details.exception?.description ?? details.exception?.value ?? details.text ?? 'unknown exception';
  const location = details.url ? ` at ${details.url}:${details.lineNumber ?? 0}:${details.columnNumber ?? 0}` : '';
  return `runtime exception: ${exception}${location}`;
}

function errorBlock(errors) {
  return errors.slice(0, 10).map((error, index) => `${index + 1}. ${error}`).join('\n');
}

async function readRuntimeStatus(client) {
  const expression = `(() => {
    const canvas = document.querySelector('canvas');
    if (!canvas) return { ready: false, reason: 'no canvas element' };
    const rect = canvas.getBoundingClientRect();
    const perf = globalThis.__FISSION_PERF ?? { frames: [], inputLatencies: [] };
    return {
      ready: rect.width > 0 && rect.height > 0,
      width: Math.round(rect.width),
      height: Math.round(rect.height),
      gpu: typeof navigator.gpu !== 'undefined',
      renderer: globalThis.__FISSION_RENDERER_INFO ?? null,
      frames: Array.isArray(perf.frames) ? perf.frames.slice(-120) : [],
      inputLatencies: Array.isArray(perf.inputLatencies) ? perf.inputLatencies.slice(-30) : [],
      title: document.title,
    };
  })()`;
  const result = await client.send('Runtime.evaluate', { expression, returnByValue: true });
  if (result.exceptionDetails) {
    throw new Error(formatException(result.exceptionDetails));
  }
  return result.result?.value ?? { ready: false, reason: 'evaluation returned no value' };
}

function average(values) {
  if (!values.length) return 0;
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

async function clickCanvasCenter(client, status) {
  const x = Math.max(1, Math.floor(status.width / 2));
  const y = Math.max(1, Math.floor(status.height / 2));
  await client.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, button: 'none' });
  await client.send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 });
  await client.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 });
}

async function main() {
  const wsUrl = await waitForTarget();
  const client = new CdpClient(wsUrl);
  await client.open();
  try {
    await Promise.all([
      client.send('Runtime.enable'),
      client.send('Log.enable'),
      client.send('Page.enable'),
    ]);

    const deadline = Date.now() + 60_000;
    let readySince = null;
    let lastStatus = null;
    while (Date.now() < deadline) {
      if (client.errors.length > 0) {
        throw new Error(`browser reported runtime/console errors:\n${errorBlock(client.errors)}`);
      }
      lastStatus = await readRuntimeStatus(client);
      if (lastStatus.ready && lastStatus.renderer) {
        readySince ??= Date.now();
        if (Date.now() - readySince >= 1_500) {
          const renderer = lastStatus.renderer.active;
          if (lastStatus.gpu && renderer === 'canvas2d-software' && !lastStatus.renderer.fallback_reason && process.env.FISSION_ALLOW_WEBGPU_FALLBACK !== '1') {
            throw new Error(`WebGPU is exposed but Fission used canvas2d-software without a fallback reason: ${JSON.stringify(lastStatus.renderer)}`);
          }
          await clickCanvasCenter(client, lastStatus);
          const inputDeadline = Date.now() + 10_000;
          while (Date.now() < inputDeadline) {
            lastStatus = await readRuntimeStatus(client);
            if ((lastStatus.inputLatencies ?? []).length > 0) break;
            await sleep(100);
          }
          const frames = lastStatus.frames ?? [];
          const latencies = lastStatus.inputLatencies ?? [];
          if (frames.length < 2) {
            throw new Error(`web perf smoke did not capture enough frame samples: ${JSON.stringify(lastStatus)}`);
          }
          if (latencies.length < 1) {
            throw new Error(`web perf smoke did not capture input latency samples: ${JSON.stringify(lastStatus)}`);
          }
          const avgFrame = average(frames.slice(-30));
          const avgLatency = average(latencies.slice(-10));
          if (avgFrame > Number(process.env.FISSION_WEB_MAX_AVG_FRAME_MS ?? 80)) {
            throw new Error(`web average frame time ${avgFrame.toFixed(2)}ms exceeded smoke threshold`);
          }
          if (avgLatency > Number(process.env.FISSION_WEB_MAX_INPUT_LATENCY_MS ?? 180)) {
            throw new Error(`web input latency ${avgLatency.toFixed(2)}ms exceeded smoke threshold`);
          }
          console.log(`Web app renderer ${renderer}; canvas ${lastStatus.width}x${lastStatus.height}; avg frame ${avgFrame.toFixed(2)}ms; avg input latency ${avgLatency.toFixed(2)}ms.`);
          return;
        }
      } else {
        readySince = null;
      }
      await sleep(250);
    }
    throw new Error(`web app did not render a non-empty canvas with renderer diagnostics. Last state: ${JSON.stringify(lastStatus)}`);
  } finally {
    client.close();
  }
}

main().catch((error) => {
  console.error(error.stack ?? error.message ?? String(error));
  process.exit(1);
});
NODE
"#
    .to_string()
}
fn render_app_main(package_name: &str) -> String {
    let lib_name = package_name.replace('-', "_");
    format!(
        r#"#[cfg(target_os = "android")]
fn main() {{}}

#[cfg(target_arch = "wasm32")]
fn main() {{}}

#[cfg(target_os = "ios")]
fn main() -> anyhow::Result<()> {{
    {lib_name}::run_mobile()
}}

#[cfg(not(any(target_arch = "wasm32", target_os = "ios", target_os = "android")))]
fn main() -> anyhow::Result<()> {{
    {lib_name}::run_desktop()
}}
"#
    )
}

const APP_LIB: &str = r#"pub mod app;

use crate::app::CounterApp;
use fission::prelude::*;

#[cfg(target_os = "android")]
const ANDROID_TEST_CONTROL_PORT: u16 = 48761;

#[cfg(any(target_os = "android", target_os = "ios"))]
fn mobile_app() -> MobileApp<crate::app::CounterState, CounterApp> {
    let app = MobileApp::<crate::app::CounterState, _>::new(CounterApp).with_title("Fission App");
    #[cfg(target_os = "android")]
    let app = app.with_test_control_port(ANDROID_TEST_CONTROL_PORT);
    app
}

#[cfg(target_arch = "wasm32")]
fn web_app() -> WebApp<crate::app::CounterState, CounterApp> {
    WebApp::<crate::app::CounterState, _>::new(CounterApp).with_title("Fission App")
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    DesktopApp::<crate::app::CounterState, _>::new(CounterApp).run()
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn run_mobile() -> anyhow::Result<()> {
    mobile_app().run()
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app_handle: AndroidApp) {
    let _ = mobile_app().run_with_android_app(app_handle);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    web_app()
        .run()
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}
"#;

const APP_RS: &str = r#"use fission::prelude::*;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CounterState {
    pub count: i32,
}

impl GlobalState for CounterState {}

#[fission_reducer(Increment)]
fn on_increment(state: &mut CounterState) {
    state.count += 1;
}

#[derive(Clone)]
pub struct CounterApp;

impl From<CounterApp> for Widget {
    fn from(component: CounterApp) -> Self {
        let (ctx, view) = fission::build::current::<CounterState>();
        let increment = with_reducer!(ctx, Increment, on_increment);

        Column {
            gap: Some(16.0),
            children: vec![
                Text::new(format!("Count: {}", view.state().count)).size(28.0).into(),
                Button {
                    on_press: Some(increment),
                    child: Some(Text::new("Increment").into()),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()

    }
}
"#;
