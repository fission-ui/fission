#!/usr/bin/env bash
set -euo pipefail

readonly DEFERRED_EXIT=75
readonly CRATES_IO_API="https://crates.io/api/v1/crates"

crates=(
  fission-command-process
  fission-design-system-codegen
  fission-diagnostics
  fission-i18n
  fission-icons
  fission-ir
  fission-macros
  fission-store
  fission-test-driver
  fission-text-engine
  fission-command-server
  fission-layout
  fission-semantics
  fission-store-sqlite
  fission-theme
  fission-command-core
  fission-core
  fission-render
  fission-3d
  fission-devtools
  fission-render-vello
  fission-shell
  fission-widgets
  fission-charts
  fission-shell-site
  fission-shell-terminal
  fission-shell-winit
  fission-shell-desktop
  fission-shell-mobile
  fission-shell-server
  fission-shell-web
  fission-test
  fission-command-site
  fission
  fission-command-run
  fission-command-package
  fission-command-release
  fission-command-ui
  cargo-fission
)

usage() {
  cat <<'EOF'
Usage: publish_release_crates.sh <version>
       publish_release_crates.sh --list
       publish_release_crates.sh --preflight <version> <crate>

Publishes Fission's crates in dependency order. Existing versions are accepted
only when their registry archive identifies the current release commit. Exit 75
means registry propagation or throttling deferred the release safely.
EOF
}

if [[ ${1:-} == "--list" ]]; then
  printf '%s\n' "${crates[@]}"
  exit 0
fi

preflight_crate=""
if [[ ${1:-} == "--preflight" ]]; then
  if [[ $# -ne 3 ]]; then
    usage >&2
    exit 2
  fi
  version=$2
  preflight_crate=$3
elif [[ $# -eq 1 ]]; then
  version=$1
else
  usage >&2
  exit 2
fi
readonly version preflight_crate
if [[ ! $version =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]]; then
  echo "invalid release version: $version" >&2
  exit 2
fi

readonly expected_commit=${RELEASE_COMMIT:-$(git rev-parse HEAD)}
readonly user_agent="fission-release/${version} (https://github.com/fission-ui/fission)"
readonly target_dir=${CARGO_TARGET_DIR:-target}

if [[ $(git rev-parse HEAD) != "$expected_commit" ]]; then
  echo "release checkout does not match RELEASE_COMMIT=$expected_commit" >&2
  exit 1
fi
if [[ -n $(git status --porcelain) ]]; then
  echo "release checkout is dirty" >&2
  exit 1
fi

metadata=$(cargo metadata --locked --no-deps --format-version 1)
for crate in "${crates[@]}"; do
  actual_version=$(jq -r --arg crate "$crate" '.packages[] | select(.name == $crate) | .version' <<<"$metadata")
  if [[ $actual_version != "$version" ]]; then
    echo "$crate has version ${actual_version:-<missing>}, expected $version" >&2
    exit 1
  fi
done

metadata_file=$(mktemp "${RUNNER_TEMP:-/tmp}/fission-release-metadata.XXXXXX.json")
preflight_config=$(mktemp "${RUNNER_TEMP:-/tmp}/fission-release-preflight.XXXXXX.toml")
printf '%s\n' "$metadata" >"$metadata_file"
python3 - "$metadata_file" "$preflight_config" "${crates[@]}" <<'PY'
import json
import pathlib
import sys

metadata_path, output_path, *crate_names = sys.argv[1:]
metadata = json.loads(pathlib.Path(metadata_path).read_text())
wanted = set(crate_names)
packages = {package["name"]: package for package in metadata["packages"]}

with pathlib.Path(output_path).open("w") as output:
    output.write("[patch.crates-io]\n")
    for name in crate_names:
        manifest = pathlib.Path(packages[name]["manifest_path"])
        output.write(f"{json.dumps(name)} = {{ path = {json.dumps(str(manifest.parent))} }}\n")
PY

defer() {
  local reason=$1
  echo "RELEASE_DEFERRED $reason" >&2
  exit "$DEFERRED_EXIT"
}

registry_response() {
  local crate=$1
  local body=$2
  local headers=$3
  local code retry_after

  if ! code=$(curl \
    --silent --show-error \
    --retry 0 \
    --user-agent "$user_agent" \
    --dump-header "$headers" \
    --output "$body" \
    --write-out '%{http_code}' \
    "$CRATES_IO_API/$crate/$version"); then
    echo "failed to query crates.io for $crate $version" >&2
    return 1
  fi

  case "$code" in
    200|404)
      printf '%s\n' "$code"
      ;;
    429)
      retry_after=$(awk 'BEGIN { IGNORECASE=1 } /^retry-after:/ { gsub("\\r", "", $2); print $2 }' "$headers")
      defer "crates.io throttled $crate${retry_after:+; Retry-After=$retry_after}"
      ;;
    502|503|504)
      defer "crates.io returned HTTP $code for $crate"
      ;;
    *)
      echo "crates.io returned unexpected HTTP $code for $crate $version" >&2
      return 1
      ;;
  esac
}

