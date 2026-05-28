#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
HOST="${FISSION_WEB_HOST:-127.0.0.1}"
REQUESTED_PORT="${FISSION_WEB_PORT:-8123}"
PORT="$REQUESTED_PORT"
if [[ -z "${FISSION_WEB_PORT:-}" ]]; then
  PORT=$(python3 - "$HOST" "$REQUESTED_PORT" <<'PY'
import socket
import sys

host = sys.argv[1]
start = int(sys.argv[2])
for port in range(start, start + 51):
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            probe.bind((host, port))
        except OSError:
            continue
        print(port)
        raise SystemExit(0)
raise SystemExit(f"no free web port found from {host}:{start}")
PY
)
  if [[ "$PORT" != "$REQUESTED_PORT" ]]; then
    printf 'Port %s:%s is already in use; using %s:%s.\n' "$HOST" "$REQUESTED_PORT" "$HOST" "$PORT"
  fi
fi
URL="http://${HOST}:${PORT}/platforms/web/"

"$SCRIPT_DIR/build-wasm.sh"

printf 'Serving %s\n' "$URL"
printf 'Press Ctrl+C to stop the local server.\n'
if [[ "${FISSION_WEB_OPEN:-0}" == "1" ]]; then
  if command -v open >/dev/null 2>&1; then
    open "$URL"
  elif command -v xdg-open >/dev/null 2>&1; then
    xdg-open "$URL"
  elif command -v cmd.exe >/dev/null 2>&1; then
    cmd.exe /C start "$URL"
  else
    printf 'No browser opener found. Open %s manually.\n' "$URL"
  fi
fi

cd "$PROJECT_DIR"
python3 -m http.server "$PORT" --bind "$HOST"
