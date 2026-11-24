#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

branch="$(git config --file .gitmodules --get submodule.third_party/vello.branch)"
if [[ "$branch" != fission ]]; then
  echo >&2 "error: third_party/vello must track the fork's fission integration branch"
  exit 1
fi
if ! git -C third_party/vello rev-parse --verify refs/remotes/origin/fission >/dev/null 2>&1; then
  git -C third_party/vello fetch --no-tags origin fission
fi
if ! git -C third_party/vello merge-base --is-ancestor HEAD refs/remotes/origin/fission; then
  echo >&2 "error: the recorded Vello checkout is not integrated into origin/fission"
  exit 1
fi

echo "Vello tracks fission at tested snapshot $(git -C third_party/vello rev-parse HEAD)"