verify_registry_archive() {
  local crate=$1
  local response_body=$2
  local expected_checksum archive archive_checksum unpack_dir package_root

  expected_checksum=$(jq -er '.version.checksum' "$response_body")
  archive=$(mktemp "${RUNNER_TEMP:-/tmp}/${crate}-${version}.XXXXXX.crate")
  unpack_dir=$(mktemp -d "${RUNNER_TEMP:-/tmp}/${crate}-${version}.XXXXXX")

  curl --fail --silent --show-error --location --retry 3 \
    --user-agent "$user_agent" \
    "$CRATES_IO_API/$crate/$version/download" \
    --output "$archive"
  archive_checksum=$(sha256sum "$archive" | awk '{print $1}')
  if [[ $archive_checksum != "$expected_checksum" ]]; then
    echo "$crate $version download checksum does not match crates.io metadata" >&2
    return 1
  fi

  tar -xzf "$archive" -C "$unpack_dir"
  package_root="$unpack_dir/$crate-$version"
  if [[ ! -f $package_root/.cargo_vcs_info.json || ! -f $package_root/Cargo.toml.orig ]]; then
    echo "$crate $version archive lacks release provenance files" >&2
    return 1
  fi
  if ! jq -e --arg commit "$expected_commit" \
    '.git.sha1 == $commit and ((.git.dirty // false) == false)' \
    "$package_root/.cargo_vcs_info.json" >/dev/null; then
    echo "$crate $version was not packaged from release commit $expected_commit" >&2
    return 1
  fi
  python3 - "$package_root/Cargo.toml.orig" "$crate" "$version" <<'PY'
import pathlib
import sys
import tomllib

manifest = tomllib.loads(pathlib.Path(sys.argv[1]).read_text())
package = manifest.get("package", {})
if package.get("name") != sys.argv[2] or package.get("version") != sys.argv[3]:
    raise SystemExit("published Cargo.toml.orig identity does not match the release")
PY

  echo "RELEASE_VERIFIED $crate $version checksum=$expected_checksum commit=$expected_commit"
}

