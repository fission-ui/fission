#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(cd -- "$SCRIPT_DIR/.." && pwd)
SHOWCASE_DIR="$ROOT_DIR/examples/showcase/platforms/web"
DESTINATION="$ROOT_DIR/docs2/generated-assets/showcase-app"
PROFILE="${FISSION_WEB_PROFILE:-release}"

(
  cd "$ROOT_DIR"
  FISSION_WEB_PROFILE="$PROFILE" ./examples/showcase/platforms/web/build-wasm.sh
)

mkdir -p "$DESTINATION/pkg"
cp "$SHOWCASE_DIR/index.html" "$DESTINATION/index.html"
cp "$SHOWCASE_DIR/bootstrap.mjs" "$DESTINATION/bootstrap.mjs"
cp "$SHOWCASE_DIR/app-icon.png" "$DESTINATION/app-icon.png"
cp -R "$SHOWCASE_DIR/pkg/." "$DESTINATION/pkg/"

echo "Staged the $PROFILE showcase app at docs2/generated-assets/showcase-app"
