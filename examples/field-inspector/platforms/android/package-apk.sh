#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
TARGET="${ANDROID_TARGET_TRIPLE:-aarch64-linux-android}"
PACKAGE_NAME="field-inspector"
LIB_NAME="field_inspector"
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

if [[ -n "${FISSION_GRADLE:-}" ]]; then
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
cleanup_android_signing_temp() {
  if [[ -n "$SIGNING_TEMP_DIR" ]]; then
    rm -rf "$SIGNING_TEMP_DIR"
  fi
}
trap cleanup_android_signing_temp EXIT

if [[ "$PROFILE" == "release" ]]; then
  if [[ -z "${ANDROID_KEYSTORE:-}" && -n "${ANDROID_KEYSTORE_BASE64:-}" ]]; then
    SIGNING_TEMP_DIR=$(mktemp -d)
    ANDROID_KEYSTORE="$SIGNING_TEMP_DIR/upload.jks"
    export ANDROID_KEYSTORE
    python3 - "$ANDROID_KEYSTORE" <<'PYBASE64'
import base64
import os
import sys

out_path = sys.argv[1]
raw = os.environ["ANDROID_KEYSTORE_BASE64"]
with open(out_path, "wb") as handle:
    handle.write(base64.b64decode(raw))
PYBASE64
  fi
  if [[ -z "${ANDROID_KEYSTORE:-}" ]]; then
    printf 'Release Android builds require ANDROID_KEYSTORE or ANDROID_KEYSTORE_BASE64 from a secret source.\n' >&2
    exit 1
  fi
  if [[ -z "${ANDROID_KEYSTORE_PASSWORD:-}" ]]; then
    printf 'Release Android builds require ANDROID_KEYSTORE_PASSWORD from a secret source.\n' >&2
    exit 1
  fi
  if [[ -z "${ANDROID_KEYSTORE_ALIAS:-}" ]]; then
    ANDROID_KEYSTORE_ALIAS=upload
    export ANDROID_KEYSTORE_ALIAS
  fi
  if [[ -z "${ANDROID_KEY_PASSWORD:-}" ]]; then
    ANDROID_KEY_PASSWORD="$ANDROID_KEYSTORE_PASSWORD"
    export ANDROID_KEY_PASSWORD
  fi
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
JNI_DIR="$SCRIPT_DIR/app/src/main/jniLibs/arm64-v8a"
GENERATED_RES_DIR="$SCRIPT_DIR/app/src/main/res/drawable-nodpi"
mkdir -p "$JNI_DIR" "$GENERATED_RES_DIR"
cp "$SO_PATH" "$JNI_DIR/lib$LIB_NAME.so"
shopt -s nullglob
APP_ICONS=("$SCRIPT_DIR"/res/drawable-nodpi/app_icon.* "$SCRIPT_DIR"/res/drawable/app_icon.*)
if (( ${#APP_ICONS[@]} == 0 )); then
  cp "$PROJECT_DIR/assets/app-icon.png" "$GENERATED_RES_DIR/app_icon.png"
fi
shopt -u nullglob
shopt -s nullglob
SPLASH_IMAGES=("$SCRIPT_DIR"/res/drawable-nodpi/fission_splash_image.*)
if (( ${#SPLASH_IMAGES[@]} == 0 )); then
  cp "$PROJECT_DIR/assets/app-icon.png" "$GENERATED_RES_DIR/fission_splash_image.png"
fi
shopt -u nullglob

"${GRADLE_CMD[@]}" -p "$SCRIPT_DIR" ":app:assemble$GRADLE_VARIANT"

APK="$SCRIPT_DIR/app/build/outputs/apk/$GRADLE_OUTPUT_DIR/app-$GRADLE_OUTPUT_DIR.apk"
if [[ ! -f "$APK" ]]; then
  printf 'Gradle did not produce the expected APK: %s\n' "$APK" >&2
  exit 1
fi
printf '%s\n' "$APK"
