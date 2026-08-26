#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

manifest_paths=(Cargo.toml)
while IFS= read -r manifest; do
  manifest_paths+=("$manifest")
done < <(find crates -name Cargo.toml -type f | sort)

if rg -n '^\s*\[(patch|replace)(\.|\])' "${manifest_paths[@]}"; then
  echo >&2 "error: publishable Fission manifests must not override registry dependencies with [patch] or [replace]"
  exit 1
fi

if rg -n '\bgit\s*=|\bpath\s*=\s*"[^"]*third_party/' "${manifest_paths[@]}"; then
  echo >&2 "error: publishable Fission manifests must resolve external dependencies from the registry"
  exit 1
fi

echo "published dependency boundaries are registry-resolvable"
