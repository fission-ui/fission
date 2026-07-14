#!/usr/bin/env bash
set -euo pipefail

check_absent() {
  local crate="$1"
  local reason="$2"
  local pattern="$3"
  local tree

  tree="$(cargo tree -p "$crate" --edges normal --prefix none)"
  if grep -E "$pattern" <<<"$tree" >/tmp/fission-boundary-matches.txt; then
    echo "dependency boundary violation in $crate: $reason" >&2
    cat /tmp/fission-boundary-matches.txt >&2
    rm -f /tmp/fission-boundary-matches.txt
    exit 1
  fi
  rm -f /tmp/fission-boundary-matches.txt
}

check_absent \
  "fission-command-site" \
  "static-site commands must not pull cloud/store/provider-secret or native runtime command crates" \
  '^(aws-|reqwest|ureq|jsonwebtoken|rpassword|rfd|dbus|zbus|ashpd|winit|wgpu|vello|android-activity|fission-shell-winit|fission-shell-mobile|fission-command-package|fission-command-release|fission-command-ui) '

check_absent \
  "fission-command-run" \
  "run/build/test commands must not pull cloud/store/provider-secret or release UI crates" \
  '^(aws-|reqwest|jsonwebtoken|rpassword|fission-command-package|fission-command-release|fission-command-ui) '

echo "command dependency boundaries passed"
