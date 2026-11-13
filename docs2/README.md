# Fission website and tutorials

`docs2` is the Compose-based Fission website. Its pages introduce a goal, explain
the state and operations behind it, and let the reader try the app whilst
following the code. The reference section has a different job: describe the
individual APIs when a reader needs a specific contract or option.

The site owns its tutorial parser and build tooling. Those are documentation
features, not additions to the Fission framework.

Static website lessons build their actual HTML projects. Their marked files
include a complete `fission.toml` and `src/lib.rs` with `build_site()`. They do
not substitute a canvas app for document navigation. Use an outer four-backtick
fence when embedding a Markdown file that itself contains triple-backtick code.

## Build and run locally

Run these commands from the Fission workspace root. Use the project's configured
shared Cargo target; do not create another target directory to work around a
build lock. Install the `wasm32-unknown-unknown` Rust target and the
`wasm-bindgen-cli` version matching `wasm-bindgen` in the workspace `Cargo.lock`.
The showcase script still requires `wasm-pack`; tutorial packaging does not.

```sh
rustup target add wasm32-unknown-unknown
# Match the wasm-bindgen package version in Cargo.lock when installing the CLI.
cargo install wasm-bindgen-cli --version 0.2.126 --locked
./scripts/build_docs2_showcase_app.sh
fission site build --project-dir docs2
fission site serve --project-dir docs2 --port 6467 --startup-timeout-seconds 3600
```

The showcase script stages the real `examples/showcase` Web app under
`generated-assets/showcase-app`. The site build then extracts cumulative tutorial
sources, compiles and packages their Web previews, renders the routes and checks
internal links. Its output is `docs2/dist/site`, as configured in `fission.toml`.

Build separately before serving when the cache is cold. Build progress is
streamed to the terminal, with cache counts and elapsed times for compilation,
packaging and the complete tutorial phase. The build must still compile a new or
changed checkpoint before rendering the site; a cache is not permission to show
an older version of its code.

### Preview builds and caches

Each distinct cumulative app has a stable generated Cargo package. Preparation
writes changed files and removes obsolete ones, rather than recreating every
project on each build. Previews with different Fission capability features use
separate generated workspaces. Cargo compiles uncached previews in release mode once, then
`wasm-bindgen` packages those exact artifacts for the browser. Packaging does not
invoke Cargo again.

The normal tutorial build does not run Binaryen's additional `wasm-opt` pass.
Rust release optimisation remains enabled. To request the additional pass,
install `wasm-opt` and opt in explicitly:

```sh
FISSION_TUTORIAL_WASM_OPT=true fission site build --project-dir docs2
```

Optimised and normal packages have separate cache entries. `WASM_BINDGEN` and
`WASM_OPT` can name the corresponding executables when they are not on `PATH`.
The builder rejects a `wasm-bindgen` CLI which does not match the workspace's
locked crate version. A requested optimisation which cannot run fails the
build; it does not silently become a normal package.

Compilation and packaging have separate limits:

- `FISSION_TUTORIAL_BUILD_PARALLELISM` controls Cargo compilation jobs (default `2`).
- `FISSION_TUTORIAL_PACKAGE_PARALLELISM` controls concurrent packaging workers
  (default `2`).

Both limits accept `1` through `64`. Choose lower limits when sharing CPU and
memory with other work. Optional `wasm-opt` runs with `BINARYEN_CORES=1` inside
each worker, so worker limits also bound its parallelism. For example:

```sh
FISSION_TUTORIAL_BUILD_PARALLELISM=3 \
FISSION_TUTORIAL_PACKAGE_PARALLELISM=2 \
fission site build --project-dir docs2
```

Completed browser packages and their integrity manifests are cached beneath
`$CARGO_TARGET_DIR/tutorial-preview-cache`. Actual HTML tutorial output is cached
under `$CARGO_TARGET_DIR/tutorial-sites`; its cache does not depend on WebAssembly
optimisation settings. Each restored entry is checked against its source,
relevant framework code, features, toolchain and build settings, as well as its
output integrity. Missing or mismatched entries are rebuilt. Editing only prose
does not invalidate an unchanged extracted app.

The website workflow restores those two caches as well as Cargo's native and
WebAssembly dependency output. It saves completed preview packages even if a
later tutorial or link check fails, provided the job was not cancelled. Generated
published files under `docs2/generated-assets/tutorials` are reconstructed from
validated packages; they are not the cache authority. These caches are disposable
and may be removed to force a cold build. A successful warm build is not evidence
that a cold build meets the one-hour CI deadline; time both separately.

## Write a goal tutorial

Add an MDX file under `content/goals`. Its frontmatter names the outcome, route,
app and ordering:

```yaml
---
layout: goal
title: Build a packing checklist
description: Track quantities and packed items before a trip.
goal: See what still needs packing and update the correct item.
path: /blog/build-a-packing-checklist/
order: 120
difficulty: Beginner
duration: 40 minutes
run_command: "fission run --target web --project-dir ."
app: packing-checklist
outcomes:
  - Show quantities and packed status without relying on colour
  - Keep an item's identity when completed rows are hidden
---
```

The route is a public link, so keep it stable when improving the lesson. Use
`aliases` only for an intentional additional route; an alias must not shadow a
reference page or another tutorial.

Content markers connect the explanation to the runnable app:

- `<!-- fission:step id="packing-list" -->` opens a step, closed by
  `<!-- /fission:step -->`. Give it a level-two heading and a stable slug.
