#!/usr/bin/env bash
set -euo pipefail

if ! command -v rg >/dev/null 2>&1; then
  printf 'Rust file-size boundary check requires ripgrep (rg)\n' >&2
  exit 1
fi

readonly max_lines=2000

scan_rust_files() {
  local path
  local line_count
  local status

  while IFS= read -r -d '' path; do
    status=0
    line_count="$(awk 'END { print NR }' "$path")" || status=$?
    if (( status != 0 )); then
      printf 'failed to count lines in %q (status %d)\n' "$path" "$status" >&2
      return "$status"
    fi

    if (( line_count > max_lines )); then
      printf '%d %q\n' "$line_count" "$path"
    fi
  done
}

status=0
violations="$(
  rg \
    --files \
    -0 \
    --hidden \
    --glob '*.rs' \
    --glob '!.git/**' \
    --glob '!**/.git/**' \
    --glob '!third_party/**' \
    | scan_rust_files
)" || status=$?

if (( status != 0 )); then
  printf 'failed to enumerate first-party Rust files (status %d)\n' "$status" >&2
  exit "$status"
fi

if [[ -n "$violations" ]]; then
  printf 'first-party Rust files must not exceed %d lines:\n' "$max_lines" >&2
  printf '%s\n' "$violations" | sort -nr | sed 's/^/  /' >&2
  exit 1
fi

printf 'first-party Rust file-size boundary passed (maximum %d lines)\n' "$max_lines"
