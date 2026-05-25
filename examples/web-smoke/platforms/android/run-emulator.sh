#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
ADB="$ANDROID_HOME/platform-tools/adb"
EMULATOR_BIN="$ANDROID_HOME/emulator/emulator"
AVDMANAGER="${ANDROID_AVDMANAGER:-$ANDROID_HOME/cmdline-tools/latest/bin/avdmanager}"

detect_latest_emulator_api() {
  find "$ANDROID_HOME/system-images" -path '*/google_apis/arm64-v8a' -type d 2>/dev/null \
    | sed -n 's#.*system-images/android-\([0-9][0-9]*\)/google_apis/arm64-v8a#\1#p' \
    | sort -n \
    | tail -1
}

android_system_image_path() {
  local image="$1"
  image="${image#system-images;}"
  printf '%s/system-images/%s\n' "$ANDROID_HOME" "${image//;/\/}"
}

ANDROID_EMULATOR_API_LEVEL="${ANDROID_EMULATOR_API_LEVEL:-$(detect_latest_emulator_api)}"
if [[ -z "$ANDROID_EMULATOR_API_LEVEL" ]]; then
  printf 'No Android arm64 google_apis emulator image found under %s/system-images.\nInstall one with sdkmanager "system-images;android-35;google_apis;arm64-v8a" or set ANDROID_SYSTEM_IMAGE.\n' "$ANDROID_HOME" >&2
  exit 1
fi
AVD_NAME="${ANDROID_AVD_NAME:-FissionApi${ANDROID_EMULATOR_API_LEVEL}Arm64}"
SYSTEM_IMAGE="${ANDROID_SYSTEM_IMAGE:-system-images;android-${ANDROID_EMULATOR_API_LEVEL};google_apis;arm64-v8a}"
DEVICE_PORT="${ANDROID_TEST_CONTROL_DEVICE_PORT:-48761}"
HOST_PORT="${FISSION_TEST_CONTROL_PORT:-48761}"
HEADLESS="${ANDROID_EMULATOR_HEADLESS:-0}"
RESTART_EMULATOR="${ANDROID_EMULATOR_RESTART:-0}"

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

RUNNING_EMULATOR=$("$ADB" devices | awk '/^emulator-.*device$/ { print $1; exit }')
if [[ -n "$RUNNING_EMULATOR" && "$RESTART_EMULATOR" == "1" ]]; then
  "$ADB" -s "$RUNNING_EMULATOR" emu kill >/dev/null || true
  until ! "$ADB" devices | grep -q '^emulator-'; do
    sleep 1
  done
  RUNNING_EMULATOR=""
fi

if [[ -z "$RUNNING_EMULATOR" ]]; then
  EMULATOR_ARGS=(-avd "$AVD_NAME" -gpu "${ANDROID_EMULATOR_GPU:-swiftshader_indirect}" -no-audio)
  if [[ "$HEADLESS" == "1" ]]; then
    EMULATOR_ARGS+=(-no-window)
  fi
  printf 'Launching emulator %s (%s)\n' "$AVD_NAME" "$([[ "$HEADLESS" == "1" ]] && echo headless || echo visible)"
  "$EMULATOR_BIN" "${EMULATOR_ARGS[@]}" >/tmp/fission-android-emulator.log 2>&1 &
  "$ADB" wait-for-device
  until "$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r' | grep -q '^1$'; do
    sleep 1
  done
else
  printf 'Using existing emulator %s\n' "$RUNNING_EMULATOR"
  if [[ "$HEADLESS" != "1" ]]; then
    printf 'If the window is not visible, restart with ANDROID_EMULATOR_RESTART=1 to relaunch a visible emulator.\n'
  fi
fi

APK=$("$SCRIPT_DIR/package-apk.sh")
"$ADB" install -r "$APK"
"$ADB" forward "tcp:$HOST_PORT" "tcp:$DEVICE_PORT"
"$ADB" shell am start -n com.example.web_smoke/android.app.NativeActivity >/dev/null
printf 'APK=%s\n' "$APK"
