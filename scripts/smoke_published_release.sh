#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 || ! ${1:-} =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]]; then
  echo "usage: smoke_published_release.sh <version>" >&2
  exit 2
fi

readonly version=$1
readonly smoke_root=${RUNNER_TEMP:?RUNNER_TEMP must be set}/fission-release-smoke
readonly cargo_home="$smoke_root/cargo-home"
readonly cargo_target="$smoke_root/target"
readonly install_root="$smoke_root/install"
readonly app_dir="$smoke_root/app"
readonly run_log="$smoke_root/fission-run.log"
readonly port=8123

mkdir -p "$smoke_root"
export CARGO_HOME="$cargo_home"
export CARGO_TARGET_DIR="$cargo_target"
export PATH="$install_root/bin:$PATH"

rustup target add wasm32-unknown-unknown
cargo install wasm-pack --version 0.13.1 --locked --root "$install_root"
cargo install cargo-fission --version "$version" --locked --root "$install_root"

if [[ $(fission --version) != "fission $version" ]]; then
  echo "installed fission binary did not report version $version" >&2
  exit 1
fi

fission init "$app_dir" --name release-smoke
fission add-target web --project-dir "$app_dir"
python3 - "$app_dir/Cargo.toml" "$version" <<'PY'
import pathlib
import sys
import tomllib

manifest = tomllib.loads(pathlib.Path(sys.argv[1]).read_text())
requirement = manifest["dependencies"]["fission"]["version"]
if requirement != sys.argv[2]:
    raise SystemExit(
        f"generated application requested fission {requirement}, expected {sys.argv[2]}"
    )
PY

setsid fission run \
  --project-dir "$app_dir" \
  --target web \
  --host 127.0.0.1 \
  --port "$port" \
  --no-open >"$run_log" 2>&1 &
server_pid=$!
cleanup() {
  if kill -0 "$server_pid" 2>/dev/null; then
    kill -- "-$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
}
trap cleanup EXIT

ready=false
for _ in $(seq 1 600); do
  if curl --fail --silent "http://127.0.0.1:$port/" >"$smoke_root/index.html"; then
    ready=true
    break
  fi
  if ! kill -0 "$server_pid" 2>/dev/null; then
    echo "fission run exited before serving the generated application" >&2
    sed -n '1,240p' "$run_log" >&2
    exit 1
  fi
  sleep 2
done

if [[ $ready != true ]]; then
  echo "generated Web application was not served within 20 minutes" >&2
  sed -n '1,240p' "$run_log" >&2
  exit 1
fi
if [[ ! -s $smoke_root/index.html ]] || ! rg -q '<script|\.js' "$smoke_root/index.html"; then
  echo "generated Web application returned an incomplete HTML shell" >&2
  sed -n '1,160p' "$smoke_root/index.html" >&2
  exit 1
fi

echo "RELEASE_SMOKE_COMPLETE cargo-fission=$version app=$app_dir url=http://127.0.0.1:$port/"
echo "Rendered-frame browser qualification remains a deliberate follow-up for the next release."
