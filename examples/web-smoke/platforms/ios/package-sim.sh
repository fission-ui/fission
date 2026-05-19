#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
TARGET="${IOS_SIM_TARGET:-aarch64-apple-ios-sim}"
PROFILE="${IOS_SIM_PROFILE:-debug}"
PACKAGE_NAME="web-smoke"
BUNDLE_ID="${IOS_BUNDLE_ID:-com.example.web_smoke}"
DISPLAY_NAME="${IOS_DISPLAY_NAME:-WebSmoke}"
EXECUTABLE_NAME="${IOS_EXECUTABLE_NAME:-web_smoke}"
BUNDLE_NAME="${IOS_BUNDLE_NAME:-$DISPLAY_NAME.app}"
BUILD_DIR="$SCRIPT_DIR/build/$PROFILE"
BUNDLE_DIR="$BUILD_DIR/$BUNDLE_NAME"

BUILD_ARGS=(build --manifest-path "$PROJECT_DIR/Cargo.toml" --target "$TARGET" --package "$PACKAGE_NAME")
ARTIFACT_DIR=debug
if [[ "$PROFILE" == "release" ]]; then
  BUILD_ARGS+=(--release)
  ARTIFACT_DIR=release
fi

cargo "${BUILD_ARGS[@]}"
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
python3 - <<'PY' "$SCRIPT_DIR/Info.plist" "$BUNDLE_DIR/Info.plist" "$BUNDLE_ID" "$DISPLAY_NAME" "$EXECUTABLE_NAME"
import plistlib
import sys

source, dest, bundle_id, display_name, executable_name = sys.argv[1:]
with open(source, "rb") as handle:
    plist = plistlib.load(handle)
plist["CFBundleIdentifier"] = bundle_id
plist["CFBundleDisplayName"] = display_name
plist["CFBundleName"] = display_name
plist["CFBundleExecutable"] = executable_name
with open(dest, "wb") as handle:
    plistlib.dump(plist, handle, sort_keys=False)
PY
cp "$PROJECT_DIR/assets/app-icon.png" "$BUNDLE_DIR/AppIcon.png"
printf 'APPL????' > "$BUNDLE_DIR/PkgInfo"
printf '%s\n' "$BUNDLE_DIR"
