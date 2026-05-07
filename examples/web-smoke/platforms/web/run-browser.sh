#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
REPO_ROOT=$(cd -- "$PROJECT_DIR/../.." && pwd)
HOST="${FISSION_WEB_HOST:-127.0.0.1}"
PORT="${FISSION_WEB_PORT:-8123}"
URL="http://${HOST}:${PORT}/examples/web-smoke/platforms/web/"

"$SCRIPT_DIR/build-wasm.sh"

printf 'Serving %s\n' "$URL"
printf 'Press Ctrl+C to stop the local server.\n'
if [[ "${FISSION_WEB_OPEN:-0}" == "1" ]]; then
  open "$URL"
fi

cd "$REPO_ROOT"
python3 -m http.server "$PORT" --bind "$HOST"
