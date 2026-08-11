use super::*;

pub(super) fn render_android_capabilities_java() -> &'static str {
    include_str!("../assets/android/rs/fission/runtime/FissionAndroidCapabilities.java")
}

pub(super) fn render_android_package_script(project: &FissionProject) -> String {
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

pub(super) fn render_android_aab_package_script(project: &FissionProject) -> String {
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

pub(super) struct AndroidGradlePackageKind {
    task_prefix: &'static str,
    output_subdir: &'static str,
    extension: &'static str,
    label: &'static str,
}

pub(super) fn render_android_gradle_package_script(
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

pub(super) fn render_android_run_script(project: &FissionProject) -> String {
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

pub(super) fn render_android_test_script() -> String {
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
