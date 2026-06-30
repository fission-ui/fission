#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
TARGET="${IOS_SIM_TARGET:-aarch64-apple-ios-sim}"
PROFILE="${IOS_SIM_PROFILE:-debug}"
PACKAGE_NAME="field-inspector"
BUNDLE_ID="${IOS_BUNDLE_ID:-com.fission.examples.fieldinspector}"
DISPLAY_NAME="${IOS_DISPLAY_NAME:-FieldInspector}"
EXECUTABLE_NAME="${IOS_EXECUTABLE_NAME:-field_inspector}"
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
cp "$SCRIPT_DIR/Info.plist" "$BUNDLE_DIR/Info.plist"
PLUTIL=$(xcrun --find plutil 2>/dev/null || command -v plutil || true)
if [[ -z "$PLUTIL" ]]; then
  printf 'plutil not found. Install Xcode command line tools to package the iOS simulator app.\n' >&2
  exit 1
fi
"$PLUTIL" -replace CFBundleIdentifier -string "$BUNDLE_ID" "$BUNDLE_DIR/Info.plist"
"$PLUTIL" -replace CFBundleDisplayName -string "$DISPLAY_NAME" "$BUNDLE_DIR/Info.plist"
"$PLUTIL" -replace CFBundleName -string "$DISPLAY_NAME" "$BUNDLE_DIR/Info.plist"
"$PLUTIL" -replace CFBundleExecutable -string "$EXECUTABLE_NAME" "$BUNDLE_DIR/Info.plist"
cp "$PROJECT_DIR/assets/app-icon.png" "$BUNDLE_DIR/AppIcon.png"
shopt -s nullglob
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
printf 'APPL????' > "$BUNDLE_DIR/PkgInfo"
printf '%s\n' "$BUNDLE_DIR"
