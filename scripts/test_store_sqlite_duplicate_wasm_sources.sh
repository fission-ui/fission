#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fission_root="$(cd "$script_dir/.." && pwd)"
fixture="$fission_root/crates/core/fission-store-sqlite/tests/fixtures/duplicate-source-link"
target_dir="${CARGO_TARGET_DIR:-$fission_root/target/store-sqlite-duplicate-source-link}"

CARGO_TARGET_DIR="$target_dir" cargo build \
  --manifest-path "$fixture/Cargo.toml" \
  --locked \
  --target wasm32-unknown-unknown \
  --release
