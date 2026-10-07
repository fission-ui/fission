#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

base_tree="$(cargo tree --locked -p fission --no-default-features --prefix none)"
for dependency in \
  fission-assets \
  fission-game \
  fission-physics \
  fission-physics-rapier2d \
  fission-physics-rapier3d \
  fission-scene \
  fission-scene2d \
  fission-scene3d \
  gltf \
  rapier2d \
  rapier3d
do
  if grep -Eq "^${dependency} " <<<"$base_tree"; then
    echo >&2 "error: no-feature fission unexpectedly includes ${dependency}"
    exit 1
  fi
done

rapier2d_tree="$(cargo tree --locked -p fission --no-default-features --features physics-rapier2d --prefix none)"
if grep -Eq '^rapier3d ' <<<"$rapier2d_tree"; then
  echo >&2 "error: physics-rapier2d unexpectedly enables rapier3d"
  exit 1
fi

rapier3d_tree="$(cargo tree --locked -p fission --no-default-features --features physics-rapier3d --prefix none)"
if grep -Eq '^rapier2d ' <<<"$rapier3d_tree"; then
  echo >&2 "error: physics-rapier3d unexpectedly enables rapier2d"
  exit 1
fi

echo "scene, game, asset, glTF, and physics dependencies remain opt-in"
