use crate::{write_file, FissionProject, Target};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const DEFAULT_SPLASH_BACKGROUND: &str = "#F8FAFC";
const DEFAULT_SPLASH_IMAGE: &str = "assets/app-icon.png";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplashConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resize_mode: Option<SplashResizeMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub android_animated_icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub android_animation_duration_ms: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SplashResizeMode {
    Center,
    Contain,
    Cover,
}

pub(crate) fn apply_platform_splash_config(root: &Path, project: &FissionProject) -> Result<()> {
    validate_splash_config(project)?;
    if project.targets.contains(&Target::Android) {
        apply_android_splash_config(root, project)?;
    }
    if project.targets.contains(&Target::Ios) {
        apply_ios_splash_config(root, project)?;
    }
    Ok(())
}

fn validate_splash_config(project: &FissionProject) -> Result<()> {
    let color = splash_background_color(project);
    parse_hex_color(color)
        .with_context(|| format!("invalid app.splash.background_color `{color}`"))?;
    if let Some(config) = &project.app.splash {
        if let Some(duration) = config.android_animation_duration_ms {
            if duration == 0 {
                bail!("app.splash.android_animation_duration_ms must be greater than zero");
            }
        }
        if let Some(animated_icon) = &config.android_animated_icon {
            if Path::new(animated_icon)
                .extension()
                .and_then(|value| value.to_str())
                != Some("xml")
            {
                bail!(
                    "app.splash.android_animated_icon must point to an Android XML drawable resource"
                );
            }
        }
    }
    Ok(())
}

fn apply_android_splash_config(root: &Path, project: &FissionProject) -> Result<()> {
    let color = android_color_literal(splash_background_color(project))?;
    let static_icon = copy_android_splash_image(root, project)?;
    let animated_icon = copy_android_animated_splash_icon(root, project)?;
    let window_icon = animated_icon.as_deref().unwrap_or(&static_icon);

    write_file(
        &root.join("platforms/android/res/values/colors.xml"),
        &render_android_splash_colors(&color),
    )?;
    write_file(
        &root.join("platforms/android/res/values/styles.xml"),
        &render_android_splash_styles(window_icon, project),
    )?;
    write_file(
        &root.join("platforms/android/res/drawable/fission_splash_background.xml"),
        &render_android_splash_background(&static_icon),
    )?;
    ensure_android_manifest_uses_splash_theme(root)?;
    ensure_android_package_script_copies_resources(root)
}

fn apply_ios_splash_config(root: &Path, project: &FissionProject) -> Result<()> {
    let color = parse_hex_color(splash_background_color(project))?;
    let image_name = copy_ios_splash_image(root, project)?;
    write_file(
        &root.join("platforms/ios/LaunchScreen.storyboard"),
        &render_ios_launch_storyboard(&color, image_name.as_deref(), splash_resize_mode(project)),
    )?;
    ensure_ios_plist_launch_storyboard(root)?;
    ensure_ios_package_script_copies_launch_screen(root)
}

fn splash_background_color(project: &FissionProject) -> &str {
    project
        .app
        .splash
        .as_ref()
        .and_then(|config| config.background_color.as_deref())
        .unwrap_or(DEFAULT_SPLASH_BACKGROUND)
}

fn splash_resize_mode(project: &FissionProject) -> SplashResizeMode {
    project
        .app
        .splash
        .as_ref()
        .and_then(|config| config.resize_mode)
        .unwrap_or(SplashResizeMode::Contain)
}

#[derive(Clone, Copy)]
struct RgbaColor {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

fn parse_hex_color(value: &str) -> Result<RgbaColor> {
    let hex = value
        .strip_prefix('#')
        .with_context(|| "expected #RRGGBB or #RRGGBBAA")?;
    if hex.len() != 6 && hex.len() != 8 {
        bail!("expected #RRGGBB or #RRGGBBAA");
    }
    let parse_byte = |range: std::ops::Range<usize>| -> Result<u8> {
        u8::from_str_radix(&hex[range], 16).with_context(|| "expected hexadecimal color digits")
    };
    Ok(RgbaColor {
        red: parse_byte(0..2)?,
        green: parse_byte(2..4)?,
        blue: parse_byte(4..6)?,
        alpha: if hex.len() == 8 {
            parse_byte(6..8)?
        } else {
            255
        },
    })
}

fn android_color_literal(value: &str) -> Result<String> {
    let color = parse_hex_color(value)?;
    if color.alpha == 255 {
        Ok(format!(
            "#{:02X}{:02X}{:02X}",
            color.red, color.green, color.blue
        ))
    } else {
        Ok(format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            color.alpha, color.red, color.green, color.blue
        ))
    }
}

