Third-party dependencies that need local patches live here as git submodules.

Current submodules
- `android-activity`
  - upstream: `https://github.com/rust-mobile/android-activity`
  - fork: `git@github.com:worka-ai/android-activity.git`
  - branch: `worka`

Policy
- Do not commit unpacked crates into this tree.
- If a third-party dependency needs a local patch, fork it, land the patch on a
  maintained branch, and point a submodule at that fork.
