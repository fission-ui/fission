#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

branch="$(git config --file .gitmodules --get submodule.third_party/vello.branch)"
if [[ "$branch" != fission-integration ]]; then
  echo >&2 "error: third_party/vello must track the fork's fission-integration branch"
  exit 1
fi
remote_ref="refs/remotes/origin/$branch"
snapshot="$(git rev-parse HEAD:third_party/vello)"
checkout="$(git -C third_party/vello rev-parse HEAD)"
if [[ "$checkout" != "$snapshot" ]]; then
  echo >&2 "error: the Vello checkout differs from the committed gitlink; run git submodule update"
  exit 1
fi
refspec="refs/heads/$branch:$remote_ref"
git -C third_party/vello fetch --no-tags origin "$refspec"
if [[ "$(git -C third_party/vello rev-parse --is-shallow-repository)" == true ]] &&
   ! git -C third_party/vello merge-base --is-ancestor "$snapshot" "$remote_ref"; then
  git -C third_party/vello fetch --no-tags --unshallow origin "$refspec"
fi
if ! git -C third_party/vello merge-base --is-ancestor "$snapshot" "$remote_ref"; then
  echo >&2 "error: the recorded Vello checkout is not integrated into origin/$branch"
  exit 1
fi

echo "Vello tracks $branch at tested snapshot $snapshot"