fn copy_android_splash_image(root: &Path, project: &FissionProject) -> Result<String> {
    let resource_name = "fission_splash_image";
    if let Some(source) = configured_splash_image(project) {
        let source = root.join(source);
        let extension = android_bitmap_extension(&source)?;
        let destination = root
            .join("platforms/android/res/drawable-nodpi")
            .join(format!("{resource_name}.{extension}"));
        copy_required_asset(&source, &destination)?;
    } else {
        let source = root.join(DEFAULT_SPLASH_IMAGE);
        android_bitmap_extension(&source)?;
        if !source.exists() {
            bail!("default splash asset does not exist: {}", source.display());
        }
    }
    Ok(format!("@drawable/{resource_name}"))
}

fn copy_android_animated_splash_icon(
    root: &Path,
    project: &FissionProject,
) -> Result<Option<String>> {
    let Some(source) = project
        .app
        .splash
        .as_ref()
        .and_then(|config| config.android_animated_icon.as_deref())
    else {
        return Ok(None);
    };
    let destination = root.join("platforms/android/res/drawable/fission_splash_animated_icon.xml");
    copy_required_asset(&root.join(source), &destination)?;
    Ok(Some("@drawable/fission_splash_animated_icon".to_string()))
}

fn copy_ios_splash_image(root: &Path, project: &FissionProject) -> Result<Option<String>> {
    if let Some(source) = configured_splash_image(project) {
        let source = root.join(source);
        let extension = ios_image_extension(&source)?;
        let destination = root
            .join("platforms/ios")
            .join(format!("SplashImage.{extension}"));
        copy_required_asset(&source, &destination)?;
    } else {
        let source = root.join(DEFAULT_SPLASH_IMAGE);
        ios_image_extension(&source)?;
        if !source.exists() {
            bail!("default splash asset does not exist: {}", source.display());
        }
    }
    Ok(Some("SplashImage".to_string()))
}

fn configured_splash_image(project: &FissionProject) -> Option<&str> {
    project
        .app
        .splash
        .as_ref()
        .and_then(|config| config.image.as_deref())
}

fn copy_required_asset(source: &Path, destination: &Path) -> Result<()> {
    if !source.exists() {
        bail!(
            "configured splash asset does not exist: {}",
            source.display()
        );
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

fn android_bitmap_extension(path: &Path) -> Result<String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .with_context(|| {
            format!(
                "splash image must have a file extension: {}",
                path.display()
            )
        })?;
    match extension.as_str() {
        "png" | "jpg" | "jpeg" | "webp" => Ok(extension),
        _ => bail!("Android splash image must be png, jpg, jpeg, or webp"),
    }
}

fn ios_image_extension(path: &Path) -> Result<String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .with_context(|| {
            format!(
                "splash image must have a file extension: {}",
                path.display()
            )
        })?;
    match extension.as_str() {
        "png" | "jpg" | "jpeg" => Ok(extension),
        _ => bail!("iOS splash image must be png, jpg, or jpeg"),
    }
}

fn render_android_splash_colors(color: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <color name="fission_splash_background">{color}</color>
</resources>
"#
    )
}

fn render_android_splash_styles(window_icon: &str, project: &FissionProject) -> String {
    let duration = project
        .app
        .splash
        .as_ref()
        .and_then(|config| config.android_animation_duration_ms)
        .unwrap_or(800);
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <style name="FissionLaunchTheme" parent="@android:style/Theme.Material.NoActionBar">
        <item name="android:windowNoTitle">true</item>
        <item name="android:windowActionBar">false</item>
        <item name="android:windowDisablePreview">false</item>
        <item name="android:windowBackground">@drawable/fission_splash_background</item>
        <item name="android:windowSplashScreenBackground">@color/fission_splash_background</item>
        <item name="android:windowSplashScreenAnimatedIcon">{window_icon}</item>
        <item name="android:windowSplashScreenAnimationDuration">{duration}</item>
    </style>
</resources>
"#
    )
}

fn render_android_splash_background(static_icon: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<layer-list xmlns:android="http://schemas.android.com/apk/res/android">
    <item android:drawable="@color/fission_splash_background" />
    <item android:gravity="center">
        <bitmap
            android:gravity="center"
            android:src="{static_icon}" />
    </item>
</layer-list>
"#
    )
}