- `<!-- fission:file path="src/app.rs" -->` immediately precedes a fenced,
  complete source file. It is not a patch fragment.
- `<!-- fission:preview -->` places the Web preview for that cumulative step.
- `<!-- fission:checkpoint -->` and `<!-- /fission:checkpoint -->` surround a
  specific result the reader can observe.

Each step inherits earlier files and replaces those it declares again. Keep
normal `src/lib.rs` launchers in the lesson as well as the component: changing
`app.rs` alone can leave the generated starter app running. A separate
`preview/lib.rs` is the website harness, staged as `src/preview.rs`. Any sample
records or deterministic test providers belong in that harness or explicit test
fixtures, not in production defaults.

Use Compose for meaningful screens and reusable widgets. Keep product facts in
shared state, retained interface memory beside its component, and outside work
behind effects, jobs, services or capabilities. Explain what a success actually
establishes: a validated draft is not a submitted order, permission is not a
captured position, and a simulator launch is not a signed-device check.

Store and SQL previews select the real Web SQLite provider and stage its
official bridge, worker, WASM and notice assets. The bootstrap gives each
tutorial a stable database namespace shared by its checkpoints. There is no
in-memory fallback. The documentation host must still provide cross-origin
isolation for the containing page and embedded app; missing headers are a
hosting blocker, not a successful persistence demonstration.
The preview checks that prerequisite before starting SQLite and shows a
configuration message if it is missing, rather than remaining on a loader.

## Check the authored app as well as its preview

To validate or refresh every interactive checkpoint without rendering the site
again, use the same preview pipeline directly:

```sh
cargo run -p fission-docs2 --example check_goal_sources -- --all-previews
```

This still builds changed apps and validates complete cached packages. It does
not replace the full site build and its link checks.

The normal preview build compiles the Web harness. That alone does not prove
that a separate native launcher or its test configuration compiles. The focused
source-audit helper checks both paths for the supported desktop/Web lessons:

```sh
cargo run --locked --manifest-path docs2/Cargo.toml \
  --example check_goal_sources -- rust-login-page single-page-web-app-rust
```

Set `CARGO_TARGET_DIR` to the shared project target before running this helper.
It stages cumulative source beneath that target, checks each native library,
runs its tests when present, and checks the authored Web library and preview
independently. Authored native test-host examples are checked separately. It is a
developer audit, not an Android or iOS device test, and does not yet infer every lesson's
optional dependency or shell configuration.

For a Static site lesson the same helper runs its actual HTML builder and link
validation instead. The chart-reference helper checks Rust examples and resolves
their explicitly named shared widget modules to unique authored definitions. It
reports any unresolved cross-page assembly rather than counting it as checked:

```sh
cargo run --locked --manifest-path docs2/Cargo.toml \
  --example check_reference_sources

cargo run --locked --manifest-path docs2/Cargo.toml \
  --example check_reference_sources -- widgets
```

The widget check selects complete component examples. Fragments which rely on
an enclosing view, app model or action are listed separately in its assembly
report, not completed with invented state and counted as passing. The native
check enables the interactive-canvas and terminal-widget features where those
references require them; this does not make those widgets available on Web or
mobile targets.

Neither source checker establishes rendered behaviour or visual quality.

To package just one final checkpoint for browser interaction checks:

```sh
cargo run --locked --manifest-path docs2/Cargo.toml \
  --example check_goal_sources -- --preview rust-sqlite-crud-app
```

After source checks, drive the app's actual actions and inspect desktop and
mobile layouts. `examples/visual_review.rs` provides a LiveTest browser
controller for manual `POST /cmd` requests. The design references are in
`design/reference`. A successful build is not evidence that an interaction or
layout matches those designs.

## Deployment and incoming links

The website workflow builds and qualifies `docs2/dist/site` on pull requests;
only a successful main-branch build can deploy that output to GitHub Pages.
It does not deploy the removed `documentation` project or publish framework
crates. `data/legacy-routes.txt` records the 851 URLs captured before migration.
Site construction checks that each still has a page or an explicit redirect.
Dated blog URLs redirect through Fission's existing route-outcome API, with
route-scoped `noindex`; tag archives retain their filtered content.

The crate directory uses a disposable SQLite index refreshed from public package
metadata by `update-crate-index`. The CI job restores and refreshes this index;
it is not checked in. Without it, local builds show an explicit unavailable
state, not invented package data. A corrupt existing index fails the build.

SQLite previews require HTTPS and the response headers
`Cross-Origin-Opener-Policy: same-origin` and
`Cross-Origin-Embedder-Policy: require-corp` on the containing page and embedded
app. Configure these at the production host before cutover. GitHub Pages does
not infer them from this repository. For local browser QA, serve the built
output with `python3 -B scripts/serve_docs2_qa.py --port 6467`; this helper does
not configure production hosting.

## Find the implementation

- `src/app.rs` configures routes, metadata, theme and locale.
- `src/screens` and `src/widgets` contain Compose pages and reusable UI.
- `src/content.rs` parses tutorial metadata, files and step markers.
- `src/tutorials.rs` stages sources and builds Web previews.
- `content/goals` contains goal-based lessons.
- `content/legacy/docs` preserves existing documentation URLs.
- `content/reference` describes APIs under the reference navigation.
- `content/compose` explains the authoring API.
- `design` and `i18n` hold the checked-in design and translation sources.

English and Spanish UI bundles use the same keys. Tutorial app translations are
part of each lesson's extracted files; changing the site's language does not
automatically translate an independently embedded app.
