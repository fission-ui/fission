#!/usr/bin/env bash
set -euo pipefail

readonly DEFERRED_EXIT=75
readonly CRATES_IO_API="https://crates.io/api/v1/crates"

crates=(
  fission-command-process
  fission-design-system-codegen
  fission-diagnostics
  fission-assets
  fission-scene
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
  fission-scene2d
  fission-scene3d
  fission-game
  fission-physics
  fission-physics-rapier2d
  fission-physics-rapier3d
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

Publishes Fission's crates in dependency order. Existing framework-version
archives must identify the current release commit. Existing independently
versioned alpha prerequisites are verified but may identify their original
release commit. Exit 75 means registry propagation or throttling deferred the
release safely.
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
readonly repository_root=$(git rev-parse --show-toplevel)

if [[ $(git rev-parse HEAD) != "$expected_commit" ]]; then
  echo "release checkout does not match RELEASE_COMMIT=$expected_commit" >&2
  exit 1
fi
if [[ -n $(git status --porcelain) ]]; then
  echo "release checkout is dirty" >&2
  exit 1
fi

metadata=$(cargo metadata --locked --no-deps --format-version 1)
declare -A crate_versions
declare -A crate_paths
for crate in "${crates[@]}"; do
  actual_version=$(jq -r --arg crate "$crate" '.packages[] | select(.name == $crate) | .version' <<<"$metadata")
  api_status=$(jq -r --arg crate "$crate" '.packages[] | select(.name == $crate) | .metadata.fission["api-status"] // "stable"' <<<"$metadata")
  manifest_path=$(jq -r --arg crate "$crate" '.packages[] | select(.name == $crate) | .manifest_path' <<<"$metadata")
  if [[ -z $actual_version ]]; then
    echo "$crate is missing from workspace metadata" >&2
    exit 1
  fi
  if [[ $actual_version != "$version" && $api_status != "alpha" ]]; then
    echo "$crate has version $actual_version, expected framework version $version" >&2
    exit 1
  fi
  crate_versions["$crate"]=$actual_version
  crate_paths["$crate"]=$(dirname "${manifest_path#"$repository_root"/}")
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
  local crate_version=$2
  local body=$3
  local headers=$4
  local code retry_after

  if ! code=$(curl \
    --silent --show-error \
    --retry 0 \
    --user-agent "$user_agent" \
    --dump-header "$headers" \
    --output "$body" \
    --write-out '%{http_code}' \
    "$CRATES_IO_API/$crate/$crate_version"); then
    echo "failed to query crates.io for $crate $crate_version" >&2
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
      echo "crates.io returned unexpected HTTP $code for $crate $crate_version" >&2
      return 1
      ;;
  esac
}

