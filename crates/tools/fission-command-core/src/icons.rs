use crate::{FissionProject, Target};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_APP_ICON: &str = "assets/app-icon.png";

#[derive(Clone, Debug)]
pub struct ResolvedIcon {
    pub path: PathBuf,
    pub configured: bool,
}

#[derive(Debug, Deserialize, Default)]
struct IconManifest {
    package: Option<PackageManifest>,
}

#[derive(Debug, Deserialize, Default)]
struct PackageManifest {
    icon: Option<String>,
    icons: Option<PackageIcons>,
}

#[derive(Debug, Deserialize, Default)]
struct PackageIcons {
    mode: Option<IconMode>,
    source: Option<String>,
    monochrome: Option<String>,
    background_color: Option<String>,
    safe_zone: Option<IconSafeZone>,
    allow_upscale: Option<bool>,
    android: Option<AndroidIcons>,
    ios: Option<AppleIcons>,
    macos: Option<AppleIcons>,
    windows: Option<WindowsIcons>,
    linux: Option<LinuxIcons>,
    web: Option<WebIcons>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum IconMode {
    Generate,
    Provided,
    Mixed,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum IconSafeZone {
    Named(String),
    Fraction(f32),
}

#[derive(Debug, Deserialize, Default)]
struct AndroidIcons {
    source: Option<String>,
    foreground: Option<String>,
    background: Option<String>,
    monochrome: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct AppleIcons {
    source: Option<String>,
    dark: Option<String>,
    tinted: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct WindowsIcons {
    source: Option<String>,
    light: Option<String>,
    dark: Option<String>,
    unplated: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct LinuxIcons {
    source: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct WebIcons {
    source: Option<String>,
    favicon: Option<String>,
    maskable: Option<String>,
}

pub fn resolve_app_icon(root: &Path, target: Target) -> Result<Option<ResolvedIcon>> {
    if target == Target::Server {
        return Ok(None);
    }
    let manifest = read_icon_manifest(root)?;
    if let Some(configured) = configured_icon_path(&manifest, target) {
        let path = root.join(configured);
        validate_icon_file(&path, target)?;
        return Ok(Some(ResolvedIcon {
            path,
            configured: true,
        }));
    }

    for relative in fallback_icon_paths(target) {
        let path = root.join(relative);
        if path.is_file() {
            return Ok(Some(ResolvedIcon {
                path,
                configured: false,
            }));
        }
    }
    Ok(None)
}

pub(crate) fn apply_platform_icon_config(root: &Path, project: &FissionProject) -> Result<()> {
    if project.targets.contains(&Target::Android) {
        apply_android_icon_config(root)?;
    }
    if project.targets.contains(&Target::Ios) {
        apply_ios_icon_config(root)?;
    }
    Ok(())
}

fn read_icon_manifest(root: &Path) -> Result<IconManifest> {
    let path = root.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: IconManifest = toml::from_str(&data)
        .with_context(|| format!("failed to parse icon config in {}", path.display()))?;
    validate_icon_metadata(&manifest)?;
    Ok(manifest)
}

fn configured_icon_path(manifest: &IconManifest, target: Target) -> Option<&str> {
    if target == Target::Server {
        return None;
    }
    let package = manifest.package.as_ref()?;
    let icons = package.icons.as_ref();
    let platform = match target {
        Target::Android => icons
            .and_then(|icons| icons.android.as_ref())
            .and_then(|icons| icons.source.as_deref().or(icons.foreground.as_deref())),
        Target::Ios => icons
            .and_then(|icons| icons.ios.as_ref())
            .and_then(|icons| icons.source.as_deref()),
        Target::Macos => icons
            .and_then(|icons| icons.macos.as_ref())
            .and_then(|icons| icons.source.as_deref()),
        Target::Windows => icons
            .and_then(|icons| icons.windows.as_ref())
            .and_then(|icons| icons.source.as_deref()),
        Target::Linux => icons
            .and_then(|icons| icons.linux.as_ref())
            .and_then(|icons| icons.source.as_deref()),
        Target::Web | Target::Site => icons
            .and_then(|icons| icons.web.as_ref())
            .and_then(|icons| icons.source.as_deref().or(icons.favicon.as_deref())),
        Target::Server | Target::Terminal => None,
    };
    platform
        .or_else(|| icons.and_then(|icons| icons.source.as_deref()))
        .or(package.icon.as_deref())
}

fn fallback_icon_paths(target: Target) -> &'static [&'static str] {
    match target {
        Target::Macos => &[
            "assets/app-icon.icns",
            "assets/AppIcon.icns",
            "assets/app-icon.png",
            "assets/icon.png",
        ],
        Target::Windows => &[
            "assets/app-icon.ico",
            "assets/AppIcon.ico",
            "assets/app-icon.png",
            "assets/icon.png",
        ],
        Target::Linux => &[
            "assets/app-icon.svg",
            "assets/app-icon.png",
            "assets/icon.svg",
            "assets/icon.png",
        ],
        Target::Server | Target::Terminal => &[],
        _ => &[DEFAULT_APP_ICON, "assets/icon.png"],
    }
}

fn validate_icon_file(path: &Path, target: Target) -> Result<()> {
    if !path.is_file() {
        bail!("configured icon does not exist: {}", path.display());
    }
    let extension = normalized_extension(path).with_context(|| {
        format!(
            "configured icon must have a file extension: {}",
            path.display()
        )
    })?;
    let allowed = match target {
        Target::Android => &["png", "jpg", "jpeg", "webp", "xml"][..],
        Target::Ios => &["png", "jpg", "jpeg"][..],
        Target::Macos => &["icns", "png", "jpg", "jpeg"][..],
        Target::Windows => &["ico", "png", "jpg", "jpeg"][..],
        Target::Linux => &["png", "svg"][..],
        Target::Server | Target::Terminal => &[][..],
        Target::Web | Target::Site => &["ico", "png", "jpg", "jpeg", "svg", "webp"][..],
    };
    if !allowed.contains(&extension.as_str()) {
        bail!(
            "configured {} icon must use one of: {}",
            target.as_str(),
            allowed.join(", ")
        );
    }
    Ok(())
}

fn apply_android_icon_config(root: &Path) -> Result<()> {
    let Some(icon) = resolve_app_icon(root, Target::Android)? else {
        return Ok(());
    };
    if !icon.configured {
        return Ok(());
    }
    let extension = normalized_extension(&icon.path)?;
    let (destination, resource_ref) = if extension == "xml" {
        (
            root.join("platforms/android/res/drawable/app_icon.xml"),
            "@drawable/app_icon",
        )
    } else {
        (
            root.join("platforms/android/res/drawable-nodpi/app_icon.")
                .with_extension(&extension),
            "@drawable/app_icon",
        )
    };
    copy_required_asset(&icon.path, &destination)?;
    ensure_android_manifest_icon_ref(root, resource_ref)?;
    ensure_android_package_script_copies_icon_resources(root)
}

fn apply_ios_icon_config(root: &Path) -> Result<()> {
    let Some(icon) = resolve_app_icon(root, Target::Ios)? else {
        return Ok(());
    };
    if !icon.configured {
        return Ok(());
    }
    let extension = normalized_extension(&icon.path)?;
    let destination = root
        .join("platforms/ios")
        .join(format!("AppIcon.{extension}"));
    copy_required_asset(&icon.path, &destination)?;
    ensure_ios_package_script_copies_icon(root)
}

fn ensure_android_manifest_icon_ref(root: &Path, resource_ref: &str) -> Result<()> {
    let path = root.join("platforms/android/AndroidManifest.xml");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if existing.contains(&format!("android:icon=\"{resource_ref}\"")) {
        return Ok(());
    }
    let updated = replace_android_icon_attr(&existing, resource_ref);
    fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))
}

fn replace_android_icon_attr(existing: &str, resource_ref: &str) -> String {
    let Some(start) = existing.find("android:icon=\"") else {
        return existing.replacen(
            "android:label=",
            &format!("android:icon=\"{resource_ref}\"\n        android:label="),
            1,
        );
    };
    let value_start = start + "android:icon=\"".len();
    let Some(relative_end) = existing[value_start..].find('"') else {
        return existing.to_string();
    };
    let end = value_start + relative_end;
    let mut updated = existing.to_string();
    updated.replace_range(value_start..end, resource_ref);
    updated
}

fn ensure_android_package_script_copies_icon_resources(root: &Path) -> Result<()> {
    let path = root.join("platforms/android/package-apk.sh");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if existing.contains("app_icon.png") && existing.contains("res/drawable-nodpi/app_icon.*") {
        return Ok(());
    }
    let marker =
        "cp \"$PROJECT_DIR/assets/app-icon.png\" \"$APK_ROOT/res/drawable-nodpi/app_icon.png\"\n";
    let replacement = r#"shopt -s nullglob
APP_ICONS=("$SCRIPT_DIR"/res/drawable-nodpi/app_icon.* "$SCRIPT_DIR"/res/drawable/app_icon.*)
if (( ${#APP_ICONS[@]} == 0 )); then
  cp "$PROJECT_DIR/assets/app-icon.png" "$APK_ROOT/res/drawable-nodpi/app_icon.png"
fi
shopt -u nullglob
"#;
    let updated = if existing.contains(marker) {
        existing.replacen(marker, replacement, 1)
    } else {
        existing
    };
    fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))
}

fn ensure_ios_package_script_copies_icon(root: &Path) -> Result<()> {
    let path = root.join("platforms/ios/package-sim.sh");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if existing.contains("PLATFORM_APP_ICONS") {
        return Ok(());
    }
    let marker = "cp \"$PROJECT_DIR/assets/app-icon.png\" \"$BUNDLE_DIR/AppIcon.png\"\n";
    let replacement = r#"shopt -s nullglob
PLATFORM_APP_ICONS=("$SCRIPT_DIR"/AppIcon.*)
if (( ${#PLATFORM_APP_ICONS[@]} == 0 )); then
  cp "$PROJECT_DIR/assets/app-icon.png" "$BUNDLE_DIR/AppIcon.png"
else
  app_icon="${PLATFORM_APP_ICONS[0]}"
  cp "$app_icon" "$BUNDLE_DIR/$(basename "$app_icon")"
fi
shopt -u nullglob
"#;
    let updated = if existing.contains(marker) {
        existing.replacen(marker, replacement, 1)
    } else {
        existing
    };
    fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))
}

fn copy_required_asset(source: &Path, destination: &Path) -> Result<()> {
    if !source.exists() {
        bail!("configured icon asset does not exist: {}", source.display());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, destination).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

pub fn copy_icon_for_bundle(
    root: &Path,
    target: Target,
    destination: &Path,
) -> Result<Option<PathBuf>> {
    let Some(icon) = resolve_app_icon(root, target)? else {
        return Ok(None);
    };
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&icon.path, destination).with_context(|| {
        format!(
            "failed to copy {} to {}",
            icon.path.display(),
            destination.display()
        )
    })?;
    Ok(Some(icon.path))
}

pub fn normalized_extension(path: &Path) -> Result<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .with_context(|| format!("path has no extension: {}", path.display()))
}

fn validate_icon_metadata(manifest: &IconManifest) -> Result<()> {
    if let Some(icons) = manifest
        .package
        .as_ref()
        .and_then(|package| package.icons.as_ref())
    {
        let _ = (
            &icons.mode,
            &icons.monochrome,
            &icons.background_color,
            icons.allow_upscale,
        );
        if let Some(safe_zone) = &icons.safe_zone {
            match safe_zone {
                IconSafeZone::Named(value) if matches!(value.as_str(), "platform" | "none") => {}
                IconSafeZone::Named(value) => bail!(
                    "package.icons.safe_zone must be `platform`, `none`, or a numeric fraction; got `{value}`"
                ),
                IconSafeZone::Fraction(value) if (0.0..=1.0).contains(value) => {}
                IconSafeZone::Fraction(value) => bail!(
                    "package.icons.safe_zone numeric fraction must be between 0.0 and 1.0; got {value}"
                ),
            }
        }
        if let Some(android) = &icons.android {
            let _ = (&android.background, &android.monochrome);
        }
        if let Some(ios) = &icons.ios {
            let _ = (&ios.dark, &ios.tinted);
        }
        if let Some(macos) = &icons.macos {
            let _ = (&macos.dark, &macos.tinted);
        }
        if let Some(windows) = &icons.windows {
            let _ = (&windows.light, &windows.dark, &windows.unplated);
        }
        if let Some(web) = &icons.web {
            let _ = &web.maskable;
        }
    }
    Ok(())
}
