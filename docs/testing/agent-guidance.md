# Application guidance validation

Validated on 2026-10-08 against public main
`9ba39c6554fd340127e52df2d40d4e0e1e2b50b8` with CLI/API version 0.15.1,
guidance version 2 and manifest schema 1. This branch does not depend on pending
shared execution/results, test review or generic test-control work.
Repository contributor `AGENTS.md` remains
unchanged.

## Package checks

```sh
CARGO_NET_OFFLINE=true cargo test -p fission-command-core -p cargo-fission --locked --offline
cargo check -p fission-command-core -p cargo-fission --locked --offline
cargo fmt -p fission-command-core -p cargo-fission -- --check
bash scripts/check_command_dependency_boundaries.sh
bash scripts/check_published_dependency_boundaries.sh
cargo package -p fission-command-core --list --allow-dirty
cargo package -p cargo-fission --list --allow-dirty
```

The test run passed 113 tests: 82 command-core, 27 CLI library and four CLI
integration tests. Guidance tests check exact generated versions/hashes and all
local links, read-only checks, idempotent update timestamps, missing files,
hash-proven older bundles, full-update conflicts, custom root/nested instructions,
exact legacy migration, malformed manifests, path traversal, duplicate paths,
file/directory collisions, symlinks, interrupted transactions and changes between
inspection and staging. CLI tests exercise JSON/exit behavior, real init/update,
custom fallback paths and edited legacy instructions. Package lists contain the
router, four references and legacy template. Skill frontmatter validation passed;
that checks syntax, not assistant behavior.

Strict Clippy has pre-existing failures in `fission-ir` (`derivable_impls`),
command-core (`collapsible_if`, `useless_format`) and CLI (`if_same_then_else`).
The changed packages passed `--all-targets --no-deps -D warnings` with only those
three command-core/CLI lint classes allowed. No unrelated lint cleanup is included.

## Fresh generated fixture

A new Git-root app was initialized with the built CLI. Its printed instructions,
local skill and relevant references were read before implementation. Supported
target commands added `static-site` and `web`. Generated `fission.toml` was
snapshotted after those commands and remained byte-identical through validation.
The fixture used a path dependency to this public checkout; guidance check
correctly reported `local_path_unverified` with all assets current. Updating
assets succeeds independently of that unresolved compatibility evidence.

All eight Rust fenced examples were extracted from the generated references
into the fixture and compiled against the checkout, including codegen, tokens,
i18n, local state, route parameters, the route handler, static-page construction
and browser-driver calls. The native compile check passed; the thin Web shell
with the documented route handler also passed a real Web/WASM build. DSP/font
inputs were copied as described in the guide.

Content-only `site check`, `site build` and `site routes` passed. The custom Rust
static builder rendered the expected Welcome page. A real 390×844 Chromium
static capture was opened and showed that page. The Web bridge test passed
semantic-identifier lookup, a disclosure toggle from hidden to visible, and
navigation to `/items/42` with the expected parameter text. The initial strict
browser runs exposed missing favicon resources in main's scaffolds; the fixture
staged the actual generated icon at the served URLs before rerunning. The guide
documents this conditional recovery; this PR does not alter those scaffolds.

**Web pixel validation remains incomplete.** The inspected Web capture was
blank despite passing semantic assertions. A retry after an explicit pump and
750 ms wait was also blank. No successful Web appearance or full paint-readiness
claim follows from the bridge checks. Renderer/capture investigation belongs to
separate work; the guide explicitly requires inspected pixels and treats blank
captures as failed visual checks. Same-session real browser resize and retained
state across resize were not tested or claimed.

## Router review

These are author self-review cases, not an independent assistant evaluation:

| Representative request | Applicable route and expected scope |
| --- | --- |
| Build a simple informational website | Read setup/targets; choose static-site when no runtime state is needed; load design/testing as needed. |
| Build a Web dashboard with interactive details and item URLs | Read setup/targets plus widgets/state/routing; choose web; use retained local state and a thin route handler; compile and exercise browser behavior. |
| Change one screen in an existing app | Trace its entrypoint/component and dependency first; load relevant references; preserve existing instructions/configuration; do not reinitialize it. |
| Request visual review or verify state through an in-session resize | Serve with run/site serve and inspect installed test help/source; this base provides smoke testing and no verified resize/frame acknowledgment; do not invent standalone preview/review flags or treat separate sessions as preservation evidence. |

The small generated entrypoint links the local router without copying all
references into every prompt. Filename/frontmatter/link checks establish
packaging and discoverability; they cannot prove an assistant will follow the
router or avoid unsupported APIs.
