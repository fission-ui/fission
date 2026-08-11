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

mod platform_sync;
pub use platform_sync::stage_project_assets;
use platform_sync::{
    apply_mobile_run_script_hardening, apply_native_module_config,
    apply_platform_capability_config, sync_ios_native_module_sources, target_scaffold_dir_exists,
    IOS_INFO_PLIST_PLUTIL_PATCH,
};

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

mod scaffold;
pub(crate) use scaffold::write_file;
use scaffold::*;

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

mod android_gradle;
use android_gradle::*;

mod ios_templates;
use ios_templates::*;

mod android_manifest;
use android_manifest::*;

mod android_scripts;
use android_scripts::*;

mod templates;
use templates::*;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
