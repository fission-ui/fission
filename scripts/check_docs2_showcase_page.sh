#!/usr/bin/env bash
set -euo pipefail

site_root=${1:?usage: check_docs2_showcase_page.sh <generated-site-root>}
home_page="$site_root/index.html"
showcase_page="$site_root/showcase/index.html"
showcase_app="$site_root/showcase-app/index.html"

test -f "$home_page"
test -f "$showcase_page"
test -f "$showcase_app"
test -f "$site_root/showcase-app/pkg/example_showcase.js"
find "$site_root/showcase-app/pkg" -maxdepth 1 -name '*_bg.wasm' -print -quit | grep -q .

require() {
  local file=$1
  local text=$2
  if ! grep -Fq -- "$text" "$file"; then
    echo "missing expected showcase markup in $file: $text" >&2
    exit 1
  fi
}

require "$home_page" 'Try Fission in the browser'
require "$showcase_page" '<title>Try Fission in your browser — Rust app showcase</title>'
require "$showcase_page" 'src="/showcase-app/?fission_renderer=canvas2d-software"'
require "$showcase_page" 'data-fission-semantics="site-showcase-frame"'
require "$showcase_page" '"@type":"WebApplication"'
require "$site_root/site.css" 'grid-template-columns: minmax(0, 1fr);'
require "$site_root/site.css" 'grid-template-rows: auto minmax(0, 1fr);'
require "$site_root/site.css" 'data-fission-semantics="site-showcase-frame"'