fn render_ios_launch_storyboard(
    color: &RgbaColor,
    image_name: Option<&str>,
    resize_mode: SplashResizeMode,
) -> String {
    let content_mode = match resize_mode {
        SplashResizeMode::Center => "center",
        SplashResizeMode::Contain => "scaleAspectFit",
        SplashResizeMode::Cover => "scaleAspectFill",
    };
    let image_view = image_name.map(|name| {
        format!(
            r#"
                        <imageView clipsSubviews="YES" userInteractionEnabled="NO" contentMode="{content_mode}" image="{image}" translatesAutoresizingMaskIntoConstraints="NO" id="splash-image">
                            <rect key="frame" x="79" y="320" width="256" height="256"/>
                        </imageView>"#,
            image = xml_escape_attr(name)
        )
    }).unwrap_or_default();
    let image_constraints = if image_name.is_some() {
        r#"
                        <constraint firstItem="splash-image" firstAttribute="centerX" secondItem="splash-root" secondAttribute="centerX" id="splash-center-x"/>
                        <constraint firstItem="splash-image" firstAttribute="centerY" secondItem="splash-root" secondAttribute="centerY" id="splash-center-y"/>
                        <constraint firstItem="splash-image" firstAttribute="width" constant="256" id="splash-width"/>
                        <constraint firstItem="splash-image" firstAttribute="height" constant="256" id="splash-height"/>"#
    } else {
        ""
    };
    let resources = image_name
        .map(|name| {
            format!(
                r#"
    <resources>
        <image name="{image}" width="256" height="256"/>
    </resources>"#,
                image = xml_escape_attr(name)
            )
        })
        .unwrap_or_default();

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<document type="com.apple.InterfaceBuilder3.CocoaTouch.Storyboard.XIB" version="3.0" toolsVersion="23504" targetRuntime="iOS.CocoaTouch" propertyAccessControl="none" useAutolayout="YES" launchScreen="YES" useTraitCollections="YES" colorMatched="YES" initialViewController="splash-controller">
    <dependencies>
        <deployment identifier="iOS"/>
        <plugIn identifier="com.apple.InterfaceBuilder.IBCocoaTouchPlugin" version="23506"/>
        <capability name="documents saved in the Xcode 8 format" minToolsVersion="8.0"/>
    </dependencies>
    <scenes>
        <scene sceneID="splash-scene">
            <objects>
                <viewController id="splash-controller" sceneMemberID="viewController">
                    <view key="view" contentMode="scaleToFill" id="splash-root">
                        <rect key="frame" x="0.0" y="0.0" width="414" height="896"/>
                        <autoresizingMask key="autoresizingMask" widthSizable="YES" heightSizable="YES"/>
                        <subviews>{image_view}
                        </subviews>
                        <color key="backgroundColor" red="{red:.6}" green="{green:.6}" blue="{blue:.6}" alpha="{alpha:.6}" colorSpace="custom" customColorSpace="sRGB"/>
                        <constraints>{image_constraints}
                        </constraints>
                    </view>
                </viewController>
                <placeholder placeholderIdentifier="IBFirstResponder" id="splash-first-responder" userLabel="First Responder" sceneMemberID="firstResponder"/>
            </objects>
        </scene>
    </scenes>{resources}
</document>
"#,
        red = f32::from(color.red) / 255.0,
        green = f32::from(color.green) / 255.0,
        blue = f32::from(color.blue) / 255.0,
        alpha = f32::from(color.alpha) / 255.0,
    )
}

fn xml_escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn ensure_android_manifest_uses_splash_theme(root: &Path) -> Result<()> {
    let path = root.join("platforms/android/AndroidManifest.xml");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if existing.contains("android:theme=\"@style/FissionLaunchTheme\"") {
        return Ok(());
    }
    let updated = if existing.contains("android:launchMode=\"singleTask\">") {
        existing.replacen(
            "android:launchMode=\"singleTask\">",
            "android:launchMode=\"singleTask\"\n            android:theme=\"@style/FissionLaunchTheme\">",
            1,
        )
    } else {
        existing.replacen(
            "android:exported=\"true\"",
            "android:exported=\"true\"\n            android:theme=\"@style/FissionLaunchTheme\"",
            1,
        )
    };
    fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))
}

