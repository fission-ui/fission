#!/usr/bin/env bash
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
