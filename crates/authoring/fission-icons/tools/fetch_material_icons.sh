#!/usr/bin/env bash
set -euo pipefail

REPO_URL="https://github.com/worka-ai/material-design-icons.git"
DEST_DIR="${1:-crates/authoring/fission-icons/material-design-icons}"

if [ -d "$DEST_DIR/.git" ]; then
  echo "Material icons repo already exists at $DEST_DIR"
  exit 0
fi

git clone --depth 1 --filter=blob:none --sparse "$REPO_URL" "$DEST_DIR"
(
  cd "$DEST_DIR"
  git sparse-checkout set src
)
