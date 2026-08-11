#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
  printf 'non-graphical dependency check requires cargo\n' >&2
  exit 1
fi

readonly interactive_package_pattern='^(fission-render-[^[:space:]]+|fission-(skia|vello|winit)[^[:space:]]*|parley([_-][^[:space:]]+)?|skia([_-][^[:space:]]+)?|vello([_-][^[:space:]]+)?|wgpu([_-][^[:space:]]+)?|winit([_-][^[:space:]]+)?) v'

collect_interactive_packages() {
  local package

  while IFS= read -r package; do
    if [[ "$package" =~ $interactive_package_pattern ]]; then
      printf '%s\n' "$package"
    fi
  done
}

check_graph() {
  local label="$1"
  shift
  local graph
  local matches

  if ! graph="$(cargo tree --edges normal --prefix none --target all "$@")"; then
    printf 'failed to resolve %s dependency graph\n' "$label" >&2
    exit 1
  fi

  matches="$(collect_interactive_packages <<<"$graph")"
  if [[ -n "$matches" ]]; then
    printf '%s acquired interactive graphics dependencies:\n' "$label" >&2
    printf '  %s\n' "$matches" >&2
    exit 1
  fi
}

check_graph "static-site shell" -p fission-shell-site
check_graph "SSR shell" -p fission-shell-server
check_graph "terminal shell" -p fission-shell-terminal
check_graph \
  "public static-site profile" \
  -p fission \
  --no-default-features \
  --features site
check_graph \
  "public SSR profile" \
  -p fission \
  --no-default-features \
  --features server
check_graph \
  "public terminal profile" \
  -p fission \
  --no-default-features \
  --features terminal-shell
check_graph \
  "neutral 3D model" \
  -p fission-3d-model

printf 'non-graphical dependency graphs contain no interactive backends\n'
