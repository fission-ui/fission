#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
TARGET="${ANDROID_TARGET_TRIPLE:-aarch64-linux-android}"
PACKAGE_NAME="web-smoke"
LIB_NAME="web_smoke"
PROFILE="${ANDROID_PROFILE:-debug}"
ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
ANDROID_MIN_API_LEVEL="${ANDROID_MIN_API_LEVEL:-${ANDROID_API_LEVEL:-24}}"

find_android_ndk() {
  if [[ -n "${ANDROID_NDK:-}" ]]; then
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
}

detect_android_toolchain() {
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
}

detect_latest_android_api() {
  find "$ANDROID_HOME/platforms" -maxdepth 1 -type d -name 'android-*' 2>/dev/null \
    | sed 's#.*android-##' \
    | sort -n \
    | tail -1
}

detect_build_tools_dir() {
  if [[ -n "${ANDROID_BUILD_TOOLS:-}" ]]; then
    if [[ -d "$ANDROID_BUILD_TOOLS" ]]; then
      printf '%s\n' "$ANDROID_BUILD_TOOLS"
      return
    fi
    if [[ -d "$ANDROID_HOME/build-tools/$ANDROID_BUILD_TOOLS" ]]; then
      printf '%s\n' "$ANDROID_HOME/build-tools/$ANDROID_BUILD_TOOLS"
      return
    fi
  fi
  find "$ANDROID_HOME/build-tools" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | sort -V | tail -1
}

ANDROID_TARGET_API_LEVEL="${ANDROID_TARGET_API_LEVEL:-$(detect_latest_android_api)}"
if [[ -z "$ANDROID_TARGET_API_LEVEL" ]]; then
  printf 'No Android platform found under %s/platforms. Install one with sdkmanager "platforms;android-35" or newer.\n' "$ANDROID_HOME" >&2
  exit 1
fi

ANDROID_NDK=$(find_android_ndk)
ANDROID_TOOLCHAIN="${ANDROID_TOOLCHAIN:-$(detect_android_toolchain)}"
CC_aarch64_linux_android="${CC_aarch64_linux_android:-$ANDROID_TOOLCHAIN/aarch64-linux-android${ANDROID_MIN_API_LEVEL}-clang}"
AR_aarch64_linux_android="${AR_aarch64_linux_android:-$ANDROID_TOOLCHAIN/llvm-ar}"
CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="${CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER:-$CC_aarch64_linux_android}"
CARGO_TARGET_AARCH64_LINUX_ANDROID_AR="${CARGO_TARGET_AARCH64_LINUX_ANDROID_AR:-$AR_aarch64_linux_android}"
export ANDROID_HOME ANDROID_NDK ANDROID_MIN_API_LEVEL ANDROID_TARGET_API_LEVEL ANDROID_TOOLCHAIN CC_aarch64_linux_android AR_aarch64_linux_android
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER CARGO_TARGET_AARCH64_LINUX_ANDROID_AR

BUILD_TOOLS=$(detect_build_tools_dir)
if [[ -z "$BUILD_TOOLS" || ! -d "$BUILD_TOOLS" ]]; then
  printf 'Android build-tools not found. Install them with sdkmanager "build-tools;35.0.0" or set ANDROID_BUILD_TOOLS.\n' >&2
  exit 1
fi
ANDROID_JAR="$ANDROID_HOME/platforms/android-$ANDROID_TARGET_API_LEVEL/android.jar"
if [[ ! -f "$ANDROID_JAR" ]]; then
  printf 'Android platform android-%s not found. Install it with sdkmanager "platforms;android-%s" or set ANDROID_TARGET_API_LEVEL.\n' "$ANDROID_TARGET_API_LEVEL" "$ANDROID_TARGET_API_LEVEL" >&2
  exit 1
fi
AAPT="$BUILD_TOOLS/aapt"
ZIPALIGN="$BUILD_TOOLS/zipalign"
APKSIGNER="$BUILD_TOOLS/apksigner"
for tool in "$AAPT" "$ZIPALIGN" "$APKSIGNER"; do
  if [[ ! -x "$tool" ]]; then
    printf 'Required Android build tool is missing or not executable: %s\n' "$tool" >&2
    exit 1
  fi
