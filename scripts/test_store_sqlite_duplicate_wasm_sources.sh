#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fission_root="$(cd "$script_dir/.." && pwd)"
fixture="$fission_root/crates/core/fission-store-sqlite/tests/fixtures/duplicate-source-link"
target_dir="${CARGO_TARGET_DIR:-$fission_root/target}"
package_dir="$target_dir/duplicate-sqlite-source-package"

CARGO_TARGET_DIR="$target_dir" cargo build \
  --manifest-path "$fixture/Cargo.toml" \
  --locked \
  --target wasm32-unknown-unknown \
  --release

# Duplicate descriptor exports are diagnosed by wasm-bindgen, not by Cargo's
# linker. Package the exact artifact so this regression exercises that boundary.
wasm-bindgen \
  "$target_dir/wasm32-unknown-unknown/release/fission_store_sqlite_duplicate_source_link.wasm" \
  --target web \
  --out-dir "$package_dir"

# Exercise the packaged bindings too, so a dead-stripped source identity cannot
# make the descriptor regression appear to pass without retaining both calls.
# wasm-bindgen's Web output contains ESM in .js files. Declare that package type
# explicitly instead of relying on an experimental Node command-line option.
cp "$script_dir/test_store_sqlite_duplicate_wasm_package.json" "$package_dir/package.json"
node "$fixture/check-package.mjs" "$package_dir"