audit_package() {
  local crate=$1
  local package_dir="$target_dir/package/$crate-$version"

  # Patch first-party registry dependencies back to this immutable checkout for
  # the dry run. This lets every archive compile before the first new version is
  # uploaded, while the actual published manifests retain their registry
  # version requirements.
  cargo publish --locked --dry-run --config "$preflight_config" -p "$crate"
  if [[ ! -d $package_dir ]]; then
    echo "cargo did not reconstruct $package_dir" >&2
    return 1
  fi

  if find "$package_dir" -type f \( \
    -name '.env' -o -name '.env.*' -o -name 'id_rsa*' -o \
    -name '*.pem' -o -name '*.key' -o -name '*.p12' -o -name '*.pfx' \
  \) -print -quit | grep -q .; then
    echo "suspicious credential filename in $crate package" >&2
    return 1
  fi
  if rg -l --hidden --glob '!Cargo.lock' \
    '(AKIA[0-9A-Z]{16}|gh[pousr]_[A-Za-z0-9]{30,})' \
    "$package_dir" | grep -q .; then
    echo "high-confidence credential pattern in $crate package" >&2
    return 1
  fi
  if rg -l -U --hidden --glob '!Cargo.lock' \
    -- '-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----\r?\n[A-Za-z0-9+/]{40,}' \
    "$package_dir" | grep -q .; then
    echo "private key material in $crate package" >&2
    return 1
  fi
}

if [[ -n $preflight_crate ]]; then
  if [[ ! " ${crates[*]} " =~ " ${preflight_crate} " ]]; then
    echo "unknown release crate: $preflight_crate" >&2
    exit 2
  fi
  audit_package "$preflight_crate"
  echo "RELEASE_PREFLIGHT_COMPLETE $preflight_crate $version"
  exit 0
fi

publish_crate() {
  local crate=$1
  local log_file status
  log_file=$(mktemp "${RUNNER_TEMP:-/tmp}/${crate}-publish.XXXXXX.log")

  set +e
  cargo publish --locked --no-verify -p "$crate" 2>&1 | tee "$log_file"
  status=${PIPESTATUS[0]}
  set -e
  if [[ $status -eq 0 ]]; then
    return 0
  fi

  # Cargo can time out while waiting for the index after the upload succeeded.
  # The caller always checks crates.io again before deciding whether to retry.
  if rg -qi '429|too many requests|rate.?limit|timed out.*index|failed to get a 200 OK response.*(502|503|504)' "$log_file"; then
    return "$DEFERRED_EXIT"
  fi
  return "$status"
}

unpublished=()
for crate in "${crates[@]}"; do
  response_body=$(mktemp "${RUNNER_TEMP:-/tmp}/${crate}-response.XXXXXX.json")
  response_headers=$(mktemp "${RUNNER_TEMP:-/tmp}/${crate}-headers.XXXXXX")
  code=$(registry_response "$crate" "$response_body" "$response_headers")

  if [[ $code == 200 ]]; then
    verify_registry_archive "$crate" "$response_body"
    continue
  fi

  unpublished+=("$crate")
done

for crate in "${unpublished[@]}"; do
  echo "RELEASE_STEP package-and-audit $crate $version"
  audit_package "$crate"
done

for crate in "${unpublished[@]}"; do
  response_body=$(mktemp "${RUNNER_TEMP:-/tmp}/${crate}-response.XXXXXX.json")
  response_headers=$(mktemp "${RUNNER_TEMP:-/tmp}/${crate}-headers.XXXXXX")
  code=$(registry_response "$crate" "$response_body" "$response_headers")
  if [[ $code == 200 ]]; then
    verify_registry_archive "$crate" "$response_body"
    continue
  fi

  echo "RELEASE_STEP publish $crate $version"

  set +e
  publish_crate "$crate"
  publish_status=$?
  set -e

  # Check the registry after both success and failure. An upload may have
  # succeeded even when Cargo timed out waiting for index propagation.
  code=$(registry_response "$crate" "$response_body" "$response_headers")
  if [[ $code == 200 ]]; then
    verify_registry_archive "$crate" "$response_body"
    continue
  fi
  if [[ $publish_status -eq $DEFERRED_EXIT ]]; then
    defer "publish of $crate was throttled or is awaiting registry propagation"
  fi
  if [[ $publish_status -ne 0 ]]; then
    echo "cargo publish failed for $crate with exit code $publish_status" >&2
    exit "$publish_status"
  fi
  defer "$crate was uploaded but is not visible from crates.io yet"
done

echo "RELEASE_COMPLETE ${#crates[@]} crates at $version from $expected_commit"