done

BUILD_ARGS=(build --manifest-path "$PROJECT_DIR/Cargo.toml" --lib --target "$TARGET" --package "$PACKAGE_NAME")
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

SO_PATH="$TARGET_DIR/$TARGET/$ARTIFACT_DIR/lib$LIB_NAME.so"
BUILD_DIR="$SCRIPT_DIR/build/$PROFILE"
APK_ROOT="$BUILD_DIR/apk-root"
UNALIGNED_APK="$BUILD_DIR/$PACKAGE_NAME-unaligned.apk"
ALIGNED_APK="$BUILD_DIR/$PACKAGE_NAME-aligned.apk"
SIGNED_APK="$BUILD_DIR/$PACKAGE_NAME.apk"
KEYSTORE="${ANDROID_DEBUG_KEYSTORE:-$HOME/.android/debug.keystore}"

rm -rf "$APK_ROOT"
mkdir -p "$APK_ROOT/lib/arm64-v8a" "$APK_ROOT/res/drawable-nodpi" "$BUILD_DIR"
cp "$SO_PATH" "$APK_ROOT/lib/arm64-v8a/lib$LIB_NAME.so"
cp "$PROJECT_DIR/assets/app-icon.png" "$APK_ROOT/res/drawable-nodpi/app_icon.png"
shopt -s nullglob
SPLASH_IMAGES=("$SCRIPT_DIR"/res/drawable-nodpi/fission_splash_image.*)
if (( ${#SPLASH_IMAGES[@]} == 0 )); then
  cp "$PROJECT_DIR/assets/app-icon.png" "$APK_ROOT/res/drawable-nodpi/fission_splash_image.png"
fi
shopt -u nullglob
if [[ -d "$SCRIPT_DIR/res" ]]; then
  mkdir -p "$APK_ROOT/res"
  cp -R "$SCRIPT_DIR/res/." "$APK_ROOT/res/"
fi

BUILD_MANIFEST="$BUILD_DIR/AndroidManifest.xml"
python3 - <<'PY' "$SCRIPT_DIR/AndroidManifest.xml" "$BUILD_MANIFEST" "$ANDROID_MIN_API_LEVEL" "$ANDROID_TARGET_API_LEVEL"
import pathlib
import re
import sys

source, dest, min_api, target_api = sys.argv[1:]
manifest = open(source, encoding="utf-8").read()
manifest = re.sub(r'android:minSdkVersion="\d+"', f'android:minSdkVersion="{min_api}"', manifest)
manifest = re.sub(r'android:targetSdkVersion="\d+"', f'android:targetSdkVersion="{target_api}"', manifest)
has_code = "true" if pathlib.Path(dest).with_name("apk-root").joinpath("classes.dex").exists() else "false"
manifest = re.sub(r'android:hasCode="(?:true|false)"', f'android:hasCode="{has_code}"', manifest)
open(dest, "w", encoding="utf-8").write(manifest)
PY

"$AAPT" package -f -F "$UNALIGNED_APK" -M "$BUILD_MANIFEST" -S "$APK_ROOT/res" -I "$ANDROID_JAR"
(cd "$APK_ROOT" && zip -qr "$UNALIGNED_APK" lib)
"$ZIPALIGN" -f 4 "$UNALIGNED_APK" "$ALIGNED_APK"

if [[ ! -f "$KEYSTORE" ]]; then
  mkdir -p "$(dirname "$KEYSTORE")"
  keytool -genkeypair -v \
    -keystore "$KEYSTORE" \
    -storepass android \
    -alias androiddebugkey \
    -keypass android \
    -dname "CN=Android Debug,O=Android,C=US" \
    -keyalg RSA \
    -keysize 2048 \
    -validity 10000 >/dev/null 2>&1
fi

"$APKSIGNER" sign \
  --ks "$KEYSTORE" \
  --ks-pass pass:android \
  --key-pass pass:android \
  --out "$SIGNED_APK" \
  "$ALIGNED_APK"

printf '%s\n' "$SIGNED_APK"
