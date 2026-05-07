#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
BUNDLE_DIR=$("$SCRIPT_DIR/package-sim.sh")
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

xcrun simctl boot "$DEVICE_ID" >/dev/null 2>&1 || true
xcrun simctl bootstatus "$DEVICE_ID" -b
xcrun simctl install "$DEVICE_ID" "$BUNDLE_DIR"

if [[ -n "${FISSION_TEST_CONTROL_PORT:-}" ]]; then
  SIMCTL_CHILD_FISSION_TEST_CONTROL_PORT="${FISSION_TEST_CONTROL_PORT}" \
    xcrun simctl launch --terminate-running-process "$DEVICE_ID" ai.worka.fission.mobile-smoke
else
  xcrun simctl launch --terminate-running-process "$DEVICE_ID" ai.worka.fission.mobile-smoke
fi
