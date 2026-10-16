#!/usr/bin/env bash
set -euo pipefail

site_root=${1:?usage: check_showcase_page.sh <generated-site-root>}
home_page="$site_root/index.html"
showcase_page="$site_root/showcase/index.html"

test -f "$home_page"
test -f "$showcase_page"

require() {
  local file=$1
  local text=$2
  if ! grep -Fq -- "$text" "$file"; then
    echo "missing expected showcase markup in $file: $text" >&2
    exit 1
  fi
}

require "$home_page" 'href="showcase/"'
require "$home_page" 'See it in action'
require "$home_page" 'Try Fission in the browser'
require "$home_page" 'aria-label="Open the interactive Fission Rust GUI and WebAssembly example showcase"'

require "$showcase_page" '<title>Fission Rust GUI WASM example apps</title>'
require "$showcase_page" 'name="description" content="Try interactive Fission Rust GUI and WASM example apps in your browser.'
require "$showcase_page" 'rel="canonical" href="https://fission.rs/showcase/"'
require "$showcase_page" 'property="og:title" content="Fission Rust GUI WASM example apps"'
require "$showcase_page" 'name="twitter:title" content="Fission Rust GUI WASM example apps"'
require "$showcase_page" 'Fission Rust GUI and WASM example apps'
require "$showcase_page" 'src="/showcase-app/"'
require "$showcase_page" 'aria-label="Interactive Fission Rust GUI and WebAssembly example application showcase"'
require "$showcase_page" 'title="Interactive Fission Rust GUI and WebAssembly example application showcase"'
require "$showcase_page" '"@type":"WebApplication"'
