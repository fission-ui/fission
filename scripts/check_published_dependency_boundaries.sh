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

if rg -n '\bgit\s*=' "${manifest_paths[@]}"; then
  echo >&2 "error: publishable Fission manifests must resolve external dependencies from the registry"
  exit 1
fi

# Fission owns and publishes these forks under `fission-*` names. A dependency
# on one keeps its local path for repository builds and names the published
# version for crates.io, so they are published first, in this order:
# fission-vello-common, fission-vello-gpu-shaders, fission-glifo,
# fission-vello-cpu, fission-vello-gpu, then fission-render-vello and the
# crates that depend on it.
approved_forks=(
  "fission-vello-common:third_party/vello/vello_common"
  "fission-vello-gpu-shaders:third_party/vello/vello_gpu_shaders"
  "fission-glifo:third_party/vello/glifo"
  "fission-vello-cpu:third_party/vello/vello_cpu"
  "fission-vello-gpu:third_party/vello/vello_gpu"
)

status=0
while IFS= read -r hit; do
  manifest="${hit%%:*}"
  rest="${hit#*:}"
  line_number="${rest%%:*}"
  line="${rest#*:}"
  path="$(sed -n 's/.*path *= *"\([^"]*\)".*/\1/p' <<<"$line")"
  package="$(sed -n 's/.*package *= *"\([^"]*\)".*/\1/p' <<<"$line")"
  version="$(sed -n 's/.*version *= *"\([^"]*\)".*/\1/p' <<<"$line")"
  resolved="$(cd "$(dirname "$manifest")" 2>/dev/null && cd "$path" 2>/dev/null && pwd || true)"
  fork_dir=""
  for fork in "${approved_forks[@]}"; do
    if [[ "$package" == "${fork%%:*}" && "$resolved" == "$repo_root/${fork#*:}" ]]; then
      fork_dir="${fork#*:}"
    fi
  done
  if [[ -z "$fork_dir" ]]; then
    echo >&2 "error: $manifest:$line_number depends on a third_party path that is not an approved Fission fork package"
    status=1
    continue
  fi
  if [[ -z "$version" ]]; then
    echo >&2 "error: $manifest:$line_number must name the published version of $package"
    status=1
    continue
  fi
  fork_version="$(sed -n 's/^version *= *"\([^"]*\)".*/\1/p' "$fork_dir/Cargo.toml" | head -n 1)"
  if [[ "$version" != "$fork_version" ]]; then
    echo >&2 "error: $manifest:$line_number requires $package $version, but $fork_dir/Cargo.toml is $fork_version"
    status=1
  fi
done < <(rg -n --no-heading --with-filename 'path\s*=\s*"[^"]*third_party/' "${manifest_paths[@]}" || true)

if [[ $status -ne 0 ]]; then
  exit 1
fi

echo "published dependency boundaries are registry-resolvable"
