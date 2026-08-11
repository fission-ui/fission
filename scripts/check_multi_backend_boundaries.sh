#!/usr/bin/env bash
set -euo pipefail

if ! command -v rg >/dev/null 2>&1; then
  printf 'multi-backend boundary check requires ripgrep (rg)\n' >&2
  exit 1
fi

readonly implementation_crate_name='(fission-render-[[:alnum:]_-]+|fission-(skia|vello|winit)[[:alnum:]_-]*|parley([_-][[:alnum:]_-]+)?|skia([_-][[:alnum:]_-]+)?|vello([_-][[:alnum:]_-]+)?|wgpu([_-][[:alnum:]_-]+)?|winit([_-][[:alnum:]_-]+)?)'
readonly implementation_dependency_pattern="^[[:space:]]*['\"]?${implementation_crate_name}['\"]?[[:space:]]*(\\.workspace[[:space:]]*)?="
readonly implementation_dotted_dependency_pattern="^[[:space:]]*([^#=]+\\.[[:space:]]*)?['\"]?((dev|build)-)?dependencies['\"]?[[:space:]]*\\.[[:space:]]*['\"]?${implementation_crate_name}['\"]?[[:space:]]*(\\.workspace[[:space:]]*)?="
readonly implementation_dependency_table_pattern="^[[:space:]]*\\[[^]]*['\"]?((dev|build)-)?dependencies['\"]?[[:space:]]*\\.[[:space:]]*['\"]?${implementation_crate_name}['\"]?[[:space:]]*\\]"
readonly implementation_package_pattern="^[[:space:]]*([^#]*[{},.][[:space:]]*)?package[[:space:]]*=[[:space:]]*['\"]${implementation_crate_name}['\"]"
readonly implementation_source_name='(fission_render_[[:alnum:]_]+|fission_(skia|vello|winit)[[:alnum:]_]*|parley([_][[:alnum:]_]+)?|skia([_][[:alnum:]_]+)?|vello([_][[:alnum:]_]+)?|wgpu([_][[:alnum:]_]+)?|winit([_][[:alnum:]_]+)?)'
readonly implementation_source_pattern="(^|[^[:alnum:]_])${implementation_source_name}(::|_[[:alnum:]_])"

search_or_no_match() {
  local status=0

  rg "$@" || status=$?
  if (( status == 1 )); then
    return 0
  fi
  return "$status"
}

collect_neutral_tree_matches() {
  search_or_no_match -n --glob 'Cargo.toml' "$implementation_dependency_pattern" "$@" || return $?
  search_or_no_match -n --glob 'Cargo.toml' "$implementation_dotted_dependency_pattern" "$@" || return $?
  search_or_no_match -n --glob 'Cargo.toml' "$implementation_dependency_table_pattern" "$@" || return $?
  search_or_no_match -n --glob 'Cargo.toml' "$implementation_package_pattern" "$@" || return $?
  search_or_no_match -n --glob '*.rs' "$implementation_source_pattern" "$@" || return $?
}

check_neutral_tree() {
  local label="$1"
  shift
  local matches
  local status=0

  matches="$(collect_neutral_tree_matches "$@" | sort -u)" || status=$?
  if (( status != 0 )); then
    printf 'failed to scan %s for backend implementation dependencies (status %d)\n' \
      "$label" "$status" >&2
    exit "$status"
  fi

  if [[ -n "$matches" ]]; then
    printf '%s must remain independent of interactive backend implementations:\n' "$label" >&2
    printf '  %s\n' "$matches" >&2
    exit 1
  fi
}

check_fission_3d_neutral_sources() {
  local matches
  local status=0

  matches="$(
    search_or_no_match \
      -n \
      --glob '*.rs' \
      --glob '!crates/core/fission-3d/src/render.rs' \
      "$implementation_source_pattern" \
      crates/core/fission-3d/src \
      | sort -u
  )" || status=$?
  if (( status != 0 )); then
    printf 'failed to scan fission-3d neutral facade sources (status %d)\n' "$status" >&2
    exit "$status"
  fi

  if [[ -n "$matches" ]]; then
    printf 'fission-3d facade sources outside src/render.rs must remain backend-neutral:\n' >&2
    printf '  %s\n' "$matches" >&2
    exit 1
  fi
}

# `fission-3d` retains its existing renderer re-export solely for public API
# compatibility. The actual neutral contract is `fission-3d-model`, and
# production hosts depend on model/renderer crates separately. Its manifest and
# `src/render.rs` are the compatibility boundary; all other facade source stays
# under the source-level gate while the model crate remains fully neutral.
declare -a neutral_core_paths=()
for path in crates/core/*; do
  if [[ "$path" != "crates/core/fission-3d" ]]; then
    neutral_core_paths+=("$path")
  fi
done

check_neutral_tree "core and authoring crates" "${neutral_core_paths[@]}" crates/authoring
check_fission_3d_neutral_sources
check_neutral_tree "render contracts" crates/rendering/fission-render
check_neutral_tree "standalone software renderer" crates/rendering/fission-render-software
check_neutral_tree \
  "static, SSR, and terminal shells" \
  crates/shell/fission-shell-site \
  crates/shell/fission-shell-server \
  crates/shell/fission-shell-terminal

printf 'multi-backend dependency boundaries passed\n'
