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
const GENERATED_APP_AGENTS_MARKER: &str = "<!-- fission-cli-generated-agents:v1 -->";
const GENERATED_APP_AGENTS_MD: &str = include_str!("../assets/AGENTS.md");

mod desktop_features;
mod icons;
mod linux_native;
mod macos_native;
mod macos_signing;
mod native_cargo;
mod native_variant;
mod splash;
mod windows_native;
pub use desktop_features::{read_desktop_cargo_options, DesktopCargoOptions};
pub use icons::{copy_icon_for_bundle, normalized_extension, resolve_app_icon, ResolvedIcon};
pub use linux_native::{
    build_linux_native_modules, stage_linux_native_products, test_linux_native_modules,
    BuiltLinuxNativeProduct, NativeLinuxModuleConfig, NativeLinuxProductConfig,
    NativeLinuxProductKind,
};
pub use macos_native::{
    build_macos_native_modules, embed_and_sign_macos_native_modules, test_macos_native_modules,
    MacosNativeBundleMode, NativeMacosModuleConfig, NativeMacosProductConfig,
    NativeMacosProductKind, NativeMacosProductSigningConfig,
};
pub use macos_signing::{
    read_macos_package_config, read_macos_package_config_for_profile,
    read_macos_package_config_for_profile_and_variant, read_macos_run_config,
    read_macos_run_config_for_profile, sign_macos_app_if_configured, MacosPackageConfig,
};
pub use native_variant::{ensure_native_variant_target, variant_output_path, NativeVariant};
pub use splash::{SplashConfig, SplashResizeMode};
pub use windows_native::{
    build_windows_native_modules, stage_windows_runtime_products, test_windows_native_modules,
    BuiltWindowsNativeProduct, NativeWindowsModuleConfig, NativeWindowsProductConfig,
    NativeWindowsProductKind,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    Android,
    Ios,
    Linux,
    Macos,
    #[value(name = "ssr", alias = "server")]
    #[serde(rename = "ssr", alias = "server")]
    Server,
    #[value(name = "static-site", alias = "site")]
    #[serde(rename = "static-site", alias = "site")]
    Site,
    Terminal,
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
            Self::Server => "ssr",
            Self::Site => "static-site",
            Self::Terminal => "terminal",
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
            Self::Server => "platforms/ssr/README.md",
            Self::Site => "platforms/site/README.md",
            Self::Terminal => "platforms/terminal/README.md",
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
    #[serde(alias = "identifier", alias = "id")]
    pub app_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub splash: Option<SplashConfig>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<NativeModuleConfig>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReleaseVersionConfig {
    pub version: Option<String>,
    pub build: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseVersionToml {
    app: Option<AppReleaseVersionConfig>,
    package: Option<PackageReleaseVersionConfig>,
    release: Option<ReleaseRootVersionConfig>,
    #[serde(default)]
    releases: Vec<ReleaseEntryVersionConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct AppReleaseVersionConfig {
    version: Option<String>,
    build: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
struct PackageReleaseVersionConfig {
    android: Option<AndroidReleaseVersionConfig>,
    ios: Option<IosReleaseVersionConfig>,
    macos: Option<MacosReleaseVersionConfig>,
    windows: Option<WindowsReleaseVersionConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct AndroidReleaseVersionConfig {
    version_code: Option<u64>,
    version_name: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct IosReleaseVersionConfig {
    marketing_version: Option<String>,
    build_number: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct MacosReleaseVersionConfig {
    marketing_version: Option<String>,
    build_number: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct WindowsReleaseVersionConfig {
    version: Option<String>,
    identity_name: Option<String>,
    publisher: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseRootVersionConfig {
    active_release: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseEntryVersionConfig {
    id: Option<String>,
    version: Option<String>,
    build: Option<u64>,
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
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub variants: BTreeSet<NativeVariant>,
    #[serde(default, skip_serializing_if = "NativeAndroidModuleConfig::is_empty")]
    pub android: NativeAndroidModuleConfig,
    #[serde(default, skip_serializing_if = "NativeIosModuleConfig::is_empty")]
    pub ios: NativeIosModuleConfig,
    #[serde(default, skip_serializing_if = "NativeLinuxModuleConfig::is_empty")]
    pub linux: NativeLinuxModuleConfig,
    #[serde(default, skip_serializing_if = "NativeMacosModuleConfig::is_empty")]
    pub macos: NativeMacosModuleConfig,
    #[serde(default, skip_serializing_if = "NativeWindowsModuleConfig::is_empty")]
    pub windows: NativeWindowsModuleConfig,
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
    write_generated_app_agents(root)?;
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

pub fn cargo_package_version(root: &Path) -> Option<String> {
    let manifest = fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let value: toml::Value = toml::from_str(&manifest).ok()?;
    value
        .get("package")
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_string)
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
        (Target::Server, "platforms/ssr"),
        (Target::Site, "content"),
        (Target::Terminal, "platforms/terminal"),
        (Target::Web, "platforms/web"),
        (Target::Windows, "platforms/windows"),
    ] {
        if root.join(relative).exists() {
            targets.insert(target);
        }
    }
    for (target, relative) in [
        (Target::Server, "platforms/server"),
        (Target::Site, "platforms/site"),
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

pub fn resolve_release_version_config(
    project_dir: &Path,
    target: Option<Target>,
) -> Result<ReleaseVersionConfig> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: ReleaseVersionToml = toml::from_str(&data).unwrap_or_default();
    let active = manifest
        .release
        .as_ref()
        .and_then(|release| release.active_release.as_deref())
        .and_then(|id| {
            manifest
                .releases
                .iter()
                .find(|release| release.id.as_deref() == Some(id))
        });

    let mut version = active
        .and_then(|release| release.version.clone())
        .or_else(|| manifest.app.as_ref().and_then(|app| app.version.clone()));
    let mut build = active
        .and_then(|release| release.build)
        .or_else(|| manifest.app.as_ref().and_then(|app| app.build));

    match target {
        Some(Target::Android) => {
            if let Some(android) = manifest
                .package
                .as_ref()
                .and_then(|package| package.android.as_ref())
            {
                version = android.version_name.clone().or(version);
                build = android.version_code.or(build);
            }
        }
        Some(Target::Ios) => {
            if let Some(ios) = manifest
                .package
                .as_ref()
                .and_then(|package| package.ios.as_ref())
            {
                version = ios.marketing_version.clone().or(version);
                build = ios
                    .build_number
                    .as_deref()
                    .and_then(|value| value.parse::<u64>().ok())
                    .or(build);
            }
        }
        Some(Target::Macos) => {
            if let Some(macos) = manifest
                .package
                .as_ref()
                .and_then(|package| package.macos.as_ref())
            {
                version = macos.marketing_version.clone().or(version);
                build = macos
                    .build_number
                    .as_deref()
                    .and_then(|value| value.parse::<u64>().ok())
                    .or(build);
            }
        }
        Some(Target::Windows) => {
            if let Some(windows) = manifest
                .package
                .as_ref()
                .and_then(|package| package.windows.as_ref())
            {
                version = windows.version.clone().or(version);
            }
        }
        _ => {}
    }

    if version.is_none() {
        version = cargo_package_version(project_dir);
    }
    Ok(ReleaseVersionConfig { version, build })
}

pub fn sync_release_platform_config(
    project_dir: &Path,
    target: Target,
    release: &ReleaseVersionConfig,
) -> Result<()> {
    match target {
        Target::Android => sync_android_release_config(project_dir, release),
        Target::Ios => sync_ios_release_config(project_dir, release),
        Target::Macos => sync_macos_release_config(project_dir, release),
        Target::Windows => sync_windows_release_config(project_dir, release),
        _ => Ok(()),
    }
}

pub fn sync_resolved_release_platform_config(
    project_dir: &Path,
    target: Target,
) -> Result<ReleaseVersionConfig> {
    let release = resolve_release_version_config(project_dir, Some(target))?;
    sync_release_platform_config(project_dir, target, &release)?;
    Ok(release)
}

fn sync_android_release_config(project_dir: &Path, release: &ReleaseVersionConfig) -> Result<()> {
    let path = project_dir.join("platforms/android/app/build.gradle.kts");
    if !path.exists() {
        return Ok(());
    }
    let version = release.version.as_deref().unwrap_or("0.1.0");
    let build = release.build.unwrap_or(1);
    rewrite_file_lines(&path, |trimmed| {
        if trimmed.starts_with("versionCode =") {
            Some(format!(
                "        versionCode = (System.getenv(\"ANDROID_VERSION_CODE\") ?: \"{build}\").toInt()"
            ))
        } else if trimmed.starts_with("versionName =") {
            Some(format!(
                "        versionName = System.getenv(\"ANDROID_VERSION_NAME\") ?: \"{version}\""
            ))
        } else {
            None
        }
    })
}

fn sync_ios_release_config(project_dir: &Path, release: &ReleaseVersionConfig) -> Result<()> {
    let path = project_dir.join("platforms/ios/package-sim.sh");
    if path.exists() {
        let version = release.version.as_deref().unwrap_or("0.1.0");
        let build = release.build.unwrap_or(1);
        let existing = fs::read_to_string(&path)?;
        let mut data = existing.clone();
        if !data.contains("IOS_MARKETING_VERSION") {
            data = data.replace(
                "BUNDLE_NAME=\"${IOS_BUNDLE_NAME:-$DISPLAY_NAME.app}\"\n",
                &format!(
                    "BUNDLE_NAME=\"${{IOS_BUNDLE_NAME:-$DISPLAY_NAME.app}}\"\nIOS_MARKETING_VERSION=\"${{IOS_MARKETING_VERSION:-{version}}}\"\nIOS_BUILD_NUMBER=\"${{IOS_BUILD_NUMBER:-{build}}}\"\n"
                ),
            );
        } else {
            data = data
                .lines()
                .map(|line| {
                    if line.starts_with("IOS_MARKETING_VERSION=") {
                        format!("IOS_MARKETING_VERSION=\"${{IOS_MARKETING_VERSION:-{version}}}\"")
                    } else if line.starts_with("IOS_BUILD_NUMBER=") {
                        format!("IOS_BUILD_NUMBER=\"${{IOS_BUILD_NUMBER:-{build}}}\"")
                    } else {
                        line.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            data.push('\n');
        }
        if data != existing {
            fs::write(&path, data)?;
        }
    }
    let plist = project_dir.join("platforms/ios/Info.plist");
    if plist.exists() {
        let version = release.version.as_deref().unwrap_or("0.1.0");
        let build = release.build.unwrap_or(1).to_string();
        rewrite_plist_string(&plist, "CFBundleShortVersionString", version)?;
        rewrite_plist_string(&plist, "CFBundleVersion", &build)?;
    }
    Ok(())
}

fn sync_macos_release_config(project_dir: &Path, release: &ReleaseVersionConfig) -> Result<()> {
    let plist = project_dir.join("platforms/macos/Info.plist");
    if plist.exists() {
        let version = release.version.as_deref().unwrap_or("0.1.0");
        let build = release.build.unwrap_or(1).to_string();
        rewrite_plist_string(&plist, "CFBundleShortVersionString", version)?;
        rewrite_plist_string(&plist, "CFBundleVersion", &build)?;
    }
    Ok(())
}

fn sync_windows_release_config(project_dir: &Path, release: &ReleaseVersionConfig) -> Result<()> {
    let config = read_windows_release_config(project_dir)?;
    let manifests = [
        project_dir.join("platforms/windows/Package.appxmanifest"),
        project_dir.join("platforms/windows/AppxManifest.xml"),
        project_dir.join("platforms/windows/appxmanifest.xml"),
    ];
    let has_manifest = manifests.iter().any(|path| path.exists());
    if !has_manifest {
        return Ok(());
    }

    let version = normalized_windows_package_version(release)?;
    for path in manifests.into_iter().filter(|path| path.exists()) {
        rewrite_windows_appx_manifest(
            &path,
            &version,
            config.identity_name.as_deref(),
            config.publisher.as_deref(),
        )?;
    }
    Ok(())
}

fn read_windows_release_config(project_dir: &Path) -> Result<WindowsReleaseVersionConfig> {
    let path = project_dir.join("fission.toml");
    let data = fs::read_to_string(&path).unwrap_or_default();
    let manifest: ReleaseVersionToml = toml::from_str(&data).unwrap_or_default();
    Ok(manifest
        .package
        .and_then(|package| package.windows)
        .unwrap_or_default())
}

pub fn normalize_windows_package_version(
    version: Option<&str>,
    build: Option<u64>,
) -> Result<String> {
    let version = version.unwrap_or("0.1.0");
    let parts = version.split('.').collect::<Vec<_>>();
    if parts.is_empty() || parts.len() > 4 {
        bail!("Windows package version `{version}` must have one to four numeric components");
    }
    let mut normalized = Vec::with_capacity(4);
    for part in &parts {
        let value = part
            .parse::<u16>()
            .with_context(|| format!("Windows package version `{version}` must be numeric"))?;
        normalized.push(value.to_string());
    }
    while normalized.len() < 3 {
        normalized.push("0".to_string());
    }
    if normalized.len() == 3 {
        let build = build.unwrap_or(0);
        if build > u16::MAX as u64 {
            bail!("Windows package build `{build}` must fit in a 16-bit version component");
        }
        normalized.push(build.to_string());
    }
    Ok(normalized.join("."))
}

fn normalized_windows_package_version(release: &ReleaseVersionConfig) -> Result<String> {
    normalize_windows_package_version(release.version.as_deref(), release.build)
}

fn rewrite_windows_appx_manifest(
    path: &Path,
    version: &str,
    identity_name: Option<&str>,
    publisher: Option<&str>,
) -> Result<()> {
    let existing = fs::read_to_string(path)?;
    let mut updated = rewrite_xml_attribute_on_tag(&existing, "Identity", "Version", version);
    if let Some(identity_name) = identity_name.filter(|value| !value.trim().is_empty()) {
        updated = rewrite_xml_attribute_on_tag(&updated, "Identity", "Name", identity_name.trim());
    }
    if let Some(publisher) = publisher.filter(|value| !value.trim().is_empty()) {
        updated = rewrite_xml_attribute_on_tag(&updated, "Identity", "Publisher", publisher.trim());
    }
    if updated != existing {
        fs::write(path, updated)?;
    }
    Ok(())
}

fn rewrite_xml_attribute_on_tag(input: &str, tag: &str, attribute: &str, value: &str) -> String {
    let Some(tag_start) = input.find(&format!("<{tag}")) else {
        return input.to_string();
    };
    let Some(relative_end) = input[tag_start..].find('>') else {
        return input.to_string();
    };
    let tag_end = tag_start + relative_end;
    let mut output = input.to_string();
    let tag_text = &input[tag_start..=tag_end];
    let escaped = escape_xml_attribute(value);
    let updated_tag = if let Some(attribute_start) = tag_text.find(&format!("{attribute}=\"")) {
        let value_start = attribute_start + attribute.len() + 2;
        if let Some(relative_quote) = tag_text[value_start..].find('"') {
            let value_end = value_start + relative_quote;
            let mut tag_output = tag_text.to_string();
            tag_output.replace_range(value_start..value_end, &escaped);
            tag_output
        } else {
            tag_text.to_string()
        }
    } else {
        let insert_at = tag_text
            .rfind('/')
            .filter(|slash| *slash + 1 == tag_text.len() - 1)
            .unwrap_or(tag_text.len() - 1);
        let mut tag_output = tag_text.to_string();
        tag_output.insert_str(insert_at, &format!(" {attribute}=\"{escaped}\""));
        tag_output
    };
    output.replace_range(tag_start..=tag_end, &updated_tag);
    output
}

fn escape_xml_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn rewrite_file_lines<F>(path: &Path, mut replacement: F) -> Result<()>
where
    F: FnMut(&str) -> Option<String>,
{
    let existing = fs::read_to_string(path)?;
    let mut updated = String::new();
    for line in existing.lines() {
        if let Some(new_line) = replacement(line.trim_start()) {
            updated.push_str(&new_line);
            updated.push('\n');
        } else {
            updated.push_str(line);
            updated.push('\n');
        }
    }
    if updated != existing {
        fs::write(path, updated)?;
    }
    Ok(())
}

fn rewrite_plist_string(path: &Path, key: &str, value: &str) -> Result<()> {
    let existing = fs::read_to_string(path)?;
    let mut lines = existing.lines().peekable();
    let mut updated = String::new();
    while let Some(line) = lines.next() {
        updated.push_str(line);
        updated.push('\n');
        if line.trim() == format!("<key>{key}</key>") {
            let _ = lines.next();
            updated.push_str(&format!("  <string>{value}</string>\n"));
        }
    }
    if updated != existing {
        fs::write(path, updated)?;
    }
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

/// Stages the project's `assets` directory as an application resource tree.
///
/// Desktop run and package commands use the same `assets` layout on every
/// platform so applications can ship large resources without compiling them
/// into the executable. A project without an `assets` directory is valid.
pub fn stage_project_assets(
    project_dir: &Path,
    destination_root: &Path,
) -> Result<Option<PathBuf>> {
    let source = project_dir.join("assets");
    if !source.exists() {
        return Ok(None);
    }
    if !source.is_dir() {
        bail!(
            "project assets path {} must be a directory",
            source.display()
        );
    }
    let destination = destination_root.join("assets");
    if destination.exists() {
        fs::remove_dir_all(&destination).with_context(|| {
            format!(
                "failed to clear staged project assets {}",
                destination.display()
            )
        })?;
    }
    copy_dir_contents(&source, &destination).with_context(|| {
        format!(
            "failed to stage project assets from {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(Some(destination))
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
    let mut updated = existing.clone();
    if updated.contains("import plistlib") {
        let Some(start) = updated.find("python3 - <<'PY' \"$SCRIPT_DIR/Info.plist\"") else {
            return Ok(());
        };
        let Some(relative_end) = updated[start..].find("\nPY") else {
            return Ok(());
        };
        let end = start + relative_end + "\nPY\n".len();
        updated.replace_range(start..end, IOS_INFO_PLIST_PLUTIL_PATCH);
    }
    if !updated.contains("IOS_MARKETING_VERSION") {
        updated = updated.replacen(
            "BUNDLE_NAME=\"${IOS_BUNDLE_NAME:-$DISPLAY_NAME.app}\"\n",
            "BUNDLE_NAME=\"${IOS_BUNDLE_NAME:-$DISPLAY_NAME.app}\"\nIOS_MARKETING_VERSION=\"${IOS_MARKETING_VERSION:-0.1.0}\"\nIOS_BUILD_NUMBER=\"${IOS_BUILD_NUMBER:-1}\"\n",
            1,
        );
    }
    if updated != existing {
        fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
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
    updated = updated.replace(
        "versionCode = 1",
        "versionCode = (System.getenv(\"ANDROID_VERSION_CODE\") ?: \"1\").toInt()",
    );
    updated = updated.replace(
        "versionName = \"0.1.0\"",
        "versionName = System.getenv(\"ANDROID_VERSION_NAME\") ?: \"0.1.0\"",
    );
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
"$PLUTIL" -replace CFBundleShortVersionString -string "$IOS_MARKETING_VERSION" "$BUNDLE_DIR/Info.plist"
"$PLUTIL" -replace CFBundleVersion -string "$IOS_BUILD_NUMBER" "$BUNDLE_DIR/Info.plist"
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
    if target == Target::Site && project_dir.join("content").exists() {
        return true;
    }
    if target == Target::Site && project_dir.join("platforms/site").exists() {
        return true;
    }
    if target == Target::Server && project_dir.join("platforms/server").exists() {
        return true;
    }
    Path::new(target.scaffold_relative_path())
        .parent()
        .is_some_and(|relative| project_dir.join(relative).exists())
}

fn write_project_config(root: &Path, project: &FissionProject) -> Result<()> {
    let path = root.join("fission.toml");
    let mut doc = if path.exists() {
        let existing = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        existing
            .parse::<DocumentMut>()
            .with_context(|| format!("failed to parse {}", path.display()))?
    } else {
        toml::to_string_pretty(project)?
            .parse::<DocumentMut>()
            .context("failed to render initial fission.toml")?
    };
    update_project_config_document(root, &mut doc, project);
    write_file(&path, &doc.to_string())
}

fn update_project_config_document(root: &Path, doc: &mut DocumentMut, project: &FissionProject) {
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
    if item_field_is_missing(&doc["app"], "version") {
        doc["app"]["version"] =
            value(cargo_package_version(root).unwrap_or_else(|| "0.1.0".to_string()));
    }
    if item_field_is_missing(&doc["app"], "build") {
        doc["app"]["build"] = value(1);
    }
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
    ensure_package_defaults(doc, project);
    ensure_distribution_defaults(doc, project);
}

fn ensure_package_defaults(doc: &mut DocumentMut, project: &FissionProject) {
    if project.targets.contains(&Target::Android) {
        let version_name = item_field_string(&doc["app"], "version")
            .unwrap_or("0.1.0")
            .to_string();
        let version_code = item_field_integer(&doc["app"], "build").unwrap_or(1);
        let android = ensure_package_target_table(doc, "android");
        set_default_string(android, "package_name", &project.app.app_id);
        set_default_integer(android, "version_code", version_code);
        set_default_string(android, "version_name", &version_name);
        set_default_integer(android, "min_sdk", 24);
        set_default_integer(android, "target_sdk", 35);
        set_default_string(android, "keystore_alias", "upload");
        set_default_string(android, "keystore_env", "ANDROID_KEYSTORE");
        set_default_string(android, "keystore_base64_env", "ANDROID_KEYSTORE_BASE64");
        set_default_string(
            android,
            "keystore_password_env",
            "ANDROID_KEYSTORE_PASSWORD",
        );
        set_default_string(android, "key_password_env", "ANDROID_KEY_PASSWORD");
    }

    if project.targets.contains(&Target::Ios) {
        let marketing_version = item_field_string(&doc["app"], "version")
            .unwrap_or("0.1.0")
            .to_string();
        let build_number = item_field_integer(&doc["app"], "build")
            .unwrap_or(1)
            .to_string();
        let ios = ensure_package_target_table(doc, "ios");
        set_default_string(ios, "bundle_id", &project.app.app_id);
        set_default_string(ios, "marketing_version", &marketing_version);
        set_default_string(ios, "build_number", &build_number);
    }

    if project.targets.contains(&Target::Macos) {
        let marketing_version = item_field_string(&doc["app"], "version")
            .unwrap_or("0.1.0")
            .to_string();
        let build_number = item_field_integer(&doc["app"], "build")
            .unwrap_or(1)
            .to_string();
        let macos = ensure_package_target_table(doc, "macos");
        set_default_string(macos, "bundle_id", &project.app.app_id);
        set_default_string(macos, "marketing_version", &marketing_version);
        set_default_string(macos, "build_number", &build_number);
        set_default_string(macos, "minimum_os", "13.0");
    }

    if project.targets.contains(&Target::Windows) {
        let package_version = item_field_string(&doc["app"], "version")
            .unwrap_or("0.1.0")
            .to_string();
        let windows = ensure_package_target_table(doc, "windows");
        set_default_string(windows, "identity_name", &windows_identity_name(project));
        set_default_string(windows, "publisher", windows_publisher_name());
        set_default_string(windows, "version", &package_version);
        set_default_string(windows, "installer", "msix");
        set_default_string(
            windows,
            "certificate_thumbprint_env",
            "WINDOWS_CERTIFICATE_THUMBPRINT",
        );
        set_default_string(
            windows,
            "certificate_base64_env",
            "WINDOWS_CERTIFICATE_BASE64",
        );
        set_default_string(
            windows,
            "certificate_password_env",
            "WINDOWS_CERTIFICATE_PASSWORD",
        );
    }

    if let Some(package) = doc
        .as_table_mut()
        .get_mut("package")
        .and_then(Item::as_table_mut)
    {
        if package
            .iter()
            .all(|(_, item)| item.as_table().is_some() || item.as_array_of_tables().is_some())
        {
            package.set_implicit(true);
        }
    }
}

fn ensure_distribution_defaults(doc: &mut DocumentMut, project: &FissionProject) {
    if project.targets.contains(&Target::Android) {
        let play_store = ensure_distribution_target_table(doc, "play_store");
        set_default_string(play_store, "package_name", &project.app.app_id);
        set_default_string(play_store, "default_track", "internal");
        set_default_string(play_store, "release_status", "completed");
        set_default_string(play_store, "access_token_env", "PLAY_STORE_ACCESS_TOKEN");
        set_default_string(
            play_store,
            "service_account_json_env",
            "PLAY_STORE_SERVICE_ACCOUNT_JSON",
        );
        set_default_string(
            play_store,
            "service_account_json_base64_env",
            "PLAY_STORE_SERVICE_ACCOUNT_JSON_BASE64",
        );
        set_default_string(
            play_store,
            "google_application_credentials_env",
            "GOOGLE_APPLICATION_CREDENTIALS",
        );
    }

    if project.targets.contains(&Target::Ios) {
        let app_store = ensure_distribution_target_table(doc, "app_store");
        set_default_string(app_store, "bundle_id", &project.app.app_id);
        set_default_string(
            app_store,
            "access_token_env",
            "APP_STORE_CONNECT_ACCESS_TOKEN",
        );
        set_default_string(app_store, "issuer_id_env", "APP_STORE_CONNECT_ISSUER_ID");
        set_default_string(app_store, "key_id_env", "APP_STORE_CONNECT_KEY_ID");
        set_default_string(app_store, "api_key_env", "APP_STORE_CONNECT_API_KEY");
        set_default_string(
            app_store,
            "api_key_base64_env",
            "APP_STORE_CONNECT_API_KEY_BASE64",
        );
        set_default_string(
            app_store,
            "api_key_path_env",
            "APP_STORE_CONNECT_API_KEY_PATH",
        );
        set_default_string(app_store, "default_track", "testflight");
    }

    if project.targets.contains(&Target::Windows) {
        let microsoft_store = ensure_distribution_target_table(doc, "microsoft_store");
        set_default_string(
            microsoft_store,
            "package_identity_name",
            &windows_identity_name(project),
        );
        set_default_string(microsoft_store, "package_type", "msix");
        set_default_string(microsoft_store, "token_env", "MICROSOFT_STORE_TOKEN");
        set_default_string(microsoft_store, "tenant_id_env", "AZURE_TENANT_ID");
        set_default_string(microsoft_store, "client_id_env", "AZURE_CLIENT_ID");
        set_default_string(
            microsoft_store,
            "client_secret_env",
            "MICROSOFT_STORE_CLIENT_SECRET",
        );
        set_default_string(
            microsoft_store,
            "seller_id_env",
            "MICROSOFT_STORE_SELLER_ID",
        );
    }

    if let Some(distribution) = doc
        .as_table_mut()
        .get_mut("distribution")
        .and_then(Item::as_table_mut)
    {
        if distribution
            .iter()
            .all(|(_, item)| item.as_table().is_some() || item.as_array_of_tables().is_some())
        {
            distribution.set_implicit(true);
        }
    }
}

fn ensure_package_target_table<'a>(doc: &'a mut DocumentMut, target: &str) -> &'a mut Item {
    let missing_or_not_table = match doc.as_table().get("package") {
        Some(item) => !item.is_table(),
        None => true,
    };
    if missing_or_not_table {
        let mut table = Table::new();
        table.set_implicit(true);
        doc["package"] = Item::Table(table);
    }
    let target_missing_or_not_table = match doc["package"].as_table_like() {
        Some(package) => package.get(target).is_none_or(|item| !item.is_table()),
        None => true,
    };
    if target_missing_or_not_table {
        doc["package"][target] = Item::Table(Table::new());
    }
    &mut doc["package"][target]
}

fn ensure_distribution_target_table<'a>(doc: &'a mut DocumentMut, provider: &str) -> &'a mut Item {
    let missing_or_not_table = match doc.as_table().get("distribution") {
        Some(item) => !item.is_table(),
        None => true,
    };
    if missing_or_not_table {
        let mut table = Table::new();
        table.set_implicit(true);
        doc["distribution"] = Item::Table(table);
    }
    let provider_missing_or_not_table = match doc["distribution"].as_table_like() {
        Some(distribution) => distribution
            .get(provider)
            .is_none_or(|item| !item.is_table()),
        None => true,
    };
    if provider_missing_or_not_table {
        doc["distribution"][provider] = Item::Table(Table::new());
    }
    &mut doc["distribution"][provider]
}

fn set_default_string(item: &mut Item, key: &str, value_: &str) {
    if item_field_is_missing(item, key) {
        item[key] = value(value_.to_string());
    }
}

fn set_default_integer(item: &mut Item, key: &str, value_: i64) {
    if item_field_is_missing(item, key) {
        item[key] = value(value_);
    }
}

fn item_field_is_missing(item: &Item, key: &str) -> bool {
    item.as_table_like()
        .and_then(|table| table.get(key))
        .is_none()
}

fn item_field_string<'a>(item: &'a Item, key: &str) -> Option<&'a str> {
    item.as_table_like()
        .and_then(|table| table.get(key))
        .and_then(Item::as_value)
        .and_then(Value::as_str)
}

fn item_field_integer(item: &Item, key: &str) -> Option<i64> {
    item.as_table_like()
        .and_then(|table| table.get(key))
        .and_then(Item::as_value)
        .and_then(Value::as_integer)
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
                    "Run `fission package --target android --format aab --release --project-dir .` or `./platforms/android/package-aab.sh` to create the signed Play Store app bundle.",
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
                    "Run `fission package --target ios --format ipa --release --project-dir .` or `./platforms/ios/package-ipa.sh` to create a signed IPA when IOS_SIGNING_IDENTITY is configured.",
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
            "SSR",
            "Server-rendered Fission target. The CLI runs the app through the server shell for dynamic HTML, revalidated pages, server jobs, signed actions, worker artifacts, and focused browser islands.",
            &[
                "Configure `[server].entry` in `fission.toml` so the CLI can invoke the server app.",
                "Run `fission server check --project-dir .` to render all declared server routes.",
                "Run `fission server serve --project-dir .` to serve the app locally.",
                "Run `fission server artifacts --project-dir .` to generate browser worker and island WASM shims.",
                "Run `fission package --target ssr --format docker-image --release --project-dir .` to package the server app as an OCI/Docker image.",
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
                    "Run `fission package --target static-site --format static --release --project-dir .` to package the generated site.",
                    "Unsupported interactive widgets fail during the static render instead of silently falling back to JavaScript.",
                ],
            )
        }
        Target::Terminal => platform_readme(
            "Terminal",
            "Terminal target. The CLI treats this as a terminal-shell app using the project's normal Rust entrypoint and terminal-shell feature.",
            &[
                "Use `fission::terminal::TerminalApp` or a target-aware app entrypoint for terminal rendering.",
                "Run `fission run --target terminal --project-dir .` to execute the app in the current terminal.",
                "Run `fission test --target terminal --project-dir .` for Rust tests until terminal-shell package formats are defined by the terminal-shell RFC.",
                "This target enables the `terminal-shell` Fission feature but does not imply native desktop, web, or mobile shells.",
            ],
        ),
        Target::Windows => {
            scaffold_windows_bundle(root, project, write_policy)?;
            platform_readme(
                "Windows",
                "Runnable desktop target with release packaging scaffolds for EXE, MSI, and MSIX distribution.",
                &[
                    "Run `fission run --project-dir .` from the project root to launch the desktop app and attach output.",
                    "Run `fission build --project-dir . --release` for a release desktop build.",
                    "Run `fission package --target windows --format exe --release --project-dir .` to copy the signed release executable into a package artifact.",
                    "Run `fission package --target windows --format msix --release --project-dir .` or `./platforms/windows/package-msix.ps1` to create an MSIX package with `makeappx`.",
                    "Run `fission package --target windows --format msi --release --project-dir .` or `./platforms/windows/package-msi.ps1` to create an MSI package with WiX.",
                    "Set `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_BASE64`, or `WINDOWS_CERTIFICATE_THUMBPRINT` plus `WINDOWS_CERTIFICATE_PASSWORD` where needed; never commit certificate files or passwords.",
                    "Edit `[package.windows]` in `fission.toml` for Store package identity, publisher identity, package version, and installer preference.",
                    "For an unpackaged NSIS app, build the architecture-matched shortcut helper with `./platforms/windows/build-shortcut-aumid-helper.ps1 -Architecture x64` (or `arm64`) and include `platforms/windows/fission-shortcut-aumid.nsh`. Embed the helper once, then apply one stable AppUserModelID to every Start Menu shortcut after `CreateShortCut`.",
                    "Pass that exact AppUserModelID to `DesktopApp::with_windows_app_user_model_id`; package identity remains authoritative for MSIX, so the explicit value is only the unpackaged fallback.",
                    "Sign the compiled shortcut helper before embedding it in a signed installer. The helper deliberately fails installation if it cannot persist the shortcut identity.",
                    "The generated MSIX manifest stages the desktop executable as a full-trust Windows app and copies `assets/app-icon.png` into the package asset set by default.",
                ],
            )
        }
        Target::Linux | Target::Macos => platform_readme(
            match target {
                Target::Linux => "Linux",
                Target::Macos => "macOS",
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
    let ipa_script = render_ios_ipa_package_script(project);
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
        &root.join("platforms/ios/package-ipa.sh"),
        &ipa_script,
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
            "platforms/ios/package-ipa.sh",
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
    let package_aab_script = render_android_aab_package_script(project);
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
        &root.join("platforms/android/package-aab.sh"),
        &package_aab_script,
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
            "platforms/android/package-aab.sh",
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

fn scaffold_windows_bundle(
    root: &Path,
    project: &FissionProject,
    write_policy: WritePolicy,
) -> Result<()> {
    let executable = windows_executable_name(root, project);
    write_file_with_policy(
        &root.join("platforms/windows/Package.appxmanifest"),
        &render_windows_appx_manifest(project, &executable),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/windows/package-msix.ps1"),
        &render_windows_msix_package_script(project, &executable),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/windows/package-msi.ps1"),
        &render_windows_msi_package_script(project, &executable),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/windows/shortcut-aumid-helper.cpp"),
        render_windows_shortcut_aumid_helper_source(),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/windows/build-shortcut-aumid-helper.ps1"),
        render_windows_shortcut_aumid_helper_build_script(),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/windows/fission-shortcut-aumid.nsh"),
        render_windows_shortcut_aumid_nsis_include(),
        write_policy,
    )?;
    Ok(())
}

fn windows_executable_name(root: &Path, project: &FissionProject) -> String {
    let stem = cargo_package_name(root).unwrap_or_else(|| sanitize_file_stem(&project.app.name));
    format!("{stem}.exe")
}

fn windows_identity_name(project: &FissionProject) -> String {
    let mut out = project
        .app
        .app_id
        .chars()
        .map(|ch| match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '-' => ch,
            '_' => '.',
            _ => '.',
        })
        .collect::<String>();
    while out.contains("..") {
        out = out.replace("..", ".");
    }
    out = out.trim_matches(['.', '-']).to_string();
    if out.is_empty() {
        "Fission.App".to_string()
    } else {
        out
    }
}

fn windows_publisher_name() -> &'static str {
    "CN=Fission Developer"
}

fn render_windows_appx_manifest(project: &FissionProject, executable: &str) -> String {
    let display_name = escape_xml_attribute(&project.app.name);
    let identity_name = escape_xml_attribute(&windows_identity_name(project));
    let publisher = escape_xml_attribute(windows_publisher_name());
    let install_dir = escape_xml_attribute(&sanitize_file_stem(&project.app.name));
    let executable = escape_xml_attribute(executable);
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap rescap">
  <Identity Name="{identity_name}" Publisher="{publisher}" Version="0.1.0.1" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>{display_name}</DisplayName>
    <PublisherDisplayName>Fission Developer</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.22621.0" />
  </Dependencies>
  <Resources>
    <Resource Language="en-us" />
  </Resources>
  <Applications>
    <Application Id="App" Executable="VFS\ProgramFilesX64\{install_dir}\{executable}" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="{display_name}" Description="{display_name}" BackgroundColor="transparent" Square150x150Logo="Assets\Square150x150Logo.png" Square44x44Logo="Assets\Square44x44Logo.png" />
    </Application>
  </Applications>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
</Package>
"#
    )
}

fn render_windows_msix_package_script(project: &FissionProject, executable: &str) -> String {
    let app_name = sanitize_file_stem(&project.app.name);
    let package_name = windows_identity_name(project);
    let template = r#"$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Resolve-Path (Join-Path $ScriptDir "..\..")
$Profile = if ($env:WINDOWS_PROFILE) { $env:WINDOWS_PROFILE } else { "debug" }
$CargoProfileArg = if ($Profile -eq "release") { @("--release") } else { @() }
$ExecutableName = if ($env:WINDOWS_EXECUTABLE_NAME) { $env:WINDOWS_EXECUTABLE_NAME } else { "__EXECUTABLE__" }
$BinaryPath = if ($env:WINDOWS_BINARY) { $env:WINDOWS_BINARY } else { Join-Path $ProjectDir "target\$Profile\$ExecutableName" }
$OutRoot = Join-Path $ProjectDir "target\fission\windows\msix"
$LayoutDir = Join-Path $OutRoot "layout"
$AppDir = Join-Path $LayoutDir "VFS\ProgramFilesX64\__APP_NAME__"
$AssetsDir = Join-Path $LayoutDir "Assets"
$MsixPath = Join-Path $OutRoot "__PACKAGE_NAME__-$Profile.msix"

if (-not $env:WINDOWS_BINARY) {
  cargo build @CargoProfileArg --manifest-path (Join-Path $ProjectDir "Cargo.toml")
}
if (-not (Test-Path $BinaryPath)) {
  throw "Windows executable was not found at $BinaryPath. Set WINDOWS_BINARY or WINDOWS_EXECUTABLE_NAME if the crate name changed."
}
$MakeAppx = Get-Command makeappx -ErrorAction SilentlyContinue
if (-not $MakeAppx) {
  throw "makeappx was not found. Install Windows SDK MSIX packaging tools and ensure makeappx is on PATH."
}

Remove-Item -Recurse -Force $LayoutDir -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $AppDir, $AssetsDir | Out-Null
Copy-Item $BinaryPath (Join-Path $AppDir $ExecutableName) -Force
if ($env:FISSION_WINDOWS_NATIVE_PRODUCTS_MANIFEST) {
  $NativeManifest = Get-Content -Raw $env:FISSION_WINDOWS_NATIVE_PRODUCTS_MANIFEST | ConvertFrom-Json
  foreach ($Product in $NativeManifest.products) {
    if ($Product.kind -eq "driver-package") {
      throw "MSIX native product manifest must not contain driver package $($Product.name)."
    }
    $NativeDestination = Join-Path $AppDir $Product.destination
    $NativeParent = Split-Path -Parent $NativeDestination
    New-Item -ItemType Directory -Force $NativeParent | Out-Null
    if (Test-Path $Product.source -PathType Container) {
      Copy-Item $Product.source $NativeDestination -Recurse -Force
    } else {
      Copy-Item $Product.source $NativeDestination -Force
    }
  }
}
Copy-Item (Join-Path $ScriptDir "Package.appxmanifest") (Join-Path $LayoutDir "AppxManifest.xml") -Force

$IconSource = if ($env:WINDOWS_APP_ICON) { $env:WINDOWS_APP_ICON } else { Join-Path $ProjectDir "assets\app-icon.png" }
if (Test-Path $IconSource) {
  Copy-Item $IconSource (Join-Path $AssetsDir "StoreLogo.png") -Force
  Copy-Item $IconSource (Join-Path $AssetsDir "Square44x44Logo.png") -Force
  Copy-Item $IconSource (Join-Path $AssetsDir "Square150x150Logo.png") -Force
}

& $MakeAppx.Source pack /d $LayoutDir /p $MsixPath /overwrite | Out-Host

$Certificate = $env:WINDOWS_CERTIFICATE
$TempCertificate = $null
try {
  if (-not $Certificate -and $env:WINDOWS_CERTIFICATE_BASE64) {
    $TempCertificate = Join-Path ([System.IO.Path]::GetTempPath()) ("fission-windows-cert-" + [System.Guid]::NewGuid().ToString() + ".pfx")
    [System.IO.File]::WriteAllBytes($TempCertificate, [System.Convert]::FromBase64String($env:WINDOWS_CERTIFICATE_BASE64))
    $Certificate = $TempCertificate
  }
  $Thumbprint = $env:WINDOWS_CERTIFICATE_THUMBPRINT
  if ($Certificate -or $Thumbprint) {
    $SignTool = Get-Command signtool -ErrorAction SilentlyContinue
    if (-not $SignTool) {
      throw "signtool was not found. Install Windows SDK signing tools or set WINDOWS_SKIP_SIGNING=1 for unsigned local packages."
    }
    $SignArgs = @("sign", "/fd", "SHA256")
    if ($Certificate) {
      $SignArgs += @("/f", $Certificate)
      if ($env:WINDOWS_CERTIFICATE_PASSWORD) { $SignArgs += @("/p", $env:WINDOWS_CERTIFICATE_PASSWORD) }
    } else {
      $SignArgs += @("/sha1", $Thumbprint)
    }
    $SignArgs += $MsixPath
    & $SignTool.Source @SignArgs | Out-Host
  } elseif ($Profile -eq "release" -and $env:WINDOWS_SKIP_SIGNING -ne "1") {
    throw "Release MSIX packaging requires WINDOWS_CERTIFICATE, WINDOWS_CERTIFICATE_BASE64, or WINDOWS_CERTIFICATE_THUMBPRINT from a secure secret source. Set WINDOWS_SKIP_SIGNING=1 only for local unsigned validation."
  }
} finally {
  if ($TempCertificate) { Remove-Item -Force $TempCertificate -ErrorAction SilentlyContinue }
}

Write-Output $MsixPath
"#;
    template
        .replace("__APP_NAME__", &app_name)
        .replace("__PACKAGE_NAME__", &package_name)
        .replace("__EXECUTABLE__", executable)
}

fn render_windows_msi_package_script(project: &FissionProject, executable: &str) -> String {
    let app_name = sanitize_file_stem(&project.app.name);
    let display_name = project.app.name.clone();
    let upgrade_code = deterministic_guid(&project.app.app_id);
    let manufacturer = "Fission Developer";
    let template = r#"$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Resolve-Path (Join-Path $ScriptDir "..\..")
$Profile = if ($env:WINDOWS_PROFILE) { $env:WINDOWS_PROFILE } else { "debug" }
$CargoProfileArg = if ($Profile -eq "release") { @("--release") } else { @() }
$ExecutableName = if ($env:WINDOWS_EXECUTABLE_NAME) { $env:WINDOWS_EXECUTABLE_NAME } else { "__EXECUTABLE__" }
$BinaryPath = if ($env:WINDOWS_BINARY) { $env:WINDOWS_BINARY } else { Join-Path $ProjectDir "target\$Profile\$ExecutableName" }
$OutRoot = Join-Path $ProjectDir "target\fission\windows\msi"
$MsiPath = Join-Path $OutRoot "__APP_NAME__-$Profile.msi"
$Version = if ($env:WINDOWS_MSI_VERSION) { $env:WINDOWS_MSI_VERSION } else { "0.1.0" }
$UpgradeCode = if ($env:WINDOWS_MSI_UPGRADE_CODE) { $env:WINDOWS_MSI_UPGRADE_CODE } else { "__UPGRADE_CODE__" }

if (-not $env:WINDOWS_BINARY) {
  cargo build @CargoProfileArg --manifest-path (Join-Path $ProjectDir "Cargo.toml")
}
if (-not (Test-Path $BinaryPath)) {
  throw "Windows executable was not found at $BinaryPath. Set WINDOWS_BINARY or WINDOWS_EXECUTABLE_NAME if the crate name changed."
}
New-Item -ItemType Directory -Force $OutRoot | Out-Null

$Wix = Get-Command wix -ErrorAction SilentlyContinue
$Candle = Get-Command candle -ErrorAction SilentlyContinue
$Light = Get-Command light -ErrorAction SilentlyContinue
if ($Wix) {
  $WxsPath = Join-Path $OutRoot "package.wxs"
  @"
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package Name="__DISPLAY_NAME__" Manufacturer="__MANUFACTURER__" Version="$Version" UpgradeCode="$UpgradeCode" Scope="perMachine">
    <MajorUpgrade DowngradeErrorMessage="A newer version of __DISPLAY_NAME__ is already installed." />
    <MediaTemplate EmbedCab="yes" />
    <StandardDirectory Id="ProgramFiles6432Folder">
      <Directory Id="INSTALLFOLDER" Name="__APP_NAME__">
        <Component Id="MainExecutable" Guid="*">
          <File Id="AppExe" Source="$BinaryPath" KeyPath="yes" />
        </Component>
      </Directory>
    </StandardDirectory>
    <Feature Id="MainFeature" Title="__DISPLAY_NAME__" Level="1">
      <ComponentRef Id="MainExecutable" />
    </Feature>
  </Package>
</Wix>
"@ | Set-Content -Encoding UTF8 $WxsPath
  & $Wix.Source build $WxsPath -o $MsiPath | Out-Host
} elseif ($Candle -and $Light) {
  $WxsPath = Join-Path $OutRoot "package-wix3.wxs"
  $WixObj = Join-Path $OutRoot "package.wixobj"
  @"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="__DISPLAY_NAME__" Language="1033" Version="$Version" Manufacturer="__MANUFACTURER__" UpgradeCode="$UpgradeCode">
    <Package InstallerVersion="500" Compressed="yes" InstallScope="perMachine" />
    <MajorUpgrade DowngradeErrorMessage="A newer version of __DISPLAY_NAME__ is already installed." />
    <MediaTemplate EmbedCab="yes" />
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder">
        <Directory Id="INSTALLFOLDER" Name="__APP_NAME__">
          <Component Id="MainExecutable" Guid="*">
            <File Id="AppExe" Source="$BinaryPath" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>
    <Feature Id="MainFeature" Title="__DISPLAY_NAME__" Level="1">
      <ComponentRef Id="MainExecutable" />
    </Feature>
  </Product>
</Wix>
"@ | Set-Content -Encoding UTF8 $WxsPath
  & $Candle.Source -nologo -arch x64 -out $WixObj $WxsPath | Out-Host
  & $Light.Source -nologo -out $MsiPath $WixObj | Out-Host
} else {
  throw "WiX was not found. Install WiX Toolset (`wix`) or WiX 3 (`candle` and `light`) to package an MSI."
}

$Certificate = $env:WINDOWS_CERTIFICATE
$TempCertificate = $null
try {
  if (-not $Certificate -and $env:WINDOWS_CERTIFICATE_BASE64) {
    $TempCertificate = Join-Path ([System.IO.Path]::GetTempPath()) ("fission-windows-cert-" + [System.Guid]::NewGuid().ToString() + ".pfx")
    [System.IO.File]::WriteAllBytes($TempCertificate, [System.Convert]::FromBase64String($env:WINDOWS_CERTIFICATE_BASE64))
    $Certificate = $TempCertificate
  }
  $Thumbprint = $env:WINDOWS_CERTIFICATE_THUMBPRINT
  if ($Certificate -or $Thumbprint) {
    $SignTool = Get-Command signtool -ErrorAction SilentlyContinue
    if (-not $SignTool) {
      throw "signtool was not found. Install Windows SDK signing tools or set WINDOWS_SKIP_SIGNING=1 for unsigned local packages."
    }
    $SignArgs = @("sign", "/fd", "SHA256")
    if ($Certificate) {
      $SignArgs += @("/f", $Certificate)
      if ($env:WINDOWS_CERTIFICATE_PASSWORD) { $SignArgs += @("/p", $env:WINDOWS_CERTIFICATE_PASSWORD) }
    } else {
      $SignArgs += @("/sha1", $Thumbprint)
    }
    $SignArgs += $MsiPath
    & $SignTool.Source @SignArgs | Out-Host
  } elseif ($Profile -eq "release" -and $env:WINDOWS_SKIP_SIGNING -ne "1") {
    throw "Release MSI packaging requires WINDOWS_CERTIFICATE, WINDOWS_CERTIFICATE_BASE64, or WINDOWS_CERTIFICATE_THUMBPRINT from a secure secret source. Set WINDOWS_SKIP_SIGNING=1 only for local unsigned validation."
  }
} finally {
  if ($TempCertificate) { Remove-Item -Force $TempCertificate -ErrorAction SilentlyContinue }
}

Write-Output $MsiPath
"#;
    template
        .replace("__APP_NAME__", &app_name)
        .replace("__DISPLAY_NAME__", &display_name)
        .replace("__MANUFACTURER__", manufacturer)
        .replace("__UPGRADE_CODE__", &upgrade_code)
        .replace("__EXECUTABLE__", executable)
}

fn render_windows_shortcut_aumid_helper_source() -> &'static str {
    r#"#include <windows.h>

#include <cwchar>
#include <cwctype>
#include <cstdio>

#include <propkey.h>
#include <propvarutil.h>
#include <shobjidl.h>
#include <wrl/client.h>

namespace {

using Microsoft::WRL::ComPtr;

bool IsValidAppUserModelId(const wchar_t* app_user_model_id) {
  if (app_user_model_id == nullptr) {
    return false;
  }

  const size_t length = std::wcslen(app_user_model_id);
  if (length == 0 || length > 128) {
    return false;
  }

  for (size_t index = 0; index < length; ++index) {
    if (std::iswspace(app_user_model_id[index]) != 0) {
      return false;
    }
  }

  return true;
}

int ReportFailure(const wchar_t* operation, HRESULT result) {
  std::fwprintf(
      stderr,
      L"%ls failed (HRESULT 0x%08lX).\n",
      operation,
      static_cast<unsigned long>(result));
  return 1;
}

}  // namespace

int wmain(int argc, wchar_t** argv) {
  if (argc != 3) {
    std::fwprintf(
        stderr,
        L"Usage: fission-shortcut-aumid.exe <shortcut.lnk> <app-user-model-id>\n");
    return 2;
  }

  const wchar_t* shortcut_path = argv[1];
  const wchar_t* app_user_model_id = argv[2];
  if (!IsValidAppUserModelId(app_user_model_id)) {
    std::fwprintf(
        stderr,
        L"The AppUserModelID must contain 1-128 UTF-16 code units and no whitespace.\n");
    return 3;
  }

  const HRESULT initialize_result =
      CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
  const bool should_uninitialize = SUCCEEDED(initialize_result);
  if (FAILED(initialize_result) && initialize_result != RPC_E_CHANGED_MODE) {
    return ReportFailure(L"CoInitializeEx", initialize_result);
  }

  int exit_code = 0;
  ComPtr<IShellLinkW> shell_link;
  HRESULT result = CoCreateInstance(
      CLSID_ShellLink,
      nullptr,
      CLSCTX_INPROC_SERVER,
      IID_PPV_ARGS(&shell_link));
  if (FAILED(result)) {
    exit_code = ReportFailure(L"CoCreateInstance(CLSID_ShellLink)", result);
    goto finish;
  }

  {
    ComPtr<IPersistFile> persist_file;
    result = shell_link.As(&persist_file);
    if (FAILED(result)) {
      exit_code = ReportFailure(L"QueryInterface(IPersistFile)", result);
      goto finish;
    }

    result = persist_file->Load(shortcut_path, STGM_READWRITE);
    if (FAILED(result)) {
      exit_code = ReportFailure(L"IPersistFile::Load", result);
      goto finish;
    }

    ComPtr<IPropertyStore> property_store;
    result = shell_link.As(&property_store);
    if (FAILED(result)) {
      exit_code = ReportFailure(L"QueryInterface(IPropertyStore)", result);
      goto finish;
    }

    PROPVARIANT app_id_value;
    PropVariantInit(&app_id_value);
    result = InitPropVariantFromString(app_user_model_id, &app_id_value);
    if (SUCCEEDED(result)) {
      result = property_store->SetValue(PKEY_AppUserModel_ID, app_id_value);
    }
    if (SUCCEEDED(result)) {
      result = property_store->Commit();
    }
    PropVariantClear(&app_id_value);
    if (FAILED(result)) {
      exit_code = ReportFailure(L"IPropertyStore::SetValue/Commit", result);
      goto finish;
    }

    result = persist_file->Save(shortcut_path, TRUE);
    if (FAILED(result)) {
      exit_code = ReportFailure(L"IPersistFile::Save", result);
      goto finish;
    }
  }

finish:
  if (should_uninitialize) {
    CoUninitialize();
  }
  return exit_code;
}
"#
}

fn render_windows_shortcut_aumid_helper_build_script() -> &'static str {
    r#"[CmdletBinding()]
param(
  [ValidateSet("x64", "arm64")]
  [string] $Architecture = $(if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x64" })
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Resolve-Path (Join-Path $ScriptDir "..\..")
$SourcePath = Join-Path $ScriptDir "shortcut-aumid-helper.cpp"
$OutputDirectory = Join-Path $ProjectDir "target\fission\windows\shortcut-aumid\$Architecture"
$OutputPath = Join-Path $OutputDirectory "fission-shortcut-aumid.exe"
$ObjectPath = Join-Path $OutputDirectory "fission-shortcut-aumid.obj"

if (-not (Test-Path $SourcePath -PathType Leaf)) {
  throw "The shortcut AUMID helper source was not found at $SourcePath."
}

$VsWhere = Get-Command vswhere.exe -ErrorAction SilentlyContinue
if (-not $VsWhere -and ${env:ProgramFiles(x86)}) {
  $BundledVsWhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
  if (Test-Path $BundledVsWhere -PathType Leaf) {
    $VsWhere = Get-Item $BundledVsWhere
  }
}
if (-not $VsWhere) {
  throw "vswhere.exe was not found. Install Visual Studio Build Tools with the target C++ toolchain."
}
$VsWherePath = if ($VsWhere -is [System.IO.FileInfo]) {
  $VsWhere.FullName
} else {
  $VsWhere.Source
}

$RequiredComponent = if ($Architecture -eq "arm64") {
  "Microsoft.VisualStudio.Component.VC.Tools.ARM64"
} else {
  "Microsoft.VisualStudio.Component.VC.Tools.x86.x64"
}
$Installation = & $VsWherePath -latest -products * -requires $RequiredComponent -property installationPath
if (-not $Installation) {
  throw "Visual Studio Build Tools with component $RequiredComponent were not found for $Architecture."
}
$VsDevCmd = Join-Path $Installation "Common7\Tools\VsDevCmd.bat"
if (-not (Test-Path $VsDevCmd -PathType Leaf)) {
  throw "VsDevCmd.bat was not found at $VsDevCmd."
}

$DeveloperCommand = "call `"$VsDevCmd`" -no_logo -arch=$Architecture -host_arch=amd64 && set"
$EnvironmentLines = & $env:ComSpec /d /c $DeveloperCommand
if ($LASTEXITCODE -ne 0) {
  throw "Visual Studio failed to initialize the $Architecture C++ build environment."
}
foreach ($Line in $EnvironmentLines) {
  $Separator = $Line.IndexOf("=")
  if ($Separator -gt 0) {
    $Name = $Line.Substring(0, $Separator)
    $Value = $Line.Substring($Separator + 1)
    [Environment]::SetEnvironmentVariable($Name, $Value, "Process")
  }
}

$Compiler = Get-Command cl.exe -ErrorAction SilentlyContinue
if (-not $Compiler) {
  throw "cl.exe was not available after initializing the $Architecture C++ build environment."
}

New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$CompileArguments = @(
  "/nologo",
  "/EHsc",
  "/MT",
  "/DUNICODE",
  "/D_UNICODE",
  "/Fo$ObjectPath",
  "/Fe$OutputPath",
  $SourcePath,
  "/link",
  "ole32.lib",
  "shell32.lib",
  "propsys.lib"
)
& $Compiler.Source @CompileArguments | Out-Host
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $OutputPath -PathType Leaf)) {
  throw "The $Architecture shortcut AUMID helper build failed."
}

Write-Output $OutputPath
"#
}

fn render_windows_shortcut_aumid_nsis_include() -> &'static str {
    r#"!ifndef FISSION_SHORTCUT_AUMID_NSH
!define FISSION_SHORTCUT_AUMID_NSH

!include "LogicLib.nsh"

; Embed the architecture-matched helper once in an installer section.
!macro FissionEmbedShortcutAppUserModelIdHelper HELPER_PATH
  InitPluginsDir
  File "/oname=$PLUGINSDIR\fission-shortcut-aumid.exe" "${HELPER_PATH}"
!macroend

; Apply the same stable AppUserModelID passed to
; WinitApp::with_windows_app_user_model_id or
; DesktopApp::with_windows_app_user_model_id. Call this after CreateShortCut.
!macro FissionSetShortcutAppUserModelId SHORTCUT_PATH APP_USER_MODEL_ID
  Push $0
  Push $1
  nsExec::ExecToStack /TIMEOUT=30000 '"$PLUGINSDIR\fission-shortcut-aumid.exe" "${SHORTCUT_PATH}" "${APP_USER_MODEL_ID}"'
  Pop $0
  Pop $1
  ${If} $0 != 0
    DetailPrint "Failed to apply AppUserModelID to ${SHORTCUT_PATH}: exit=$0 output=$1"
    MessageBox MB_ICONSTOP|MB_OK "Windows notification identity setup failed. The installation cannot continue."
    Pop $1
    Pop $0
    SetErrors
    Abort
  ${EndIf}
  Pop $1
  Pop $0
!macroend

!endif
"#
}

fn sanitize_file_stem(value: &str) -> String {
    let stem = value
        .chars()
        .map(|ch| match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' => ch,
            _ => '-',
        })
        .collect::<String>()
        .trim_matches(['-', '.', '_'])
        .to_string();
    if stem.is_empty() {
        "app".to_string()
    } else {
        stem
    }
}

fn deterministic_guid(value: &str) -> String {
    fn fnv64(seed: u64, value: &str) -> u64 {
        let mut hash = seed;
        for byte in value.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }
    let left = fnv64(0xcbf29ce484222325, value);
    let right = fnv64(0x84222325cbf29ce4, value);
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&left.to_be_bytes());
    bytes[8..].copy_from_slice(&right.to_be_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
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

fn write_generated_app_agents(project_root: &Path) -> Result<()> {
    let repo_root = find_git_root(project_root).unwrap_or_else(|| project_root.to_path_buf());
    let root_agents = repo_root.join("AGENTS.md");
    if let Some(existing) = read_optional_string(&root_agents)? {
        if is_generated_app_agents(&existing) {
            return write_file_with_policy(
                &root_agents,
                GENERATED_APP_AGENTS_MD,
                WritePolicy::Overwrite,
            );
        }

        let fission_agents = repo_root.join("AGENTS.fission.md");
        let write_policy = read_optional_string(&fission_agents)?
            .filter(|existing| is_generated_app_agents(existing))
            .map(|_| WritePolicy::Overwrite)
            .unwrap_or(WritePolicy::PreserveExisting);

        return write_file_with_policy(&fission_agents, GENERATED_APP_AGENTS_MD, write_policy);
    }

    write_file_with_policy(
        &root_agents,
        GENERATED_APP_AGENTS_MD,
        WritePolicy::Overwrite,
    )
}

fn read_optional_string(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn is_generated_app_agents(contents: &str) -> bool {
    contents.contains(GENERATED_APP_AGENTS_MARKER)
        || contents == GENERATED_APP_AGENTS_MD
        || (contents.contains("# Fission App Guidelines")
            && contents.contains(
                "These instructions apply when building or reviewing a Fission-based app",
            )
            && contents.contains("## Source-Grounded Work")
            && contents.contains("## Validation"))
}

fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = fs::canonicalize(start).ok()?;
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
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
    if targets.contains(&Target::Terminal) {
        features.push("terminal-shell");
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

val releaseKeystore = System.getenv("ANDROID_KEYSTORE")
val releaseStorePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
val releaseKeyAlias = System.getenv("ANDROID_KEYSTORE_ALIAS") ?: "upload"
val releaseKeyPassword = System.getenv("ANDROID_KEY_PASSWORD") ?: releaseStorePassword
val hasReleaseSigning = !releaseKeystore.isNullOrBlank() &&
    !releaseStorePassword.isNullOrBlank() &&
    !releaseKeyAlias.isNullOrBlank() &&
    !releaseKeyPassword.isNullOrBlank()

android {{
    namespace = "{app_id}"
    compileSdk = (System.getenv("ANDROID_TARGET_API_LEVEL") ?: "35").toInt()

    defaultConfig {{
        applicationId = "{app_id}"
        minSdk = (System.getenv("ANDROID_MIN_API_LEVEL") ?: "24").toInt()
        targetSdk = (System.getenv("ANDROID_TARGET_API_LEVEL") ?: "35").toInt()
        versionCode = (System.getenv("ANDROID_VERSION_CODE") ?: "1").toInt()
        versionName = System.getenv("ANDROID_VERSION_NAME") ?: "0.1.0"
    }}

    sourceSets {{
        getByName("main") {{
            manifest.srcFile("../AndroidManifest.xml")
            java.srcDirs("../java")
            res.srcDirs("../res", "src/main/res")
            jniLibs.srcDirs("src/main/jniLibs")
        }}
    }}

    signingConfigs {{
        create("release") {{
            if (hasReleaseSigning) {{
                storeFile = file(releaseKeystore!!)
                storePassword = releaseStorePassword
                keyAlias = releaseKeyAlias
                keyPassword = releaseKeyPassword
            }}
        }}
    }}

    buildTypes {{
        getByName("debug") {{
            isDebuggable = true
        }}
        getByName("release") {{
            isDebuggable = false
            if (hasReleaseSigning) {{
                signingConfig = signingConfigs.getByName("release")
            }}
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

import android.app.NativeActivity;
import android.media.MediaPlayer;
import android.media.PlaybackParams;
import android.os.Bundle;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.VideoView;

import java.util.HashMap;
import java.util.Map;

public final class FissionActivity extends NativeActivity {
    private static volatile FissionActivity INSTANCE;
    private static final Map<Long, FissionVideoSlot> VIDEOS = new HashMap<>();

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        INSTANCE = this;
    }

    @Override
    protected void onDestroy() {
        runOnUiThread(() -> {
            synchronized (VIDEOS) {
                for (FissionVideoSlot slot : VIDEOS.values()) {
                    slot.destroy();
                }
                VIDEOS.clear();
            }
        });
        INSTANCE = null;
        super.onDestroy();
    }

    public static void fissionCreateVideo(long id, String source) {
        runOnUiThreadOrRecordError(id, () -> {
            synchronized (VIDEOS) {
                FissionVideoSlot previous = VIDEOS.remove(id);
                if (previous != null) {
                    previous.destroy();
                }
                FissionVideoSlot slot = new FissionVideoSlot(INSTANCE, source);
                VIDEOS.put(id, slot);
            }
        });
    }

    public static void fissionUpdateVideoSurface(
            long id,
            int left,
            int top,
            int width,
            int height,
            boolean visible
    ) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null) {
                slot.update(left, top, width, height, visible);
            }
        });
    }

    public static void fissionSetVideoVisible(long id, boolean visible) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null && slot.view != null) {
                slot.view.setVisibility(visible ? View.VISIBLE : View.GONE);
            }
        });
    }

    public static void fissionDestroyVideo(long id) {
        runOnUiThreadOrRecordError(id, () -> {
            synchronized (VIDEOS) {
                FissionVideoSlot slot = VIDEOS.remove(id);
                if (slot != null) {
                    slot.destroy();
                }
            }
        });
    }

    public static void fissionPlayVideo(long id) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null) {
                slot.ended = false;
                slot.view.start();
            }
        });
    }

    public static void fissionPauseVideo(long id) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null) {
                slot.view.pause();
            }
        });
    }

    public static void fissionStopVideo(long id) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null) {
                slot.view.pause();
                slot.view.seekTo(0);
                slot.ended = false;
            }
        });
    }

    public static void fissionSeekVideo(long id, long positionMs) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null) {
                slot.view.seekTo((int)Math.max(0L, Math.min(positionMs, Integer.MAX_VALUE)));
            }
        });
    }

    public static void fissionSetVideoRate(long id, float rate) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null) {
                slot.rate = Math.max(0.1f, rate);
                slot.applyPlaybackParams();
            }
        });
    }

    public static void fissionSetVideoVolume(long id, float volume) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null) {
                slot.volume = Math.max(0.0f, Math.min(volume, 1.0f));
                slot.applyVolume();
            }
        });
    }

    public static void fissionSetVideoMuted(long id, boolean muted) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null) {
                slot.muted = muted;
                slot.applyVolume();
            }
        });
    }

    public static long fissionVideoPosition(long id) {
        FissionVideoSlot slot = slot(id);
        return slot == null || slot.view == null ? 0L : Math.max(0, slot.view.getCurrentPosition());
    }

    public static long fissionVideoDuration(long id) {
        FissionVideoSlot slot = slot(id);
        return slot == null || !slot.ready ? -1L : Math.max(0, slot.durationMs);
    }

    public static boolean fissionVideoReady(long id) {
        FissionVideoSlot slot = slot(id);
        return slot != null && slot.ready;
    }

    public static boolean fissionVideoEnded(long id) {
        FissionVideoSlot slot = slot(id);
        return slot != null && slot.ended;
    }

    public static String fissionVideoError(long id) {
        FissionVideoSlot slot = slot(id);
        return slot == null ? null : slot.error;
    }

    private static FissionVideoSlot slot(long id) {
        synchronized (VIDEOS) {
            return VIDEOS.get(id);
        }
    }

    private static void runOnUiThreadOrRecordError(long id, Runnable action) {
        FissionActivity activity = INSTANCE;
        if (activity == null) {
            recordError(id, "Fission Android video host is not attached to FissionActivity");
            return;
        }
        activity.runOnUiThread(() -> {
            try {
                action.run();
            } catch (Throwable error) {
                recordError(id, "Android video host error: " + error);
            }
        });
    }

    private static void recordError(long id, String error) {
        synchronized (VIDEOS) {
            FissionVideoSlot slot = VIDEOS.get(id);
            if (slot == null) {
                slot = new FissionVideoSlot(error);
                VIDEOS.put(id, slot);
            } else {
                slot.error = error;
            }
        }
    }

    private static final class FissionVideoSlot {
        final VideoView view;
        MediaPlayer mediaPlayer;
        volatile boolean ready;
        volatile boolean ended;
        volatile int durationMs = -1;
        volatile String error;
        volatile float rate = 1.0f;
        volatile float volume = 1.0f;
        volatile boolean muted;

        FissionVideoSlot(String error) {
            this.view = null;
            this.error = error;
        }

        FissionVideoSlot(FissionActivity activity, String source) {
            this.view = new VideoView(activity);
            this.view.setVisibility(View.GONE);
            this.view.setZOrderOnTop(true);
            this.view.setOnPreparedListener(player -> {
                mediaPlayer = player;
                ready = true;
                ended = false;
                durationMs = Math.max(0, view.getDuration());
                applyVolume();
                applyPlaybackParams();
            });
            this.view.setOnCompletionListener(player -> ended = true);
            this.view.setOnErrorListener((player, what, extra) -> {
                error = "Android MediaCodec playback error: what=" + what + ", extra=" + extra;
                return true;
            });
            this.view.setVideoPath(source);
            FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(1, 1);
            activity.addContentView(this.view, params);
        }

        void update(int left, int top, int width, int height, boolean visible) {
            if (view == null) {
                return;
            }
            FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(
                    Math.max(1, width),
                    Math.max(1, height)
            );
            view.setLayoutParams(params);
            view.setX(left);
            view.setY(top);
            view.setVisibility(visible ? View.VISIBLE : View.GONE);
        }

        void applyPlaybackParams() {
            if (mediaPlayer == null) {
                return;
            }
            PlaybackParams params = mediaPlayer.getPlaybackParams();
            params.setSpeed(rate);
            mediaPlayer.setPlaybackParams(params);
        }

        void applyVolume() {
            if (mediaPlayer == null) {
                return;
            }
            float effective = muted ? 0.0f : volume;
            mediaPlayer.setVolume(effective, effective);
        }

        void destroy() {
            if (view == null) {
                return;
            }
            view.stopPlayback();
            ViewGroup parent = (ViewGroup)view.getParent();
            if (parent != null) {
                parent.removeView(view);
            }
        }
    }
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
IOS_MARKETING_VERSION="${{IOS_MARKETING_VERSION:-0.1.0}}"
IOS_BUILD_NUMBER="${{IOS_BUILD_NUMBER:-1}}"
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

fn render_ios_ipa_package_script(project: &FissionProject) -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
IOS_TARGET="${{IOS_TARGET:-aarch64-apple-ios}}"
IOS_PROFILE="${{IOS_PROFILE:-release}}"
IOS_SIGNING_IDENTITY="${{IOS_SIGNING_IDENTITY:-}}"
IOS_PROVISIONING_PROFILE="${{IOS_PROVISIONING_PROFILE:-}}"
IOS_REQUIRE_PROVISIONING_PROFILE="${{IOS_REQUIRE_PROVISIONING_PROFILE:-1}}"
IPA_DIR="$SCRIPT_DIR/build/ipa"
PAYLOAD_DIR="$IPA_DIR/Payload"
IPA_PATH="$IPA_DIR/{package_name}.ipa"

if [[ "$IOS_PROFILE" == "release" && -z "$IOS_SIGNING_IDENTITY" ]]; then
  printf 'Release IPA packaging requires IOS_SIGNING_IDENTITY from a secure local or CI secret source.\n' >&2
  exit 1
fi

BUNDLE_DIR=$(IOS_SIM_TARGET="$IOS_TARGET" IOS_SIM_PROFILE="$IOS_PROFILE" "$SCRIPT_DIR/package-sim.sh")

if [[ -n "$IOS_PROVISIONING_PROFILE" ]]; then
  cp "$IOS_PROVISIONING_PROFILE" "$BUNDLE_DIR/embedded.mobileprovision"
elif [[ "$IOS_PROFILE" == "release" && "$IOS_REQUIRE_PROVISIONING_PROFILE" == "1" ]]; then
  printf 'Release IPA packaging requires IOS_PROVISIONING_PROFILE, or set IOS_REQUIRE_PROVISIONING_PROFILE=0 for an explicitly unsigned-profile test package.\n' >&2
  exit 1
fi

if [[ -n "$IOS_SIGNING_IDENTITY" ]]; then
  CODESIGN_ARGS=(--force --sign "$IOS_SIGNING_IDENTITY")
  if [[ -n "${{IOS_ENTITLEMENTS:-}}" ]]; then
    CODESIGN_ARGS+=(--entitlements "$IOS_ENTITLEMENTS")
  elif [[ -f "$SCRIPT_DIR/Entitlements.plist" ]]; then
    CODESIGN_ARGS+=(--entitlements "$SCRIPT_DIR/Entitlements.plist")
  fi
  codesign "${{CODESIGN_ARGS[@]}}" "$BUNDLE_DIR"
  codesign --verify --deep --strict "$BUNDLE_DIR"
fi

rm -rf "$PAYLOAD_DIR"
mkdir -p "$PAYLOAD_DIR"
cp -R "$BUNDLE_DIR" "$PAYLOAD_DIR/"
rm -f "$IPA_PATH"
(cd "$IPA_DIR" && zip -qry "$IPA_PATH" Payload)
printf '%s\n' "$IPA_PATH"
"#,
        package_name = project.app.name,
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
    render_android_gradle_package_script(
        project,
        AndroidGradlePackageKind {
            task_prefix: "assemble",
            output_subdir: "apk",
            extension: "apk",
            label: "APK",
        },
    )
}

fn render_android_aab_package_script(project: &FissionProject) -> String {
    render_android_gradle_package_script(
        project,
        AndroidGradlePackageKind {
            task_prefix: "bundle",
            output_subdir: "bundle",
            extension: "aab",
            label: "AAB",
        },
    )
}

struct AndroidGradlePackageKind {
    task_prefix: &'static str,
    output_subdir: &'static str,
    extension: &'static str,
    label: &'static str,
}

fn render_android_gradle_package_script(
    project: &FissionProject,
    kind: AndroidGradlePackageKind,
) -> String {
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

SIGNING_TEMP_DIR=""
cleanup_android_signing_temp() {{
  if [[ -n "$SIGNING_TEMP_DIR" ]]; then
    rm -rf "$SIGNING_TEMP_DIR"
  fi
}}
trap cleanup_android_signing_temp EXIT

if [[ "$PROFILE" == "release" ]]; then
  if [[ -z "${{ANDROID_KEYSTORE:-}}" && -n "${{ANDROID_KEYSTORE_BASE64:-}}" ]]; then
    SIGNING_TEMP_DIR=$(mktemp -d)
    ANDROID_KEYSTORE="$SIGNING_TEMP_DIR/upload.jks"
    export ANDROID_KEYSTORE
    python3 - "$ANDROID_KEYSTORE" <<'PY'
import base64
import os
import sys

out_path = sys.argv[1]
raw = os.environ["ANDROID_KEYSTORE_BASE64"]
with open(out_path, "wb") as handle:
    handle.write(base64.b64decode(raw))
PY
  fi
  if [[ -z "${{ANDROID_KEYSTORE:-}}" ]]; then
    printf 'Release Android builds require ANDROID_KEYSTORE or ANDROID_KEYSTORE_BASE64 from a secret source.\n' >&2
    exit 1
  fi
  if [[ -z "${{ANDROID_KEYSTORE_PASSWORD:-}}" ]]; then
    printf 'Release Android builds require ANDROID_KEYSTORE_PASSWORD from a secret source.\n' >&2
    exit 1
  fi
  if [[ -z "${{ANDROID_KEYSTORE_ALIAS:-}}" ]]; then
    ANDROID_KEYSTORE_ALIAS=upload
    export ANDROID_KEYSTORE_ALIAS
  fi
  if [[ -z "${{ANDROID_KEY_PASSWORD:-}}" ]]; then
    ANDROID_KEY_PASSWORD="$ANDROID_KEYSTORE_PASSWORD"
    export ANDROID_KEY_PASSWORD
  fi
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

"${{GRADLE_CMD[@]}}" -p "$SCRIPT_DIR" ":app:{task_prefix}$GRADLE_VARIANT"

ARTIFACT="$SCRIPT_DIR/app/build/outputs/{output_subdir}/$GRADLE_OUTPUT_DIR/app-$GRADLE_OUTPUT_DIR.{extension}"
if [[ ! -f "$ARTIFACT" ]]; then
  printf 'Gradle did not produce the expected {label}: %s\n' "$ARTIFACT" >&2
  exit 1
fi
printf '%s\n' "$ARTIFACT"
"#,
        package_name = project.app.name,
        lib_name = lib_name,
        task_prefix = kind.task_prefix,
        output_subdir = kind.output_subdir,
        extension = kind.extension,
        label = kind.label,
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
    <script type="module" src="/bootstrap.mjs"></script>
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
URL="http://${{HOST}}:${{PORT}}/"

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

cargo fission serve-web --project-dir "$PROJECT_DIR" --host "$HOST" --port "$PORT"
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
URL="http://${HOST}:${PORT}/"
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
cd "$SCRIPT_DIR"
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

#[cfg(test)]
mod tests {
    use super::*;

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
            fs::read_to_string(dir.join("platforms/windows/build-shortcut-aumid-helper.ps1"))
                .unwrap();
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
        assert!(text.contains(
            "service_account_json_base64_env = \"PLAY_STORE_SERVICE_ACCOUNT_JSON_BASE64\""
        ));
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
}
