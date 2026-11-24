#!/usr/bin/env bash
set -euo pipefail

readonly SQLITE_COMMIT="4ebc7fdcf459e8d88eb5b019c2949bda86565528"
readonly SQLITE_REPOSITORY="https://github.com/sqlite/sqlite.git"
readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly OUTPUT_ARGUMENT="${1:?usage: build_sqlite_wasm.sh OUTPUT_DIRECTORY}"
mkdir -p "$OUTPUT_ARGUMENT"
readonly OUTPUT_DIR="$(cd -- "$OUTPUT_ARGUMENT" && pwd)"
readonly WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

mkdir -p "$OUTPUT_DIR"
readonly SOURCE_DIR="$WORK_DIR/sqlite"
git init --quiet "$SOURCE_DIR"
git -C "$SOURCE_DIR" remote add origin "$SQLITE_REPOSITORY"
git -C "$SOURCE_DIR" fetch --quiet --depth 1 origin version-3.53.0
git -C "$SOURCE_DIR" checkout --quiet --detach FETCH_HEAD

actual_commit="$(git -C "$SOURCE_DIR" rev-parse HEAD)"
if [[ "$actual_commit" != "$SQLITE_COMMIT" ]]; then
  echo "SQLite source revision mismatch: expected $SQLITE_COMMIT, got $actual_commit" >&2
  exit 1
fi

python3 "$SCRIPT_DIR/patch_sqlite_wasm_api.py" "$SOURCE_DIR"
cd "$SOURCE_DIR"
./configure --enable-all
make -j2 sqlite3.c
make -C ext/wasm -j2 o2

readonly DIST_DIR="$SOURCE_DIR/ext/wasm/jswasm"
for artifact in sqlite3.mjs sqlite3.wasm sqlite3-opfs-async-proxy.js; do
  test -s "$DIST_DIR/$artifact"
  cp "$DIST_DIR/$artifact" "$OUTPUT_DIR/$artifact"
done

# Fission's vendored proxy has no whitespace on otherwise blank lines. The
# published 3.53.0 package contains indentation on those same blank lines.
python3 "$SCRIPT_DIR/normalize_sqlite_wasm_proxy.py" "$OUTPUT_DIR/sqlite3-opfs-async-proxy.js"

grep -Fq 'SQLITE_VERSION "3.53.0"' "$OUTPUT_DIR/sqlite3.mjs"
wasm_exports="$(wasm-objdump -x "$OUTPUT_DIR/sqlite3.wasm")"
for function in sqlite3_backup_finish sqlite3_backup_init sqlite3_backup_step; do
  grep -Fq "\"$function\"" "$OUTPUT_DIR/sqlite3.mjs"
  grep -Fq "$function" <<<"$wasm_exports"
done
echo "Built SQLite WASM 3.53.0 artifacts in $OUTPUT_DIR"
