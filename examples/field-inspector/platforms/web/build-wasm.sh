#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
PROFILE="${FISSION_WEB_PROFILE:-dev}"
FISSION_BIN="${FISSION_BIN:-fission}"
BUILD_ARGS=(build --target web --project-dir "$PROJECT_DIR")

if [[ "$PROFILE" == "release" ]]; then
  BUILD_ARGS+=(--release)
fi

"$FISSION_BIN" "${BUILD_ARGS[@]}"
