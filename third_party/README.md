Third-party dependencies that need local patches live here as git submodules.

A plain clone leaves these directories empty and the workspace will not build. Clone with
`git clone --recurse-submodules`, or run `git submodule update --init --recursive` in an
existing checkout.

Current submodules
- `android-activity`
  - upstream: `https://github.com/rust-mobile/android-activity`
  - fork: `git@github.com:worka-ai/android-activity.git`
  - branch: `worka`
- `vello`
  - fork: `https://github.com/worka-ai/vello.git`
  - branch: `fission/sparse-strips`
  - provides the `fission-vello-cpu` renderer used by `fission-render-vello`

Policy
- Do not commit unpacked crates into this tree.
- If a third-party dependency needs a local patch, fork it, land the patch on a
  maintained branch, and point a submodule at that fork.
