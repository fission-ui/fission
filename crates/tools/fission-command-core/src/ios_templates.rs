use super::*;

pub(super) fn render_ios_host_package(project: &FissionProject) -> String {
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

pub(super) fn render_ios_native_modules_package(project: &FissionProject) -> String {
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

pub(super) fn render_ios_swift_package_dependency(package: &NativeIosSwiftPackageConfig) -> String {
    let version = package
        .from
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("0.0.0");
    format!(".package(url: {:?}, from: {:?})", package.url, version)
}

pub(super) fn render_ios_swift_product_dependency(package: &NativeIosSwiftPackageConfig) -> String {
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

pub(super) fn render_ios_host_native_capabilities_swift() -> &'static str {
    r#"import Foundation
import FissionNativeModules

public enum FissionHostNativeCapabilities {
    public static func present(name: String, requestID: UInt64, payload: Data, completion: @escaping (Result<Data, Error>) -> Void) -> Bool {
        FissionNativeCapabilityRegistry.shared.present(name: name, requestID: requestID, payload: payload, completion: completion)
    }
}
"#
}

pub(super) fn render_ios_native_capabilities_swift() -> &'static str {
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

pub(super) const IOS_NATIVE_MODULES_README: &str = r#"# iOS native modules

This Swift package is the app-owned integration point for native capability modules.

Fission generates `Package.swift` from `fission.toml` `[native]` module declarations. Capability
crates can provide Swift sources or package dependencies here without adding product-specific
logic to Fission itself.
"#;

pub(super) fn render_ios_plist(project: &FissionProject, executable: &str) -> String {
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

pub(super) fn render_ios_info_plist_capability_entries(project: &FissionProject) -> String {
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

pub(super) fn render_ios_package_script(
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

pub(super) fn render_ios_ipa_package_script(project: &FissionProject) -> String {
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

pub(super) fn render_ios_run_script(project: &FissionProject) -> String {
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

pub(super) fn render_ios_test_script() -> String {
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

pub(super) fn render_ios_entitlements_plist(project: &FissionProject) -> String {
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

pub(super) const IOS_NFC_ENTITLEMENTS_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
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

pub(super) const IOS_WIFI_ENTITLEMENTS_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
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
