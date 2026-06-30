#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
BUNDLE_DIR=$("$SCRIPT_DIR/package-sim.sh")
BUNDLE_ID="${IOS_BUNDLE_ID:-ai.worka.fission.mobile-smoke}"
DEVICE_ID="${IOS_SIM_DEVICE_ID:-}"

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

if [[ "${IOS_SIM_HEADLESS:-0}" != "1" ]] && command -v open >/dev/null 2>&1; then
  open -a Simulator --args -CurrentDeviceUDID "$DEVICE_ID" >/dev/null 2>&1 \
    || open -a Simulator >/dev/null 2>&1 \
    || true
fi

xcrun simctl boot "$DEVICE_ID" >/dev/null 2>&1 || true
xcrun simctl bootstatus "$DEVICE_ID" -b
if [[ "${IOS_SIM_UNINSTALL_BEFORE_INSTALL:-1}" == "1" ]]; then
  xcrun simctl uninstall "$DEVICE_ID" "$BUNDLE_ID" >/dev/null 2>&1 || true
fi
xcrun simctl install "$DEVICE_ID" "$BUNDLE_DIR"

if [[ -n "${FISSION_TEST_CONTROL_PORT:-}" ]]; then
  SIMCTL_CHILD_FISSION_TEST_CONTROL_PORT="${FISSION_TEST_CONTROL_PORT}" \
    xcrun simctl launch --terminate-running-process "$DEVICE_ID" "$BUNDLE_ID"
else
  xcrun simctl launch --terminate-running-process "$DEVICE_ID" "$BUNDLE_ID"
fi