verify_registry_archive() {
  local crate=$1
  local crate_version=$2
  local response_body=$3
  local expected_checksum archive archive_checksum unpack_dir package_root published_commit

  expected_checksum=$(jq -er '.version.checksum' "$response_body")
  archive=$(mktemp "${RUNNER_TEMP:-/tmp}/${crate}-${crate_version}.XXXXXX.crate")
  unpack_dir=$(mktemp -d "${RUNNER_TEMP:-/tmp}/${crate}-${crate_version}.XXXXXX")

  curl --fail --silent --show-error --location --retry 3 \
    --user-agent "$user_agent" \
    "$CRATES_IO_API/$crate/$crate_version/download" \
    --output "$archive"
  archive_checksum=$(sha256sum "$archive" | awk '{print $1}')
  if [[ $archive_checksum != "$expected_checksum" ]]; then
    echo "$crate $crate_version download checksum does not match crates.io metadata" >&2
    return 1
  fi

  tar -xzf "$archive" -C "$unpack_dir"
  package_root="$unpack_dir/$crate-$crate_version"
  if [[ ! -f $package_root/.cargo_vcs_info.json || ! -f $package_root/Cargo.toml.orig ]]; then
    echo "$crate $crate_version archive lacks release provenance files" >&2
    return 1
  fi
  if [[ $crate_version == "$version" ]]; then
    if ! jq -e --arg commit "$expected_commit" \
      '.git.sha1 == $commit and ((.git.dirty // false) == false)' \
      "$package_root/.cargo_vcs_info.json" >/dev/null; then
      echo "$crate $crate_version was not packaged from release commit $expected_commit" >&2
      return 1
    fi
  elif ! jq -e \
    '(.git.sha1 | type == "string" and test("^[0-9a-f]{40}$")) and ((.git.dirty // false) == false)' \
    "$package_root/.cargo_vcs_info.json" >/dev/null; then
    echo "$crate $crate_version lacks clean source provenance" >&2
    return 1
  else
    published_commit=$(jq -er '.git.sha1' "$package_root/.cargo_vcs_info.json")
    if ! git cat-file -e "$published_commit^{commit}"; then
      echo "$crate $crate_version was published from unavailable commit $published_commit" >&2
      return 1
    fi
    if ! git diff --quiet "$published_commit" "$expected_commit" -- "${crate_paths[$crate]}"; then
      echo "$crate $crate_version changed after publication; bump its alpha version" >&2
      return 1
    fi
  fi
  python3 - "$package_root/Cargo.toml.orig" "$crate" "$crate_version" <<'PY'
import pathlib
import sys
import tomllib

manifest = tomllib.loads(pathlib.Path(sys.argv[1]).read_text())
package = manifest.get("package", {})
if package.get("name") != sys.argv[2] or package.get("version") != sys.argv[3]:
    raise SystemExit("published Cargo.toml.orig identity does not match the release")
PY

  echo "RELEASE_VERIFIED $crate $crate_version checksum=$expected_checksum"
}

audit_package() {
  local crate=$1
  local crate_version=$2
  local package_dir="$target_dir/package/$crate-$crate_version"

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
  preflight_version=${crate_versions[$preflight_crate]}
  audit_package "$preflight_crate" "$preflight_version"
  echo "RELEASE_PREFLIGHT_COMPLETE $preflight_crate $preflight_version"
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
  crate_version=${crate_versions[$crate]}
  response_body=$(mktemp "${RUNNER_TEMP:-/tmp}/${crate}-response.XXXXXX.json")
  response_headers=$(mktemp "${RUNNER_TEMP:-/tmp}/${crate}-headers.XXXXXX")
  code=$(registry_response "$crate" "$crate_version" "$response_body" "$response_headers")

  if [[ $code == 200 ]]; then
    verify_registry_archive "$crate" "$crate_version" "$response_body"
    continue
  fi

  unpublished+=("$crate")
done

for crate in "${unpublished[@]}"; do
  crate_version=${crate_versions[$crate]}
  echo "RELEASE_STEP package-and-audit $crate $crate_version"
  audit_package "$crate" "$crate_version"
done

for crate in "${unpublished[@]}"; do
  crate_version=${crate_versions[$crate]}
  response_body=$(mktemp "${RUNNER_TEMP:-/tmp}/${crate}-response.XXXXXX.json")
  response_headers=$(mktemp "${RUNNER_TEMP:-/tmp}/${crate}-headers.XXXXXX")
  code=$(registry_response "$crate" "$crate_version" "$response_body" "$response_headers")
  if [[ $code == 200 ]]; then
    verify_registry_archive "$crate" "$crate_version" "$response_body"
    continue
  fi

  echo "RELEASE_STEP publish $crate $crate_version"

  set +e
  publish_crate "$crate"
  publish_status=$?
  set -e

  # Check the registry after both success and failure. An upload may have
  # succeeded even when Cargo timed out waiting for index propagation.
  code=$(registry_response "$crate" "$crate_version" "$response_body" "$response_headers")
  if [[ $code == 200 ]]; then
    verify_registry_archive "$crate" "$crate_version" "$response_body"
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