fn ensure_android_package_script_copies_resources(root: &Path) -> Result<()> {
    let path = root.join("platforms/android/package-apk.sh");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if existing.contains("fission_splash_image.png")
        && existing.contains("cp -R \"$SCRIPT_DIR/res/.\" \"$APK_ROOT/res/\"")
    {
        return Ok(());
    }
    let (marker, fallback_icon) = if existing
        .contains("cp \"$ICON_SOURCE\" \"$APK_ROOT/res/drawable-nodpi/app_icon.png\"\n")
    {
        (
            "cp \"$ICON_SOURCE\" \"$APK_ROOT/res/drawable-nodpi/app_icon.png\"\n",
            "$ICON_SOURCE",
        )
    } else {
        (
            "cp \"$PROJECT_DIR/assets/app-icon.png\" \"$APK_ROOT/res/drawable-nodpi/app_icon.png\"\n",
            "$PROJECT_DIR/assets/app-icon.png",
        )
    };
    let insertion = r#"shopt -s nullglob
SPLASH_IMAGES=("$SCRIPT_DIR"/res/drawable-nodpi/fission_splash_image.*)
if (( ${#SPLASH_IMAGES[@]} == 0 )); then
  cp "__FALLBACK_ICON__" "$APK_ROOT/res/drawable-nodpi/fission_splash_image.png"
fi
shopt -u nullglob
if [[ -d "$SCRIPT_DIR/res" ]]; then
  mkdir -p "$APK_ROOT/res"
  cp -R "$SCRIPT_DIR/res/." "$APK_ROOT/res/"
fi
"#
    .replace("__FALLBACK_ICON__", fallback_icon);
    let old_start = "if [[ -d \"$SCRIPT_DIR/res\" ]]; then\n";
    let updated = if let Some(start) = existing.find(old_start) {
        if let Some(relative_end) = existing[start..].find("fi\n") {
            let end = start + relative_end + "fi\n".len();
            let mut updated = existing.clone();
            updated.replace_range(start..end, &insertion);
            updated
        } else {
            existing.replacen(marker, &(marker.to_string() + &insertion), 1)
        }
    } else {
        existing.replacen(marker, &(marker.to_string() + &insertion), 1)
    };
    fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))
}

fn ensure_ios_plist_launch_storyboard(root: &Path) -> Result<()> {
    let path = root.join("platforms/ios/Info.plist");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if existing.contains("UILaunchStoryboardName") {
        return Ok(());
    }
    let entry = "  <key>UILaunchStoryboardName</key>\n  <string>LaunchScreen</string>\n";
    let updated = existing.replacen(
        "  <key>MinimumOSVersion</key>",
        &format!("{entry}  <key>MinimumOSVersion</key>"),
        1,
    );
    fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))
}

fn ensure_ios_package_script_copies_launch_screen(root: &Path) -> Result<()> {
    let path = root.join("platforms/ios/package-sim.sh");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if existing.contains("ibtool")
        && existing.contains("LaunchScreen.storyboardc")
        && existing.contains("$BUNDLE_DIR/SplashImage.png")
    {
        return Ok(());
    }
    let marker = "cp \"$PROJECT_DIR/assets/app-icon.png\" \"$BUNDLE_DIR/AppIcon.png\"\n";
    let insertion = r#"shopt -s nullglob
SPLASH_IMAGES=("$SCRIPT_DIR"/SplashImage.*)
if (( ${#SPLASH_IMAGES[@]} == 0 )); then
  cp "$PROJECT_DIR/assets/app-icon.png" "$BUNDLE_DIR/SplashImage.png"
else
  for splash_image in "${SPLASH_IMAGES[@]}"; do
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
"#;
    let old_start = "shopt -s nullglob\n";
    let old_end = "    \"$SCRIPT_DIR/LaunchScreen.storyboard\"\nfi\n";
    let updated = if let Some(start) = existing.find(old_start) {
        if existing[start..].contains("LaunchScreen.storyboard") {
            if let Some(relative_end) = existing[start..].find(old_end) {
                let end = start + relative_end + old_end.len();
                let mut updated = existing.clone();
                updated.replace_range(start..end, insertion);
                updated
            } else {
                existing.replacen(marker, &(marker.to_string() + insertion), 1)
            }
        } else {
            let mut updated = existing.clone();
            updated.insert_str(start, insertion);
            updated
        }
    } else {
        existing.replacen(marker, &(marker.to_string() + insertion), 1)
    };
    fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))
}
