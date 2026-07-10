#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
IOS_TARGET="${IOS_TARGET:-aarch64-apple-ios}"
IOS_PROFILE="${IOS_PROFILE:-release}"
IOS_SIGNING_IDENTITY="${IOS_SIGNING_IDENTITY:-}"
IOS_PROVISIONING_PROFILE="${IOS_PROVISIONING_PROFILE:-}"
IOS_REQUIRE_PROVISIONING_PROFILE="${IOS_REQUIRE_PROVISIONING_PROFILE:-1}"
IPA_DIR="$SCRIPT_DIR/build/ipa"
PAYLOAD_DIR="$IPA_DIR/Payload"
IPA_PATH="$IPA_DIR/web-smoke.ipa"

if [[ "$IOS_PROFILE" == "release" && -z "$IOS_SIGNING_IDENTITY" ]]; then
  printf 'Release IPA packaging requires IOS_SIGNING_IDENTITY from a secure local or CI secret source.
' >&2
  exit 1
fi

BUNDLE_DIR=$(IOS_SIM_TARGET="$IOS_TARGET" IOS_SIM_PROFILE="$IOS_PROFILE" "$SCRIPT_DIR/package-sim.sh")

if [[ -n "$IOS_PROVISIONING_PROFILE" ]]; then
  cp "$IOS_PROVISIONING_PROFILE" "$BUNDLE_DIR/embedded.mobileprovision"
elif [[ "$IOS_PROFILE" == "release" && "$IOS_REQUIRE_PROVISIONING_PROFILE" == "1" ]]; then
  printf 'Release IPA packaging requires IOS_PROVISIONING_PROFILE, or set IOS_REQUIRE_PROVISIONING_PROFILE=0 for an explicitly unsigned-profile test package.
' >&2
  exit 1
fi

if [[ -n "$IOS_SIGNING_IDENTITY" ]]; then
  CODESIGN_ARGS=(--force --sign "$IOS_SIGNING_IDENTITY")
  if [[ -n "${IOS_ENTITLEMENTS:-}" ]]; then
    CODESIGN_ARGS+=(--entitlements "$IOS_ENTITLEMENTS")
  elif [[ -f "$SCRIPT_DIR/Entitlements.plist" ]]; then
    CODESIGN_ARGS+=(--entitlements "$SCRIPT_DIR/Entitlements.plist")
  fi
  codesign "${CODESIGN_ARGS[@]}" "$BUNDLE_DIR"
  codesign --verify --deep --strict "$BUNDLE_DIR"
fi

rm -rf "$PAYLOAD_DIR"
mkdir -p "$PAYLOAD_DIR"
cp -R "$BUNDLE_DIR" "$PAYLOAD_DIR/"
rm -f "$IPA_PATH"
(cd "$IPA_DIR" && zip -qry "$IPA_PATH" Payload)
printf '%s
' "$IPA_PATH"
